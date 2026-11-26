use crate::{
    health,
    model::{now, now_millis},
    process,
    service_logs::{self, LogSink},
    service_model::*,
    service_storage, storage,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::Read,
    os::unix::process::ExitStatusExt,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

static STOP: AtomicBool = AtomicBool::new(false);
extern "C" fn request_stop(_signal: i32) {
    STOP.store(true, Ordering::SeqCst);
}

#[derive(Serialize, Deserialize)]
pub struct WorkerSpec {
    pub database: PathBuf,
    pub run_id: String,
    pub definition: ServiceDefinition,
    pub program: PathBuf,
    pub arguments: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub secret_values: Vec<String>,
}

pub fn from_stdin() -> Result<(), String> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(262_145)
        .read_to_end(&mut bytes)
        .map_err(|_| "无法读取 runner 配置")?;
    if bytes.len() > 262_144 {
        return Err("runner 配置超过限制".into());
    }
    let spec: WorkerSpec = serde_json::from_slice(&bytes).map_err(|_| "runner 配置无效")?;
    run(spec)
}

fn signal_self_group() {
    // The runner itself owns this group and its signal handler keeps it alive to drain logs.
    unsafe {
        libc::kill(-(std::process::id() as i32), libc::SIGTERM);
    }
}

pub fn run(spec: WorkerSpec) -> Result<(), String> {
    let db = storage::open(&spec.database)?;
    let mut run = service_storage::run(&db, &spec.run_id)?;
    let worker = process::identity(std::process::id()).ok_or("无法验证 runner 身份")?;
    if run.worker_pid != Some(worker.pid)
        || run.pgid != Some(worker.pgid)
        || worker.pid != worker.pgid
        || run.worker_start.as_deref() != Some(&worker.start)
    {
        return Err("runner 身份与运行记录不匹配".into());
    }
    unsafe {
        libc::signal(
            libc::SIGTERM,
            request_stop as *const () as libc::sighandler_t,
        );
        libc::signal(
            libc::SIGINT,
            request_stop as *const () as libc::sighandler_t,
        );
        libc::signal(libc::SIGHUP, libc::SIG_IGN);
    }
    let sink = Arc::new(Mutex::new(LogSink::new(
        std::path::Path::new(&run.log_path),
        spec.definition.log_limit_kb,
        spec.secret_values,
    )?));
    sink.lock()
        .map_err(|_| "日志锁异常")?
        .write("system", "服务运行开始")?;
    let ports = process::list_ports()?;
    if ports
        .iter()
        .any(|binding| spec.definition.expected_ports.contains(&binding.port))
    {
        run.state = ServiceState::Blocked;
        run.error = Some("启动前端口已被占用，已阻止执行".into());
        run.stopped_at = Some(now());
        service_storage::save_run(&db, &run)?;
        return Ok(());
    }
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.arguments)
        .env_clear()
        .envs(&spec.environment)
        .current_dir(&spec.definition.working_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => {
            run.state = ServiceState::Crashed;
            run.error = Some("可执行文件启动失败，请检查路径、权限与运行环境".into());
            run.stopped_at = Some(now());
            sink.lock()
                .map_err(|_| "日志锁异常")?
                .write("system", run.error.as_deref().unwrap())?;
            service_storage::save_run(&db, &run)?;
            return Ok(());
        }
    };
    run.pid = Some(child.id());
    run.state = ServiceState::Running;
    service_storage::save_run(&db, &run)?;
    let mut last_persisted = serde_json::to_string(&run).unwrap_or_default();
    let found = Arc::new(AtomicBool::new(false));
    let log_failed = Arc::new(AtomicBool::new(false));
    let stdout = child.stdout.take().ok_or("stdout 不可用")?;
    let stderr = child.stderr.take().ok_or("stderr 不可用")?;
    let mut readers = Vec::new();
    for (input, source) in [
        (Box::new(stdout) as Box<dyn Read + Send>, "stdout"),
        (Box::new(stderr) as Box<dyn Read + Send>, "stderr"),
    ] {
        let sink = sink.clone();
        let found = found.clone();
        let failed = log_failed.clone();
        let phrase = spec.definition.readiness.phrase.clone();
        readers.push(std::thread::spawn(move || {
            service_logs::collect(input, source, sink, phrase, found, failed)
        }));
    }
    let started = Instant::now();
    let mut next_health = Instant::now();
    let mut stopped = None;
    let mut terminal_failure = false;
    let mut exit = None;
    loop {
        if let Some(status) = child.try_wait().map_err(|_| "无法读取服务退出状态")? {
            if exit.is_none() {
                run.exit_code = Some(
                    status
                        .code()
                        .unwrap_or_else(|| -status.signal().unwrap_or(1)),
                );
                exit = Some(status);
                if !status.success() && !run.stop_requested && !STOP.load(Ordering::SeqCst) {
                    terminal_failure = true;
                    run.error = Some(format!(
                        "启动进程异常退出（{}）",
                        run.exit_code.unwrap_or(-1)
                    ));
                    signal_self_group();
                }
            }
        }
        let members = process::group_members(worker.pgid)?
            .into_iter()
            .filter(|pid| *pid != worker.pid)
            .collect::<Vec<_>>();
        if STOP.load(Ordering::SeqCst) && !run.stop_requested {
            run.stop_requested = true;
            run.stop_requested_at_ms = Some(now_millis());
            stopped = Some(Instant::now());
        }
        if exit.is_some() && members.is_empty() {
            if run.ready_at.is_none() && !run.stop_requested && !STOP.load(Ordering::SeqCst) {
                terminal_failure = true;
                run.error = Some("进程在就绪检查通过前退出".into());
            }
            break;
        }
        if STOP.load(Ordering::SeqCst) {
            // The deadline is independent of the next health probe interval.
            if stopped.is_none() {
                stopped = Some(Instant::now());
                run.stop_requested = true;
                run.stop_requested_at_ms = Some(now_millis());
            }
            run.state = ServiceState::Stopping;
            if stopped.is_some_and(|at| {
                at.elapsed() > Duration::from_secs(spec.definition.stop_grace_seconds)
            }) && run.error.is_none()
            {
                run.error = Some("SIGTERM 宽限期已过，进程组仍存活；可独立确认强制停止".into());
            }
        } else if run.ready_at.is_none()
            && started.elapsed() >= Duration::from_secs(spec.definition.readiness.timeout_seconds)
        {
            terminal_failure = true;
            run.error = Some("就绪检查超时，已请求停止本次进程组".into());
            let _ = sink
                .lock()
                .map_err(|_| "日志锁异常")?
                .write("system", run.error.as_deref().unwrap());
            run.stop_requested = true;
            run.stop_requested_at_ms = Some(now_millis());
            run.state = ServiceState::Stopping;
            stopped = Some(Instant::now());
            signal_self_group();
        } else if Instant::now() >= next_health {
            if health::ready(
                &spec.definition,
                worker.pgid,
                started.elapsed(),
                found.load(Ordering::Relaxed),
            ) {
                run.state = ServiceState::Ready;
                if run.ready_at.is_none() {
                    run.ready_at = Some(now());
                    sink.lock()
                        .map_err(|_| "日志锁异常")?
                        .write("system", "就绪检查通过")?;
                }
                if run.error.as_deref() == Some("就绪检查暂未通过") {
                    run.error = None;
                }
            } else {
                run.state = ServiceState::Running;
                if run.ready_at.is_some() {
                    run.error = Some("就绪检查暂未通过".into());
                }
            }
            let next_interval = if run.state == ServiceState::Ready {
                spec.definition
                    .readiness
                    .interval_ms
                    .saturating_mul(4)
                    .min(2000)
            } else {
                spec.definition.readiness.interval_ms
            };
            next_health = Instant::now() + Duration::from_millis(next_interval);
        }
        if log_failed.load(Ordering::Relaxed) && run.error.is_none() {
            run.error = Some("日志采集或写入失败，请检查磁盘空间".into());
        }
        run.log_truncated = sink.lock().map_err(|_| "日志锁异常")?.truncated;
        let current_snapshot = serde_json::to_string(&run).unwrap_or_default();
        if current_snapshot != last_persisted {
            service_storage::save_run(&db, &run)?;
            last_persisted = current_snapshot;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    // Every remaining member of our group is gone. Do not wait indefinitely on escaped descendants.
    let deadline = Instant::now() + Duration::from_millis(500);
    while readers.iter().any(|thread| !thread.is_finished()) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    for thread in readers {
        if thread.is_finished() {
            let _ = thread.join();
        }
    }
    run.state = if terminal_failure || (!run.stop_requested && exit.is_some_and(|s| !s.success())) {
        ServiceState::Crashed
    } else {
        ServiceState::Stopped
    };
    run.stopped_at = Some(now());
    run.log_truncated = sink.lock().map_err(|_| "日志锁异常")?.truncated;
    sink.lock()
        .map_err(|_| "日志锁异常")?
        .write("system", "进程组已退出")?;
    service_storage::save_run(&db, &run)?;
    storage::audit(
        &db,
        "service_exit",
        &run.id,
        if run.state == ServiceState::Crashed {
            "CRASHED"
        } else {
            "STOPPED"
        },
    )?;
    Ok(())
}
