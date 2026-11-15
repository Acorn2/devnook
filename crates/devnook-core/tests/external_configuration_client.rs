#![cfg(target_os = "macos")]

use devnook_core::{access::RequestContext, ipc::CoreClient};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    os::unix::{fs::PermissionsExt, net::UnixListener},
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

fn process() -> Value {
    json!({
        "pid":42,"ppid":1,"pgid":42,"uid":501,"name":"node",
        "command":"node --inspect private-observed-command.mjs","executable":"/usr/local/bin/node",
        "cwd":"/projects/app/api","start_time":"fixture","elapsed":"1:00",
        "fingerprint":"42-start","start_identity":"start","parents":[],
        "project_id":"app","suggested_project_id":null,"confidence":"HIGH",
        "source":"OBSERVED","can_stop":true,"stop_reason":"",
        "bindings":[{
            "pid":42,"name":"node","address":"127.0.0.1","port":4318,
            "family":"IPv4","first_seen":1,"project_id":"app","command":"node",
            "cwd":"/projects/app/api","source":"OBSERVED","service_id":null,
            "process_kind":"SERVICE"
        }]
    })
}

fn projects() -> Value {
    json!([{
        "id":"app","name":"app","root_path":"/projects/app","canonical_path":"/projects/app",
        "project_type":"NODE","frameworks":[],"build_tools":[],"evidence":[],
        "is_git":true,"is_monorepo":false,"parent_path":null,"runnable":true,
        "needs_review":false,"imported":true,"default_ide":"DEFAULT",
        "is_favorite":false,"category":"","created_at":0,"updated_at":0
    }])
}

fn draft() -> Value {
    json!({
        "definition":{
            "id":"","project_id":"app","name":"api","runtime_type":"NODE",
            "executable":"pnpm","args":["dev"],"shell_mode":false,
            "working_directory":"/projects/app/api","environment":{},"secret_keys":[],
            "expected_ports":[9999],"readiness":devnook_core::service_model::Readiness::default(),
            "stop_grace_seconds":5,"log_limit_kb":1024,"revision":"","created_at":0,"updated_at":0
        },
        "command":"pnpm dev","source":"package.json scripts.dev"
    })
}

fn ok(result: Value) -> Value {
    json!({"result":result,"error":null})
}

fn unknown() -> Value {
    json!({"result":null,"error":"未知 Core 操作"})
}

// A private socket substitutes only the daemon. The real client framing, version,
// context, and fallback order run without launching or stopping any process.
fn with_server(
    steps: Vec<(&'static str, Value)>,
    context: RequestContext,
    check: impl FnOnce(CoreClient),
) {
    let temp = tempfile::tempdir().unwrap();
    let client = CoreClient::new_deferred(
        temp.path(),
        PathBuf::from("/does-not-exist/devnook-test-launcher"),
    )
    .unwrap()
    .with_context(context);
    let socket = temp.path().join("core.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let server = thread::spawn(move || {
        for (method, response) in steps {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "missing {method} request");
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(e) => panic!("{e}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = vec![];
            stream.read_to_end(&mut bytes).unwrap();
            let request: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(request["version"], 10);
            assert_eq!(request["method"], method);
            match method {
                "external_service_configuration" | "inspect_process" => {
                    assert_eq!(request["params"]["pid"], 42)
                }
                "service_drafts" => assert_eq!(request["params"]["project_id"], "app"),
                _ => {}
            }
            stream
                .write_all(&serde_json::to_vec(&response).unwrap())
                .unwrap();
        }
    });
    check(client);
    server.join().unwrap();
}

fn legacy_steps(current: Value, drafts: Value) -> Vec<(&'static str, Value)> {
    vec![
        ("external_service_configuration", unknown()),
        ("inspect_process", ok(process())),
        ("list_projects", ok(projects())),
        ("service_drafts", ok(drafts)),
        ("inspect_process", ok(current)),
    ]
}

#[test]
fn legacy_daemon_preview_uses_only_reads_and_manifest_command() {
    with_server(
        legacy_steps(process(), json!([draft()])),
        Default::default(),
        |c| {
            let preview = c.external_service_configuration(42).unwrap();
            assert!(!preview.handoff_supported);
            assert_eq!(preview.draft.command, "pnpm dev");
            assert_eq!(preview.draft.definition.executable, "pnpm");
            assert_eq!(preview.draft.definition.expected_ports, [4318]);
            assert!(preview.draft.definition.environment.is_empty());
            assert!(preview.draft.definition.secret_keys.is_empty());
        },
    );
}

#[test]
fn legacy_preview_autofills_custom_script_in_root_and_component() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::create_dir(root.join("video-library")).unwrap();
    fs::write(
        root.join("video-library/server.mjs"),
        "throw new Error('must not run')",
    )
    .unwrap();
    fs::write(
        root.join("package.json"),
        r#"{"scripts":{"start":"node other.mjs","library":"node video-library/server.mjs"}}"#,
    )
    .unwrap();
    fs::write(
        root.join("video-library/package.json"),
        r#"{"name":"product-video-library","scripts":{"start":"node server.mjs"}}"#,
    )
    .unwrap();
    for (directory, command, name, entry) in [
        (
            root.clone(),
            "/usr/local/bin/node video-library/server.mjs",
            "library",
            "video-library/server.mjs",
        ),
        (
            root.join("video-library"),
            "node server.mjs",
            "product-video-library",
            "server.mjs",
        ),
    ] {
        let mut detail = process();
        detail["cwd"] = json!(directory);
        detail["command"] = json!(command);
        let mut project = projects();
        project[0]["canonical_path"] = json!(root);
        let steps = vec![
            ("external_service_configuration", unknown()),
            ("inspect_process", ok(detail.clone())),
            ("list_projects", ok(project)),
            ("service_drafts", ok(json!([]))),
            ("inspect_process", ok(detail)),
        ];
        with_server(steps, Default::default(), |c| {
            let preview = c.external_service_configuration(42).unwrap();
            assert_eq!(preview.draft.definition.name, name);
            assert_eq!(preview.draft.definition.runtime_type, "NODE");
            assert_eq!(preview.draft.command, format!("node {entry}"));
            assert_eq!(preview.draft.definition.args, [entry]);
            assert_eq!(
                preview.draft.definition.working_directory,
                directory.to_str().unwrap()
            );
            assert_eq!(preview.draft.definition.expected_ports, [4318]);
            assert!(preview.draft.definition.environment.is_empty());
            assert!(!preview.draft.definition.shell_mode);
            assert!(!preview.handoff_supported);
        });
    }
}

#[test]
fn autofill_rejects_unrelated_scripts_shell_debug_secrets_and_unsafe_entries() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::write(root.join("server.mjs"), "throw new Error('must not run')").unwrap();
    fs::create_dir(root.join("node_modules")).unwrap();
    fs::write(root.join("node_modules/server.mjs"), "").unwrap();
    std::os::unix::fs::symlink(root.join("server.mjs"), root.join("alias.mjs")).unwrap();
    for (observed, manifest) in [
        ("node server.mjs", "node different.mjs"),
        ("node --inspect server.mjs", "node --inspect server.mjs"),
        ("node -e hello", "node -e hello"),
        (
            "node server.mjs --token=fake-test-value",
            "node server.mjs --token=fake-test-value",
        ),
        (
            "node server.mjs --api-key=fake-test-value",
            "node server.mjs --api-key=fake-test-value",
        ),
        ("node server.mjs", "node server.mjs && touch SHOULD_NOT_RUN"),
        ("node alias.mjs", "node alias.mjs"),
        ("node ../outside.mjs", "node ../outside.mjs"),
        (
            "node node_modules/server.mjs",
            "node node_modules/server.mjs",
        ),
        ("node missing.mjs", "node missing.mjs"),
    ] {
        fs::write(
            root.join("package.json"),
            json!({"scripts":{"library":manifest}}).to_string(),
        )
        .unwrap();
        let mut detail = process();
        detail["cwd"] = json!(root);
        detail["command"] = json!(observed);
        let mut project = projects();
        project[0]["canonical_path"] = json!(root);
        with_server(
            vec![
                ("external_service_configuration", unknown()),
                ("inspect_process", ok(detail.clone())),
                ("list_projects", ok(project)),
                ("service_drafts", ok(json!([]))),
                ("inspect_process", ok(detail)),
            ],
            Default::default(),
            |c| {
                let preview = c.external_service_configuration(42).unwrap();
                assert!(preview.draft.command.is_empty(), "{manifest}");
                assert!(preview.draft.definition.executable.is_empty());
            },
        );
        assert!(!root.join("SHOULD_NOT_RUN").exists());
    }
}

#[test]
fn missing_or_ambiguous_manifest_never_replays_observed_command() {
    for drafts in [json!([]), json!([draft(), draft()])] {
        with_server(legacy_steps(process(), drafts), Default::default(), |c| {
            let preview = c.external_service_configuration(42).unwrap();
            assert!(preview.draft.command.is_empty());
            assert!(preview.draft.definition.executable.is_empty());
            assert_eq!(
                preview.draft.definition.working_directory,
                "/projects/app/api"
            );
            assert_eq!(preview.draft.definition.expected_ports, [4318]);
        });
    }
}

#[test]
fn changed_identity_project_cwd_or_listener_rejects_legacy_preview() {
    for (field, changed) in [
        ("fingerprint", json!("42-reused")),
        ("pid", json!(43)),
        ("cwd", json!("/projects/app/other")),
        ("project_id", json!("other")),
        ("bindings", json!([])),
    ] {
        let mut current = process();
        current[field] = changed;
        with_server(
            legacy_steps(current, json!([draft()])),
            Default::default(),
            |c| {
                assert!(c.external_service_configuration(42).is_err(), "{field}");
            },
        );
    }
    let mut current = process();
    current["bindings"][0]["port"] = json!(4319);
    with_server(
        legacy_steps(current, json!([draft()])),
        Default::default(),
        |c| {
            assert!(c
                .external_service_configuration(42)
                .unwrap_err()
                .contains("身份已变化"));
        },
    );
}

#[test]
fn legacy_preview_keeps_service_and_project_eligibility_checks() {
    for (field, changed) in [
        ("source", json!("DEVNOOK")),
        ("confidence", json!("UNKNOWN")),
        ("cwd", json!("/projects/app-other")),
        ("cwd", Value::Null),
        ("project_id", Value::Null),
        ("bindings", json!([])),
    ] {
        let mut detail = process();
        detail[field] = changed;
        with_server(
            vec![
                ("external_service_configuration", unknown()),
                ("inspect_process", ok(detail)),
                ("list_projects", ok(projects())),
            ],
            Default::default(),
            |c| {
                assert!(c.external_service_configuration(42).is_err(), "{field}");
            },
        );
    }
}

#[test]
fn native_daemon_and_real_errors_do_not_trigger_fallback() {
    with_server(
        vec![(
            "external_service_configuration",
            ok(json!({"process":process(),"draft":draft()})),
        )],
        Default::default(),
        |c| {
            assert!(
                c.external_service_configuration(42)
                    .unwrap()
                    .handoff_supported
            );
        },
    );
    for error in [
        "Core 协议版本不一致",
        "MCP 无权调用",
        "无法核验外部服务工作目录",
    ] {
        with_server(
            vec![(
                "external_service_configuration",
                json!({"result":null,"error":error}),
            )],
            Default::default(),
            |c| {
                assert_eq!(c.external_service_configuration(42).unwrap_err(), error);
            },
        );
    }
}

#[test]
fn mcp_cannot_use_the_local_compatibility_path() {
    with_server(
        vec![("external_service_configuration", unknown())],
        RequestContext {
            origin: "MCP".into(),
            credential: String::new(),
            read_only: true,
        },
        |c| {
            assert_eq!(
                c.external_service_configuration(42).unwrap_err(),
                "未知 Core 操作"
            );
        },
    );
}
