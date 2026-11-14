pub mod access;
pub mod browser;
pub mod external_service;
pub mod health;
pub mod ide;
pub mod ipc;
pub mod model;
pub mod process;
mod process_kind;
pub mod scanner;
pub mod secrets;
pub mod service_drafts;
pub mod service_logs;
pub mod service_model;
pub mod service_storage;
pub mod service_worker;
pub mod storage;
pub mod supervisor;
pub mod workspace;

use model::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use uuid::Uuid;

pub struct Core {
    db: Mutex<Connection>,
    scans: Mutex<HashMap<String, Arc<scanner::ScanJob>>>,
    stops: Mutex<HashMap<String, (ProcessDetail, u64)>>,
    external_handoffs: Mutex<HashMap<String, external_service::PendingHandoff>>,
    first_seen: Mutex<HashMap<String, u64>>,
    protected: HashSet<u32>,
    pub(crate) supervisor: supervisor::Supervisor,
    pub(crate) workspace_confirmations: Mutex<HashMap<String, workspace::WorkspacePreview>>,
    pub(crate) agent_confirmations: Mutex<HashMap<String, (String, String, String, String)>>,
    pub(crate) access_gate: Mutex<()>,
    pub(crate) agent_scans: Mutex<HashMap<String, String>>,
}

impl Core {
    pub fn new(database: &Path) -> Result<Self, String> {
        Self::with_runtime(
            database,
            supervisor::RunnerLauncher::current()?,
            Arc::new(secrets::KeychainStore),
        )
    }
    pub fn with_runtime(
        database: &Path,
        launcher: supervisor::RunnerLauncher,
        secrets: Arc<dyn secrets::SecretStore>,
    ) -> Result<Self, String> {
        let mut protected: HashSet<_> = if cfg!(target_os = "macos") {
            process::parent_chain(std::process::id())
                .into_iter()
                .map(|p| p.pid)
                .collect()
        } else {
            HashSet::new()
        };
        protected.insert(std::process::id());
        Ok(Self {
            db: Mutex::new(storage::open(database)?),
            scans: Mutex::new(HashMap::new()),
            stops: Mutex::new(HashMap::new()),
            external_handoffs: Mutex::new(HashMap::new()),
            first_seen: Mutex::new(HashMap::new()),
            protected,
            workspace_confirmations: Mutex::new(HashMap::new()),
            agent_confirmations: Mutex::new(HashMap::new()),
            access_gate: Mutex::new(()),
            agent_scans: Mutex::new(HashMap::new()),
            supervisor: supervisor::Supervisor {
                database: database.to_path_buf(),
                launcher,
                secrets,
                operations: Mutex::new(()),
                confirmations: Mutex::new(HashMap::new()),
            },
        })
    }
    pub fn projects(&self) -> Result<Vec<Project>, String> {
        storage::list(&*self.db.lock().map_err(|_| "项目库锁异常")?)
    }
    pub fn settings(&self) -> Result<Settings, String> {
        storage::settings(&*self.db.lock().map_err(|_| "设置锁异常")?)
    }
    pub fn development_apps(&self) -> Result<ide::DevelopmentApps, String> {
        let (settings, projects, remembered) = {
            let db = self.db.lock().map_err(|_| "应用列表锁异常")?;
            (
                storage::settings(&db)?,
                storage::list(&db)?,
                storage::remembered_apps(&db)?,
            )
        };
        ide::discover_with_remembered(&settings, &projects, &remembered)
    }
    pub fn inspect_development_app(&self, path: &str) -> Result<ide::DevelopmentApp, String> {
        ide::inspect_app(Path::new(path))
    }
    pub fn remember_development_app(&self, path: &str) -> Result<ide::DevelopmentApp, String> {
        let app = self.inspect_development_app(path)?;
        storage::remember_app(&*self.db.lock().map_err(|_| "应用列表锁异常")?, &app)?;
        Ok(app)
    }
    pub fn set_project_app(&self, id: &str, target: &str) -> Result<Vec<Project>, String> {
        let app = self.inspect_development_app(
            target
                .strip_prefix("APP:")
                .ok_or("请选择已安装的开发应用")?,
        )?;
        if app.kind == "TERMINAL" {
            return Err("终端请通过目录与终端入口打开".into());
        }
        let db = self.db.lock().map_err(|_| "项目库锁异常")?;
        let project = storage::list(&db)?
            .into_iter()
            .find(|p| p.candidate.id == id)
            .ok_or("项目不存在")?;
        storage::edit(
            &db,
            ProjectEdit {
                id: id.into(),
                name: project.candidate.name,
                default_ide: app.id,
                category: project.category,
                is_favorite: project.is_favorite,
            },
        )
    }
    pub fn save_settings(&self, settings: Settings) -> Result<(), String> {
        storage::save_settings(&*self.db.lock().map_err(|_| "设置锁异常")?, &settings)
    }
    pub fn edit_project(&self, edit: ProjectEdit) -> Result<Vec<Project>, String> {
        storage::edit(&*self.db.lock().map_err(|_| "项目库锁异常")?, edit)
    }
    pub fn remove_project(&self, id: &str) -> Result<(), String> {
        let _operation = self
            .supervisor
            .operations
            .lock()
            .map_err(|_| "服务操作锁异常")?;
        for view in self
            .services()?
            .iter()
            .filter(|s| s.definition.project_id == id)
        {
            self.ensure_service_available(&view.definition.id)?;
            self.ensure_service_unreferenced(&view.definition.id)?;
        }
        if self.services()?.iter().any(|v| {
            v.definition.project_id == id
                && v.current_run.as_ref().is_some_and(|r| r.state.active())
        }) {
            return Err("该项目仍有活动服务，请先停止服务再移除档案".into());
        }
        let db = self.db.lock().map_err(|_| "项目库锁异常")?;
        storage::audit(&db, "remove_project", id, "record_only")?;
        db.execute(
            "UPDATE service_definitions SET archived=1 WHERE project_id=?1",
            [id],
        )
        .map_err(|e| e.to_string())?;
        db.execute("DELETE FROM projects WHERE id=?1", [id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn scan_start(&self, root: String, depth: usize) -> Result<String, String> {
        if depth > 12 {
            return Err("扫描深度应为 0（特征扫描）或 1–12（深度补扫）".into());
        }
        let root = PathBuf::from(root)
            .canonicalize()
            .map_err(|e| format!("扫描目录不可访问：{e}"))?;
        if !root.is_dir() {
            return Err("请选择目录".into());
        }
        let projects = self.projects()?;
        let imported = projects
            .iter()
            .map(|p| p.candidate.canonical_path.clone())
            .collect();
        let known = projects
            .iter()
            .flat_map(|p| {
                std::iter::once(&p.candidate.canonical_path)
                    .chain(p.candidate.components.iter().map(|c| &c.canonical_path))
            })
            .map(PathBuf::from)
            .collect();
        let mut scans = self.scans.lock().map_err(|_| "扫描锁异常")?;
        if scans
            .values()
            .any(|s| s.progress.lock().unwrap().status == "scanning")
        {
            return Err("已有扫描正在运行，请先取消".into());
        }
        scans.clear();
        let id = Uuid::new_v4().to_string();
        let job = Arc::new(scanner::ScanJob {
            cancel: AtomicBool::new(false),
            progress: Mutex::new(ScanProgress {
                id: id.clone(),
                status: "scanning".into(),
                ..Default::default()
            }),
        });
        scans.insert(id.clone(), job.clone());
        std::thread::spawn(move || scanner::run_with_known(root, depth, imported, known, job));
        Ok(id)
    }
    pub fn scan_status(&self, id: &str) -> Result<ScanProgress, String> {
        let scans = self.scans.lock().map_err(|_| "扫描锁异常")?;
        let job = scans.get(id).ok_or("扫描会话已失效")?;
        let result = job.progress.lock().map_err(|_| "扫描进度异常")?.clone();
        Ok(result)
    }
    pub fn scan_cancel(&self, id: &str) -> Result<(), String> {
        let scans = self.scans.lock().map_err(|_| "扫描锁异常")?;
        let job = scans.get(id).ok_or("扫描会话已失效")?;
        job.cancel.store(true, Ordering::SeqCst);
        job.progress.lock().map_err(|_| "扫描进度异常")?.status = "cancelled".into();
        Ok(())
    }
    pub fn import_projects(&self, scan_id: &str, ids: Vec<String>) -> Result<Vec<Project>, String> {
        // Serialize cancellation and commit so a cancelled preview cannot race an import.
        let scans = self.scans.lock().map_err(|_| "扫描锁异常")?;
        let job = scans.get(scan_id).ok_or("扫描会话已失效")?;
        let progress = job.progress.lock().map_err(|_| "扫描进度异常")?;
        if job.cancel.load(Ordering::SeqCst) || progress.status != "completed" {
            return Err("只有已完成且未取消的扫描可以导入".into());
        }
        let unique: HashSet<_> = ids.into_iter().collect();
        let candidates: Vec<_> = progress
            .candidates
            .iter()
            .filter(|c| unique.contains(&c.id))
            .cloned()
            .collect();
        if candidates.len() != unique.len() {
            return Err("候选项目已失效，请重新扫描".into());
        }
        for candidate in &candidates {
            let canonical = Path::new(&candidate.canonical_path)
                .canonicalize()
                .map_err(|_| "候选目录已移除")?;
            if canonical.to_string_lossy() != candidate.canonical_path {
                return Err("候选目录已改变，请重新扫描".into());
            }
            for component in &candidate.components {
                let path = Path::new(&component.canonical_path)
                    .canonicalize()
                    .map_err(|_| "组件目录已移除，请重新扫描")?;
                if path.to_string_lossy() != component.canonical_path
                    || !path.starts_with(&canonical)
                {
                    return Err("组件目录已改变或超出项目范围，请重新扫描".into());
                }
            }
        }
        let settings = self.settings()?;
        storage::import(
            &mut *self.db.lock().map_err(|_| "项目库锁异常")?,
            candidates,
            &settings,
            !progress.incomplete,
        )
    }
    pub fn ports(&self) -> Result<Vec<PortBinding>, String> {
        let mut ports = process::list_ports()?;
        let pids: Vec<_> = ports
            .iter()
            .map(|p| p.pid)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let directories = process::working_directories(&pids).unwrap_or_default();
        let commands = process::commands(&pids);
        let projects = self.projects()?;
        let roots: Vec<_> = projects
            .iter()
            .map(|p| Path::new(&p.candidate.canonical_path))
            .collect();
        let kinds: HashMap<_, _> = pids
            .iter()
            .map(|pid| {
                let kind = process_kind::classify(
                    process::executable_path(*pid).as_deref(),
                    commands.get(pid).map(String::as_str).unwrap_or_default(),
                    directories.get(pid).map(String::as_str),
                    &roots,
                );
                (*pid, kind)
            })
            .collect();
        let managed = self
            .services()?
            .into_iter()
            .filter_map(|view| {
                let run = view.current_run?;
                if run.state.active() && supervisor::anchor_valid(&run) {
                    Some(run)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let assignments: HashMap<String, String> = {
            let db = self.db.lock().map_err(|_| "项目库锁异常")?;
            let mut statement = db
                .prepare("SELECT fingerprint, project_id FROM process_assignments")
                .map_err(|e| e.to_string())?;
            let rows = statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(|e| e.to_string())?;
            rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
        };
        let overrides: HashMap<u32, String> = pids
            .iter()
            .filter_map(|pid| {
                if !assignments
                    .keys()
                    .any(|fingerprint| fingerprint.starts_with(&format!("{pid}-")))
                {
                    return None;
                }
                let detail = process::inspect(*pid).ok()?;
                assignments
                    .get(&detail.fingerprint)
                    .map(|id| (*pid, id.clone()))
            })
            .collect();
        let mut times = self.first_seen.lock().map_err(|_| "端口快照锁异常")?;
        let mut active = HashSet::new();
        for port in &mut ports {
            let key = format!(
                "{}:{}:{}:{}",
                port.pid, port.address, port.port, port.family
            );
            port.first_seen = *times.entry(key.clone()).or_insert_with(now);
            active.insert(key);
            port.project_id = directories
                .get(&port.pid)
                .and_then(|cwd| process::match_project(cwd, &projects))
                .map(|p| p.candidate.id.clone());
            if let Some(id) = overrides.get(&port.pid) {
                port.project_id = Some(id.clone());
            }
            port.cwd = directories.get(&port.pid).cloned();
            port.command = commands.get(&port.pid).cloned().unwrap_or_default();
            port.process_kind = kinds[&port.pid];
            if let Some(group) = process::identity(port.pid)
                .and_then(|p| managed.iter().find(|r| r.pgid == Some(p.pgid)))
            {
                port.project_id = Some(group.project_id.clone());
                port.source = "DEVNOOK".into();
                port.service_id = Some(group.service_id.clone());
                port.process_kind = ProcessKind::Service;
            }
        }
        times.retain(|k, _| active.contains(k));
        Ok(ports)
    }
    pub fn inspect(&self, pid: u32) -> Result<ProcessDetail, String> {
        let mut detail = process::inspect(pid)?;
        let projects = self.projects()?;
        let assignment: Option<String> = self
            .db
            .lock()
            .map_err(|_| "项目库锁异常")?
            .query_row(
                "SELECT project_id FROM process_assignments WHERE fingerprint=?1",
                [&detail.fingerprint],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some(id) = assignment {
            detail.project_id = Some(id);
            detail.confidence = "MANUAL".into();
        } else if let Some(project) = detail
            .cwd
            .as_ref()
            .and_then(|cwd| process::match_project(cwd, &projects))
        {
            detail.project_id = Some(project.candidate.id.clone());
            detail.confidence = "HIGH".into();
        } else if let Some(project) = projects
            .iter()
            .filter(|p| detail.command.contains(&p.candidate.canonical_path))
            .max_by_key(|p| p.candidate.canonical_path.len())
        {
            detail.suggested_project_id = Some(project.candidate.id.clone());
            detail.confidence = "MEDIUM".into();
        }
        detail.bindings = self.ports()?.into_iter().filter(|p| p.pid == pid).collect();
        detail.parents = process::parent_chain(detail.ppid);
        if let Some(binding) = detail.bindings.iter().find(|b| b.source == "DEVNOOK") {
            detail.source = "DEVNOOK".into();
            detail.service_id = binding.service_id.clone();
            detail.can_stop = false;
            detail.stop_reason = "由 DevNook 托管，请通过服务管理停止完整进程组".into();
            return Ok(detail);
        }
        match process::stop_eligibility(&detail, &self.protected) {
            Ok(()) => {
                detail.can_stop = true;
                detail.stop_reason = "已确认归属；可在确认后发送 SIGTERM".into();
            }
            Err(reason) => detail.stop_reason = reason,
        }
        Ok(detail)
    }
    pub fn assign_process(
        &self,
        pid: u32,
        fingerprint: &str,
        project_id: &str,
    ) -> Result<ProcessDetail, String> {
        let detail = process::inspect(pid)?;
        if self
            .ports()?
            .iter()
            .any(|p| p.pid == pid && p.source == "DEVNOOK")
        {
            return Err("托管进程的项目归属由服务配置决定".into());
        }
        if detail.fingerprint != fingerprint {
            return Err("进程身份已经改变，请刷新后重试".into());
        }
        if detail.uid != unsafe { libc::geteuid() } {
            return Err("不能修改其他用户进程的归属".into());
        }
        let db = self.db.lock().map_err(|_| "项目库锁异常")?;
        if !storage::list(&db)?
            .iter()
            .any(|p| p.candidate.id == project_id)
        {
            return Err("项目记录不存在".into());
        }
        db.execute("INSERT INTO process_assignments VALUES(?1,?2,?3) ON CONFLICT(fingerprint) DO UPDATE SET project_id=excluded.project_id", params![fingerprint, project_id, now()]).map_err(|e| e.to_string())?;
        storage::audit(&db, "assign_process", fingerprint, project_id)?;
        drop(db);
        self.inspect(pid)
    }
    pub fn prepare_stop(&self, pid: u32) -> Result<StopPreview, String> {
        let process = self.inspect(pid)?;
        if process.source == "DEVNOOK" {
            return Err("请通过服务管理停止托管进程组".into());
        }
        process::stop_eligibility(&process, &self.protected)?;
        let token = Uuid::new_v4().to_string();
        let mut stops = self.stops.lock().map_err(|_| "确认锁异常")?;
        stops.retain(|_, (_, created)| now().saturating_sub(*created) <= 30);
        stops.insert(token.clone(), (process.clone(), now()));
        Ok(StopPreview {
            token,
            process,
            expires_in_seconds: 30,
        })
    }
    pub fn confirm_stop(&self, token: &str, confirmed_pid: u32) -> Result<(), String> {
        let (previous, issued) = self
            .stops
            .lock()
            .map_err(|_| "确认锁异常")?
            .remove(token)
            .ok_or("停止确认已失效，请重新操作")?;
        if now().saturating_sub(issued) > 30 || previous.pid != confirmed_pid {
            return Err("停止确认过期或 PID 不匹配".into());
        }
        let current = self.inspect(previous.pid)?;
        if current.fingerprint != previous.fingerprint || current.project_id != previous.project_id
        {
            return Err("进程身份或项目归属已变化，已阻止停止".into());
        }
        process::stop_eligibility(&current, &self.protected)?;
        let db = self.db.lock().map_err(|_| "审计锁异常")?;
        storage::audit(&db, "stop_requested", &current.fingerprint, "SIGTERM")?;
        if process::start_identity(current.pid) != current.start_identity {
            storage::audit(&db, "stop_signal", &current.fingerprint, "identity_changed")?;
            return Err("进程已退出或 PID 被复用，已阻止信号".into());
        }
        // Positive PID only. External process groups and SIGKILL are never used.
        let result = unsafe { libc::kill(current.pid as i32, libc::SIGTERM) };
        storage::audit(
            &db,
            "stop_signal",
            &current.fingerprint,
            if result == 0 { "sent" } else { "failed" },
        )?;
        if result != 0 {
            return Err(format!(
                "发送 SIGTERM 失败：{}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }
    pub fn open_project(&self, id: &str, target: &str) -> Result<(), String> {
        let project = self
            .projects()?
            .into_iter()
            .find(|p| p.candidate.id == id)
            .ok_or("项目不存在")?;
        ide::open(
            &project,
            if target == "DEFAULT" {
                &project.default_ide
            } else {
                target
            },
            &self.settings()?,
        )
    }
}
