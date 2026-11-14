use devnook_core::{
    model::{ProjectEdit, ScanProgress, Settings},
    process, Core,
};
use std::{
    fs,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tempfile::TempDir;

fn write(root: &Path, name: &str, contents: &str) {
    let target = root.join(name);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(target, contents).unwrap();
}
fn complete(core: &Core, id: &str) -> ScanProgress {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let progress = core.scan_status(id).unwrap();
        if progress.status != "scanning" {
            return progress;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn import_all(core: &Core, root: &Path) -> Vec<devnook_core::model::Project> {
    let id = core.scan_start(root.to_string_lossy().into(), 4).unwrap();
    let progress = complete(core, &id);
    core.import_projects(
        &id,
        progress.candidates.iter().map(|c| c.id.clone()).collect(),
    )
    .unwrap()
}

#[test]
fn scanning_languages_monorepo_skip_symlinks_and_no_execution() {
    let temp = TempDir::new().unwrap();
    write(
        temp.path(),
        "web/package.json",
        r#"{"dependencies":{"react":"*","vite":"*"},"scripts":{"dev":"touch SHOULD_NOT_EXIST"},"workspaces":["packages/*"]}"#,
    );
    write(temp.path(), "web/pnpm-lock.yaml", "lockfileVersion: '9.0'");
    write(
        temp.path(),
        "web/packages/ui/package.json",
        r#"{"name":"ui"}"#,
    );
    write(
        temp.path(),
        "java/pom.xml",
        "<project><modules><module>api</module></modules>spring-boot</project>",
    );
    write(
        temp.path(),
        "gradle/build.gradle.kts",
        "plugins { id(\"org.springframework.boot\") }",
    );
    write(
        temp.path(),
        "python/pyproject.toml",
        "[project]\ndependencies = ['fastapi']",
    );
    write(temp.path(), "rust/Cargo.toml", "[package]\nname='test'");
    write(temp.path(), "go/go.mod", "module example.com/test");
    write(temp.path(), "web/node_modules/ignored/package.json", "{}");
    write(temp.path(), "java/target/ignored/pom.xml", "<project/>");
    write(temp.path(), "backup/ignored/package.json", "{}");
    write(temp.path(), "broken/package.json", "INVALID");
    std::os::unix::fs::symlink(temp.path().join("web"), temp.path().join("alias")).unwrap();
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let id = core
        .scan_start(temp.path().to_string_lossy().into(), 5)
        .unwrap();
    let p = complete(&core, &id);
    assert_eq!(p.candidates.len(), 7);
    let web = p.candidates.iter().find(|c| c.name == "web").unwrap();
    assert!(web.runnable && web.is_monorepo);
    assert!(web.build_tools.contains(&"pnpm".into()));
    assert!(web.frameworks.contains(&"React".into()));
    let nested = web
        .components
        .iter()
        .find(|c| c.name == "packages/ui")
        .unwrap();
    assert!(!nested.runnable);
    assert!(
        p.candidates
            .iter()
            .find(|c| c.name == "broken")
            .unwrap()
            .needs_review
    );
    assert!(!temp.path().join("web/SHOULD_NOT_EXIST").exists());
    assert!(core.projects().unwrap().is_empty());
}

#[test]
fn persistence_dedup_edit_and_remove_preserve_source() {
    let temp = TempDir::new().unwrap();
    write(temp.path(), "app/package.json", "{}");
    let database = temp.path().join("registry.db");
    let core = Core::new(&database).unwrap();
    let imported = import_all(&core, temp.path());
    assert_eq!(imported.len(), 1);
    assert_eq!(import_all(&core, temp.path()).len(), 1);
    let id = imported[0].candidate.id.clone();
    core.edit_project(ProjectEdit {
        id: id.clone(),
        name: "Changed".into(),
        default_ide: "CURSOR".into(),
        is_favorite: true,
        category: "产品".into(),
    })
    .unwrap();
    let settings = Settings {
        refresh_seconds: 5,
        default_ide: "CURSOR".into(),
        ..Settings::default()
    };
    core.save_settings(settings).unwrap();
    drop(core);
    let core = Core::new(&database).unwrap();
    let project = &core.projects().unwrap()[0];
    assert_eq!(project.candidate.name, "Changed");
    assert!(project.is_favorite);
    assert_eq!(project.category, "产品");
    assert_eq!(core.settings().unwrap().refresh_seconds, 5);
    core.remove_project(&id).unwrap();
    assert!(core.projects().unwrap().is_empty());
    assert!(temp.path().join("app/package.json").exists());
}

#[test]
fn cancelled_scan_cannot_import_and_depth_is_bounded() {
    let temp = TempDir::new().unwrap();
    write(temp.path(), "a/b/package.json", "{}");
    let job = Arc::new(devnook_core::scanner::ScanJob {
        cancel: AtomicBool::new(true),
        progress: Mutex::new(ScanProgress::default()),
    });
    devnook_core::scanner::run(temp.path().into(), 5, Default::default(), job.clone());
    assert_eq!(job.progress.lock().unwrap().status, "cancelled");
    assert!(job.cancel.load(Ordering::Relaxed));
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let id = core
        .scan_start(temp.path().to_string_lossy().into(), 1)
        .unwrap();
    assert!(complete(&core, &id).candidates.is_empty());
    assert!(core
        .import_projects(&id, vec!["forged-candidate".into()])
        .is_err());
    core.scan_cancel(&id).unwrap();
    // Cancellation also invalidates a scan that completed before the request arrived.
    assert!(core.import_projects(&id, vec![]).is_err());
    assert!(core
        .scan_start(temp.path().to_string_lossy().into(), 13)
        .is_err());
    assert!(core.scan_start("/path/does/not/exist".into(), 2).is_err());
}

#[test]
fn lsof_keeps_ipv4_ipv6_addresses_and_multiple_ports() {
    let rows = process::parse_ports("p100\ncnode\nf10\ntIPv4\nn127.0.0.1:3000\nf11\ntIPv6\nn[::1]:3000\nf12\ntIPv4\nn*:5173\nf13\ntIPv4\nn*:5173\np200\ncprocess with spaces\nf5\ntIPv6\nn*:8080\n");
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[1].family, "IPv6");
    assert_eq!(rows[1].address, "[::1]");
    assert_eq!(rows[3].name, "process with spaces");
    assert_eq!(rows[2].port, 5173);
    assert!(process::parse_ports("pbad\ncnode\ntIPv4\nnnot-an-address").is_empty());
}

#[test]
fn longest_directory_match_has_component_boundaries() {
    let temp = TempDir::new().unwrap();
    write(temp.path(), "app/package.json", "{}");
    write(temp.path(), "app/packages/web/package.json", "{}");
    write(temp.path(), "app/packages/web/.git", "gitdir: not-followed");
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let projects = import_all(&core, temp.path());
    let path = fs::canonicalize(temp.path()).unwrap();
    assert_eq!(
        process::match_project(
            &path.join("app/packages/web/src").to_string_lossy(),
            &projects
        )
        .unwrap()
        .candidate
        .name,
        "web"
    );
    assert!(process::match_project(&path.join("app-other").to_string_lossy(), &projects).is_none());
}

#[test]
fn command_secrets_are_not_exposed() {
    let result = process::redact_command(
        "node api --token secret --password=pass API_KEY=key https://user:pass@example.com",
    );
    assert!(!result.contains("secret"));
    assert!(!result.contains("=pass"));
    assert!(!result.contains("=key"));
    assert!(!result.contains("user:pass"));
}

#[test]
fn settings_and_ide_errors_are_explicit() {
    let temp = TempDir::new().unwrap();
    write(temp.path(), "app/package.json", "{}");
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let project = import_all(&core, temp.path()).remove(0);
    let mut settings = Settings::default();
    settings.refresh_seconds = 0;
    assert!(core.save_settings(settings.clone()).is_err());
    settings.refresh_seconds = 2;
    settings.idea_path = temp.path().join("Missing.app").to_string_lossy().into();
    assert!(core.save_settings(settings.clone()).is_err());
    assert!(core
        .open_project(&project.candidate.id, "VSCODE;touch should-not-exist")
        .is_err());
    if cfg!(target_os = "macos") {
        let error = devnook_core::ide::open(&project, "IDEA", &settings).unwrap_err();
        assert!(error.contains("无法打开"));
    }
    assert!(!temp.path().join("app/should-not-exist").exists());
}

#[test]
#[ignore = "only invoked as an isolated fixture by the safety integration test"]
fn fixture_server() {
    let Some(file) = std::env::var_os("DEVNOOK_FIXTURE_PORT") else {
        return;
    };
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    fs::write(file, listener.local_addr().unwrap().port().to_string()).unwrap();
    loop {
        std::thread::sleep(Duration::from_secs(1));
        std::hint::black_box(&listener);
    }
}

#[test]
#[cfg(target_os = "macos")]
fn real_macos_discovery_identity_assignment_and_confirmed_sigterm() {
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let temp = TempDir::new().unwrap();
    write(temp.path(), "app/package.json", "{}");
    let canonical = fs::canonicalize(temp.path().join("app")).unwrap();
    let port_file = temp.path().join("port");
    let mut child = Child(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "fixture_server", "--ignored", "--nocapture"])
            .env("DEVNOOK_FIXTURE_PORT", &port_file)
            .current_dir(&canonical)
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while !port_file.exists() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let pid = child.0.id();
    let unknown = core.inspect(pid).unwrap();
    assert!(!unknown.can_stop);
    assert_eq!(unknown.confidence, "NONE");
    assert!(unknown.bindings.iter().any(|b| b.pid == pid));
    assert!(unknown.bindings.iter().all(|b| b.project_id.is_none()));
    assert!(core.prepare_stop(pid).is_err());
    let projects = import_all(&core, temp.path());
    let project = &projects[0];
    let detail = core.inspect(pid).unwrap();
    assert_eq!(detail.confidence, "HIGH");
    assert_eq!(detail.project_id.as_ref(), Some(&project.candidate.id));
    assert!(detail.can_stop);
    assert_eq!(detail.cwd.as_deref(), canonical.to_str());
    // A listener started before import is linked on the next snapshot, without takeover.
    assert!(detail.bindings.iter().all(|b| {
        b.project_id.as_ref() == Some(&project.candidate.id)
            && b.source == "OBSERVED"
            && b.service_id.is_none()
    }));
    assert!(detail
        .bindings
        .iter()
        .any(|b| b.port.to_string() == fs::read_to_string(&port_file).unwrap()));
    assert!(core
        .assign_process(pid, "reused-pid", &project.candidate.id)
        .is_err());
    assert_eq!(
        core.assign_process(pid, &detail.fingerprint, &project.candidate.id)
            .unwrap()
            .confidence,
        "MANUAL"
    );
    let associated = core.ports().unwrap();
    assert!(associated.iter().filter(|b| b.pid == pid).all(|b| {
        b.project_id.as_ref() == Some(&project.candidate.id)
            && b.source == "OBSERVED"
            && b.service_id.is_none()
            && b.process_kind == devnook_core::model::ProcessKind::Unknown
    }));
    assert!(child.0.try_wait().unwrap().is_none());
    assert!(core.prepare_stop(std::process::id()).is_err());
    let wrong = core.prepare_stop(pid).unwrap();
    assert!(core.confirm_stop(&wrong.token, pid + 1).is_err());
    assert!(child.0.try_wait().unwrap().is_none());
    let confirmation = core.prepare_stop(pid).unwrap();
    core.confirm_stop(&confirmation.token, pid).unwrap();
    assert!(core.confirm_stop(&confirmation.token, pid).is_err());
    let deadline = Instant::now() + Duration::from_secs(4);
    while child.0.try_wait().unwrap().is_none() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(30));
    }
    assert!(core.ports().unwrap().iter().all(|b| b.pid != pid));
    assert!(core.inspect(pid).is_err());
}
