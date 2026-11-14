#![cfg(target_os = "macos")]
use devnook_core::{
    service_model::*,
    supervisor::{self, RunnerLauncher},
    Core,
};
use std::{
    collections::BTreeMap,
    fs,
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};

// Runner fixture uses the same production worker code, with the test harness as its entry point.
#[test]
#[ignore = "internal Java service runner fixture"]
fn fixture_runner() {
    devnook_core::service_worker::from_stdin().unwrap();
}

#[test]
fn real_java_service_starts_and_stops_as_a_managed_process_group() {
    let temp = tempfile::TempDir::new().unwrap();
    let app = temp.path().join("app");
    fs::create_dir(&app).unwrap();
    fs::write(app.join("pom.xml"), "<project/>").unwrap();
    fs::write(app.join("FixtureServer.java"),r#"import java.net.*; public class FixtureServer { public static void main(String[] args) throws Exception { ServerSocket socket = new ServerSocket(Integer.parseInt(System.getenv("FIXTURE_PORT")), 50, InetAddress.getByName("127.0.0.1")); System.out.println("JAVA_READY"); while (true) { Thread.sleep(100); } } }"#).unwrap();
    let javac = supervisor::resolve_program("javac").expect("Java smoke tests require a JDK");
    assert!(Command::new(javac)
        .arg(app.join("FixtureServer.java"))
        .status()
        .unwrap()
        .success());
    let launcher = RunnerLauncher {
        executable: std::env::current_exe().unwrap(),
        arguments: vec![
            "--exact".into(),
            "fixture_runner".into(),
            "--ignored".into(),
            "--nocapture".into(),
        ],
    };
    let core = Core::with_runtime(
        &temp.path().join("devnook.db"),
        launcher,
        Arc::new(devnook_core::secrets::KeychainStore),
    )
    .unwrap();
    let scan = core
        .scan_start(temp.path().to_string_lossy().into(), 2)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let progress = loop {
        let p = core.scan_status(&scan).unwrap();
        if p.status == "completed" {
            break p;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    };
    let project = core
        .import_projects(
            &scan,
            progress.candidates.iter().map(|c| c.id.clone()).collect(),
        )
        .unwrap()
        .remove(0);
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let definition = ServiceDefinition {
        id: "".into(),
        project_id: project.candidate.id,
        name: "java-api".into(),
        runtime_type: "JAVA".into(),
        executable: supervisor::resolve_program("java")
            .unwrap()
            .to_string_lossy()
            .into(),
        args: vec![
            "-cp".into(),
            fs::canonicalize(&app).unwrap().to_string_lossy().into(),
            "FixtureServer".into(),
        ],
        shell_mode: false,
        working_directory: fs::canonicalize(&app).unwrap().to_string_lossy().into(),
        environment: BTreeMap::from([("FIXTURE_PORT".into(), port.to_string())]),
        secret_keys: vec![],
        dependencies: vec![],
        expected_ports: vec![port],
        readiness: Readiness {
            kind: ReadinessKind::Port,
            port: Some(port),
            timeout_seconds: 10,
            ..Readiness::default()
        },
        stop_grace_seconds: 2,
        log_limit_kb: 64,
        revision: "".into(),
        created_at: 0,
        updated_at: 0,
    };
    let service = core
        .save_service(ServiceSave {
            definition,
            secret_values: Default::default(),
        })
        .unwrap();
    let preview = core.prepare_service_action(&service.id, "start").unwrap();
    let run = core.confirm_service_action(&preview.token).unwrap();
    struct Cleanup(Option<ServiceRun>);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            if let Some(run) = &self.0 {
                if supervisor::anchor_valid(run) {
                    unsafe {
                        libc::kill(-(run.pgid.unwrap() as i32), libc::SIGKILL);
                    }
                }
            }
        }
    }
    let _cleanup = Cleanup(Some(run));
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        let current = core.service_history(&service.id).unwrap().remove(0);
        if current.state == ServiceState::Ready {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Java service did not become ready: {:?}",
            current
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    core.stop_service(&service.id).unwrap();
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        let current = core.service_history(&service.id).unwrap().remove(0);
        if current.state == ServiceState::Stopped {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Java service did not stop: {:?}",
            current
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(!core.ports().unwrap().iter().any(|b| b.port == port));
}
