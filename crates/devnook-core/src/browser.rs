//! GUI-only browser actions compose existing v10 read APIs. No new daemon RPC,
//! service control, user-supplied URL, or MCP capability is introduced.
use crate::{ipc::CoreClient, model::*, service_model::ServiceState};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, net::IpAddr, process::Command};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BrowserTarget {
    Managed { service_id: String },
    External { pid: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserAddress {
    pub url: String,
    pub pid: u32,
    pub fingerprint: String,
}

fn urls(binding: &PortBinding) -> Vec<String> {
    if binding.port == 0 {
        return vec![];
    }
    let address = binding.address.trim_matches(['[', ']']);
    let ip = if address == "*" {
        match binding.family.as_str() {
            "IPv4" => "0.0.0.0".parse::<IpAddr>().ok(),
            "IPv6" => "::".parse::<IpAddr>().ok(),
            _ => None,
        }
    } else {
        address.parse::<IpAddr>().ok()
    };
    let Some(mut ip) = ip else {
        return vec![];
    };
    if ip.is_unspecified() {
        ip = if ip.is_ipv4() {
            "127.0.0.1".parse().unwrap()
        } else {
            "::1".parse().unwrap()
        };
    }
    if ip.is_multicast() || ip == IpAddr::from([255, 255, 255, 255]) {
        return vec![];
    }
    let host = match ip {
        IpAddr::V4(ip) => ip.to_string(),
        IpAddr::V6(ip) => format!("[{ip}]"),
    };
    ["http", "https"]
        .into_iter()
        .filter_map(|scheme| {
            url::Url::parse(&format!("{scheme}://{host}:{}/", binding.port))
                .ok()
                .map(|url| url.to_string())
        })
        .collect()
}

fn matches(target: &BrowserTarget, binding: &PortBinding) -> bool {
    if binding.process_kind != ProcessKind::Service {
        return false;
    }
    match target {
        BrowserTarget::Managed { service_id } => {
            binding.source == "DEVNOOK" && binding.service_id.as_ref() == Some(service_id)
        }
        BrowserTarget::External { pid } => {
            binding.pid == *pid && binding.source != "DEVNOOK" && binding.project_id.is_some()
        }
    }
}

fn addresses(target: &BrowserTarget, details: &[ProcessDetail]) -> Vec<BrowserAddress> {
    let mut seen = HashSet::new();
    let mut result = vec![];
    for detail in details {
        for binding in &detail.bindings {
            if binding.pid != detail.pid || !matches(target, binding) {
                continue;
            }
            for url in urls(binding) {
                if seen.insert(url.clone()) {
                    result.push(BrowserAddress {
                        url,
                        pid: detail.pid,
                        fingerprint: detail.fingerprint.clone(),
                    });
                }
            }
        }
    }
    // Prefer HTTP and IPv4 when available, while keeping every actual listener
    // selectable. HTTPS is an explicit choice; TCP alone does not prove HTTP.
    result.sort_by_key(|a| {
        (
            a.url.starts_with("https:"),
            a.url.contains('['),
            a.url.clone(),
        )
    });
    result
}

fn checked_address<'a>(
    current: &'a [BrowserAddress],
    requested: &BrowserAddress,
) -> Result<&'a str, String> {
    current
        .iter()
        .find(|address| *address == requested)
        .map(|address| address.url.as_str())
        .ok_or_else(|| "服务监听或进程身份已变化，请刷新地址后重试".into())
}

impl CoreClient {
    pub fn service_browser_addresses(
        &self,
        target: &BrowserTarget,
    ) -> Result<Vec<BrowserAddress>, String> {
        if let BrowserTarget::Managed { service_id } = target {
            let view = self
                .services()?
                .into_iter()
                .find(|s| s.definition.id == *service_id)
                .ok_or("服务配置不存在")?;
            if !view.current_run.is_some_and(|run| {
                matches!(
                    run.state,
                    ServiceState::Starting | ServiceState::Running | ServiceState::Ready
                )
            }) {
                return Ok(vec![]);
            }
        }
        let ports = self.ports()?;
        let mut pids = ports
            .iter()
            .filter(|p| matches(target, p))
            .map(|p| p.pid)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        pids.sort_unstable();
        let mut details = vec![];
        for pid in pids {
            // A listener can exit between the snapshot and inspection. Do not
            // reuse stale addresses; a later refresh can rediscover it.
            details.push(self.inspect(pid)?);
        }
        Ok(addresses(target, &details))
    }

    pub fn open_service_browser(
        &self,
        target: &BrowserTarget,
        address: &BrowserAddress,
    ) -> Result<(), String> {
        if !cfg!(target_os = "macos") {
            return Err("浏览器快捷打开目前仅支持 macOS".into());
        }
        let current = self.service_browser_addresses(target)?;
        let url = checked_address(&current, address)?;
        let status = Command::new("/usr/bin/open")
            .arg(url)
            .status()
            .map_err(|_| "无法打开默认浏览器，请检查系统默认浏览器设置")?;
        if !status.success() {
            return Err("默认浏览器未接受打开请求".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(address: &str, family: &str) -> PortBinding {
        PortBinding {
            pid: 42,
            name: "node".into(),
            address: address.into(),
            family: family.into(),
            port: 5173,
            first_seen: 0,
            project_id: Some("project".into()),
            command: String::new(),
            cwd: None,
            source: "OBSERVED".into(),
            service_id: None,
            process_kind: ProcessKind::Service,
        }
    }

    fn detail(bindings: Vec<PortBinding>) -> ProcessDetail {
        ProcessDetail {
            pid: 42,
            ppid: 1,
            pgid: 42,
            uid: 501,
            name: "node".into(),
            command: "node server.js".into(),
            executable: None,
            cwd: None,
            start_time: String::new(),
            elapsed: String::new(),
            fingerprint: "start-a".into(),
            start_identity: Some("start-a".into()),
            bindings,
            parents: vec![],
            project_id: Some("project".into()),
            suggested_project_id: None,
            confidence: "HIGH".into(),
            source: "OBSERVED".into(),
            can_stop: false,
            stop_reason: String::new(),
            service_id: None,
        }
    }

    #[test]
    fn address_choices_merge_duplicate_hosts_keep_distinct_ports_and_carry_identity() {
        let mut second = binding("127.0.0.1", "IPv4");
        second.port = 3000;
        let choices = addresses(
            &BrowserTarget::External { pid: 42 },
            &[detail(vec![
                binding("*", "IPv4"),
                binding("127.0.0.1", "IPv4"),
                binding("[::1]", "IPv6"),
                second,
            ])],
        );
        assert_eq!(choices.len(), 6);
        assert_eq!(choices[0].url, "http://127.0.0.1:3000/");
        assert!(choices
            .iter()
            .all(|a| a.pid == 42 && a.fingerprint == "start-a"));
        assert!(choices
            .iter()
            .all(|a| url::Url::parse(&a.url).unwrap().path() == "/"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn gui_browser_discovery_composes_only_existing_v10_read_calls() {
        use std::{
            fs,
            io::{Read, Write},
            os::unix::{fs::PermissionsExt, net::UnixListener},
            path::PathBuf,
            thread,
            time::{Duration, Instant},
        };
        let temp = tempfile::TempDir::new().unwrap();
        let directory = temp.path().join("core");
        let client =
            CoreClient::new_deferred(&directory, PathBuf::from("/does-not-exist/test-launcher"))
                .unwrap();
        let socket = directory.join("core.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut methods = vec![];
            while methods.len() < 2 {
                let (mut stream, _) = match listener.accept() {
                    Ok(value) => value,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline);
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(e) => panic!("{e}"),
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut data = vec![];
                stream.read_to_end(&mut data).unwrap();
                let request: serde_json::Value = serde_json::from_slice(&data).unwrap();
                assert_eq!(request["version"], 10);
                let method = request["method"].as_str().unwrap().to_owned();
                let result = match method.as_str() {
                    "list_ports" => serde_json::to_value(vec![binding("*", "IPv4")]).unwrap(),
                    "inspect_process" => {
                        assert_eq!(request["params"]["pid"], 42);
                        serde_json::to_value(detail(vec![binding("*", "IPv4")])).unwrap()
                    }
                    _ => panic!("unexpected non-read method: {method}"),
                };
                methods.push(method);
                stream
                    .write_all(
                        &serde_json::to_vec(&serde_json::json!({"result":result,"error":null}))
                            .unwrap(),
                    )
                    .unwrap();
            }
            methods
        });
        let choices = client
            .service_browser_addresses(&BrowserTarget::External { pid: 42 })
            .unwrap();
        assert_eq!(choices.len(), 2);
        assert_eq!(choices[0].url, "http://127.0.0.1:5173/");
        assert_eq!(server.join().unwrap(), ["list_ports", "inspect_process"]);
    }

    #[test]
    fn actual_ip_bindings_produce_only_http_origins_and_wildcards_use_loopback() {
        for (input, family, host) in [
            ("*", "IPv4", "127.0.0.1"),
            ("0.0.0.0", "IPv4", "127.0.0.1"),
            ("*", "IPv6", "[::1]"),
            ("[::]", "IPv6", "[::1]"),
            ("127.0.0.2", "IPv4", "127.0.0.2"),
            ("[::1]", "IPv6", "[::1]"),
            ("192.168.1.10", "IPv4", "192.168.1.10"),
        ] {
            assert_eq!(
                urls(&binding(input, family)),
                vec![
                    format!("http://{host}:5173/"),
                    format!("https://{host}:5173/")
                ]
            );
        }
    }

    #[test]
    fn invalid_addresses_cannot_become_arbitrary_urls_or_open_arguments() {
        for address in [
            "example.com",
            "127.0.0.1@evil.test",
            "-a",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "127.0.0.1/path",
            "255.255.255.255",
            "224.0.0.1",
            "[ff02::1]",
        ] {
            assert!(urls(&binding(address, "IPv4")).is_empty());
        }
        let mut zero = binding("127.0.0.1", "IPv4");
        zero.port = 0;
        assert!(urls(&zero).is_empty());
    }

    #[test]
    fn browsing_never_infers_managed_ownership_from_project_or_service_id_alone() {
        let managed = BrowserTarget::Managed {
            service_id: "web".into(),
        };
        let external = BrowserTarget::External { pid: 42 };
        let mut port = binding("127.0.0.1", "IPv4");
        port.service_id = Some("web".into());
        assert!(!matches(&managed, &port));
        assert!(matches(&external, &port));
        port.source = "DEVNOOK".into();
        assert!(matches(&managed, &port));
        assert!(!matches(&external, &port));
        port.source = "OBSERVED".into();
        port.project_id = None;
        assert!(!matches(&external, &port));
        port.project_id = Some("project".into());
        port.process_kind = ProcessKind::DevelopmentTool;
        assert!(!matches(&external, &port));
    }

    #[test]
    fn opening_requires_an_exact_current_address_and_process_identity() {
        let address = BrowserAddress {
            url: "http://127.0.0.1:5173/".into(),
            pid: 42,
            fingerprint: "start-a".into(),
        };
        assert!(checked_address(std::slice::from_ref(&address), &address).is_ok());
        for changed in [
            BrowserAddress {
                pid: 43,
                ..address.clone()
            },
            BrowserAddress {
                fingerprint: "start-b".into(),
                ..address.clone()
            },
            BrowserAddress {
                url: "https://evil.test/".into(),
                ..address.clone()
            },
            BrowserAddress {
                url: "http://127.0.0.1:5173/?token=secret".into(),
                ..address.clone()
            },
        ] {
            assert!(checked_address(std::slice::from_ref(&address), &changed).is_err());
        }
        assert!(checked_address(&[], &address).is_err());
    }
}
