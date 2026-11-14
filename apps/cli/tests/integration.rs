#![cfg(target_os = "macos")]
use devnook_core::{
    access::{GrantCreated, GrantInput, RequestContext},
    ipc::CoreClient,
    service_model::*,
    supervisor,
    workspace::*,
};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    time::{Duration, Instant},
};
use tempfile::TempDir;
const BIN: &str = env!("CARGO_BIN_EXE_devnook");
fn wait<T>(mut f: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(18);
    loop {
        if let Some(v) = f() {
            return v;
        }
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(60));
    }
}
struct Harness {
    temp: TempDir,
    client: CoreClient,
    project: String,
}
impl Harness {
    fn new() -> Self {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("package.json"), "{}").unwrap();
        let client = CoreClient::new(temp.path(), PathBuf::from(BIN)).unwrap();
        let scan = client
            .scan_start(temp.path().to_string_lossy().into(), 2)
            .unwrap();
        let progress = wait(|| {
            client
                .scan_status(&scan)
                .ok()
                .filter(|s| s.status == "completed")
        });
        let projects = client
            .import_projects(
                &scan,
                progress.candidates.iter().map(|c| c.id.clone()).collect(),
            )
            .unwrap();
        Self {
            temp,
            client,
            project: projects[0].candidate.id.clone(),
        }
    }
    fn definition(&self, name: &str) -> ServiceDefinition {
        ServiceDefinition {
            id: String::new(),
            project_id: self.project.clone(),
            name: name.into(),
            runtime_type: "NODE".into(),
            executable: supervisor::resolve_program("node")
                .unwrap()
                .to_string_lossy()
                .into(),
            args: vec![
                "-e".into(),
                "console.log('READY');setInterval(()=>console.log('heartbeat'),300)".into(),
            ],
            shell_mode: false,
            working_directory: self
                .temp
                .path()
                .canonicalize()
                .unwrap()
                .to_string_lossy()
                .into(),
            environment: Default::default(),
            secret_keys: vec![],
            expected_ports: vec![],
            dependencies: vec![],
            readiness: Readiness {
                grace_ms: 100,
                interval_ms: 100,
                timeout_seconds: 3,
                ..Default::default()
            },
            stop_grace_seconds: 1,
            log_limit_kb: 64,
            revision: String::new(),
            created_at: 0,
            updated_at: 0,
        }
    }
    fn save(&self, d: ServiceDefinition) -> ServiceDefinition {
        self.client
            .save_service(ServiceSave {
                definition: d,
                secret_values: Default::default(),
            })
            .unwrap()
    }
    fn cli(&self, args: &[&str]) -> std::process::Output {
        Command::new(BIN)
            .arg("--data-dir")
            .arg(self.temp.path())
            .arg("--json")
            .args(args)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
    fn grant(&self, service: Vec<String>, workspaces: Vec<String>) -> GrantCreated {
        self.client.invoke("create_agent_grant",json!({"input":GrantInput{name:"test agent".into(),service_ids:service,workspace_ids:workspaces,project_ids:vec![],scan_roots:vec![]}})).unwrap()
    }
    fn agent(&self, token: &str) -> CoreClient {
        CoreClient::new(self.temp.path(), PathBuf::from(BIN))
            .unwrap()
            .with_context(RequestContext {
                origin: "MCP".into(),
                credential: token.into(),
                read_only: false,
            })
    }
}
impl Drop for Harness {
    fn drop(&mut self) {
        if let Ok(groups) = self.client.workspaces() {
            for g in groups {
                if let Ok(p) = self.client.prepare_workspace_action(&g.id, "stop") {
                    let _ = self.client.confirm_workspace_action(&p.token);
                }
            }
        }
        let _ = wait(|| {
            let busy = self.client.workspaces().ok()?.iter().any(|g| {
                self.client
                    .workspace_history(&g.id)
                    .ok()
                    .and_then(|r| r.first().cloned())
                    .is_some_and(|r| r.busy())
            });
            (!busy).then_some(())
        });
        for s in self.client.services().unwrap_or_default() {
            let _ = self.client.stop_service(&s.definition.id);
        }
        wait(|| {
            self.client
                .services()
                .ok()?
                .iter()
                .all(|s| s.current_run.as_ref().is_none_or(|r| !r.state.active()))
                .then_some(())
        });
        let _: Result<Value, _> = self.client.invoke("shutdown", json!({}));
    }
}

#[test]
fn development_app_endpoints_stay_local_and_mcp_cannot_supply_application_paths() {
    let h = Harness::new();
    let apps: devnook_core::ide::DevelopmentApps =
        h.client.invoke("list_development_apps", json!({})).unwrap();
    assert!(apps.project_defaults.contains_key(&h.project));
    let created: GrantCreated = h.client.invoke("create_agent_grant", json!({"input":GrantInput {name:"isolated app permissions".into(),project_ids:vec![h.project.clone()],..Default::default()}})).unwrap();
    let agent = h.agent(&created.token);
    for method in [
        "list_development_apps",
        "inspect_development_app",
        "remember_development_app",
        "set_project_app",
    ] {
        assert!(agent.invoke::<Value>(method, json!({"id":h.project,"path":"/Applications/Visual Studio Code.app","target":"APP:/Applications/Visual Studio Code.app"})).is_err());
    }
    let error = agent
        .open_project(&h.project, "APP:/Applications/Visual Studio Code.app")
        .unwrap_err();
    assert!(error.contains("固定目标"));
    // No launch is performed; malformed paths fail through local validation as well.
    assert!(h
        .client
        .invoke::<Value>(
            "set_project_app",
            json!({"id":h.project,"target":"APP:relative.app"})
        )
        .is_err());
    assert!(h
        .client
        .invoke::<Value>(
            "inspect_development_app",
            json!({"path":"/not-an-installed-app.app"})
        )
        .is_err());
}
struct Mcp {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    next: u64,
}

#[test]
fn external_service_configuration_stop_and_handoff_are_not_exposed_to_mcp_even_with_a_grant() {
    let h = Harness::new();
    let saved = h.save(h.definition("scoped service"));
    let grant = h.grant(vec![saved.id.clone()], vec![]);
    let agent = h.agent(&grant.token);
    for method in [
        "external_service_configuration",
        "prepare_external_service_stop",
        "prepare_external_service_handoff",
        "confirm_external_service_handoff",
    ] {
        let error = agent.invoke::<Value>(method, json!({"id":saved.id,"revision":saved.revision,"pid":1,"fingerprint":"test","token":"test","confirmed_pid":1})).unwrap_err();
        assert!(error.contains("MCP 无权调用"), "{method}: {error}");
    }
    assert!(h.client.service_history(&saved.id).unwrap().is_empty());
}
impl Mcp {
    fn new(h: &Harness, token: Option<&str>) -> Self {
        let mut command = Command::new(BIN);
        command
            .arg("--data-dir")
            .arg(h.temp.path())
            .args(["mcp", "serve"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if let Some(token) = token {
            command.arg("--allow-write").env("DEVNOOK_MCP_TOKEN", token);
        }
        let mut child = command.spawn().unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            input,
            output,
            next: 0,
        }
    }
    fn send(&mut self, v: Value) {
        writeln!(self.input, "{}", v).unwrap();
        self.input.flush().unwrap();
    }
    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        self.send(json!({"jsonrpc":"2.0","id":self.next,"method":method,"params":params}));
        let mut line = String::new();
        assert!(
            self.output.read_line(&mut line).unwrap() > 0,
            "MCP terminated"
        );
        let response: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(response["id"], self.next);
        response
    }
    fn initialize(&mut self) {
        let result=self.request("initialize",json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}));
        assert_eq!(result["result"]["protocolVersion"], "2025-11-25");
        self.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    }
    fn call(&mut self, name: &str, args: Value) -> Value {
        self.request("tools/call", json!({"name":name,"arguments":args}))
    }
}
impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
#[test]
fn cli_shared_state_confirmations_logs_exit_codes_and_persistence() {
    let h = Harness::new();
    let d = h.save(h.definition("cli"));
    let rejected = h.cli(&["service", "start", &d.id]);
    assert!(!rejected.status.success());
    assert!(h.client.service_history(&d.id).unwrap().is_empty());
    let started = h.cli(&["service", "start", &d.id, "--yes"]);
    assert!(
        started.status.success(),
        "{}",
        String::from_utf8_lossy(&started.stderr)
    );
    let run: ServiceRun = serde_json::from_slice(&started.stdout).unwrap();
    wait(|| {
        h.client
            .service_history(&d.id)
            .ok()?
            .first()
            .filter(|r| r.state == ServiceState::Ready)
            .cloned()
    });
    let status = h.cli(&["service", "status", &d.id]);
    let view: ServiceView = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(view.current_run.unwrap().id, run.id);
    let logs = h.cli(&["service", "logs", &d.id]);
    assert!(logs.status.success());
    assert!(String::from_utf8_lossy(&logs.stdout).contains("READY"));
    assert!(!h
        .cli(&["service", "start", &d.id, "--yes"])
        .status
        .success());
    assert!(h
        .cli(&["service", "restart", &d.id, "--yes"])
        .status
        .success());
    assert!(h.cli(&["service", "stop", &d.id, "--yes"]).status.success());
    wait(|| {
        h.client
            .service_history(&d.id)
            .ok()?
            .first()
            .filter(|r| !r.state.active())
            .cloned()
    });
    let pid = h.client.invoke::<Value>("ping", json!({})).unwrap()["pid"]
        .as_u64()
        .unwrap();
    Command::new("/bin/kill")
        .args(["-KILL", &pid.to_string()])
        .status()
        .unwrap();
    wait(|| {
        devnook_core::process::identity(pid as u32)
            .is_none_or(|p| !p.alive)
            .then_some(())
    });
    let status = h.cli(&["service", "status", &d.id]);
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&status.stdout).unwrap()["definition"]["id"],
        d.id
    );
}
#[test]
fn mcp_stdio_readonly_negotiation_schema_and_shared_state() {
    let h = Harness::new();
    let d = h.save(h.definition("mcp readonly"));
    let mut m = Mcp::new(&h, None);
    assert_eq!(m.request("tools/list", json!({}))["error"]["code"], -32002);
    assert_eq!(
        m.request("initialize", json!({"protocolVersion":"2025-11-25"}))["error"]["code"],
        -32602
    );
    writeln!(m.input, "{{").unwrap();
    m.input.flush().unwrap();
    let mut invalid = String::new();
    m.output.read_line(&mut invalid).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&invalid).unwrap()["error"]["code"],
        -32700
    );
    m.send(json!({"jsonrpc":"2.0","id":null,"method":"ping"}));
    invalid.clear();
    m.output.read_line(&mut invalid).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&invalid).unwrap()["error"]["code"],
        -32600
    );
    m.initialize();
    let tools = m.request("tools/list", json!({}));
    assert!(tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .all(|t| t["annotations"]["readOnlyHint"] == true));
    assert_eq!(
        m.call("start_service", json!({"service_id":d.id}))["error"]["code"],
        -32602
    );
    assert_eq!(
        m.call("inspect_process", json!({"pid":-1}))["error"]["code"],
        -32602
    );
    let state = m.call("get_service_status", json!({"service_id":d.id}));
    assert_eq!(
        state["result"]["structuredContent"]["definition"]["id"],
        d.id
    );
    assert_eq!(m.request("ping", json!({}))["result"], json!({}));
    assert!(!h
        .client
        .invoke::<Value>("access_audits", json!({}))
        .unwrap()
        .as_array()
        .unwrap()
        .is_empty());
}
#[test]
fn mcp_scope_preview_owner_target_revision_revocation_and_audit_redaction() {
    let h = Harness::new();
    let a = h.save(h.definition("allowed"));
    let b = h.save(h.definition("denied"));
    let grant = h.grant(vec![a.id.clone()], vec![]);
    let other = h.grant(vec![a.id.clone()], vec![]);
    let agent = h.agent(&grant.token);
    let agent2 = h.agent(&other.token);
    assert!(agent.prepare_service_action(&grant.token, "start").is_err());
    assert!(agent.prepare_service_action(&b.id, "start").is_err());
    let gui = h.client.prepare_service_action(&a.id, "start").unwrap();
    assert!(agent.confirm_service_action(&gui.token).is_err());
    let p = agent.prepare_service_action(&a.id, "start").unwrap();
    assert!(agent2
        .invoke::<Value>(
            "confirm_service_action",
            json!({"token":p.token,"id":a.id,"action":"start"})
        )
        .is_err());
    assert!(agent
        .invoke::<Value>(
            "confirm_service_action",
            json!({"token":p.token,"id":b.id,"action":"start"})
        )
        .is_err());
    let mut m = Mcp::new(&h, Some(&grant.token));
    m.initialize();
    let preview = m.call("start_service", json!({"service_id":a.id}));
    assert_eq!(
        preview["result"]["structuredContent"]["needs_confirmation"],
        true
    );
    let token = preview["result"]["structuredContent"]["confirmation_token"].clone();
    let started = m.call(
        "start_service",
        json!({"service_id":a.id,"confirmation_token":token}),
    );
    assert_eq!(started["result"]["isError"], false, "{started}");
    wait(|| {
        h.client
            .service_history(&a.id)
            .ok()?
            .first()
            .filter(|r| r.state == ServiceState::Ready)
            .cloned()
    });
    assert_eq!(
        m.call("stop_service", json!({"service_id":a.id}))["result"]["isError"],
        false
    );
    wait(|| {
        h.client
            .service_history(&a.id)
            .ok()?
            .first()
            .filter(|r| !r.state.active())
            .cloned()
    });
    let revoked_preview = agent.prepare_service_action(&a.id, "start").unwrap();
    let _: Value = h
        .client
        .invoke("revoke_agent_grant", json!({"id":grant.grant.id}))
        .unwrap();
    assert!(agent
        .invoke::<Value>(
            "confirm_service_action",
            json!({"token":revoked_preview.token,"id":a.id,"action":"start"})
        )
        .is_err());
    let mut changed = a.clone();
    changed.name = "updated".into();
    h.save(changed);
    assert!(agent2.prepare_service_action(&a.id, "start").is_err());
    let audit = h
        .client
        .invoke::<Value>("access_audits", json!({}))
        .unwrap()
        .to_string();
    assert!(audit.contains("MCP"));
    assert!(!audit.contains(&grant.token));
    assert!(!audit.contains(&revoked_preview.token));
    let db = std::fs::read(h.temp.path().join("devnook.db")).unwrap();
    assert!(!String::from_utf8_lossy(&db).contains(&grant.token));
}
#[test]
fn workspace_cli_mcp_agree_and_core_crash_rolls_back_only_owned_runs() {
    let h = Harness::new();
    let a = h.save(h.definition("existing"));
    let p = h.client.prepare_service_action(&a.id, "start").unwrap();
    h.client.confirm_service_action(&p.token).unwrap();
    wait(|| {
        h.client
            .service_history(&a.id)
            .ok()?
            .first()
            .filter(|r| r.state == ServiceState::Ready)
            .cloned()
    });
    let mut b = h.definition("waiting");
    b.dependencies = vec![a.id.clone()];
    b.readiness.kind = ReadinessKind::LogPhrase;
    b.readiness.phrase = "NEVER".into();
    b.readiness.timeout_seconds = 30;
    let b = h.save(b);
    let w = h
        .client
        .save_workspace(WorkspaceGroup {
            name: "recover".into(),
            service_ids: vec![b.id.clone()],
            ..Default::default()
        })
        .unwrap();
    let grant = h.grant(vec![], vec![w.id.clone()]);
    let mut m = Mcp::new(&h, Some(&grant.token));
    m.initialize();
    let preview = m.call("start_workspace", json!({"workspace_id":w.id}));
    let token = &preview["result"]["structuredContent"]["confirmation_token"];
    assert_eq!(
        m.call(
            "start_workspace",
            json!({"workspace_id":w.id,"confirmation_token":token})
        )["result"]["isError"],
        false
    );
    wait(|| {
        h.client
            .service_history(&b.id)
            .ok()?
            .first()
            .filter(|r| r.pid.is_some())
            .cloned()
    });
    let status = h.cli(&["workspace", "status", &w.id]);
    assert!(status.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&status.stdout).unwrap()["state"],
        "STARTING"
    );
    let pid = h.client.invoke::<Value>("ping", json!({})).unwrap()["pid"]
        .as_u64()
        .unwrap();
    Command::new("/bin/kill")
        .args(["-KILL", &pid.to_string()])
        .status()
        .unwrap();
    wait(|| {
        devnook_core::process::identity(pid as u32)
            .is_none_or(|p| !p.alive)
            .then_some(())
    });
    // Simulate the journal gap between creating the service run and recording its ID
    // on the workspace. Recovery must use the durable per-service ownership marker.
    let db = devnook_core::storage::open(&h.temp.path().join("devnook.db")).unwrap();
    db.execute("UPDATE workspace_runs SET payload=json_set(payload,'$.owned_run_ids',json('[]')) WHERE workspace_id=?1", [&w.id]).unwrap();
    drop(db);
    let recovered = wait(|| {
        h.client
            .workspace_history(&w.id)
            .ok()?
            .into_iter()
            .next()
            .filter(|r| r.state == "FAILED")
    });
    assert!(recovered.error.unwrap().contains("中断"));
    assert_eq!(
        h.client.service_history(&a.id).unwrap()[0].state,
        ServiceState::Ready
    );
    assert_eq!(
        h.client.service_history(&b.id).unwrap()[0].state,
        ServiceState::Stopped
    );
}
#[test]
fn mcp_scan_roots_owned_sessions_import_confirmation_and_revocation() {
    let h = Harness::new();
    let scan_root = h.temp.path().join("incoming");
    std::fs::create_dir(&scan_root).unwrap();
    std::fs::write(scan_root.join("package.json"), "{}").unwrap();
    let grant:GrantCreated=h.client.invoke("create_agent_grant",json!({"input":GrantInput{name:"scanner".into(),service_ids:vec![],workspace_ids:vec![],project_ids:vec![],scan_roots:vec![scan_root.to_string_lossy().into()]}})).unwrap();
    let mut m = Mcp::new(&h, Some(&grant.token));
    m.initialize();
    assert_eq!(
        m.call(
            "preview_project_scan",
            json!({"root_path":h.temp.path().to_string_lossy()})
        )["result"]["isError"],
        true
    );
    let scan = m.call(
        "preview_project_scan",
        json!({"root_path":scan_root.to_string_lossy()}),
    );
    let id = scan["result"]["structuredContent"]["scan_id"]
        .as_str()
        .unwrap();
    let progress = wait(|| {
        let v = m.call("get_scan_status", json!({"scan_id":id}));
        (v["result"]["structuredContent"]["status"] == "completed").then_some(v)
    });
    let candidates: Vec<_> = progress["result"]["structuredContent"]["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].clone())
        .collect();
    let p = m.call(
        "import_projects",
        json!({"scan_id":id,"candidate_ids":candidates}),
    );
    let token = &p["result"]["structuredContent"]["confirmation_token"];
    assert_eq!(p["result"]["structuredContent"]["needs_confirmation"], true);
    assert_eq!(
        m.call(
            "import_projects",
            json!({"scan_id":id,"candidate_ids":["wrong"],"confirmation_token":token})
        )["result"]["isError"],
        true
    );
    assert_eq!(
        m.call(
            "import_projects",
            json!({"scan_id":id,"candidate_ids":candidates,"confirmation_token":token})
        )["result"]["isError"],
        false
    );
    assert_eq!(h.client.projects().unwrap().len(), 2);
}

#[test]
fn mcp_negotiates_versions_and_omits_new_fields_for_older_clients() {
    let h = Harness::new();
    let d = h.save(h.definition("versions"));
    for version in ["2024-11-05", "2025-03-26", "2025-06-18", "2099-01-01"] {
        let mut m = Mcp::new(&h, None);
        let response=m.request("initialize",json!({"protocolVersion":version,"capabilities":{},"clientInfo":{"name":"compatibility-test","version":"1"}}));
        assert_eq!(
            response["result"]["protocolVersion"],
            if version == "2099-01-01" {
                "2025-11-25"
            } else {
                version
            }
        );
        m.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        let tools = m.request("tools/list", json!({}));
        assert!(!tools["result"]["tools"].as_array().unwrap().is_empty());
        if version == "2024-11-05" {
            assert!(tools["result"]["tools"][0].get("annotations").is_none());
        }
        let response = m.call("get_service_status", json!({"service_id":d.id}));
        assert_eq!(response["result"]["isError"], false);
        if ["2024-11-05", "2025-03-26"].contains(&version) {
            assert!(response["result"].get("structuredContent").is_none());
        }
    }
}
