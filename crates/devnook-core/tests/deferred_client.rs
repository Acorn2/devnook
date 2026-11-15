#![cfg(target_os = "macos")]

use devnook_core::ipc::CoreClient;
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    os::unix::{fs::PermissionsExt, net::UnixListener},
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};
use tempfile::TempDir;

#[test]
fn deferred_gui_client_survives_incompatible_core_and_retries_without_restarting() {
    let temp = TempDir::new().unwrap();
    let directory = temp.path().join("core");
    let missing_executable = PathBuf::from("/does-not-exist/devnook-test-launcher");
    // No server or valid launcher exists yet. GUI construction must not connect or spawn.
    let gui = CoreClient::new_deferred(&directory, missing_executable.clone()).unwrap();
    assert_eq!(
        fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert!(!directory.join("core.sock").exists());

    let socket = directory.join("core.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut methods = Vec::new();
        while methods.len() < 3 {
            let (mut stream, _) = match listener.accept() {
                Ok(connection) => connection,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "missing client request");
                    thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(e) => panic!("{e}"),
            };
            // macOS can inherit O_NONBLOCK from the listening socket.
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).unwrap();
            let request: Value = serde_json::from_slice(&bytes).unwrap();
            methods.push(request["method"].as_str().unwrap().to_owned());
            // First the CLI handshake, then a GUI read fail. The final retry sees
            // a compatible backend, without replacing the client or sending control calls.
            let response = if methods.len() < 3 {
                json!({"result":null,"error":"Core 协议版本不一致，请关闭旧版本后台 Core 后重试"})
            } else {
                json!({"result":[],"error":null})
            };
            stream
                .write_all(&serde_json::to_vec(&response).unwrap())
                .unwrap();
        }
        methods
    });
    let eager = CoreClient::new(&directory, missing_executable);
    assert!(eager.err().unwrap().contains("协议版本不一致"));
    assert!(gui.projects().unwrap_err().contains("协议版本不一致"));
    assert!(gui.projects().unwrap().is_empty());
    assert_eq!(
        server.join().unwrap(),
        ["ping", "list_projects", "list_projects"]
    );
}

#[test]
fn deferred_client_still_rejects_unsafe_data_directory_and_socket() {
    let temp = TempDir::new().unwrap();
    let linked = temp.path().join("linked");
    std::os::unix::fs::symlink(temp.path(), &linked).unwrap();
    let executable = PathBuf::from("/does-not-exist/devnook-test-launcher");
    assert!(CoreClient::new_deferred(&linked, executable.clone())
        .err()
        .unwrap()
        .contains("符号链接"));

    let directory = temp.path().join("core");
    let gui = CoreClient::new_deferred(&directory, executable).unwrap();
    let listener = UnixListener::bind(directory.join("core.sock")).unwrap();
    fs::set_permissions(
        directory.join("core.sock"),
        fs::Permissions::from_mode(0o666),
    )
    .unwrap();
    listener.set_nonblocking(true).unwrap();
    // An unsafe socket is never queried, and the nonexistent launcher cannot run.
    assert!(gui.projects().is_err());
    assert!(listener.accept().is_err());
}
