use crate::model::{now, ParentProcess, PortBinding, ProcessDetail, ProcessKind, Project};
use std::{
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    io::Read,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

pub fn command(program: &str, args: &[&str]) -> Result<String, String> {
    let mut child = Command::new(program)
        .args(args)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法执行 {program}：{e}"))?;
    let stdout = child.stdout.take().ok_or("无法读取系统命令输出")?;
    let stderr = child.stderr.take().ok_or("无法读取系统命令错误")?;
    let read = |pipe: Box<dyn Read + Send>| {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            pipe.take(4_194_305)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() > 4_194_304 {
                return Err("系统命令输出超过 4 MB 上限".to_string());
            }
            Ok(String::from_utf8_lossy(&bytes).into_owned())
        })
    };
    let out = read(Box::new(stdout));
    let err = read(Box::new(stderr));
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() < Duration::from_secs(5) => {
                thread::sleep(Duration::from_millis(15))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("系统命令 {program} 超时或无法等待"));
            }
        }
    };
    let output = out.join().map_err(|_| "读取命令输出失败")??;
    let error = err.join().map_err(|_| "读取命令错误失败")??;
    if !status.success()
        && !(program == "/usr/sbin/lsof" && status.code() == Some(1) && error.trim().is_empty())
    {
        return Err(format!(
            "{program} 执行失败（{}）{}",
            status.code().unwrap_or(-1),
            if error.trim().is_empty() {
                ""
            } else {
                "：进程可能已经退出或没有访问权限"
            }
        ));
    }
    Ok(output)
}

pub fn parse_ports(text: &str) -> Vec<PortBinding> {
    let mut pid = 0;
    let mut name = String::new();
    let mut family = String::new();
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for line in text.lines() {
        let Some(tag) = line.as_bytes().first() else {
            continue;
        };
        let value = &line[1..];
        match tag {
            b'p' => {
                pid = value.parse().unwrap_or(0);
                name.clear();
                family.clear();
            }
            b'c' => name = value.into(),
            b'f' => family.clear(),
            b't' => family = value.into(),
            b'n' if pid > 0 && ["IPv4", "IPv6"].contains(&family.as_str()) => {
                if let Some((address, port)) = value.rsplit_once(':') {
                    if let Ok(port) = port.parse::<u16>() {
                        if seen.insert((pid, address.to_string(), port, family.clone())) {
                            result.push(PortBinding {
                                pid,
                                name: name.clone(),
                                address: address.into(),
                                port,
                                family: family.clone(),
                                first_seen: now(),
                                project_id: None,
                                command: String::new(),
                                cwd: None,
                                source: "OBSERVED".into(),
                                service_id: None,
                                process_kind: ProcessKind::Unknown,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }
    result.sort_by_key(|p| (p.port, p.pid, p.family.clone(), p.address.clone()));
    result
}

pub fn list_ports() -> Result<Vec<PortBinding>, String> {
    if !cfg!(target_os = "macos") {
        return Err("端口发现目前仅支持 macOS".into());
    }
    Ok(parse_ports(&command(
        "/usr/sbin/lsof",
        &["-nP", "-iTCP", "-sTCP:LISTEN", "-Fpcftn"],
    )?))
}

pub fn working_directories(pids: &[u32]) -> Result<HashMap<u32, String>, String> {
    if pids.is_empty() {
        return Ok(HashMap::new());
    }
    let ids = pids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let text = command("/usr/sbin/lsof", &["-a", "-p", &ids, "-d", "cwd", "-Fpn"])?;
    let mut pid = 0;
    let mut result = HashMap::new();
    for line in text.lines() {
        if let Some(id) = line.strip_prefix('p') {
            pid = id.parse().unwrap_or(0);
        }
        if let Some(path) = line.strip_prefix('n') {
            if pid > 0 && Path::new(path).is_absolute() {
                result.insert(pid, path.into());
            }
        }
    }
    Ok(result)
}

pub fn commands(pids: &[u32]) -> HashMap<u32, String> {
    if pids.is_empty() {
        return HashMap::new();
    }
    let ids = pids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    command("/bin/ps", &["-ww", "-p", &ids, "-o", "pid=,command="])
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let (id, value) = line.trim().split_once(char::is_whitespace)?;
            Some((id.parse().ok()?, redact_command(value.trim())))
        })
        .collect()
}

/// Kernel-reported executable, without spawning a command or reading project files.
pub fn executable_path(pid: u32) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let mut bytes = vec![0_u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
        let len = unsafe {
            libc::proc_pidpath(pid as i32, bytes.as_mut_ptr().cast(), bytes.len() as u32)
        };
        if len > 0 {
            let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
            return String::from_utf8(bytes[..end].to_vec()).ok();
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = pid;
    None
}

pub fn match_project<'a>(cwd: &str, projects: &'a [Project]) -> Option<&'a Project> {
    projects
        .iter()
        .filter(|p| Path::new(cwd).starts_with(&p.candidate.canonical_path))
        .max_by_key(|p| p.candidate.canonical_path.len())
}

pub fn redact_command(text: &str) -> String {
    let mut hide_next = false;
    text.split_whitespace()
        .map(|part| {
            if hide_next {
                hide_next = false;
                return "[REDACTED]".into();
            }
            let key = part
                .split('=')
                .next()
                .unwrap_or(part)
                .trim_start_matches('-')
                .to_lowercase();
            if [
                "token",
                "password",
                "passwd",
                "secret",
                "api_key",
                "api-key",
                "apikey",
                "access_token",
                "authorization",
            ]
            .iter()
            .any(|needle| key == *needle || key.ends_with(&format!("_{needle}")))
            {
                if let Some((name, _)) = part.split_once('=') {
                    return format!("{name}=[REDACTED]");
                }
                hide_next = true;
            }
            if part.contains("://") && part.contains('@') {
                return "[REDACTED_URL]".into();
            }
            part.to_string()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn inspect(pid: u32) -> Result<ProcessDetail, String> {
    if !cfg!(target_os = "macos") {
        return Err("进程详情目前仅支持 macOS".into());
    }
    if pid == 0 || pid > i32::MAX as u32 {
        return Err("无效 PID".into());
    }
    let id = pid.to_string();
    let ps = command(
        "/bin/ps",
        &[
            "-ww",
            "-p",
            &id,
            "-o",
            "pid=,ppid=,pgid=,uid=,lstart=,etime=,command=",
        ],
    )?;
    let fields: Vec<_> = ps.split_whitespace().collect();
    if fields.len() < 11 || fields[0].parse::<u32>().ok() != Some(pid) {
        return Err("进程已经退出或没有访问权限".into());
    }
    let number = |i: usize| {
        fields[i]
            .parse::<u32>()
            .map_err(|_| "进程字段无效".to_string())
    };
    let ppid = number(1)?;
    let pgid = number(2)?;
    let uid = number(3)?;
    let start_time = fields[4..9].join(" ");
    let elapsed = fields[9].to_string();
    let raw_command = fields[10..].join(" ");
    let paths = command(
        "/usr/sbin/lsof",
        &["-a", "-p", &id, "-d", "cwd,txt", "-Ffn"],
    )
    .unwrap_or_default();
    let mut kind = "";
    let mut cwd = None;
    let mut executable = None;
    for line in paths.lines() {
        if let Some(fd) = line.strip_prefix('f') {
            kind = fd;
        }
        if let Some(path) = line.strip_prefix('n') {
            if !Path::new(path).is_absolute() {
                continue;
            }
            if kind == "cwd" {
                cwd = Some(path.to_string());
            }
            if kind == "txt" && executable.is_none() {
                executable = Some(path.to_string());
            }
        }
    }
    let start_identity = start_identity(pid);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (
        pid,
        uid,
        &start_time,
        &start_identity,
        &raw_command,
        &cwd,
        &executable,
    )
        .hash(&mut hasher);
    let fingerprint = format!("{pid}-{:016x}", hasher.finish());
    let name = executable
        .as_ref()
        .and_then(|p| Path::new(p).file_name())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| fields[10].into());
    let (memory_rss_bytes, cpu_time_ms) = task_metrics(pid)
        .map(|(mem, cpu)| (Some(mem), Some(cpu)))
        .unwrap_or((None, None));
    Ok(ProcessDetail {
        pid,
        ppid,
        pgid,
        uid,
        name,
        command: redact_command(&raw_command),
        executable,
        cwd,
        start_time,
        elapsed,
        fingerprint,
        start_identity,
        bindings: Vec::new(),
        parents: Vec::new(),
        project_id: None,
        suggested_project_id: None,
        confidence: "NONE".into(),
        source: "OBSERVED".into(),
        can_stop: false,
        stop_reason: "尚未确认项目归属".into(),
        service_id: None,
        memory_rss_bytes,
        cpu_time_ms,
    })
}

#[cfg(target_os = "macos")]
pub fn start_identity(pid: u32) -> Option<String> {
    identity(pid).map(|p| p.start)
}

#[cfg(target_os = "macos")]
pub fn task_metrics(pid: u32) -> Option<(u64, u64)> {
    if pid == 0 || pid > i32::MAX as u32 {
        return None;
    }
    let mut task_info = std::mem::MaybeUninit::<libc::proc_taskinfo>::zeroed();
    let size = std::mem::size_of::<libc::proc_taskinfo>();
    let read = unsafe {
        libc::proc_pidinfo(
            pid as i32,
            libc::PROC_PIDTASKINFO,
            0,
            task_info.as_mut_ptr().cast(),
            size as i32,
        )
    };
    if read != size as i32 {
        return None;
    }
    let task_info = unsafe { task_info.assume_init() };
    let cpu_time_ms = (task_info.pti_total_user + task_info.pti_total_system) / 1_000_000;
    Some((task_info.pti_resident_size, cpu_time_ms))
}

#[cfg(target_os = "macos")]
pub fn group_task_metrics(pgid: u32) -> (Option<u64>, Option<u64>) {
    if let Ok(members) = group_members(pgid) {
        if !members.is_empty() {
            let mut total_mem = 0u64;
            let mut total_cpu = 0u64;
            let mut found = false;
            for pid in members {
                if let Some((mem, cpu)) = task_metrics(pid) {
                    total_mem = total_mem.saturating_add(mem);
                    total_cpu = total_cpu.saturating_add(cpu);
                    found = true;
                }
            }
            if found {
                return (Some(total_mem), Some(total_cpu));
            }
        }
    }
    (None, None)
}

#[cfg(not(target_os = "macos"))]
pub fn task_metrics(_pid: u32) -> Option<(u64, u64)> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn group_task_metrics(_pgid: u32) -> (Option<u64>, Option<u64>) {
    (None, None)
}

#[derive(Debug, Clone)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub pgid: u32,
    pub uid: u32,
    pub start: String,
    pub alive: bool,
}

#[cfg(target_os = "macos")]
pub fn identity(pid: u32) -> Option<ProcessIdentity> {
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let size = std::mem::size_of::<libc::proc_bsdinfo>();
    // libproc gives microsecond resolution, unlike the human-readable ps start time.
    let read = unsafe {
        libc::proc_pidinfo(
            pid as i32,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            size as i32,
        )
    };
    if read != size as i32 {
        return None;
    }
    let info = unsafe { info.assume_init() };
    if info.pbi_pid != pid {
        return None;
    }
    Some(ProcessIdentity {
        pid,
        pgid: info.pbi_pgid,
        uid: info.pbi_uid,
        start: format!("{}.{:06}", info.pbi_start_tvsec, info.pbi_start_tvusec),
        alive: info.pbi_status != libc::SZOMB as u32,
    })
}
#[cfg(not(target_os = "macos"))]
pub fn start_identity(_pid: u32) -> Option<String> {
    None
}
#[cfg(not(target_os = "macos"))]
pub fn identity(_pid: u32) -> Option<ProcessIdentity> {
    None
}

pub fn group_members(pgid: u32) -> Result<Vec<u32>, String> {
    #[cfg(target_os = "macos")]
    {
        // PROC_PGRP_ONLY=2 is defined by the macOS SDK sys/proc_info.h.
        // Avoid spawning ps inside the group being signalled.
        let mut capacity = 1024;
        loop {
            let mut pids = vec![0i32; capacity];
            let size = (capacity * std::mem::size_of::<i32>()) as i32;
            let bytes = unsafe { libc::proc_listpids(2, pgid, pids.as_mut_ptr().cast(), size) };
            if bytes < 0 {
                return Err("无法读取进程组成员".into());
            }
            if bytes >= size {
                if capacity >= 65_536 {
                    return Err("进程组成员超过安全查询上限".into());
                }
                capacity *= 2;
                continue;
            }
            return Ok(pids
                .into_iter()
                .take(bytes as usize / std::mem::size_of::<i32>())
                .filter(|pid| *pid > 0)
                .filter_map(|pid| {
                    identity(pid as u32)
                        .filter(|p| p.alive && p.pgid == pgid)
                        .map(|p| p.pid)
                })
                .collect());
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = pgid;
        Err("进程组发现目前仅支持 macOS".into())
    }
}

pub fn parent_chain(mut pid: u32) -> Vec<ParentProcess> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for _ in 0..10 {
        if pid <= 1 || !seen.insert(pid) {
            break;
        }
        let Ok(text) = command(
            "/bin/ps",
            &["-ww", "-p", &pid.to_string(), "-o", "ppid=,comm="],
        ) else {
            break;
        };
        let mut parts = text.trim().splitn(2, char::is_whitespace);
        let Some(parent) = parts.next().and_then(|v| v.parse::<u32>().ok()) else {
            break;
        };
        let name = parts.next().unwrap_or("未知").trim().to_string();
        result.push(ParentProcess { pid, name });
        pid = parent;
    }
    result
}

pub fn stop_eligibility(detail: &ProcessDetail, protected: &HashSet<u32>) -> Result<(), String> {
    if detail.start_identity.is_none() {
        return Err("无法精确核验进程启动时间，禁止停止".into());
    }
    if detail.pid <= 1 || protected.contains(&detail.pid) {
        return Err("保护进程：DevNook、自身父链和系统进程不可停止".into());
    }
    if detail.uid != unsafe { libc::geteuid() } {
        return Err("只能停止当前用户的进程".into());
    }
    let Some(exe) = detail.executable.as_ref() else {
        return Err("无法核验可执行文件，禁止停止".into());
    };
    if exe.starts_with("/System/")
        || exe.starts_with("/usr/libexec/")
        || exe.starts_with("/usr/sbin/")
        || exe.starts_with("/sbin/")
        || exe.contains("/Contents/MacOS/")
        || [
            "zsh", "bash", "sh", "fish", "login", "sshd", "sudo", "launchd",
        ]
        .contains(&detail.name.as_str())
    {
        return Err("系统进程、终端 Shell 和桌面应用受保护".into());
    }
    if detail.cwd.is_none()
        || detail.project_id.is_none()
        || !["HIGH", "MANUAL"].contains(&detail.confidence.as_str())
    {
        return Err("请先确认进程所属项目；未知进程只读".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "macos")]
    fn test_task_metrics_current_process() {
        let pid = std::process::id();
        let metrics = task_metrics(pid);
        assert!(metrics.is_some(), "当前进程的 task_metrics 应该成功获取");
        let (rss, _cpu) = metrics.unwrap();
        assert!(rss > 0, "当前进程物理内存 RSS 应该大于 0 字节");
    }
}
