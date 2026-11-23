//! GUI-only browser actions compose existing v10 read APIs. No new daemon RPC,
//! service control, user-supplied URL, or MCP capability is introduced.
use crate::{ipc::CoreClient, model::*, service_model::ServiceState};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    io::{Read, Write},
    net::{IpAddr, SocketAddr, TcpStream},
    process::Command,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    /// Optional localhost alias for a verified IPv6 loopback endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_url: Option<String>,
    /// GUI hint: HTML root response / known web entry, non-page response, or
    /// unknown. It never grants control capability or changes process purpose.
    #[serde(default)]
    pub web_ui: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BrowserShortcut {
    pub target: BrowserTarget,
    pub address: BrowserAddress,
}

/// GUI-local evidence from explicitly selected service inspection. Never persisted
/// or populated by a tray refresh. A changed identity cannot inherit a web hint.
#[derive(Default)]
pub(crate) struct BrowserEvidenceCache(HashMap<BrowserTarget, (Instant, Vec<BrowserAddress>)>);
impl BrowserEvidenceCache {
    fn remember(&mut self, target: BrowserTarget, addresses: Vec<BrowserAddress>, now: Instant) {
        self.0
            .retain(|_, (time, _)| now.duration_since(*time) < Duration::from_secs(120));
        if self.0.len() >= 64 && !self.0.contains_key(&target) {
            if let Some(oldest) = self
                .0
                .iter()
                .min_by_key(|(_, (time, _))| *time)
                .map(|(key, _)| key.clone())
            {
                self.0.remove(&oldest);
            }
        }
        self.0.insert(target, (now, addresses));
    }

    fn pages(&mut self, now: Instant) -> Vec<(BrowserTarget, Vec<BrowserAddress>)> {
        self.0
            .retain(|_, (time, _)| now.duration_since(*time) < Duration::from_secs(120));
        self.0
            .iter()
            .filter_map(|(target, (_, addresses))| {
                let pages: Vec<_> = addresses
                    .iter()
                    .filter(|a| a.web_ui == Some(true) && a.url.starts_with("http://"))
                    .cloned()
                    .collect();
                (!pages.is_empty()).then(|| (target.clone(), pages))
            })
            .collect()
    }
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
                        access_url: None,
                        web_ui: crate::process_kind::browser_entry(
                            detail.executable.as_deref(),
                            &detail.command,
                        )
                        .then_some(true),
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

fn html_response(headers: &[u8]) -> Option<bool> {
    let text = std::str::from_utf8(headers).ok()?;
    let mut lines = text.split("\r\n");
    let mut status = lines.next()?.split_whitespace();
    if !matches!(status.next()?, "HTTP/1.0" | "HTTP/1.1") {
        return None;
    }
    let status: u16 = status.next()?.parse().ok()?;
    // Redirects and temporary/auth errors do not establish the page's absence.
    // Never follow a Location header, even when it points to the same service.
    if !(200..300).contains(&status) && !matches!(status, 404 | 405) {
        return None;
    }
    let mut content_type = None;
    for line in lines.take_while(|line| !line.is_empty()) {
        let (name, value) = line.split_once(':')?;
        if name.eq_ignore_ascii_case("content-type") {
            if content_type.is_some() {
                return None;
            }
            content_type = Some(value.split(';').next()?.trim().to_ascii_lowercase());
        }
    }
    match content_type.as_deref()? {
        "text/html" | "application/xhtml+xml" if (200..300).contains(&status) => Some(true),
        "application/json"
        | "text/json"
        | "text/plain"
        | "application/octet-stream"
        | "text/event-stream" => Some(false),
        value if value.starts_with("application/") && value.ends_with("+json") => Some(false),
        _ => None,
    }
}

fn probe_http_page(address: &str, deadline: Instant) -> Option<bool> {
    let url = url::Url::parse(address).ok()?;
    if url.scheme() != "http"
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    let ip: IpAddr = url.host_str()?.trim_matches(['[', ']']).parse().ok()?;
    if !ip.is_loopback() {
        return None;
    }
    let socket = SocketAddr::new(ip, url.port_or_known_default()?);
    let remaining = || deadline.checked_duration_since(Instant::now());
    let mut stream = TcpStream::connect_timeout(&socket, remaining()?).ok()?;
    stream.set_write_timeout(Some(remaining()?)).ok()?;
    // No credentials, cookies, proxy, redirects, path guesses or response body
    // processing. This one request is limited to a verified local HTTP root.
    write!(stream, "GET / HTTP/1.1\r\nHost: {socket}\r\nAccept: text/html, application/xhtml+xml\r\nConnection: close\r\n\r\n").ok()?;
    let mut headers = vec![];
    let mut chunk = [0; 512];
    while headers.len() < 16 * 1024 {
        stream.set_read_timeout(Some(remaining()?)).ok()?;
        let count = stream.read(&mut chunk).ok()?;
        if count == 0 {
            return None;
        }
        headers.extend_from_slice(&chunk[..count]);
        if let Some(end) = headers.windows(4).position(|window| window == b"\r\n\r\n") {
            return html_response(&headers[..end + 4]);
        }
    }
    None
}

fn annotate_pages(addresses: &mut [BrowserAddress], mut probe: impl FnMut(&str) -> Option<bool>) {
    let hints: HashMap<_, _> = addresses
        .iter()
        .filter(|address| address.url.starts_with("http://"))
        .map(|address| (address.url.clone(), probe(&address.url)))
        .collect();
    for address in addresses {
        // Both schemes describe the same verified TCP endpoint. HTTPS itself is
        // not probed and remains an explicit manual action.
        let http = url::Url::parse(&address.url).ok().and_then(|mut url| {
            let port = url.port_or_known_default()?;
            url.set_scheme("http").ok()?;
            url.set_port(Some(port)).ok()?;
            Some(url.to_string())
        });
        if let Some(Some(hint)) = http.as_ref().and_then(|http| hints.get(http)) {
            address.web_ui = Some(*hint);
        }
    }
}

fn checked_address<'a>(
    current: &'a [BrowserAddress],
    requested: &BrowserAddress,
) -> Result<&'a BrowserAddress, String> {
    current
        .iter()
        .find(|address| {
            address.url == requested.url
                && address.pid == requested.pid
                && address.fingerprint == requested.fingerprint
        })
        .ok_or_else(|| "服务监听或进程身份已变化，请刷新地址后重试".into())
}

fn localhost_alias(address: &BrowserAddress, ports: &[PortBinding]) -> Option<String> {
    let mut url = url::Url::parse(&address.url).ok()?;
    let port = url.port_or_known_default()?;
    if url.host_str()? != "[::1]" {
        return None;
    }
    // A browser can choose either loopback family for localhost. Preserve the
    // exact host if a different process can receive the same port there.
    if ports.iter().any(|binding| {
        if binding.port != port || binding.pid == address.pid {
            return false;
        }
        let host = binding.address.trim_matches(['[', ']']);
        host == "*"
            || host.parse::<IpAddr>().is_ok_and(|ip| {
                let ip = ip.to_canonical();
                ip.is_loopback() || ip.is_unspecified()
            })
    }) {
        return None;
    }
    url.set_host(Some("localhost")).ok()?;
    Some(url.to_string())
}

impl CoreClient {
    fn browser_candidates(&self, target: &BrowserTarget) -> Result<Vec<BrowserAddress>, String> {
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
        let mut choices = addresses(target, &details);
        for address in &mut choices {
            address.access_url = localhost_alias(address, &ports);
        }
        Ok(choices)
    }

    pub fn service_browser_addresses(
        &self,
        target: &BrowserTarget,
    ) -> Result<Vec<BrowserAddress>, String> {
        let mut choices = self.browser_candidates(target)?;
        let deadline = Instant::now() + Duration::from_millis(900);
        let mut count = 0;
        annotate_pages(&mut choices, |address| {
            count += 1;
            if count > 8 || Instant::now() >= deadline {
                return None;
            }
            probe_http_page(
                address,
                deadline.min(Instant::now() + Duration::from_millis(300)),
            )
        });
        // An endpoint can change hands during I/O; retain only current facts.
        let mut current = vec![];
        for pid in choices.iter().map(|a| a.pid).collect::<HashSet<_>>() {
            current.extend(addresses(target, &[self.inspect(pid)?]));
        }
        choices.retain(|address| checked_address(&current, address).is_ok());
        self.browser_evidence
            .lock()
            .map_err(|_| "网页入口证据暂不可用")?
            .remember(target.clone(), choices.clone(), Instant::now());
        Ok(choices)
    }

    /// Read only, with no HTTP requests. Revalidate recent selected-service
    /// evidence through existing v10 facts before offering a tray shortcut.
    pub fn tray_browser_shortcuts(&self) -> Result<Vec<BrowserShortcut>, String> {
        let evidence = self
            .browser_evidence
            .lock()
            .map_err(|_| "网页入口证据暂不可用")?
            .pages(Instant::now());
        if evidence.is_empty() {
            return Ok(vec![]);
        }
        let services = if evidence
            .iter()
            .any(|(target, _)| matches!(target, BrowserTarget::Managed { .. }))
        {
            self.services()?
        } else {
            vec![]
        };
        let ports = self.ports()?;
        let mut details = HashMap::new();
        let mut result = vec![];
        for (target, pages) in evidence {
            if let BrowserTarget::Managed { service_id } = &target {
                if !services.iter().any(|s| {
                    s.definition.id == *service_id
                        && s.current_run.as_ref().is_some_and(|r| {
                            matches!(
                                r.state,
                                ServiceState::Starting
                                    | ServiceState::Running
                                    | ServiceState::Ready
                            )
                        })
                }) {
                    continue;
                }
            }
            let mut current = vec![];
            for pid in ports
                .iter()
                .filter(|p| matches(&target, p))
                .map(|p| p.pid)
                .collect::<HashSet<_>>()
            {
                let detail = details.entry(pid).or_insert_with(|| self.inspect(pid));
                if let Ok(detail) = detail {
                    current.extend(addresses(&target, std::slice::from_ref(detail)));
                }
            }
            result.extend(
                pages
                    .into_iter()
                    .filter(|page| checked_address(&current, page).is_ok())
                    .map(|address| BrowserShortcut {
                        target: target.clone(),
                        address,
                    }),
            );
        }
        Ok(result)
    }

    pub fn open_service_browser(
        &self,
        target: &BrowserTarget,
        address: &BrowserAddress,
    ) -> Result<(), String> {
        if !cfg!(target_os = "macos") {
            return Err("浏览器快捷打开目前仅支持 macOS".into());
        }
        let current = self.browser_candidates(target)?;
        let current = checked_address(&current, address)?;
        let status = Command::new("/usr/bin/open")
            .arg(current.access_url.as_deref().unwrap_or(&current.url))
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
            memory_rss_bytes: None,
            cpu_time_ms: None,
        }
    }

    #[test]
    fn tray_evidence_is_bounded_expiring_and_never_promotes_unknown_or_non_page() {
        let mut cache = BrowserEvidenceCache::default();
        let now = Instant::now();
        let target = BrowserTarget::External { pid: 42 };
        let mut address =
            addresses(&target, &[detail(vec![binding("127.0.0.1", "IPv4")])])[0].clone();
        for hint in [None, Some(false)] {
            address.web_ui = hint;
            cache.remember(target.clone(), vec![address.clone()], now);
            assert!(cache.pages(now).is_empty());
        }
        address.web_ui = Some(true);
        cache.remember(target.clone(), vec![address.clone()], now);
        assert_eq!(cache.pages(now).len(), 1);
        let mut changed = address.clone();
        changed.fingerprint = "different-start".into();
        assert!(checked_address(&[changed], &address).is_err());
        assert!(cache.pages(now + Duration::from_secs(120)).is_empty());
        for pid in 1..=65 {
            cache.remember(
                BrowserTarget::External { pid },
                vec![address.clone()],
                now + Duration::from_millis(pid as u64),
            );
        }
        assert_eq!(cache.0.len(), 64);
        assert!(!cache.0.contains_key(&BrowserTarget::External { pid: 1 }));
        cache.remember(
            BrowserTarget::External { pid: 65 },
            vec![],
            now + Duration::from_secs(1),
        );
        assert_eq!(cache.pages(now + Duration::from_secs(1)).len(), 63);
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
        assert!(
            client.tray_browser_shortcuts().unwrap().is_empty(),
            "cold tray reads must not contact Core or HTTP"
        );
        let socket = directory.join("core.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let http_listener = std::net::TcpListener::bind("[::1]:0").unwrap();
        let mut http_binding = binding("[::1]", "IPv6");
        http_binding.port = http_listener.local_addr().unwrap().port();
        let port = http_binding.port;
        let http_server = thread::spawn(move || {
            let (mut connection, _) = http_listener.accept().unwrap();
            connection
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = [0; 2048];
            let length = connection.read(&mut request).unwrap();
            assert!(std::str::from_utf8(&request[..length])
                .unwrap()
                .starts_with("GET / HTTP/1.1\r\n"));
            connection.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: 0\r\n\r\n").unwrap();
            http_listener
        });
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut methods = vec![];
            while methods.len() < 7 {
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
                    "list_ports" => serde_json::to_value(vec![http_binding.clone()]).unwrap(),
                    "inspect_process" => {
                        assert_eq!(request["params"]["pid"], 42);
                        let mut current = detail(vec![http_binding.clone()]);
                        if methods.len() >= 5 {
                            current.fingerprint = "new-owner".into();
                        }
                        serde_json::to_value(current).unwrap()
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
        assert_eq!(choices[0].url, format!("http://[::1]:{port}/"));
        assert_eq!(
            choices[0].access_url,
            Some(format!("http://localhost:{port}/"))
        );
        assert!(choices.iter().all(|choice| choice.web_ui == Some(true)));
        let shortcuts = client.tray_browser_shortcuts().unwrap();
        assert_eq!(shortcuts.len(), 1);
        assert_eq!(shortcuts[0].address, choices[0]);
        assert!(
            client.tray_browser_shortcuts().unwrap().is_empty(),
            "changed identity cannot inherit the page receipt"
        );
        assert_eq!(
            server.join().unwrap(),
            [
                "list_ports",
                "inspect_process",
                "inspect_process",
                "list_ports",
                "inspect_process",
                "list_ports",
                "inspect_process"
            ]
        );
        let http_listener = http_server.join().unwrap();
        http_listener.set_nonblocking(true).unwrap();
        assert_eq!(
            http_listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock,
            "tray refresh must not probe HTTP"
        );
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
    fn ipv6_loopback_uses_localhost_only_without_a_conflicting_listener() {
        let target = BrowserTarget::External { pid: 42 };
        for host in ["[::1]", "*", "[::]"] {
            let ipv6 = binding(host, "IPv6");
            let choices = addresses(&target, &[detail(vec![ipv6.clone()])]);
            for (choice, scheme) in choices.iter().zip(["http", "https"]) {
                assert_eq!(
                    localhost_alias(choice, std::slice::from_ref(&ipv6)),
                    Some(format!("{scheme}://localhost:5173/"))
                );
                for host in ["127.0.0.1", "*", "0.0.0.0", "[::ffff:127.0.0.1]"] {
                    let mut other = binding(host, "IPv4");
                    other.pid = 43;
                    other.project_id = None;
                    assert_eq!(localhost_alias(choice, &[ipv6.clone(), other]), None);
                }
                let mut other = binding("127.0.0.1", "IPv4");
                other.pid = 43;
                other.port = 3000;
                assert!(localhost_alias(choice, &[ipv6.clone(), other]).is_some());
            }
        }
        for (host, family) in [("127.0.0.1", "IPv4"), ("[fd00::1]", "IPv6")] {
            let listener = binding(host, family);
            let choices = addresses(&target, &[detail(vec![listener.clone()])]);
            assert!(choices.iter().all(|choice| localhost_alias(
                choice,
                std::slice::from_ref(&listener)
            )
            .is_none()));
        }
    }

    #[test]
    fn opening_requires_an_exact_current_address_and_process_identity() {
        let address = BrowserAddress {
            url: "http://127.0.0.1:5173/".into(),
            pid: 42,
            fingerprint: "start-a".into(),
            access_url: None,
            web_ui: Some(false),
        };
        assert!(checked_address(std::slice::from_ref(&address), &address).is_ok());
        let hint_only = BrowserAddress {
            access_url: Some("https://evil.test/".into()),
            web_ui: Some(true),
            ..address.clone()
        };
        assert_eq!(
            checked_address(std::slice::from_ref(&address), &hint_only).unwrap(),
            &address
        );
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

    #[test]
    fn web_entry_hints_do_not_turn_generic_node_api_listeners_into_web_pages() {
        let target = BrowserTarget::External { pid: 42 };
        let mut process = detail(vec![binding("127.0.0.1", "IPv4")]);
        process.executable = Some("/bin/node".into());
        process.command = "node /work/app/local-server/server.mjs".into();
        let api = addresses(&target, &[process.clone()]);
        assert_eq!(api.len(), 2);
        assert!(api.iter().all(|address| address.web_ui.is_none()));
        process.command = "node /work/app/node_modules/vite/bin/vite.js".into();
        let page = addresses(&target, &[process]);
        assert!(page.iter().all(|address| address.web_ui == Some(true)));
        assert_eq!(api[0].url, page[0].url);
        assert_eq!(api[0].fingerprint, page[0].fingerprint);
        let legacy: BrowserAddress = serde_json::from_value(serde_json::json!({
            "url": "http://127.0.0.1:5173/", "pid": 42, "fingerprint": "start-a"
        }))
        .unwrap();
        assert!(legacy.access_url.is_none());
        assert!(legacy.web_ui.is_none());
    }

    #[test]
    fn browser_headers_distinguish_html_from_api_without_using_status_alone() {
        for headers in [
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\r\n",
            "HTTP/1.0 200 OK\r\ncontent-type: APPLICATION/XHTML+XML\r\n\r\n",
        ] {
            assert_eq!(html_response(headers.as_bytes()), Some(true));
        }
        for headers in [
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n",
            "HTTP/1.1 404 Not Found\r\nContent-Type: application/problem+json\r\n\r\n",
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\n",
        ] {
            assert_eq!(html_response(headers.as_bytes()), Some(false));
        }
        for headers in [
            "HTTP/1.1 302 Found\r\nContent-Type: text/html\r\nLocation: /app\r\n\r\n",
            "HTTP/1.1 503 Busy\r\nContent-Type: text/html\r\n\r\n",
            "HTTP/1.1 200 OK\r\n\r\n",
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Type: application/json\r\n\r\n",
            "not HTTP\r\nContent-Type: text/html\r\n\r\n",
        ] {
            assert_eq!(html_response(headers.as_bytes()), None);
        }
    }

    #[test]
    fn browser_custom_node_pages_and_api_get_different_hints_and_unknown_stays_unknown() {
        let mut listener = binding("127.0.0.1", "IPv4");
        listener.port = 80;
        let mut choices = addresses(
            &BrowserTarget::External { pid: 42 },
            &[detail(vec![listener])],
        );
        // The same generic node server can serve a page or an API; neither the
        // runtime nor its project's framework list supplies that distinction.
        annotate_pages(&mut choices, |_| Some(true));
        assert!(choices.iter().all(|a| a.web_ui == Some(true)));
        annotate_pages(&mut choices, |_| Some(false));
        assert!(choices.iter().all(|a| a.web_ui == Some(false)));
        for choice in &mut choices {
            choice.web_ui = None;
        }
        annotate_pages(&mut choices, |_| None);
        assert!(choices.iter().all(|a| a.web_ui.is_none()));
    }

    #[test]
    fn browser_probe_is_loopback_http_root_only_and_bounded_when_peer_does_not_respond() {
        for address in [
            "https://127.0.0.1:1234/",
            "http://192.0.2.1/",
            "http://example.com/",
            "http://user:password@127.0.0.1/",
            "http://127.0.0.1/api",
            "http://127.0.0.1/?token=x",
        ] {
            assert_eq!(
                probe_http_page(address, Instant::now() + Duration::from_secs(1)),
                None
            );
        }
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}/", listener.local_addr().unwrap());
        let (done, release) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            release.recv_timeout(Duration::from_secs(2)).unwrap();
        });
        let start = Instant::now();
        assert_eq!(
            probe_http_page(&address, start + Duration::from_millis(40)),
            None
        );
        assert!(start.elapsed() < Duration::from_secs(1));
        done.send(()).unwrap();
        server.join().unwrap();
    }
}
