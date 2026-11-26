#![cfg(target_os = "macos")]
use devnook_core::{model::ProcessKind, Core};
use serde_json::Value;
use std::{
    fs,
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use tempfile::TempDir;

struct Fixture(Child);
impl Drop for Fixture {
    fn drop(&mut self) {
        // Only signal the child spawned by the test.
        unsafe {
            libc::kill(self.0.id() as i32, libc::SIGTERM);
        }
        let until = Instant::now() + Duration::from_secs(5);
        while self.0.try_wait().unwrap().is_none() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(20));
        }
        if self.0.try_wait().unwrap().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}

#[test]
fn vite_bin_and_local_python_module_are_visible_as_project_services() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap().join("project");
    fs::create_dir_all(root.join("node_modules/.bin")).unwrap();
    fs::create_dir_all(root.join("node_modules/vite/bin")).unwrap();
    fs::write(
        root.join("package.json"),
        r#"{"scripts":{"dev":"vite"},"devDependencies":{"vite":"*"} }"#,
    )
    .unwrap();
    fs::write(
        root.join("node_modules/vite/bin/vite.js"),
        "require('node:net').createServer().listen(0, '127.0.0.1');",
    )
    .unwrap();
    let vite_path = root.join("node_modules/.bin/vite");
    std::os::unix::fs::symlink("../vite/bin/vite.js", &vite_path).unwrap();
    fs::create_dir(root.join("project_api")).unwrap();
    fs::write(root.join("project_api/__init__.py"), "").unwrap();
    fs::write(
        root.join("project_api/__main__.py"),
        "import socket\nwith socket.socket() as listener:\n    listener.bind(('127.0.0.1', 0))\n    listener.listen()\n    listener.accept()\n",
    )
    .unwrap();

    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let scan = core.scan_start(root.to_string_lossy().into(), 0).unwrap();
    let preview = wait(|| {
        let p = core.scan_status(&scan).unwrap();
        (p.status == "completed").then_some(p)
    });
    let projects = core
        .import_projects(
            &scan,
            preview.candidates.iter().map(|c| c.id.clone()).collect(),
        )
        .unwrap();
    let project_id = &projects[0].candidate.id;
    let spawn = |program, args: &[&str]| {
        Fixture(
            Command::new(devnook_core::supervisor::resolve_program(program).unwrap())
                .args(args)
                .current_dir(&root)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        )
    };
    let vite = spawn("node", &[vite_path.to_str().unwrap()]);
    let python = spawn("python3", &["-B", "-m", "project_api", "web"]);
    let pids = [vite.0.id(), python.0.id()];
    let bindings = wait(|| {
        let bindings = core.ports().unwrap();
        pids.iter()
            .all(|pid| bindings.iter().any(|b| b.pid == *pid))
            .then_some(bindings)
    });
    for binding in bindings.iter().filter(|b| pids.contains(&b.pid)) {
        assert_eq!(
            binding.process_kind,
            ProcessKind::Service,
            "{}",
            binding.name
        );
        assert_eq!(binding.project_id.as_ref(), Some(project_id));
        assert_eq!(binding.source, "OBSERVED");
        assert!(binding.service_id.is_none());
    }
}

fn wait<T>(mut check: impl FnMut() -> Option<T>) -> T {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(result) = check() {
            return result;
        }
        assert!(Instant::now() < until, "fixture did not become ready");
        std::thread::sleep(Duration::from_millis(30));
    }
}

#[test]
fn tool_listener_stays_in_full_snapshot_but_child_service_is_recognized_after_import() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap().join("project");
    let tool = root.join("node_modules/@openai/codex/bin/codex.js");
    fs::create_dir_all(tool.parent().unwrap()).unwrap();
    fs::write(
        root.join("package.json"),
        r#"{"scripts":{"dev":"node server.js"}}"#,
    )
    .unwrap();
    fs::write(root.join("server.js"), r#"
const fs = require('node:fs');
const server = require('node:net').createServer();
server.listen(0, '127.0.0.1', () => fs.writeFileSync('service.json', JSON.stringify({pid:process.pid,port:server.address().port})));
process.on('SIGTERM', () => server.close(() => process.exit(0)));
"#).unwrap();
    // Disposable tool-shaped fixture, not a real Codex invocation. Both ports are
    // real kernel listeners, the child is launched by the fixture itself.
    fs::write(&tool, r#"
const fs = require('node:fs');
const child = require('node:child_process').spawn(process.execPath, ['server.js'], {stdio:'ignore'});
const server = require('node:net').createServer();
server.listen(0, '127.0.0.1', () => fs.writeFileSync('tool.json', JSON.stringify({pid:process.pid,port:server.address().port})));
process.on('SIGTERM', () => { child.kill('SIGTERM'); server.close(); child.on('exit', () => process.exit(0)); });
"#).unwrap();
    let node = ["/opt/homebrew/bin/node", "/usr/local/bin/node"]
        .into_iter()
        .find(|p| Path::new(p).is_file())
        .expect("Node required by integration tests");
    let mut fixture = Fixture(
        Command::new(node)
            .arg(&tool)
            .current_dir(&root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let ready = |name: &str| {
        fs::read(root.join(name))
            .ok()
            .and_then(|data| serde_json::from_slice::<Value>(&data).ok())
    };
    let tool_ready = wait(|| ready("tool.json"));
    let service_ready = wait(|| ready("service.json"));
    let tool_pid = tool_ready["pid"].as_u64().unwrap() as u32;
    let service_pid = service_ready["pid"].as_u64().unwrap() as u32;
    assert_eq!(fixture.0.id(), tool_pid);
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let before = core.ports().unwrap();
    assert!(before.iter().any(|b| b.pid == tool_pid
        && b.process_kind == ProcessKind::DevelopmentTool
        && b.project_id.is_none()));
    assert!(before
        .iter()
        .any(|b| b.pid == service_pid && b.process_kind == ProcessKind::Unknown));
    let scan = core.scan_start(root.to_string_lossy().into(), 0).unwrap();
    let preview = wait(|| {
        let p = core.scan_status(&scan).unwrap();
        (p.status != "scanning").then_some(p)
    });
    let projects = core
        .import_projects(
            &scan,
            preview.candidates.iter().map(|c| c.id.clone()).collect(),
        )
        .unwrap();
    let project_id = &projects[0].candidate.id;
    let after = core.ports().unwrap();
    for (pid, expected, ready) in [
        (tool_pid, ProcessKind::DevelopmentTool, &tool_ready),
        (service_pid, ProcessKind::Service, &service_ready),
    ] {
        let bindings: Vec<_> = after.iter().filter(|b| b.pid == pid).collect();
        assert!(!bindings.is_empty());
        assert!(bindings.iter().all(|b| b.process_kind == expected
            && b.project_id.as_ref() == Some(project_id)
            && b.source == "OBSERVED"
            && b.service_id.is_none()));
        assert!(bindings
            .iter()
            .any(|b| b.port as u64 == ready["port"].as_u64().unwrap()));
    }
    let detail = core.inspect(tool_pid).unwrap();
    core.assign_process(tool_pid, &detail.fingerprint, project_id)
        .unwrap();
    assert!(core
        .ports()
        .unwrap()
        .iter()
        .filter(|b| b.pid == tool_pid)
        .all(|b| b.process_kind == ProcessKind::DevelopmentTool && b.source == "OBSERVED"));
    assert_eq!(core.inspect(service_pid).unwrap().ppid, tool_pid);
    assert!(fixture.0.try_wait().unwrap().is_none());
    assert!(core.services().unwrap().is_empty());
}
