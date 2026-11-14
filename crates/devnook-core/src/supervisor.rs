use crate::{
    health,
    model::{now, now_millis},
    process,
    secrets::SecretStore,
    service_logs,
    service_model::*,
    service_storage as store,
    service_worker::WorkerSpec,
    storage, Core,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::Write,
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerLauncher {
    pub executable: PathBuf,
    pub arguments: Vec<String>,
}
impl RunnerLauncher {
    pub fn current() -> Result<Self, String> {
        Ok(Self {
            executable: std::env::current_exe().map_err(|e| e.to_string())?,
            arguments: vec!["--devnook-worker".into()],
        })
    }
}
pub struct Supervisor {
    pub database: PathBuf,
    pub launcher: RunnerLauncher,
    pub secrets: Arc<dyn SecretStore>,
    pub operations: Mutex<()>,
    pub confirmations: Mutex<HashMap<String, (String, String, String, Option<String>, u64)>>,
}

pub fn sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_uppercase();
    [
        "TOKEN",
        "PASSWORD",
        "PASSWD",
        "SECRET",
        "API_KEY",
        "PRIVATE_KEY",
        "AUTHORIZATION",
        "AUTH",
        "COOKIE",
        "ACCESS_KEY",
        "CREDENTIAL",
    ]
    .iter()
    .any(|part| key.contains(part))
        || key.ends_with("_KEY")
}
fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 100
        && key
            .bytes()
            .enumerate()
            .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || (i > 0 && b.is_ascii_digit()))
}
pub fn search_path() -> String {
    let mut paths = vec![std::env::var("PATH").unwrap_or_default()];
    if let Some(home) = std::env::var_os("HOME") {
        paths.push(format!("{}/.local/bin", PathBuf::from(home).display()));
    }
    paths.push("/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin".into());
    paths.join(":")
}
pub fn resolve_program(value: &str) -> Result<PathBuf, String> {
    use std::os::unix::fs::PermissionsExt;
    let candidates: Vec<_> = if value.contains('/') {
        vec![PathBuf::from(value)]
    } else {
        std::env::split_paths(&search_path())
            .map(|root| root.join(value))
            .collect()
    };
    candidates
        .into_iter()
        .find(|path| {
            path.is_file()
                && path.is_absolute()
                && path
                    .metadata()
                    .is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
        })
        .and_then(|path| path.canonicalize().ok())
        .ok_or("可执行文件不存在或没有执行权限；请配置绝对路径或显式 Shell Mode".into())
}

pub fn validate(mut value: ServiceDefinition, core: &Core) -> Result<ServiceDefinition, String> {
    if value.name.trim().is_empty()
        || value.name.len() > 160
        || value.executable.trim().is_empty()
        || value.executable.len() > 8192
        || value.executable.contains('\0')
    {
        return Err("服务名称或可执行命令无效".into());
    }
    if !["NODE", "PYTHON", "JAVA", "OTHER"].contains(&value.runtime_type.as_str()) {
        return Err("服务运行类型无效".into());
    }
    if value.args.len() > 256
        || value.args.iter().map(String::len).sum::<usize>() > 65536
        || value.args.iter().any(|v| v.contains('\0'))
    {
        return Err("命令参数超过限制或包含无效字符".into());
    }
    let plain = format!("{} {}", value.executable, value.args.join(" "));
    if process::redact_command(&plain).contains("[REDACTED") {
        return Err("命令参数中检测到敏感值；请通过 Keychain 环境变量传入".into());
    }
    if value.shell_mode && !value.args.is_empty() {
        return Err("Shell Mode 请将完整命令填入命令框，参数列表留空".into());
    }
    let project = core
        .projects()?
        .into_iter()
        .find(|p| p.candidate.id == value.project_id)
        .ok_or("所属项目不存在")?;
    let cwd = Path::new(&value.working_directory)
        .canonicalize()
        .map_err(|_| "工作目录不存在或不可访问")?;
    if !cwd.is_dir() || !cwd.starts_with(&project.candidate.canonical_path) {
        return Err("工作目录必须位于所属项目中".into());
    }
    value.working_directory = cwd.to_string_lossy().into();
    value.name = value.name.trim().into();
    if !(1..=300).contains(&value.readiness.timeout_seconds)
        || !(100..=5000).contains(&value.readiness.interval_ms)
        || !(100..=10000).contains(&value.readiness.grace_ms)
        || value.readiness.grace_ms > value.readiness.timeout_seconds * 1000
    {
        return Err("就绪超时、间隔或进程宽限期不合法".into());
    }
    if !(1..=30).contains(&value.stop_grace_seconds) || !(64..=10240).contains(&value.log_limit_kb)
    {
        return Err("停止宽限期应为 1–30 秒，日志限制应为 64–10240 KiB".into());
    }
    if value.environment.len() + value.secret_keys.len() > 100 {
        return Err("环境变量超过 100 个限制".into());
    }
    for (key, text) in &value.environment {
        if !valid_key(key) || text.len() > 4096 || text.contains('\0') {
            return Err("环境变量名称或值无效".into());
        }
        if sensitive_key(key) {
            return Err(format!("{key} 是敏感变量，请使用 Keychain 字段"));
        }
        if process::redact_command(text).contains("[REDACTED") {
            return Err(format!("{key} 的值包含凭据，请改用 Keychain 字段"));
        }
    }
    for key in &value.secret_keys {
        if !valid_key(key) || value.environment.contains_key(key) {
            return Err("敏感变量名称无效或与普通变量重复".into());
        }
    }
    value.secret_keys.sort();
    value.secret_keys.dedup();
    match value.readiness.kind {
        ReadinessKind::Port => {
            let port = value
                .readiness
                .port
                .filter(|p| *p > 0)
                .ok_or("端口就绪检测需要有效端口")?;
            value.expected_ports.push(port);
        }
        ReadinessKind::Http => {
            let (address, _, _) = health::http_target(&value.readiness.url)?;
            value.expected_ports.push(address.port());
        }
        ReadinessKind::LogPhrase
            if value.readiness.phrase.is_empty()
                || value.readiness.phrase.len() > 4096
                || value.readiness.phrase.contains('\n') =>
        {
            return Err("日志关键字不能为空、超过 4096 字节或包含换行".into())
        }
        _ => {}
    }
    if value.expected_ports.len() > 32 || value.expected_ports.contains(&0) {
        return Err("预期端口无效或超过限制".into());
    }
    value.expected_ports.sort();
    value.expected_ports.dedup();
    Ok(value)
}

pub fn anchor_valid(run: &ServiceRun) -> bool {
    run.worker_pid.and_then(process::identity).is_some_and(|p| {
        p.alive
            && p.uid == unsafe { libc::geteuid() }
            && Some(p.pgid) == run.pgid
            && p.pid == p.pgid
            && Some(&p.start) == run.worker_start.as_ref()
    })
}
pub fn reconcile(mut run: ServiceRun) -> ServiceRun {
    if !run.state.active() {
        return run;
    }
    if anchor_valid(&run) {
        return run;
    }
    if run.worker_pid.is_none() && now().saturating_sub(run.started_at) <= 5 {
        return run;
    }
    let members = run.pgid.and_then(|pgid| process::group_members(pgid).ok());
    if members.as_ref().is_some_and(Vec::is_empty) || run.pgid.is_none() {
        run.state = if run.stop_requested {
            ServiceState::Stopped
        } else {
            ServiceState::Crashed
        };
        run.stopped_at = Some(now());
        run.error = Some("runner 已退出；已核对无存活进程组".into());
    } else {
        run.state = ServiceState::Unknown;
        run.error = Some("runner 身份无法核验；进程组可能仍存活，禁止自动启停".into());
    }
    run
}

impl Core {
    pub fn services(&self) -> Result<Vec<ServiceView>, String> {
        let db = self.db.lock().map_err(|_| "项目库锁异常")?;
        store::definitions(&db)?
            .into_iter()
            .map(|definition| {
                let current_run = store::runs(&db, &definition.id, 1)?
                    .into_iter()
                    .next()
                    .map(reconcile);
                Ok(ServiceView {
                    definition,
                    current_run,
                })
            })
            .collect()
    }
    pub fn save_service(&self, input: ServiceSave) -> Result<ServiceDefinition, String> {
        if serde_json::to_vec(&input)
            .map_err(|_| "服务配置无效")?
            .len()
            > 131_072
        {
            return Err("服务配置超过 128 KiB 限制".into());
        }
        let _operation = self
            .supervisor
            .operations
            .lock()
            .map_err(|_| "服务操作锁异常")?;
        let mut value = validate(input.definition, self)?;
        if value.id.is_empty() {
            value.id = Uuid::new_v4().to_string();
            value.created_at = now();
        } else {
            let db = self.db.lock().map_err(|_| "项目库锁异常")?;
            let previous = store::definition(&db, &value.id)?;
            if previous.revision != value.revision {
                return Err("服务配置已被更新，请重新打开编辑器".into());
            }
            if store::runs(&db, &value.id, 1)?
                .first()
                .is_some_and(|r| reconcile(r.clone()).state.active())
            {
                return Err("请先停止服务，再修改启动配置".into());
            }
            value.created_at = previous.created_at;
        }
        let previous_revision = value.revision.clone();
        self.ensure_service_available(&value.id)?;
        self.validate_dependencies(&value)?;
        value.revision = Uuid::new_v4().to_string();
        for key in &value.secret_keys {
            let account = format!("{}:{}:{key}", value.id, value.revision);
            if let Some(secret) = input.secret_values.get(key) {
                if secret.is_empty() || secret.len() > 4096 || secret.contains('\0') {
                    return Err("Keychain 值不能为空、过长或包含无效字符".into());
                }
                self.supervisor.secrets.set(&account, secret)?;
            } else {
                let previous = self
                    .supervisor
                    .secrets
                    .get(&format!("{}:{previous_revision}:{key}", value.id))?;
                self.supervisor.secrets.set(&account, &previous)?;
            }
        }
        value.updated_at = now();
        store::save_definition(&*self.db.lock().map_err(|_| "项目库锁异常")?, &value)?;
        Ok(value)
    }
    pub fn archive_service(&self, id: &str) -> Result<(), String> {
        let _operation = self
            .supervisor
            .operations
            .lock()
            .map_err(|_| "服务操作锁异常")?;
        self.ensure_service_available(id)?;
        self.ensure_service_unreferenced(id)?;
        let db = self.db.lock().map_err(|_| "项目库锁异常")?;
        store::definition(&db, id)?;
        if store::runs(&db, id, 1)?
            .first()
            .is_some_and(|r| reconcile(r.clone()).state.active())
        {
            return Err("请先停止服务，再移除配置".into());
        }
        store::archive(&db, id)
    }
    pub fn service_history(&self, id: &str) -> Result<Vec<ServiceRun>, String> {
        Ok(
            store::runs(&*self.db.lock().map_err(|_| "项目库锁异常")?, id, 50)?
                .into_iter()
                .map(reconcile)
                .collect(),
        )
    }
    pub fn service_logs(&self, id: &str, limit: usize, query: &str) -> Result<LogPage, String> {
        if query.len() > 1024 {
            return Err("日志搜索词超过 1024 字节限制".into());
        }
        let run = store::run(&*self.db.lock().map_err(|_| "项目库锁异常")?, id)?;
        service_logs::read(
            Path::new(&run.log_path),
            run.id,
            limit,
            query,
            run.log_truncated,
        )
    }
    pub fn clear_service_log(&self, id: &str) -> Result<(), String> {
        let _operation = self
            .supervisor
            .operations
            .lock()
            .map_err(|_| "服务操作锁异常")?;
        let db = self.db.lock().map_err(|_| "项目库锁异常")?;
        let run = store::run(&db, id)?;
        if reconcile(run.clone()).state.active() {
            return Err("活动运行的日志不可清理".into());
        }
        let expected = self
            .supervisor
            .database
            .parent()
            .ok_or("数据目录不可用")?
            .join("logs/services")
            .join(format!("{}.log", run.id));
        if Path::new(&run.log_path) != expected {
            return Err("日志路径不符合当前数据目录，拒绝清理".into());
        }
        // Explicit single-run cleanup only; never recursively remove project files or logs.
        if expected.is_file() {
            fs::remove_file(&expected).map_err(|e| e.to_string())?;
        }
        storage::audit(&db, "clear_service_log", id, "confirmed_single_run")
    }
    pub fn prepare_service_action(
        &self,
        id: &str,
        action: &str,
    ) -> Result<ServiceActionPreview, String> {
        self.ensure_service_available(id)?;
        if !["start", "restart", "force_stop"].contains(&action) {
            return Err("不支持的服务操作".into());
        }
        let view = self
            .services()?
            .into_iter()
            .find(|v| v.definition.id == id)
            .ok_or("服务不存在")?;
        let active = view.current_run.as_ref().filter(|r| r.state.active());
        if action == "start" && active.is_some() {
            return Err("服务已有活动运行，禁止重复启动".into());
        }
        if action == "force_stop"
            && !active.is_some_and(|r| r.state == ServiceState::Stopping && anchor_valid(r))
        {
            return Err("只有已请求正常停止且身份可核验的托管运行可以强制停止".into());
        }
        if action == "force_stop"
            && active.is_some_and(|r| {
                r.stop_requested_at_ms.is_none_or(|at| {
                    now_millis().saturating_sub(at) < view.definition.stop_grace_seconds * 1000
                })
            })
        {
            return Err("服务仍在正常停止宽限期，请稍后再确认强制停止".into());
        }
        let token = Uuid::new_v4().to_string();
        let run_id = view.current_run.as_ref().map(|r| r.id.clone());
        let mut pending = self
            .supervisor
            .confirmations
            .lock()
            .map_err(|_| "确认锁异常")?;
        pending.retain(|_, (_, _, _, _, at)| now().saturating_sub(*at) <= 60);
        pending.insert(
            token.clone(),
            (
                id.into(),
                action.into(),
                view.definition.revision.clone(),
                run_id,
                now(),
            ),
        );
        let mut environment_keys: Vec<_> = view.definition.environment.keys().cloned().collect();
        environment_keys.extend(
            view.definition
                .secret_keys
                .iter()
                .map(|k| format!("{k}=[Keychain]")),
        );
        let command = if view.definition.shell_mode {
            format!("/bin/zsh -lc {:?}", view.definition.executable)
        } else {
            format!(
                "{:?} {}",
                view.definition.executable,
                view.definition
                    .args
                    .iter()
                    .map(|a| format!("{a:?}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        };
        Ok(ServiceActionPreview {
            token,
            action: action.into(),
            service: view.definition,
            run: view.current_run,
            command,
            environment_keys,
            expires_in_seconds: 60,
        })
    }
    pub fn confirm_service_action(&self, token: &str) -> Result<ServiceRun, String> {
        let _operation = self
            .supervisor
            .operations
            .lock()
            .map_err(|_| "服务操作锁异常")?;
        let (id, action, revision, run_id, issued) = self
            .supervisor
            .confirmations
            .lock()
            .map_err(|_| "确认锁异常")?
            .remove(token)
            .ok_or("服务操作确认已失效")?;
        if now().saturating_sub(issued) > 60 {
            return Err("服务操作确认已过期，请重新预览".into());
        }
        let definition = store::definition(&*self.db.lock().map_err(|_| "项目库锁异常")?, &id)?;
        self.ensure_service_available(&id)?;
        if definition.revision != revision {
            return Err("命令配置已改变，请重新确认".into());
        }
        let latest = self.service_history(&id)?.into_iter().next();
        if latest.as_ref().map(|r| &r.id) != run_id.as_ref() {
            return Err("运行实例已改变，请重新确认".into());
        }
        if action == "force_stop" {
            let run = latest.ok_or("没有运行实例")?;
            if run.state != ServiceState::Stopping {
                return Err("请先正常停止服务".into());
            }
            if run.stop_requested_at_ms.is_none_or(|at| {
                now_millis().saturating_sub(at) < definition.stop_grace_seconds * 1000
            }) {
                return Err("正常停止宽限期尚未结束".into());
            }
            self.signal_service_run(&run, libc::SIGKILL)?;
            return Ok(reconcile(run));
        }
        if action == "restart" {
            if let Some(run) = latest.filter(|r| r.state.active()) {
                self.stop_run(&run)?;
                let deadline =
                    Instant::now() + Duration::from_secs(definition.stop_grace_seconds + 2);
                while self
                    .service_history(&id)?
                    .first()
                    .is_some_and(|r| r.state.active())
                {
                    if Instant::now() > deadline {
                        return Err(
                            "原进程组未退出，已阻止重启；请检查状态或独立确认强制停止".into()
                        );
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
        self.start_run(definition, None)
    }
    pub fn stop_service(&self, id: &str) -> Result<ServiceRun, String> {
        let _operation = self
            .supervisor
            .operations
            .lock()
            .map_err(|_| "服务操作锁异常")?;
        self.ensure_service_available(id)?;
        let run = self
            .service_history(id)?
            .into_iter()
            .next()
            .ok_or("没有运行实例")?;
        if !run.state.active() {
            return Ok(run);
        }
        self.stop_run(&run)?;
        self.service_history(id)?
            .into_iter()
            .next()
            .ok_or("运行记录不存在".into())
    }
    pub(crate) fn stop_run(&self, run: &ServiceRun) -> Result<(), String> {
        self.signal_service_run(run, libc::SIGTERM)
    }
    fn signal_service_run(&self, run: &ServiceRun, signal: i32) -> Result<(), String> {
        if !anchor_valid(run) {
            return Err("runner 身份无法核验，禁止向进程组发送信号".into());
        }
        let db = self.db.lock().map_err(|_| "项目库锁异常")?;
        storage::audit(
            &db,
            if signal == libc::SIGKILL {
                "force_stop_service"
            } else {
                "stop_service"
            },
            &run.id,
            "requested",
        )?;
        // One stable, owned runner anchors the group. Never infer ownership from a port.
        if !anchor_valid(run) {
            return Err("runner 身份已经改变，已阻止信号".into());
        }
        if unsafe { libc::kill(-(run.pgid.ok_or("进程组不存在")? as i32), signal) } != 0 {
            return Err(format!(
                "进程组信号失败：{}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }
    pub(crate) fn start_run(
        &self,
        definition: ServiceDefinition,
        workspace_run_id: Option<String>,
    ) -> Result<ServiceRun, String> {
        let definition = validate(definition, self)?;
        for dependency in &definition.dependencies {
            if !self
                .service_history(dependency)?
                .first()
                .is_some_and(|r| r.state == ServiceState::Ready)
            {
                let run = store::failed_run(
                    &definition,
                    ServiceState::Blocked,
                    format!("依赖 {dependency} 尚未 READY，请通过工作区顺序启动"),
                    String::new(),
                );
                store::save_run(&*self.db.lock().map_err(|_| "项目库锁异常")?, &run)?;
                return Ok(run);
            }
        }
        if self
            .service_history(&definition.id)?
            .first()
            .is_some_and(|r| r.state.active())
        {
            return Err("服务已有活动运行".into());
        }
        let conflict = process::list_ports()?
            .into_iter()
            .find(|p| definition.expected_ports.contains(&p.port));
        let reserved = self.services()?.into_iter().find(|view| {
            view.current_run.as_ref().is_some_and(|r| r.state.active())
                && view
                    .definition
                    .expected_ports
                    .iter()
                    .any(|p| definition.expected_ports.contains(p))
        });
        if let Some(message) = conflict
            .map(|p| {
                format!(
                    "端口 {} 被 PID {}（{}）占用，未启动服务",
                    p.port, p.pid, p.name
                )
            })
            .or_else(|| {
                reserved.map(|v| format!("预期端口已由服务 {} 的活动运行保留", v.definition.name))
            })
        {
            let run = store::failed_run(&definition, ServiceState::Blocked, message, String::new());
            store::save_run(&*self.db.lock().map_err(|_| "项目库锁异常")?, &run)?;
            return Ok(run);
        }
        let (program, arguments) = if definition.shell_mode {
            (
                PathBuf::from("/bin/zsh"),
                vec!["-lc".into(), definition.executable.clone()],
            )
        } else {
            match resolve_program(&definition.executable) {
                Ok(program) => (program, definition.args.clone()),
                Err(error) => {
                    let run =
                        store::failed_run(&definition, ServiceState::Crashed, error, String::new());
                    store::save_run(&*self.db.lock().map_err(|_| "项目库锁异常")?, &run)?;
                    return Ok(run);
                }
            }
        };
        let mut environment = BTreeMap::new();
        for key in [
            "HOME",
            "USER",
            "LOGNAME",
            "LANG",
            "LC_ALL",
            "TMPDIR",
            "SHELL",
            "JAVA_HOME",
        ] {
            if let Ok(value) = std::env::var(key) {
                environment.insert(key.into(), value);
            }
        }
        environment.insert("PATH".into(), search_path());
        environment.insert("TERM".into(), "dumb".into());
        environment.extend(definition.environment.clone());
        let mut secret_values = Vec::new();
        for key in &definition.secret_keys {
            let value = self
                .supervisor
                .secrets
                .get(&format!("{}:{}:{key}", definition.id, definition.revision))?;
            secret_values.push(value.clone());
            environment.insert(key.clone(), value);
        }
        let directory = self
            .supervisor
            .database
            .parent()
            .ok_or("数据目录不可用")?
            .join("logs/services");
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
        let mut run = store::failed_run(
            &definition,
            ServiceState::Starting,
            String::new(),
            String::new(),
        );
        run.error = None;
        run.workspace_run_id = workspace_run_id;
        run.stopped_at = None;
        run.log_path = directory
            .join(format!("{}.log", run.id))
            .to_string_lossy()
            .into();
        let spec = WorkerSpec {
            database: self.supervisor.database.clone(),
            run_id: run.id.clone(),
            definition,
            program,
            arguments,
            environment,
            secret_values,
        };
        let bytes = serde_json::to_vec(&spec).map_err(|_| "无法编码 runner 配置")?;
        if bytes.len() > 262_144 {
            return Err("展开后的服务配置超过 runner 安全大小限制".into());
        }
        store::save_run(&*self.db.lock().map_err(|_| "项目库锁异常")?, &run)?;
        let mut command = Command::new(&self.supervisor.launcher.executable);
        command
            .args(&self.supervisor.launcher.arguments)
            .process_group(0)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(_) => {
                run.state = ServiceState::Crashed;
                run.stopped_at = Some(now());
                run.error = Some("无法创建服务 runner".into());
                store::save_run(&*self.db.lock().map_err(|_| "项目库锁异常")?, &run)?;
                return Ok(run);
            }
        };
        let deadline = Instant::now() + Duration::from_secs(2);
        let identity = loop {
            if let Some(p) = process::identity(child.id()) {
                break p;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err("无法核验新建 runner 的身份，已停止启动".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        run.worker_pid = Some(child.id());
        run.pgid = Some(identity.pgid);
        run.worker_start = Some(identity.start);
        store::save_run(&*self.db.lock().map_err(|_| "项目库锁异常")?, &run)?;
        let mut stdin = child.stdin.take().ok_or("runner 输入不可用")?;
        if stdin.write_all(&bytes).is_err() {
            let _ = child.kill();
            let _ = child.wait();
            return Err("runner 未能接收配置".into());
        }
        drop(stdin);
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        storage::audit(
            &*self.db.lock().map_err(|_| "审计锁异常")?,
            "start_service",
            &run.id,
            "confirmed",
        )?;
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let current = store::run(&*self.db.lock().map_err(|_| "项目库锁异常")?, &run.id)?;
            if current.pid.is_some()
                || !current.state.active()
                || !anchor_valid(&current)
                || Instant::now() > deadline
            {
                return Ok(reconcile(current));
            }
            std::thread::sleep(Duration::from_millis(30));
        }
    }
}
