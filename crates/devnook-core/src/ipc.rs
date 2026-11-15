use crate::{model::*, service_model::*, supervisor::RunnerLauncher, Core};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
            net::{UnixListener, UnixStream},
            process::CommandExt,
        },
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

const VERSION: u32 = 10;
#[derive(Serialize, Deserialize)]
struct Request {
    version: u32,
    method: String,
    params: Value,
    #[serde(default)]
    context: crate::access::RequestContext,
}
#[derive(Serialize, Deserialize)]
struct Response {
    result: Option<Value>,
    error: Option<String>,
}

pub fn private_directory(directory: &Path) -> Result<PathBuf, String> {
    if directory.exists() {
        let metadata = fs::symlink_metadata(directory).map_err(|e| e.to_string())?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata.uid() != unsafe { libc::geteuid() }
        {
            return Err("数据目录必须属于当前用户且不能是符号链接".into());
        }
    } else {
        fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    }
    fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    directory.canonicalize().map_err(|e| e.to_string())
}
fn peer_is_current_user(stream: &UnixStream) -> bool {
    let (mut uid, mut gid) = (0, 0);
    unsafe {
        libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) == 0 && uid == libc::geteuid()
    }
}

pub struct CoreClient {
    directory: PathBuf,
    launcher: RunnerLauncher,
    boot: Mutex<()>,
    context: crate::access::RequestContext,
    pub(crate) browser_evidence: Mutex<crate::browser::BrowserEvidenceCache>,
}
impl CoreClient {
    pub fn new(directory: &Path, executable: PathBuf) -> Result<Self, String> {
        Self::with_launcher(
            directory,
            RunnerLauncher {
                executable,
                arguments: vec!["--devnook-core".into(), directory.to_string_lossy().into()],
            },
        )
    }
    /// A GUI can show connection errors and retry without failing its window setup.
    /// Directory validation stays eager; socket identity and protocol checks still run
    /// for every request. CLI clients retain the eager handshake in `new`.
    pub fn new_deferred(directory: &Path, executable: PathBuf) -> Result<Self, String> {
        Self::from_launcher(
            directory,
            RunnerLauncher {
                executable,
                arguments: vec!["--devnook-core".into(), directory.to_string_lossy().into()],
            },
        )
    }
    pub fn with_launcher(directory: &Path, launcher: RunnerLauncher) -> Result<Self, String> {
        let client = Self::from_launcher(directory, launcher)?;
        let _: Value = client.invoke("ping", json!({}))?;
        Ok(client)
    }
    fn from_launcher(directory: &Path, launcher: RunnerLauncher) -> Result<Self, String> {
        Ok(Self {
            directory: private_directory(directory)?,
            launcher,
            boot: Mutex::new(()),
            context: Default::default(),
            browser_evidence: Mutex::new(Default::default()),
        })
    }
    pub fn with_context(mut self, context: crate::access::RequestContext) -> Self {
        self.context = context;
        self
    }
    fn connect(&self) -> Result<UnixStream, String> {
        let socket = self.directory.join("core.sock");
        let metadata = fs::symlink_metadata(&socket).map_err(|_| "Core 尚未启动")?;
        if !metadata.file_type().is_socket()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.permissions().mode() & 0o077 != 0
        {
            return Err("Core socket 所有者或权限不安全".into());
        }
        let stream = UnixStream::connect(&socket).map_err(|_| "Core 连接失败")?;
        if !peer_is_current_user(&stream) {
            return Err("Core 用户身份不匹配".into());
        }
        stream
            .set_read_timeout(Some(Duration::from_secs(45)))
            .map_err(|e| e.to_string())?;
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| e.to_string())?;
        Ok(stream)
    }
    pub fn invoke<T: DeserializeOwned>(&self, method: &str, params: Value) -> Result<T, String> {
        let mut stream = match self.connect() {
            Ok(stream) => stream,
            Err(_) => {
                let _boot = self.boot.lock().map_err(|_| "Core 启动锁异常")?;
                if let Ok(stream) = self.connect() {
                    stream
                } else {
                    let mut child = Command::new(&self.launcher.executable)
                        .args(&self.launcher.arguments)
                        .process_group(0)
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn()
                        .map_err(|_| "无法启动后台 Core")?;
                    std::thread::spawn(move || {
                        let _ = child.wait();
                    });
                    let deadline = Instant::now() + Duration::from_secs(8);
                    loop {
                        if let Ok(stream) = self.connect() {
                            break stream;
                        }
                        if Instant::now() > deadline {
                            return Err("后台 Core 启动超时，请检查数据目录或重建桌面应用".into());
                        }
                        std::thread::sleep(Duration::from_millis(50));
                    }
                }
            }
        };
        let bytes = serde_json::to_vec(&Request {
            version: VERSION,
            method: method.into(),
            params,
            context: self.context.clone(),
        })
        .map_err(|_| "请求编码失败")?;
        if bytes.len() > 262_144 {
            return Err("Core 请求超过大小限制".into());
        }
        stream.write_all(&bytes).map_err(|_| "Core 请求发送失败")?;
        stream
            .shutdown(std::net::Shutdown::Write)
            .map_err(|_| "Core 请求关闭失败")?;
        let mut bytes = Vec::new();
        stream
            .take(8_388_609)
            .read_to_end(&mut bytes)
            .map_err(|_| "Core 响应中断；写操作不会自动重试，请刷新核对结果")?;
        if bytes.len() > 8_388_608 {
            return Err("Core 响应超过限制，请缩小查询范围".into());
        }
        let response: Response = serde_json::from_slice(&bytes).map_err(|_| "Core 响应无效")?;
        if let Some(error) = response.error {
            return Err(error);
        }
        serde_json::from_value(response.result.unwrap_or(Value::Null))
            .map_err(|e| format!("Core 数据结构不兼容：{e}"))
    }
    pub fn projects(&self) -> Result<Vec<Project>, String> {
        self.invoke("list_projects", json!({}))
    }
    pub fn settings(&self) -> Result<Settings, String> {
        self.invoke("get_settings", json!({}))
    }
    pub fn save_settings(&self, settings: Settings) -> Result<(), String> {
        self.invoke("save_settings", json!({"settings":settings}))
    }
    pub fn edit_project(&self, edit: ProjectEdit) -> Result<Vec<Project>, String> {
        self.invoke("edit_project", json!({"edit":edit}))
    }
    pub fn remove_project(&self, id: &str) -> Result<(), String> {
        self.invoke("remove_project", json!({"id":id}))
    }
    pub fn scan_start(&self, root: String, depth: usize) -> Result<String, String> {
        self.invoke("scan_start", json!({"root":root,"depth":depth}))
    }
    pub fn scan_status(&self, id: &str) -> Result<ScanProgress, String> {
        self.invoke("scan_status", json!({"id":id}))
    }
    pub fn scan_cancel(&self, id: &str) -> Result<(), String> {
        self.invoke("scan_cancel", json!({"id":id}))
    }
    pub fn import_projects(&self, scan_id: &str, ids: Vec<String>) -> Result<Vec<Project>, String> {
        self.invoke("import_projects", json!({"scan_id":scan_id,"ids":ids}))
    }
    pub fn ports(&self) -> Result<Vec<PortBinding>, String> {
        self.invoke("list_ports", json!({}))
    }
    pub fn inspect(&self, pid: u32) -> Result<ProcessDetail, String> {
        self.invoke("inspect_process", json!({"pid":pid}))
    }
    pub fn assign_process(
        &self,
        pid: u32,
        fingerprint: &str,
        project_id: &str,
    ) -> Result<ProcessDetail, String> {
        self.invoke(
            "assign_process",
            json!({"pid":pid,"fingerprint":fingerprint,"project_id":project_id}),
        )
    }
    pub fn prepare_stop(&self, pid: u32) -> Result<StopPreview, String> {
        self.invoke("prepare_stop", json!({"pid":pid}))
    }
    pub fn confirm_stop(&self, token: &str, confirmed_pid: u32) -> Result<(), String> {
        self.invoke(
            "confirm_stop",
            json!({"token":token,"confirmed_pid":confirmed_pid}),
        )
    }
    pub fn open_project(&self, id: &str, target: &str) -> Result<(), String> {
        self.invoke("open_project", json!({"id":id,"target":target}))
    }
    pub fn services(&self) -> Result<Vec<ServiceView>, String> {
        self.invoke("list_services", json!({}))
    }
    /// Older v10 daemons lack the external-configuration RPC. Only local GUI/CLI
    /// preview reads may be composed; control confirmations stay in the daemon.
    pub fn external_service_configuration(
        &self,
        pid: u32,
    ) -> Result<crate::external_service::ExternalServiceConfiguration, String> {
        match self.invoke("external_service_configuration", json!({"pid":pid})) {
            Err(error)
                if error == "未知 Core 操作"
                    && ["GUI", "CLI"].contains(&self.context.origin.as_str()) => {}
            result => return result,
        }
        let process = self.inspect(pid)?;
        let projects = self.projects()?;
        crate::external_service::validate_external_service(&process, &projects)?;
        let drafts = self.invoke("service_drafts", json!({"project_id":process.project_id}))?;
        let project = projects
            .iter()
            .find(|p| Some(&p.candidate.id) == process.project_id.as_ref())
            .ok_or("项目归属不存在")?;
        let mut configuration = crate::external_service::configuration(
            process.clone(),
            drafts,
            false,
            Path::new(&project.candidate.canonical_path),
        )?;
        // Manifest reads may take time. Reject a reused PID, moved process, or
        // changed listener rather than presenting a draft for an expired snapshot.
        let current = self.inspect(pid)?;
        crate::external_service::validate_external_service(&current, &projects)?;
        let listeners = |detail: &ProcessDetail| {
            detail
                .bindings
                .iter()
                .map(|b| (b.pid, b.port, b.address.clone(), b.family.clone()))
                .collect::<std::collections::BTreeSet<_>>()
        };
        if current.pid != process.pid
            || current.fingerprint != process.fingerprint
            || current.project_id != process.project_id
            || current.cwd != process.cwd
            || listeners(&current) != listeners(&process)
        {
            return Err("外部服务身份已变化，请重新预览".into());
        }
        configuration.process = current;
        Ok(configuration)
    }
    pub fn save_service(&self, input: ServiceSave) -> Result<ServiceDefinition, String> {
        self.invoke("save_service", json!({"input":input}))
    }
    pub fn archive_service(&self, id: &str) -> Result<(), String> {
        self.invoke("archive_service", json!({"id":id}))
    }
    pub fn service_history(&self, id: &str) -> Result<Vec<ServiceRun>, String> {
        self.invoke("service_history", json!({"id":id}))
    }
    pub fn service_logs(&self, id: &str, limit: usize, query: &str) -> Result<LogPage, String> {
        self.invoke("service_logs", json!({"id":id,"limit":limit,"query":query}))
    }
    pub fn clear_service_log(&self, id: &str) -> Result<(), String> {
        self.invoke("clear_service_log", json!({"id":id}))
    }
    pub fn prepare_service_action(
        &self,
        id: &str,
        action: &str,
    ) -> Result<ServiceActionPreview, String> {
        self.invoke("prepare_service_action", json!({"id":id,"action":action}))
    }
    pub fn confirm_service_action(&self, token: &str) -> Result<ServiceRun, String> {
        self.invoke("confirm_service_action", json!({"token":token}))
    }
    pub fn stop_service(&self, id: &str) -> Result<ServiceRun, String> {
        self.invoke("stop_service", json!({"id":id}))
    }
    pub fn workspaces(&self) -> Result<Vec<crate::workspace::WorkspaceGroup>, String> {
        self.invoke("list_workspaces", json!({}))
    }
    pub fn save_workspace(
        &self,
        group: crate::workspace::WorkspaceGroup,
    ) -> Result<crate::workspace::WorkspaceGroup, String> {
        self.invoke("save_workspace", json!({"group":group}))
    }
    pub fn workspace_history(
        &self,
        id: &str,
    ) -> Result<Vec<crate::workspace::WorkspaceRun>, String> {
        self.invoke("workspace_history", json!({"id":id}))
    }
    pub fn prepare_workspace_action(
        &self,
        id: &str,
        action: &str,
    ) -> Result<crate::workspace::WorkspacePreview, String> {
        self.invoke("prepare_workspace_action", json!({"id":id,"action":action}))
    }
    pub fn confirm_workspace_action(
        &self,
        token: &str,
    ) -> Result<crate::workspace::WorkspaceRun, String> {
        self.invoke("confirm_workspace_action", json!({"token":token}))
    }
}

fn param<T: DeserializeOwned>(request: &Request, key: &str) -> Result<T, String> {
    serde_json::from_value(
        request
            .params
            .get(key)
            .cloned()
            .ok_or_else(|| format!("缺少参数 {key}"))?,
    )
    .map_err(|_| format!("参数 {key} 无效"))
}
fn value<T: Serialize>(value: Result<T, String>) -> Result<Value, String> {
    serde_json::to_value(value?).map_err(|_| "响应编码失败".into())
}
fn dispatch(core: &Arc<Core>, request: Request, shutdown: &AtomicBool) -> Result<Value, String> {
    if request.version != VERSION {
        return Err("Core 协议版本不一致，请关闭旧版本后台 Core 后重试".into());
    }
    // Local GUI/CLI and MCP writes share one authorization boundary. Context comes from
    // the adapter, never from MCP tool arguments. The private UDS is a same-user boundary.
    let _gate = core.access_gate.lock().map_err(|_| "授权锁异常")?;
    let client = match core.authorize(&request.context, &request.method, &request.params) {
        Ok(client) => client,
        Err(error) => {
            core.record_access(
                &request.context,
                "unverified",
                &request.method,
                &request.params,
                false,
            )?;
            return Err(error);
        }
    };
    let result = dispatch_authorized(core, &request, shutdown);
    if let Ok(value) = &result {
        core.bind_agent_confirmation(
            &request.context,
            &client,
            &request.method,
            &request.params,
            value,
        )?;
    }
    if request.method != "ping"
        && (request.context.origin == "MCP"
            || !["GUI", "CLI"].contains(&request.context.origin.as_str())
            || ![
                "list_projects",
                "get_settings",
                "list_development_apps",
                "inspect_development_app",
                "list_services",
                "service_drafts",
                "external_service_configuration",
                "parse_service_command",
                "list_ports",
                "inspect_process",
                "service_history",
                "service_logs",
                "list_workspaces",
                "workspace_history",
                "scan_status",
                "agent_grants",
                "access_audits",
            ]
            .contains(&request.method.as_str()))
    {
        core.record_access(
            &request.context,
            &client,
            &request.method,
            &request.params,
            result.is_ok(),
        )?;
    }
    result
}
fn dispatch_authorized(
    core: &Arc<Core>,
    request: &Request,
    shutdown: &AtomicBool,
) -> Result<Value, String> {
    match request.method.as_str() {
        "ping" => Ok(json!({"protocol": VERSION, "pid":std::process::id()})),
        "list_projects" => value(core.projects()),
        "get_settings" => value(core.settings()),
        "list_development_apps" => value(core.development_apps()),
        "inspect_development_app" => {
            value(core.inspect_development_app(&param::<String>(request, "path")?))
        }
        "remember_development_app" => {
            value(core.remember_development_app(&param::<String>(request, "path")?))
        }
        "set_project_app" => value(core.set_project_app(
            &param::<String>(request, "id")?,
            &param::<String>(request, "target")?,
        )),
        "save_settings" => value(core.save_settings(param(&request, "settings")?)),
        "edit_project" => value(core.edit_project(param(&request, "edit")?)),
        "remove_project" => value(core.remove_project(&param::<String>(&request, "id")?)),
        "scan_start" => value(core.scan_start(param(&request, "root")?, param(&request, "depth")?)),
        "scan_status" => value(core.scan_status(&param::<String>(&request, "id")?)),
        "scan_cancel" => value(core.scan_cancel(&param::<String>(&request, "id")?)),
        "import_projects" => value(core.import_projects(
            &param::<String>(&request, "scan_id")?,
            param(&request, "ids")?,
        )),
        "prepare_import_projects" => {
            let scan_id: String = param(request, "scan_id")?;
            let ids: Vec<String> = param(request, "ids")?;
            let progress = core.scan_status(&scan_id)?;
            if progress.status != "completed"
                || ids.is_empty()
                || ids
                    .iter()
                    .any(|id| !progress.candidates.iter().any(|c| &c.id == id))
            {
                return Err("请等待扫描完成并选择有效的候选 ID".into());
            }
            Ok(
                json!({"token":uuid::Uuid::new_v4().to_string(),"scan_id":scan_id,"candidates":progress.candidates.into_iter().filter(|c|ids.contains(&c.id)).collect::<Vec<_>>(),"expires_in_seconds":60}),
            )
        }
        "list_ports" => value(core.ports()),
        "inspect_process" => value(core.inspect(param(&request, "pid")?)),
        "assign_process" => value(core.assign_process(
            param(&request, "pid")?,
            &param::<String>(&request, "fingerprint")?,
            &param::<String>(&request, "project_id")?,
        )),
        "prepare_stop" => value(core.prepare_stop(param(&request, "pid")?)),
        "confirm_stop" => value(core.confirm_stop(
            &param::<String>(&request, "token")?,
            param(&request, "confirmed_pid")?,
        )),
        "open_project" => value(core.open_project(
            &param::<String>(&request, "id")?,
            &param::<String>(&request, "target")?,
        )),
        "list_services" => value(core.services()),
        "service_drafts" => value(core.service_drafts(&param::<String>(request, "project_id")?)),
        "external_service_configuration" => {
            value(core.external_service_configuration(param(request, "pid")?))
        }
        "prepare_external_service_stop" => {
            value(core.prepare_external_service_stop(param(request, "pid")?))
        }
        "prepare_external_service_handoff" => value(core.prepare_external_service_handoff(
            &param::<String>(request, "id")?,
            &param::<String>(request, "revision")?,
            param(request, "pid")?,
            &param::<String>(request, "fingerprint")?,
        )),
        "confirm_external_service_handoff" => value(core.confirm_external_service_handoff(
            &param::<String>(request, "token")?,
            param(request, "confirmed_pid")?,
        )),
        "parse_service_command" => value(crate::service_drafts::parse_command(&param::<String>(
            request, "command",
        )?)),
        "save_service" => value(core.save_service(param(&request, "input")?)),
        "archive_service" => value(core.archive_service(&param::<String>(&request, "id")?)),
        "service_history" => value(core.service_history(&param::<String>(&request, "id")?)),
        "service_logs" => value(core.service_logs(
            &param::<String>(&request, "id")?,
            param(&request, "limit")?,
            &param::<String>(&request, "query")?,
        )),
        "clear_service_log" => value(core.clear_service_log(&param::<String>(&request, "id")?)),
        "prepare_service_action" => value(core.prepare_service_action(
            &param::<String>(&request, "id")?,
            &param::<String>(&request, "action")?,
        )),
        "confirm_service_action" => {
            value(core.confirm_service_action(&param::<String>(&request, "token")?))
        }
        "stop_service" => value(core.stop_service(&param::<String>(&request, "id")?)),
        "list_workspaces" => value(core.workspaces()),
        "save_workspace" => value(core.save_workspace(param(request, "group")?)),
        "remove_workspace" => value(core.remove_workspace(&param::<String>(request, "id")?)),
        "workspace_history" => value(core.workspace_history(&param::<String>(request, "id")?)),
        "prepare_workspace_action" => value(core.prepare_workspace_action(
            &param::<String>(request, "id")?,
            &param::<String>(request, "action")?,
        )),
        "confirm_workspace_action" => {
            value(core.confirm_workspace_action(&param::<String>(request, "token")?))
        }
        "agent_grants" => value(core.agent_grants()),
        "create_agent_grant" => value(core.create_agent_grant(param(request, "input")?)),
        "revoke_agent_grant" => value(core.revoke_agent_grant(&param::<String>(request, "id")?)),
        "access_audits" => value(core.access_audits()),
        "shutdown" => {
            if !core.workspace_jobs()?.is_empty() {
                return Err("工作区仍在操作，不能退出 Core".into());
            }
            if core
                .services()?
                .iter()
                .any(|v| v.current_run.as_ref().is_some_and(|r| r.state.active()))
            {
                return Err("还有活动服务，不能退出 Core".into());
            }
            shutdown.store(true, Ordering::SeqCst);
            Ok(Value::Null)
        }
        _ => Err("未知 Core 操作".into()),
    }
}

pub fn serve(directory: &Path, launcher: RunnerLauncher) -> Result<(), String> {
    let directory = private_directory(directory)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(directory.join("core.lock"))
        .map_err(|e| e.to_string())?;
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Ok(());
    }
    let socket = directory.join("core.sock");
    if socket.exists() {
        let metadata = fs::symlink_metadata(&socket).map_err(|e| e.to_string())?;
        if !metadata.file_type().is_socket() || metadata.uid() != unsafe { libc::geteuid() } {
            return Err("Core socket 路径被非本人 socket 占用".into());
        }
        fs::remove_file(&socket).map_err(|e| e.to_string())?;
    }
    let core = Arc::new(Core::with_runtime(
        &directory.join("devnook.db"),
        launcher,
        Arc::new(crate::secrets::KeychainStore),
    )?);
    // Capture interrupted jobs before accepting requests, so newly confirmed jobs cannot
    // accidentally be classified as interrupted by the recovery thread.
    let interrupted = core.workspace_jobs()?;
    let listener = UnixListener::bind(&socket).map_err(|e| format!("无法创建 Core UDS：{e}"))?;
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let shutdown = Arc::new(AtomicBool::new(false));
    let recovering = core.clone();
    std::thread::spawn(move || {
        let _ = recovering.recover_workspace_runs(interrupted);
    });
    let count = Arc::new(AtomicUsize::new(0));
    let mut last_request = Instant::now();
    loop {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                // On macOS an accepted socket may inherit O_NONBLOCK. A large JSON response
                // must not be truncated at the first full socket buffer.
                stream.set_nonblocking(false).map_err(|e| e.to_string())?;
                last_request = Instant::now();
                if !peer_is_current_user(&stream) || count.load(Ordering::Relaxed) >= 64 {
                    continue;
                }
                count.fetch_add(1, Ordering::Relaxed);
                let count = count.clone();
                let core = core.clone();
                let shutdown = shutdown.clone();
                std::thread::spawn(move || {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
                    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
                    let result = (|| {
                        let mut bytes = Vec::new();
                        (&mut stream)
                            .take(262_145)
                            .read_to_end(&mut bytes)
                            .map_err(|_| "读取 Core 请求失败")?;
                        if bytes.len() > 262_144 {
                            return Err("Core 请求超过限制".into());
                        }
                        let request: Request =
                            serde_json::from_slice(&bytes).map_err(|_| "Core 请求无效")?;
                        dispatch(&core, request, &shutdown)
                    })();
                    let response = match result {
                        Ok(value) => Response {
                            result: Some(value),
                            error: None,
                        },
                        Err(error) => Response {
                            result: None,
                            error: Some(error),
                        },
                    };
                    if let Ok(bytes) = serde_json::to_vec(&response) {
                        let _ = stream.write_all(&bytes);
                    }
                    count.fetch_sub(1, Ordering::Relaxed);
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if last_request.elapsed() > Duration::from_secs(300)
                    && count.load(Ordering::Relaxed) == 0
                    && core.workspace_jobs()?.is_empty()
                    && !core
                        .services()?
                        .iter()
                        .any(|v| v.current_run.as_ref().is_some_and(|r| r.state.active()))
                {
                    break;
                }
                std::thread::sleep(Duration::from_millis(40));
            }
            Err(error) => return Err(format!("Core UDS 监听失败：{error}")),
        }
    }
    drop(listener);
    fs::remove_file(&socket).map_err(|e| e.to_string())?;
    drop(lock);
    Ok(())
}
