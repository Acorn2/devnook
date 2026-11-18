#![cfg(target_os = "macos")]

use devnook_core::{
    health,
    ipc::{self, CoreClient},
    process,
    secrets::SecretStore,
    service_logs::LogSink,
    service_model::*,
    service_storage,
    supervisor::{self, RunnerLauncher},
    workspace::FailureStage,
    Core,
};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    os::unix::{fs::MetadataExt, process::CommandExt},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tempfile::TempDir;

#[derive(Default)]
struct MemorySecrets(Mutex<BTreeMap<String, String>>);
impl SecretStore for MemorySecrets {
    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        self.0.lock().unwrap().insert(key.into(), value.into());
        Ok(())
    }
    fn get(&self, key: &str) -> Result<String, String> {
        self.0
            .lock()
            .unwrap()
            .get(key)
            .cloned()
            .ok_or("missing test secret".into())
    }
}
fn launcher() -> RunnerLauncher {
    RunnerLauncher {
        executable: std::env::current_exe().unwrap(),
        arguments: vec![
            "--exact".into(),
            "fixture_runner".into(),
            "--ignored".into(),
            "--nocapture".into(),
        ],
    }
}
struct Harness {
    temp: TempDir,
    core: Arc<Core>,
    project_id: String,
}
impl Harness {
    fn new() -> Self {
        let temp = TempDir::new().unwrap();
        fs::create_dir(temp.path().join("app")).unwrap();
        fs::write(temp.path().join("app/package.json"), "{}").unwrap();
        let core = Arc::new(
            Core::with_runtime(
                &temp.path().join("devnook.db"),
                launcher(),
                Arc::new(MemorySecrets::default()),
            )
            .unwrap(),
        );
        let id = core
            .scan_start(temp.path().to_string_lossy().into(), 2)
            .unwrap();
        let p = wait(|| {
            core.scan_status(&id)
                .ok()
                .filter(|p| p.status == "completed")
        });
        let projects = core
            .import_projects(&id, p.candidates.iter().map(|p| p.id.clone()).collect())
            .unwrap();
        Self {
            temp,
            core,
            project_id: projects[0].candidate.id.clone(),
        }
    }
    fn definition(&self, name: &str, mode: &str) -> ServiceDefinition {
        let mut environment = BTreeMap::new();
        environment.insert("FIXTURE_MODE".into(), mode.into());
        ServiceDefinition {
            id: String::new(),
            project_id: self.project_id.clone(),
            name: name.into(),
            runtime_type: "OTHER".into(),
            executable: std::env::current_exe().unwrap().to_string_lossy().into(),
            args: vec![
                "--exact".into(),
                "fixture_runtime".into(),
                "--ignored".into(),
                "--nocapture".into(),
            ],
            shell_mode: false,
            working_directory: fs::canonicalize(self.temp.path().join("app"))
                .unwrap()
                .to_string_lossy()
                .into(),
            environment,
            secret_keys: vec![],
            dependencies: vec![],
            expected_ports: vec![],
            readiness: Readiness {
                grace_ms: 100,
                interval_ms: 100,
                timeout_seconds: 4,
                ..Readiness::default()
            },
            stop_grace_seconds: 1,
            log_limit_kb: 64,
            revision: String::new(),
            created_at: 0,
            updated_at: 0,
        }
    }
    fn save(&self, d: ServiceDefinition) -> ServiceDefinition {
        self.core
            .save_service(ServiceSave {
                definition: d,
                secret_values: Default::default(),
            })
            .unwrap()
    }
    fn start(&self, d: &ServiceDefinition) -> ServiceRun {
        let p = self.core.prepare_service_action(&d.id, "start").unwrap();
        self.core.confirm_service_action(&p.token).unwrap()
    }
    fn state(&self, id: &str, state: ServiceState) -> ServiceRun {
        let deadline = Instant::now() + Duration::from_secs(12);
        loop {
            let current = self
                .core
                .service_history(id)
                .unwrap()
                .into_iter()
                .next()
                .unwrap();
            if current.state == state {
                return current;
            }
            assert!(
                Instant::now() < deadline,
                "wanted {:?}, got {:?}; logs: {:?}",
                state,
                current,
                self.core.service_logs(&current.id, 20, "")
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    fn stopped(&self, id: &str) {
        self.core.stop_service(id).unwrap();
        self.state(id, ServiceState::Stopped);
    }
}
impl Drop for Harness {
    fn drop(&mut self) {
        for view in self.core.services().unwrap_or_default() {
            if let Some(run) = view
                .current_run
                .filter(|r| r.state.active() && supervisor::anchor_valid(r))
            {
                unsafe {
                    libc::kill(-(run.pgid.unwrap() as i32), libc::SIGKILL);
                }
            }
        }
    }
}
fn wait<T>(mut f: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        if let Some(value) = f() {
            return value;
        }
        assert!(Instant::now() < deadline, "timed out waiting for state");
        std::thread::sleep(Duration::from_millis(50));
    }
}
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

struct ExternalNode {
    child: Child,
    directory: std::path::PathBuf,
    port: u16,
}
impl ExternalNode {
    fn new(h: &Harness, mode: &str) -> Self {
        let directory = fs::canonicalize(h.temp.path().join("app")).unwrap();
        fs::write(
            directory.join("package.json"),
            r#"{"scripts":{"dev":"node server.cjs"}}"#,
        )
        .unwrap();
        fs::write(directory.join("server.cjs"), r#"
const fs = require('node:fs');
const mode = process.argv[2] || 'normal';
const server = require('node:net').createServer();
server.listen(Number(process.env.PORT), '127.0.0.1', () => {
  console.log('NODE_READY');
  if (process.send) process.send('ready');
});
process.on('SIGTERM', () => {
  fs.writeFileSync('term-seen', 'yes');
  if (mode === 'ignore') return;
  if (mode === 'close-but-live') { server.close(); return; }
  server.close(() => {
    if (mode !== 'respawn') return process.exit(0);
    const child = require('node:child_process').spawn(process.execPath, [__filename, 'normal'], {env:process.env,stdio:['ignore','ignore','ignore','ipc']});
    fs.writeFileSync('replacement-pid', String(child.pid));
    child.once('message', () => process.exit(0));
  });
});
setInterval(() => { if (fs.existsSync('exit-now')) { server.close(); process.exit(0); } }, 20);
"#).unwrap();
        let port = free_port();
        let node = supervisor::resolve_program("node").unwrap();
        let child = Command::new(node)
            .args(["server.cjs", mode])
            .env("PORT", port.to_string())
            .current_dir(&directory)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        wait(|| TcpStream::connect(("127.0.0.1", port)).ok());
        Self {
            child,
            directory,
            port,
        }
    }
    fn saved(&self, h: &Harness) -> ServiceDefinition {
        let setup = h
            .core
            .external_service_configuration(self.child.id())
            .unwrap();
        assert_eq!(setup.draft.command, "npm run dev");
        assert!(setup.draft.definition.environment.is_empty());
        assert!(setup.draft.definition.secret_keys.is_empty());
        let mut definition = setup.draft.definition;
        definition.executable = supervisor::resolve_program("node")
            .unwrap()
            .to_string_lossy()
            .into();
        definition.args = vec!["server.cjs".into()];
        definition
            .environment
            .insert("PORT".into(), self.port.to_string());
        definition.stop_grace_seconds = 1;
        definition.readiness = Readiness {
            kind: ReadinessKind::Port,
            port: Some(self.port),
            interval_ms: 100,
            timeout_seconds: 4,
            ..Readiness::default()
        };
        h.save(definition)
    }
    fn preview(
        &self,
        h: &Harness,
        definition: &ServiceDefinition,
    ) -> devnook_core::external_service::ExternalServiceHandoffPreview {
        let detail = h.core.inspect(self.child.id()).unwrap();
        h.core
            .prepare_external_service_handoff(
                &definition.id,
                &definition.revision,
                self.child.id(),
                &detail.fingerprint,
            )
            .unwrap()
    }
    fn alive(&mut self) -> bool {
        self.child.try_wait().unwrap().is_none()
    }
}
impl Drop for ExternalNode {
    fn drop(&mut self) {
        // Disposable fixture only: its cleanup file lets even the ignore-TERM mode exit.
        let _ = fs::write(self.directory.join("exit-now"), "yes");
        if let Ok(pid) = fs::read_to_string(self.directory.join("replacement-pid")) {
            if let Ok(pid) = pid.parse::<i32>() {
                unsafe {
                    libc::kill(pid, libc::SIGTERM);
                }
            }
        }
        let until = Instant::now() + Duration::from_secs(5);
        while self.alive() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(20));
        }
        if self.alive() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

#[test]
fn external_save_and_preview_are_inert_then_handoff_creates_a_new_managed_instance() {
    let h = Harness::new();
    let untouched = h.save(h.definition("existing configuration", "worker"));
    let mut external = ExternalNode::new(&h, "normal");
    let saved = external.saved(&h);
    assert!(h.core.service_history(&saved.id).unwrap().is_empty());
    let preview = external.preview(&h, &saved);
    assert!(external.alive());
    assert!(!external.directory.join("term-seen").exists());
    assert!(h
        .core
        .confirm_external_service_handoff(&preview.token, external.child.id() + 1)
        .is_err());
    assert!(external.alive());
    assert!(h
        .core
        .confirm_external_service_handoff(&preview.token, external.child.id())
        .is_err());
    let preview = external.preview(&h, &saved);
    let run = h
        .core
        .confirm_external_service_handoff(&preview.token, external.child.id())
        .unwrap();
    assert_eq!(run.service_id, saved.id);
    h.state(&saved.id, ServiceState::Ready);
    assert!(!wait(|| (!external.alive()).then_some(false)));
    assert!(h
        .core
        .ports()
        .unwrap()
        .iter()
        .any(|b| b.port == external.port
            && b.source == "DEVNOOK"
            && b.service_id.as_ref() == Some(&saved.id)));
    assert_eq!(
        h.core
            .services()
            .unwrap()
            .iter()
            .find(|v| v.definition.id == untouched.id)
            .unwrap()
            .definition
            .revision,
        untouched.revision
    );
    assert!(h
        .core
        .confirm_external_service_handoff(&preview.token, external.child.id())
        .is_err());
    h.stopped(&saved.id);
}

#[test]
fn external_service_stop_reuses_pid_confirmation_without_adopting_the_process() {
    let h = Harness::new();
    let mut external = ExternalNode::new(&h, "normal");
    let preview = h
        .core
        .prepare_external_service_stop(external.child.id())
        .unwrap();
    assert!(external.alive());
    assert!(h
        .core
        .confirm_stop(&preview.token, external.child.id() + 1)
        .is_err());
    assert!(external.alive());
    let preview = h
        .core
        .prepare_external_service_stop(external.child.id())
        .unwrap();
    h.core
        .confirm_stop(&preview.token, external.child.id())
        .unwrap();
    wait(|| (!external.alive()).then_some(()));
    assert!(h.core.services().unwrap().is_empty());
    assert!(h
        .core
        .prepare_external_service_stop(std::process::id())
        .is_err());
}

#[test]
fn external_handoff_rejects_stale_identity_configuration_and_other_ports_before_stopping() {
    let h = Harness::new();
    let mut external = ExternalNode::new(&h, "normal");
    let mut saved = external.saved(&h);
    assert!(h
        .core
        .prepare_external_service_handoff(
            &saved.id,
            &saved.revision,
            external.child.id(),
            "stale fingerprint"
        )
        .is_err());
    let preview = external.preview(&h, &saved);
    saved.name = "edited after preview".into();
    saved = h.save(saved);
    assert!(h
        .core
        .confirm_external_service_handoff(&preview.token, external.child.id())
        .unwrap_err()
        .contains("配置或运行实例已变化"));
    assert!(external.alive());
    let extra_port = free_port();
    saved.expected_ports.push(extra_port);
    saved = h.save(saved);
    let preview = external.preview(&h, &saved);
    let _unrelated = TcpListener::bind(("127.0.0.1", extra_port)).unwrap();
    assert!(h
        .core
        .confirm_external_service_handoff(&preview.token, external.child.id())
        .unwrap_err()
        .contains("其他进程"));
    assert!(external.alive());
    assert!(!external.directory.join("term-seen").exists());
    assert!(h.core.service_history(&saved.id).unwrap().is_empty());
}

#[test]
fn external_handoff_does_not_start_when_old_process_ignores_term_or_only_closes_its_port() {
    for mode in ["ignore", "close-but-live"] {
        let h = Harness::new();
        let mut external = ExternalNode::new(&h, mode);
        let saved = external.saved(&h);
        let preview = external.preview(&h, &saved);
        let error = h
            .core
            .confirm_external_service_handoff(&preview.token, external.child.id())
            .unwrap_err();
        assert!(error.contains("未强杀"), "{error}");
        assert!(external.alive());
        assert!(external.directory.join("term-seen").exists());
        assert!(h.core.service_history(&saved.id).unwrap().is_empty());
    }
}

#[test]
fn external_handoff_preserves_a_respawned_listener_instead_of_stopping_its_group() {
    let h = Harness::new();
    let external = ExternalNode::new(&h, "respawn");
    let saved = external.saved(&h);
    let preview = external.preview(&h, &saved);
    assert!(h
        .core
        .confirm_external_service_handoff(&preview.token, external.child.id())
        .unwrap_err()
        .contains("仍被监听"));
    let pid: u32 = fs::read_to_string(external.directory.join("replacement-pid"))
        .unwrap()
        .parse()
        .unwrap();
    assert!(process::identity(pid).is_some_and(|p| p.alive));
    assert!(TcpStream::connect(("127.0.0.1", external.port)).is_ok());
    assert!(h.core.service_history(&saved.id).unwrap().is_empty());
}

#[test]
fn external_handoff_preflights_dependencies_executable_and_run_identity() {
    let h = Harness::new();
    let mut external = ExternalNode::new(&h, "normal");
    let dependency = h.save(h.definition("dependency", "worker"));
    let mut saved = external.saved(&h);
    saved.dependencies = vec![dependency.id.clone()];
    saved = h.save(saved);
    let detail = h.core.inspect(external.child.id()).unwrap();
    assert!(h
        .core
        .prepare_external_service_handoff(
            &saved.id,
            &saved.revision,
            external.child.id(),
            &detail.fingerprint
        )
        .is_err());
    h.start(&dependency);
    h.state(&dependency.id, ServiceState::Ready);
    let preview = external.preview(&h, &saved);
    h.stopped(&dependency.id);
    h.start(&dependency);
    h.state(&dependency.id, ServiceState::Ready);
    assert!(h
        .core
        .confirm_external_service_handoff(&preview.token, external.child.id())
        .is_err());
    saved.executable = "/definitely/missing/devnook-fixture".into();
    saved = h.save(saved);
    let preview = external.preview(&h, &saved);
    assert!(h
        .core
        .confirm_external_service_handoff(&preview.token, external.child.id())
        .is_err());
    assert!(external.alive());
    assert!(!external.directory.join("term-seen").exists());
    saved.executable = supervisor::resolve_program("node")
        .unwrap()
        .to_string_lossy()
        .into();
    saved = h.save(saved);
    let preview = external.preview(&h, &saved);
    assert_eq!(h.start(&saved).state, ServiceState::Blocked);
    assert!(h
        .core
        .confirm_external_service_handoff(&preview.token, external.child.id())
        .is_err());
    assert!(external.alive());
    h.stopped(&dependency.id);
}

#[test]
fn imported_frontend_backend_suggestions_run_log_restart_and_stop_without_source_changes() {
    let h = Harness::new();
    for name in ["frontend", "backend"] {
        let dir = h.temp.path().join("app").join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("package.json"),
            r#"{"scripts":{"dev":"node server.cjs"}}"#,
        )
        .unwrap();
        fs::write(dir.join("server.cjs"), format!("console.log('{name}_READY'); setInterval(()=>console.log('{name}_heartbeat'),100);" )).unwrap();
    }
    let id = h
        .core
        .scan_start(h.temp.path().join("app").to_string_lossy().into(), 0)
        .unwrap();
    let preview = wait(|| {
        h.core
            .scan_status(&id)
            .ok()
            .filter(|p| p.status == "completed")
    });
    h.core
        .import_projects(&id, vec![preview.candidates[0].id.clone()])
        .unwrap();
    assert!(h.core.services().unwrap().is_empty());
    let drafts = h.core.service_drafts(&h.project_id).unwrap();
    assert_eq!(drafts.len(), 2);
    let mut ids = Vec::new();
    for draft in drafts {
        assert_eq!(draft.command, "npm run dev");
        let definition = h.save(draft.definition);
        assert!(h.core.service_history(&definition.id).unwrap().is_empty());
        let preview = h
            .core
            .prepare_service_action(&definition.id, "start")
            .unwrap();
        assert!(h.core.service_history(&definition.id).unwrap().is_empty());
        h.core.confirm_service_action(&preview.token).unwrap();
        let run = h.state(&definition.id, ServiceState::Ready);
        wait(|| {
            h.core
                .service_logs(&run.id, 100, &format!("{}_READY", definition.name))
                .ok()
                .filter(|p| !p.lines.is_empty())
        });
        assert_eq!(
            fs::read_to_string(Path::new(&definition.working_directory).join("package.json"))
                .unwrap(),
            r#"{"scripts":{"dev":"node server.cjs"}}"#
        );
        ids.push(definition.id);
    }
    let preview = h.core.prepare_service_action(&ids[0], "restart").unwrap();
    h.core.confirm_service_action(&preview.token).unwrap();
    h.state(&ids[0], ServiceState::Ready);
    assert_eq!(h.core.service_history(&ids[0]).unwrap().len(), 2);
    for id in ids {
        h.stopped(&id);
        let run = h.state(&id, ServiceState::Stopped);
        wait(|| (!supervisor::anchor_valid(&run)).then_some(()));
        assert!(!h
            .core
            .service_logs(&run.id, 100, "")
            .unwrap()
            .lines
            .is_empty());
    }
}

#[test]
#[ignore = "internal service runner fixture"]
fn fixture_runner() {
    devnook_core::service_worker::from_stdin().unwrap();
}
#[test]
#[ignore = "isolated development runtime fixture"]
fn fixture_runtime() {
    let mode = std::env::var("FIXTURE_MODE").unwrap();
    if let Ok(file) = std::env::var("START_FILE") {
        fs::write(file, "started").unwrap();
    }
    if mode == "exit" {
        std::process::exit(7);
    }
    if mode == "ignore_term" {
        unsafe {
            libc::signal(libc::SIGTERM, libc::SIG_IGN);
        }
    }
    if mode == "fork" {
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "fixture_runtime", "--ignored", "--nocapture"])
            .env("FIXTURE_MODE", "worker")
            .spawn()
            .unwrap();
        fs::write(
            std::env::var("CHILD_PID_FILE").unwrap(),
            child.id().to_string(),
        )
        .unwrap();
        return;
    }
    if mode == "flood" {
        println!("{}", "x".repeat(70_000));
        for _ in 0..200 {
            println!("{}", "z".repeat(1000));
        }
    }
    println!("FIXTURE_READY");
    eprintln!("stderr marker");
    if let Ok(secret) = std::env::var("API_TOKEN") {
        println!("echo: {secret}");
    }
    let listener = std::env::var("FIXTURE_PORT")
        .ok()
        .map(|p| TcpListener::bind(("127.0.0.1", p.parse::<u16>().unwrap())).unwrap());
    if let Some(listener) = &listener {
        listener.set_nonblocking(true).unwrap();
    }
    let mut heartbeat = Instant::now();
    loop {
        if let Some(listener) = &listener {
            if let Ok((mut stream, _)) = listener.accept() {
                let _ = stream.set_read_timeout(Some(Duration::from_millis(150)));
                let mut request = [0u8; 1024];
                let _ = stream.read(&mut request);
                let bad = std::env::var("HEALTH_BAD_FILE").is_ok_and(|p| Path::new(&p).exists());
                let response = if bad {
                    "HTTP/1.1 503 Unavailable\r\nContent-Length: 3\r\nConnection: close\r\n\r\nbad"
                } else {
                    "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok"
                };
                let _ = stream.write_all(response.as_bytes());
            }
        }
        if heartbeat.elapsed() > Duration::from_millis(400) {
            println!("heartbeat");
            heartbeat = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn node_http_start_restart_logs_and_managed_attribution() {
    let h = Harness::new();
    let port = free_port();
    let mut d = h.definition("node-api", "worker");
    d.runtime_type = "NODE".into();
    d.executable = supervisor::resolve_program("node")
        .unwrap()
        .to_string_lossy()
        .into();
    d.args=vec!["-e".into(),"require('node:http').createServer((req,res)=>{res.writeHead(200);res.end('ok')}).listen(Number(process.env.FIXTURE_PORT),'127.0.0.1');console.log('NODE_READY');console.error('NODE_STDERR');".into()];
    d.environment
        .insert("FIXTURE_PORT".into(), port.to_string());
    d.readiness.kind = ReadinessKind::Http;
    d.readiness.url = format!("http://127.0.0.1:{port}/health");
    let d = h.save(d);
    assert!(h.core.service_history(&d.id).unwrap().is_empty());
    let started = h.start(&d);
    let ready = h.state(&d.id, ServiceState::Ready);
    assert!(health::http_ready(&d.readiness.url));
    assert!(h.core.prepare_service_action(&d.id, "start").is_err());
    assert!(h
        .core
        .save_service(ServiceSave {
            definition: d.clone(),
            secret_values: Default::default()
        })
        .is_err());
    assert!(h.core.remove_project(&h.project_id).is_err());
    assert!(h.core.archive_service(&d.id).is_err());
    let binding = h
        .core
        .ports()
        .unwrap()
        .into_iter()
        .find(|p| p.port == port)
        .unwrap();
    assert_eq!(binding.source, "DEVNOOK");
    assert_eq!(
        binding.process_kind,
        devnook_core::model::ProcessKind::Service
    );
    assert_eq!(binding.service_id.as_deref(), Some(d.id.as_str()));
    assert_eq!(binding.project_id.as_deref(), Some(h.project_id.as_str()));
    let detail = h.core.inspect(binding.pid).unwrap();
    assert!(h
        .core
        .assign_process(binding.pid, &detail.fingerprint, &h.project_id)
        .is_err());
    assert!(h.core.prepare_stop(binding.pid).is_err());
    assert_eq!(h.core.inspect(binding.pid).unwrap().source, "DEVNOOK");
    let page = wait(|| {
        let p = h.core.service_logs(&started.id, 100, "NODE_").ok()?;
        if p.lines.len() >= 2 {
            Some(p)
        } else {
            None
        }
    });
    assert!(page
        .lines
        .iter()
        .any(|l| l.contains("[stdout]") && l.contains("NODE_READY")));
    assert!(page
        .lines
        .iter()
        .any(|l| l.contains("[stderr]") && l.contains("NODE_STDERR")));
    let p = h.core.prepare_service_action(&d.id, "restart").unwrap();
    let restarted = h.core.confirm_service_action(&p.token).unwrap();
    assert_ne!(restarted.id, ready.id);
    assert!(h.core.confirm_service_action(&p.token).is_err());
    h.state(&d.id, ServiceState::Ready);
    assert!(h.core.service_history(&d.id).unwrap().len() >= 2);
    h.stopped(&d.id);
    assert!(!h.core.ports().unwrap().iter().any(|p| p.port == port));
    assert!(!h
        .core
        .service_logs(&ready.id, 100, "NODE_READY")
        .unwrap()
        .lines
        .is_empty());
}

#[test]
fn actual_ports_without_configuration_do_not_take_over_external_project_processes() {
    struct Fixture(Child);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let h = Harness::new();
    let managed_port = free_port();
    let mut d = h.definition("process-readiness", "worker");
    d.environment
        .insert("FIXTURE_PORT".into(), managed_port.to_string());
    let d = h.save(d);
    assert!(d.expected_ports.is_empty());
    h.start(&d);
    h.state(&d.id, ServiceState::Ready);
    let binding = wait(|| {
        h.core
            .ports()
            .ok()?
            .into_iter()
            .find(|b| b.port == managed_port)
    });
    assert_eq!(binding.source, "DEVNOOK");
    assert_eq!(binding.service_id.as_deref(), Some(d.id.as_str()));

    let external_port = free_port();
    let mut external = Fixture(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "fixture_runtime", "--ignored", "--nocapture"])
            .env("FIXTURE_MODE", "worker")
            .env("FIXTURE_PORT", external_port.to_string())
            .current_dir(&d.working_directory)
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let binding = wait(|| {
        h.core
            .ports()
            .ok()?
            .into_iter()
            .find(|b| b.pid == external.0.id())
    });
    assert_eq!(binding.port, external_port);
    assert_eq!(binding.project_id.as_deref(), Some(h.project_id.as_str()));
    assert_eq!(binding.source, "OBSERVED");
    assert!(binding.service_id.is_none());
    let detail = h.core.inspect(external.0.id()).unwrap();
    let assigned = h
        .core
        .assign_process(detail.pid, &detail.fingerprint, &h.project_id)
        .unwrap();
    assert_eq!(assigned.confidence, "MANUAL");
    assert_eq!(assigned.source, "OBSERVED");
    assert!(assigned.service_id.is_none());
    h.stopped(&d.id);
    assert!(external.0.try_wait().unwrap().is_none());
    assert!(h
        .core
        .ports()
        .unwrap()
        .iter()
        .any(|b| b.pid == external.0.id() && b.port == external_port));
}

#[test]
fn python_port_and_rust_log_phrase_are_real_checks() {
    let h = Harness::new();
    let port = free_port();
    let mut python = h.definition("python-api", "worker");
    python.runtime_type = "PYTHON".into();
    python.executable = supervisor::resolve_program("python3")
        .unwrap()
        .to_string_lossy()
        .into();
    python.args=vec!["-u".into(),"-c".into(),"import os;from http.server import HTTPServer,BaseHTTPRequestHandler\nclass H(BaseHTTPRequestHandler):\n def do_GET(self):\n  self.send_response(200);self.end_headers();self.wfile.write(b'ok')\nprint('PYTHON_READY',flush=True)\nHTTPServer(('127.0.0.1',int(os.environ['FIXTURE_PORT'])),H).serve_forever()".into()];
    python
        .environment
        .insert("FIXTURE_PORT".into(), port.to_string());
    python.readiness.kind = ReadinessKind::Port;
    python.readiness.port = Some(port);
    let python = h.save(python);
    h.start(&python);
    let ready = h.state(&python.id, ServiceState::Ready);
    assert!(ready.ready_at.is_some());
    h.stopped(&python.id);
    let mut log = h.definition("log-worker", "worker");
    log.readiness.kind = ReadinessKind::LogPhrase;
    log.readiness.phrase = "FIXTURE_READY".into();
    let log = h.save(log);
    h.start(&log);
    h.state(&log.id, ServiceState::Ready);
    h.stopped(&log.id);
}

#[test]
fn conflicts_and_startup_failures_do_not_execute_or_kill_external_processes() {
    let h = Harness::new();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let marker = h.temp.path().join("must-not-start");
    let mut d = h.definition("conflict", "worker");
    d.expected_ports = vec![port];
    d.environment
        .insert("START_FILE".into(), marker.to_string_lossy().into());
    let d = h.save(d);
    assert_eq!(h.start(&d).state, ServiceState::Blocked);
    assert!(!marker.exists());
    assert!(TcpStream::connect(("127.0.0.1", port)).is_ok());
    let mut missing = h.definition("missing", "worker");
    missing.executable = "/missing/devnook-command".into();
    let missing = h.save(missing);
    assert_eq!(h.start(&missing).state, ServiceState::Crashed);
    let exit = h.save(h.definition("immediate-exit", "exit"));
    h.start(&exit);
    let run = h.state(&exit.id, ServiceState::Crashed);
    assert_eq!(run.exit_code, Some(7));
    let mut timeout = h.definition("timeout", "worker");
    timeout.readiness.kind = ReadinessKind::LogPhrase;
    timeout.readiness.phrase = "NEVER_PRINTED".into();
    timeout.readiness.timeout_seconds = 1;
    timeout.readiness.interval_ms = 5000;
    let timeout = h.save(timeout);
    let started = Instant::now();
    h.start(&timeout);
    let run = h.state(&timeout.id, ServiceState::Crashed);
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "timeout must not wait for the longer probe interval"
    );
    assert!(run.error.as_ref().unwrap().contains("超时"));
    // The runner persists the terminal state before its final audit write and exit.
    wait(|| (!supervisor::anchor_valid(&run)).then_some(()));
}

#[test]
fn descendants_survive_launcher_exit_and_normal_stop_cleans_the_group() {
    let h = Harness::new();
    let file = h.temp.path().join("child-pid");
    let mut d = h.definition("forking-worker", "fork");
    d.environment
        .insert("CHILD_PID_FILE".into(), file.to_string_lossy().into());
    let d = h.save(d);
    h.start(&d);
    let run = h.state(&d.id, ServiceState::Ready);
    // Process readiness precedes the runtime's child marker on slower startup paths.
    let pid: u32 = wait(|| fs::read_to_string(&file).ok()?.parse().ok());
    assert!(process::identity(pid).unwrap().alive);
    h.stopped(&d.id);
    assert!(!process::identity(pid).is_some_and(|p| p.alive));
    // STOPPED is written by the runner before its final audit and process exit.
    // Wait for the kernel to observe that final exit as well as the runtime's exit.
    wait(|| {
        process::group_members(run.pgid.unwrap())
            .ok()?
            .is_empty()
            .then_some(())
    });
}

#[test]
fn term_grace_blocks_restart_until_force_is_separately_confirmed() {
    let h = Harness::new();
    let d = h.save(h.definition("stubborn-worker", "ignore_term"));
    h.start(&d);
    let before = h.state(&d.id, ServiceState::Ready);
    assert!(h.core.prepare_service_action(&d.id, "force_stop").is_err());
    h.core.stop_service(&d.id).unwrap();
    h.state(&d.id, ServiceState::Stopping);
    let p = h.core.prepare_service_action(&d.id, "restart").unwrap();
    assert!(h.core.confirm_service_action(&p.token).is_err());
    assert!(process::identity(before.pid.unwrap()).unwrap().alive);
    assert_eq!(h.core.service_history(&d.id).unwrap().len(), 1);
    let p = h.core.prepare_service_action(&d.id, "force_stop").unwrap();
    h.core.confirm_service_action(&p.token).unwrap();
    wait(|| {
        h.core
            .service_history(&d.id)
            .ok()?
            .first()
            .filter(|r| !r.state.active())
            .cloned()
    });
    assert!(!process::identity(before.pid.unwrap()).is_some_and(|p| p.alive));
}

#[test]
fn credentials_are_refs_logs_are_bounded_and_history_can_be_cleaned() {
    let h = Harness::new();
    let mut d = h.definition("private-worker", "worker");
    d.secret_keys = vec!["API_TOKEN".into()];
    d.readiness.kind = ReadinessKind::LogPhrase;
    d.readiness.phrase = "FIXTURE_READY".into();
    let fake = "only-a-test-value-37f90cd";
    let d = h
        .core
        .save_service(ServiceSave {
            definition: d,
            secret_values: BTreeMap::from([("API_TOKEN".into(), fake.into())]),
        })
        .unwrap();
    let r = h.start(&d);
    h.state(&d.id, ServiceState::Ready);
    let logs = wait(|| {
        let l = h.core.service_logs(&r.id, 100, "echo:").ok()?;
        if !l.lines.is_empty() {
            Some(l)
        } else {
            None
        }
    });
    assert!(logs.lines.join("\n").contains("[REDACTED]"));
    assert!(!fs::read_to_string(&r.log_path).unwrap().contains(fake));
    let db = rusqlite::Connection::open(h.temp.path().join("devnook.db")).unwrap();
    let payload: String = db
        .query_row(
            "SELECT payload FROM service_definitions WHERE id=?1",
            [&d.id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!payload.contains(fake));
    assert!(h.core.clear_service_log(&r.id).is_err());
    h.stopped(&d.id);
    h.core.clear_service_log(&r.id).unwrap();
    assert!(!Path::new(&r.log_path).exists());
    assert!(!h.core.service_history(&d.id).unwrap().is_empty());
    let mut retained = d.clone();
    retained.name = "retained-keychain-ref".into();
    let retained = h
        .core
        .save_service(ServiceSave {
            definition: retained,
            secret_values: Default::default(),
        })
        .unwrap();
    assert_ne!(retained.revision, d.revision);
    let retained_run = h.start(&retained);
    h.state(&retained.id, ServiceState::Ready);
    let retained_logs = wait(|| {
        let page = h.core.service_logs(&retained_run.id, 100, "echo:").ok()?;
        (!page.lines.is_empty()).then_some(page)
    });
    assert!(retained_logs.lines[0].contains("[REDACTED]"));
    h.stopped(&retained.id);
    let flood = h.save(h.definition("flood", "flood"));
    let r = h.start(&flood);
    let run = wait(|| {
        h.core
            .service_history(&flood.id)
            .ok()?
            .into_iter()
            .next()
            .filter(|r| r.log_truncated)
    });
    assert!(run.log_truncated);
    assert!(fs::metadata(&r.log_path).unwrap().len() <= 64 * 1024 + 256);
    h.stopped(&flood.id);
    let mut plain = h.definition("bad-secret", "worker");
    plain.environment.insert("API_TOKEN".into(), fake.into());
    assert!(h
        .core
        .save_service(ServiceSave {
            definition: plain,
            secret_values: Default::default()
        })
        .is_err());
    let mut sink = LogSink::new(&h.temp.path().join("redact.log"), 64, vec![fake.into()]).unwrap();
    sink.write("stdout", &format!("token={fake}")).unwrap();
    assert!(!fs::read_to_string(h.temp.path().join("redact.log"))
        .unwrap()
        .contains(fake));
}

#[test]
fn configuration_revisions_http_bounds_and_migration_preserve_data() {
    let h = Harness::new();
    let d = h.save(h.definition("config", "worker"));
    let preview = h.core.prepare_service_action(&d.id, "start").unwrap();
    let mut changed = d.clone();
    changed.name = "changed".into();
    h.core
        .save_service(ServiceSave {
            definition: changed,
            secret_values: Default::default(),
        })
        .unwrap();
    assert!(h.core.confirm_service_action(&preview.token).is_err());
    for url in [
        "https://127.0.0.1:8000",
        "http://example.com/health",
        "http://user:password@localhost:8000/health",
        "http://localhost/#x",
        "http://127.0.0.1:8000/health?token=do-not-store",
    ] {
        assert!(health::http_target(url).is_err());
    }
    assert!(health::http_target("http://[::1]:8000/health?a=b").is_ok());
    let mut shell = h.definition("shell", "worker");
    shell.shell_mode = true;
    shell.args = vec![];
    shell.executable = "printf 'SHELL_READY\\n'; while true; do sleep 1; done".into();
    shell.readiness.kind = ReadinessKind::LogPhrase;
    shell.readiness.phrase = "SHELL_READY".into();
    let shell = h.save(shell);
    h.start(&shell);
    h.state(&shell.id, ServiceState::Ready);
    h.stopped(&shell.id);
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("legacy.db");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("PRAGMA user_version=1; CREATE TABLE projects(id TEXT PRIMARY KEY,canonical_path TEXT UNIQUE NOT NULL,payload TEXT NOT NULL); INSERT INTO projects VALUES('p','/legacy','preserved');").unwrap();
    drop(db);
    let db = devnook_core::storage::open(&path).unwrap();
    assert_eq!(
        db.query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        3
    );
    assert!(path.with_extension("phase1-backup.db").exists());
    assert_eq!(
        db.query_row("SELECT payload FROM projects WHERE id='p'", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "preserved"
    );
}

#[test]
fn http_health_downgrades_after_readiness_is_lost() {
    let h = Harness::new();
    let port = free_port();
    let bad = h.temp.path().join("health-bad");
    let mut d = h.definition("health-api", "worker");
    d.environment
        .insert("FIXTURE_PORT".into(), port.to_string());
    d.environment
        .insert("HEALTH_BAD_FILE".into(), bad.to_string_lossy().into());
    d.readiness.kind = ReadinessKind::Http;
    d.readiness.url = format!("http://127.0.0.1:{port}/health");
    let d = h.save(d);
    h.start(&d);
    h.state(&d.id, ServiceState::Ready);
    fs::write(&bad, "bad").unwrap();
    h.state(&d.id, ServiceState::Running);
    fs::remove_file(bad).unwrap();
    h.state(&d.id, ServiceState::Ready);
    h.stopped(&d.id);
}

#[test]
fn stale_managed_identity_is_unknown_and_never_signalled() {
    let h = Harness::new();
    let d = h.save(h.definition("identity-guard", "worker"));
    h.start(&d);
    let original = h.state(&d.id, ServiceState::Ready);
    struct Resume {
        database: std::path::PathBuf,
        run: ServiceRun,
    }
    impl Drop for Resume {
        fn drop(&mut self) {
            if supervisor::anchor_valid(&self.run) {
                if let Ok(db) = devnook_core::storage::open(&self.database) {
                    let _ = service_storage::save_run(&db, &self.run);
                }
                unsafe {
                    libc::kill(self.run.worker_pid.unwrap() as i32, libc::SIGCONT);
                }
            }
        }
    }
    let resume = Resume {
        database: h.temp.path().join("devnook.db"),
        run: original.clone(),
    };
    assert_eq!(
        unsafe { libc::kill(original.worker_pid.unwrap() as i32, libc::SIGSTOP) },
        0
    );
    std::thread::sleep(Duration::from_millis(30));
    let db = rusqlite::Connection::open(h.temp.path().join("devnook.db")).unwrap();
    let mut changed = original.clone();
    changed.worker_start = Some("reused-pid".into());
    service_storage::save_run(&db, &changed).unwrap();
    assert_eq!(
        h.core.service_history(&d.id).unwrap()[0].state,
        ServiceState::Unknown
    );
    assert!(h.core.stop_service(&d.id).is_err());
    assert!(h.core.prepare_service_action(&d.id, "start").is_err());
    assert!(
        process::identity(original.worker_pid.unwrap())
            .unwrap()
            .alive
    );
    service_storage::save_run(&db, &original).unwrap();
    drop(resume);
    h.stopped(&d.id);
}

#[test]
#[ignore = "isolated Core daemon fixture"]
fn fixture_daemon() {
    let directory = std::env::var("CORE_FIXTURE_DIR").unwrap();
    ipc::serve(Path::new(&directory), launcher()).unwrap();
}
struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn daemon(directory: &Path) -> ChildGuard {
    let child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "fixture_daemon", "--ignored", "--nocapture"])
        .env("CORE_FIXTURE_DIR", directory)
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait(|| std::os::unix::net::UnixStream::connect(directory.join("core.sock")).ok());
    ChildGuard(child)
}

#[test]
fn uds_clients_share_core_and_runner_survives_client_and_core_exit() {
    let h = Harness::new();
    let mut definition = h.definition("recoverable", "worker");
    for index in 0..4 {
        definition
            .environment
            .insert(format!("LARGE_PAYLOAD_{index}"), "x".repeat(4000));
    }
    let d = h.save(definition);
    let mut daemon1 = daemon(h.temp.path());
    let client = CoreClient::with_launcher(h.temp.path(), launcher()).unwrap();
    assert_eq!(
        client.services().unwrap()[0].definition.environment["LARGE_PAYLOAD_0"].len(),
        4000
    );
    let preview = client.prepare_service_action(&d.id, "start").unwrap();
    let run = client.confirm_service_action(&preview.token).unwrap();
    wait(|| {
        client
            .service_history(&d.id)
            .ok()?
            .into_iter()
            .next()
            .filter(|r| r.state == ServiceState::Ready)
    });
    let pid = run.worker_pid.unwrap();
    let metadata = fs::metadata(h.temp.path().join("core.sock")).unwrap();
    assert_eq!(metadata.mode() & 0o777, 0o600);
    drop(client);
    assert!(process::identity(pid).unwrap().alive);
    let client2 = CoreClient::with_launcher(h.temp.path(), launcher()).unwrap();
    assert_eq!(
        client2.services().unwrap()[0]
            .current_run
            .as_ref()
            .unwrap()
            .worker_pid,
        Some(pid)
    );
    drop(client2);
    daemon1.0.kill().unwrap();
    daemon1.0.wait().unwrap();
    let length = fs::metadata(&run.log_path).unwrap().len();
    wait(|| (fs::metadata(&run.log_path).ok()?.len() > length).then_some(()));
    assert!(process::identity(pid).unwrap().alive);
    let _daemon2 = daemon(h.temp.path());
    let client3 = CoreClient::with_launcher(h.temp.path(), launcher()).unwrap();
    assert_eq!(
        client3.services().unwrap()[0]
            .current_run
            .as_ref()
            .unwrap()
            .state,
        ServiceState::Ready
    );
    client3.stop_service(&d.id).unwrap();
    wait(|| {
        client3
            .service_history(&d.id)
            .ok()?
            .into_iter()
            .next()
            .filter(|r| r.state == ServiceState::Stopped)
    });
    let _: serde_json::Value = client3.invoke("shutdown", serde_json::json!({})).unwrap();
}

fn group(h: &Harness, ids: Vec<String>) -> devnook_core::workspace::WorkspaceGroup {
    h.core
        .save_workspace(devnook_core::workspace::WorkspaceGroup {
            name: "测试工作区".into(),
            service_ids: ids,
            ..Default::default()
        })
        .unwrap()
}
fn start_group(h: &Harness, id: &str) -> devnook_core::workspace::WorkspaceRun {
    let p = h.core.prepare_workspace_action(id, "start").unwrap();
    h.core.confirm_workspace_action(&p.token).unwrap()
}
fn group_state(h: &Harness, id: &str, state: &str) -> devnook_core::workspace::WorkspaceRun {
    wait(|| {
        h.core
            .workspace_history(id)
            .ok()?
            .into_iter()
            .next()
            .filter(|r| r.state == state)
    })
}
fn stop_group(h: &Harness, id: &str) {
    let p = h.core.prepare_workspace_action(id, "stop").unwrap();
    h.core.confirm_workspace_action(&p.token).unwrap();
    group_state(h, id, "STOPPED");
}
#[test]
fn workspace_dag_ready_order_reuses_and_stops_only_owned_runs() {
    let h = Harness::new();
    let a = h.save(h.definition("existing", "worker"));
    h.start(&a);
    let original = h.state(&a.id, ServiceState::Ready);
    let mut b = h.definition("api", "worker");
    b.dependencies = vec![a.id.clone()];
    // A delayed readiness marker proves C is not started before B's readiness, not just spawn.
    b.readiness.kind = ReadinessKind::LogPhrase;
    b.readiness.phrase = "LATE_READY".into();
    b.executable = supervisor::resolve_program("node")
        .unwrap()
        .to_string_lossy()
        .into();
    let ready_file = h.temp.path().join("app/ready");
    b.args=vec!["-e".into(),format!("setTimeout(()=>{{require('node:fs').writeFileSync({:?},'ready'); console.log('LATE_READY')}},500);setInterval(()=>{{}},1000)",ready_file.to_string_lossy())];
    let b = h.save(b);
    let mut c = h.definition("web", "worker");
    c.dependencies = vec![b.id.clone()];
    c.executable = supervisor::resolve_program("node")
        .unwrap()
        .to_string_lossy()
        .into();
    c.args = vec![
        "-e".into(),
        format!(
            "if(!require('node:fs').existsSync({:?})) process.exit(9);setInterval(()=>{{}},1000)",
            ready_file.to_string_lossy()
        ),
    ];
    let c = h.save(c);
    let w = group(&h, vec![c.id.clone()]);
    let preview = h.core.prepare_workspace_action(&w.id, "start").unwrap();
    assert_eq!(
        preview
            .plan
            .iter()
            .map(|s| s.definition.id.clone())
            .collect::<Vec<_>>(),
        vec![a.id.clone(), b.id.clone(), c.id.clone()]
    );
    h.core.confirm_workspace_action(&preview.token).unwrap();
    assert!(h.core.confirm_workspace_action(&preview.token).is_err());
    let run = group_state(&h, &w.id, "READY");
    assert_eq!(run.reused_service_ids, vec![a.id.clone()]);
    assert_eq!(run.owned_run_ids.len(), 2);
    assert_eq!(h.core.service_history(&a.id).unwrap().len(), 1);
    stop_group(&h, &w.id);
    assert_eq!(h.state(&a.id, ServiceState::Ready).id, original.id);
    h.state(&b.id, ServiceState::Stopped);
    h.state(&c.id, ServiceState::Stopped);
    h.stopped(&a.id);
}
#[test]
fn workspace_failure_rolls_back_new_runs_but_preserves_preexisting() {
    let h = Harness::new();
    let a = h.save(h.definition("existing", "worker"));
    h.start(&a);
    h.state(&a.id, ServiceState::Ready);
    let mut b = h.definition("new", "worker");
    b.dependencies = vec![a.id.clone()];
    let b = h.save(b);
    let mut c = h.definition("fails", "exit");
    c.dependencies = vec![b.id.clone()];
    let c = h.save(c);
    let w = group(&h, vec![c.id.clone()]);
    start_group(&h, &w.id);
    let run = group_state(&h, &w.id, "FAILED");
    assert!(run.error.unwrap().contains("fails"));
    let failure = run.failure.unwrap();
    assert_eq!(failure.service_id, c.id);
    assert_eq!(failure.stage, FailureStage::Readiness);
    assert_eq!(
        failure.log_run_id.as_deref(),
        Some(h.core.service_history(&c.id).unwrap()[0].id.as_str())
    );
    h.state(&b.id, ServiceState::Stopped);
    h.state(&a.id, ServiceState::Ready);
    assert!(h.core.service_history(&c.id).unwrap()[0]
        .exit_code
        .is_some());
    h.stopped(&a.id);
}
#[test]
fn workspace_failed_check_keeps_the_original_log_after_a_successful_retry() {
    let h = Harness::new();
    let mut db = h.definition("database", "worker");
    db.readiness.kind = ReadinessKind::LogPhrase;
    db.readiness.phrase = "MISSING_MARKER".into();
    db.readiness.timeout_seconds = 1;
    let mut db = h.save(db);
    let w = group(&h, vec![db.id.clone()]);
    start_group(&h, &w.id);
    let failed = group_state(&h, &w.id, "FAILED");
    let original = h.core.service_history(&db.id).unwrap()[0].id.clone();
    db.readiness.phrase = "FIXTURE_READY".into();
    h.save(db);
    start_group(&h, &w.id);
    let retry = group_state(&h, &w.id, "READY");
    assert!(retry.failure.is_none());
    assert!(!retry.owned_run_ids.contains(&original));
    stop_group(&h, &w.id);
    let saved = h
        .core
        .workspace_history(&w.id)
        .unwrap()
        .into_iter()
        .find(|r| r.id == failed.id)
        .unwrap();
    let failure = saved.failure.as_ref().unwrap();
    assert_eq!(failure.stage, FailureStage::Readiness);
    assert_eq!(failure.log_run_id.as_ref(), Some(&original));
    assert_eq!(saved.plan[0].definition.readiness.phrase, "MISSING_MARKER");
    let logs = h.core.service_logs(&original, 500, "").unwrap();
    assert!(logs.lines.iter().any(|l| l.contains("就绪检查超时")));
    let mut legacy = serde_json::to_value(&saved).unwrap();
    legacy.as_object_mut().unwrap().remove("failure");
    assert!(
        serde_json::from_value::<devnook_core::workspace::WorkspaceRun>(legacy)
            .unwrap()
            .failure
            .is_none()
    );
}

#[test]
fn workspace_final_check_points_to_the_reused_run_that_lost_readiness() {
    let h = Harness::new();
    let http = |name: &str, unhealthy: &Path| {
        let port = free_port();
        let mut d = h.definition(name, "worker");
        d.environment
            .insert("FIXTURE_PORT".into(), port.to_string());
        d.environment
            .insert("HEALTH_BAD_FILE".into(), unhealthy.to_string_lossy().into());
        d.readiness.kind = ReadinessKind::Http;
        d.readiness.url = format!("http://127.0.0.1:{port}/health");
        d.readiness.timeout_seconds = 12;
        d
    };
    let api_unhealthy = h.temp.path().join("api-unhealthy");
    let web_unhealthy = h.temp.path().join("web-unhealthy");
    fs::write(&web_unhealthy, "503").unwrap();
    let api = h.save(http("api", &api_unhealthy));
    let original = h.start(&api);
    h.state(&api.id, ServiceState::Ready);
    let mut web = http("web", &web_unhealthy);
    web.dependencies.push(api.id.clone());
    let web = h.save(web);
    let w = group(&h, vec![web.id.clone()]);
    start_group(&h, &w.id);
    wait(|| {
        h.core
            .service_history(&web.id)
            .ok()?
            .into_iter()
            .next()
            .filter(|r| r.state == ServiceState::Running)
    });
    fs::write(api_unhealthy, "503").unwrap();
    h.state(&api.id, ServiceState::Running);
    fs::remove_file(web_unhealthy).unwrap();
    let failed = group_state(&h, &w.id, "FAILED");
    let failure = failed.failure.unwrap();
    assert_eq!(failure.service_id, api.id);
    assert_eq!(failure.stage, FailureStage::FinalCheck);
    assert_eq!(failure.log_run_id.as_ref(), Some(&original.id));
    assert!(!failed.owned_run_ids.contains(&original.id));
    h.state(&web.id, ServiceState::Stopped);
    assert_eq!(h.state(&api.id, ServiceState::Running).id, original.id);
    h.stopped(&api.id);
}

#[test]
fn workspace_preflight_port_conflict_identifies_service_without_a_log() {
    let h = Harness::new();
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let mut d = h.definition("api", "worker");
    d.expected_ports = vec![listener.local_addr().unwrap().port()];
    let d = h.save(d);
    let w = group(&h, vec![d.id.clone()]);
    start_group(&h, &w.id);
    let failed = group_state(&h, &w.id, "FAILED");
    let failure = failed.failure.unwrap();
    assert_eq!(failure.service_id, d.id);
    assert_eq!(failure.stage, FailureStage::Launch);
    assert!(failure.log_run_id.is_none());
    assert!(failed.error.unwrap().contains("占用"));
}

#[test]
fn workspace_cancellation_reservations_cycles_and_stale_preview() {
    let h = Harness::new();
    let a = h.save(h.definition("base", "worker"));
    let mut b = h.definition("wait", "worker");
    b.dependencies = vec![a.id.clone()];
    b.readiness.kind = ReadinessKind::LogPhrase;
    b.readiness.phrase = "NEVER".into();
    let b = h.save(b);
    let mut cycle = a.clone();
    cycle.dependencies = vec![b.id.clone()];
    assert!(h
        .core
        .save_service(ServiceSave {
            definition: cycle,
            secret_values: Default::default()
        })
        .is_err());
    let mut unknown = a.clone();
    unknown.dependencies = vec!["missing".into()];
    assert!(h
        .core
        .save_service(ServiceSave {
            definition: unknown,
            secret_values: Default::default()
        })
        .is_err());
    assert_eq!(h.start(&b).state, ServiceState::Blocked);
    let w = group(&h, vec![b.id.clone()]);
    let old = h.core.prepare_workspace_action(&w.id, "start").unwrap();
    let mut edited = a.clone();
    edited.name = "changed".into();
    let a = h.save(edited);
    assert!(h.core.confirm_workspace_action(&old.token).is_err());
    start_group(&h, &w.id);
    wait(|| {
        h.core
            .service_history(&b.id)
            .ok()?
            .first()
            .filter(|r| r.state.active())
            .cloned()
    });
    assert!(h.core.stop_service(&a.id).is_err());
    assert!(h
        .core
        .save_service(ServiceSave {
            definition: a.clone(),
            secret_values: Default::default()
        })
        .is_err());
    let overlapping = group(&h, vec![a.id.clone()]);
    let p = h
        .core
        .prepare_workspace_action(&overlapping.id, "start")
        .unwrap();
    assert!(h.core.confirm_workspace_action(&p.token).is_err());
    let cancel = h.core.prepare_workspace_action(&w.id, "stop").unwrap();
    h.core.confirm_workspace_action(&cancel.token).unwrap();
    assert!(group_state(&h, &w.id, "FAILED").failure.is_none());
    h.state(&a.id, ServiceState::Stopped);
    h.state(&b.id, ServiceState::Stopped);
    assert!(h.core.archive_service(&a.id).is_err());
    assert!(h.core.remove_project(&h.project_id).is_err());
}
#[test]
fn workspace_stop_preserves_newer_manual_restart_and_reports_failed_rollback() {
    let h = Harness::new();
    let a = h.save(h.definition("restart", "worker"));
    let w = group(&h, vec![a.id.clone()]);
    start_group(&h, &w.id);
    group_state(&h, &w.id, "READY");
    let p = h.core.prepare_service_action(&a.id, "restart").unwrap();
    let new_run = h.core.confirm_service_action(&p.token).unwrap();
    h.state(&a.id, ServiceState::Ready);
    stop_group(&h, &w.id);
    assert_eq!(h.state(&a.id, ServiceState::Ready).id, new_run.id);
    h.stopped(&a.id);
    let stubborn = h.save(h.definition("stubborn", "ignore_term"));
    let mut failed = h.definition("fail", "exit");
    failed.dependencies = vec![stubborn.id.clone()];
    let failed = h.save(failed);
    let w = group(&h, vec![failed.id.clone()]);
    start_group(&h, &w.id);
    let run = group_state(&h, &w.id, "ROLLBACK_FAILED");
    assert!(run.error.unwrap().contains("停止未完成"));
    let p = h
        .core
        .prepare_service_action(&stubborn.id, "force_stop")
        .unwrap();
    h.core.confirm_service_action(&p.token).unwrap();
    wait(|| {
        h.core
            .service_history(&stubborn.id)
            .ok()?
            .into_iter()
            .next()
            .filter(|r| !r.state.active())
    });
    stop_group(&h, &w.id);
}

#[test]
fn phase2_migration_backup_and_legacy_service_payloads() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("v2.db");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("PRAGMA user_version=2; CREATE TABLE service_definitions(id TEXT PRIMARY KEY,project_id TEXT,archived INTEGER NOT NULL DEFAULT 0,payload TEXT NOT NULL);CREATE TABLE service_runs(seq INTEGER PRIMARY KEY AUTOINCREMENT,id TEXT UNIQUE NOT NULL,service_id TEXT,state TEXT,payload TEXT);").unwrap();
    drop(db);
    let db = devnook_core::storage::open(&path).unwrap();
    assert!(path.with_extension("phase2-backup.db").exists());
    assert_eq!(
        db.query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        3
    );
    drop(db);
    devnook_core::storage::open(&path).unwrap();
    let h = Harness::new();
    let d = h.save(h.definition("legacy", "worker"));
    let mut json = serde_json::to_value(&d).unwrap();
    json.as_object_mut().unwrap().remove("dependencies");
    assert!(serde_json::from_value::<ServiceDefinition>(json)
        .unwrap()
        .dependencies
        .is_empty());
    let run = service_storage::failed_run(&d, ServiceState::Stopped, String::new(), String::new());
    let mut json = serde_json::to_value(&run).unwrap();
    json.as_object_mut().unwrap().remove("workspace_run_id");
    assert!(serde_json::from_value::<ServiceRun>(json)
        .unwrap()
        .workspace_run_id
        .is_none());
}
