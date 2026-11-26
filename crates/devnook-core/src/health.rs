use crate::{process, service_model::*};
use std::{
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream},
    time::Duration,
};

fn parse_http_targets(text: &str) -> Result<(Vec<SocketAddr>, String, String), String> {
    let url = url::Url::parse(text).map_err(|_| "健康检查 URL 无效")?;
    if url.scheme() != "http"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err("健康检查仅支持无凭据的本机 http URL".into());
    }
    let host = url.host_str().ok_or("健康检查 URL 缺少主机")?;
    if url
        .query_pairs()
        .any(|(key, _)| crate::supervisor::sensitive_key(&key))
    {
        return Err("健康检查 URL 不支持凭据参数，请使用不含密钥的本机健康端点".into());
    }
    let port = url.port_or_known_default().ok_or("健康检查缺少端口")?;
    let addresses = if host == "localhost" {
        vec![
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
            SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port),
        ]
    } else {
        let ip = host
            .trim_matches(['[', ']'])
            .parse::<IpAddr>()
            .map_err(|_| "健康检查只能访问 loopback 地址")?;
        if !ip.is_loopback() {
            return Err("健康检查只能访问 loopback 地址".into());
        }
        vec![SocketAddr::new(ip, port)]
    };
    let mut path = url.path().to_string();
    if let Some(query) = url.query() {
        path.push('?');
        path.push_str(query);
    }
    Ok((addresses, path, format!("{host}:{port}")))
}

pub fn http_target(text: &str) -> Result<(SocketAddr, String, String), String> {
    let (addresses, path, host) = parse_http_targets(text)?;
    Ok((addresses[0], path, host))
}

fn probe_http(address: SocketAddr, path: &str, host: &str) -> bool {
    let timeout = Duration::from_millis(250);
    let Ok(mut stream) = TcpStream::connect_timeout(&address, timeout) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));
    if stream
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n").as_bytes(),
        )
        .is_err()
    {
        return false;
    }
    let mut status = Vec::new();
    let mut byte = [0u8; 1];
    while status.len() < 512 {
        if stream.read(&mut byte).ok() != Some(1) {
            return false;
        }
        if byte[0] == b'\n' {
            break;
        }
        status.push(byte[0]);
    }
    let status = String::from_utf8_lossy(&status);
    let mut fields = status.split_whitespace();
    matches!(fields.next(), Some("HTTP/1.0" | "HTTP/1.1"))
        && fields
            .next()
            .and_then(|v| v.parse::<u16>().ok())
            .is_some_and(|code| (200..300).contains(&code))
}

pub fn http_ready(text: &str) -> bool {
    http_ready_if(text, |_| true)
}

fn http_ready_if(text: &str, can_probe: impl Fn(SocketAddr) -> bool) -> bool {
    let Ok((addresses, path, host)) = parse_http_targets(text) else {
        return false;
    };
    addresses
        .into_iter()
        .any(|address| can_probe(address) && probe_http(address, &path, &host))
}

fn http_listener_belongs(address: SocketAddr, pgid: u32) -> bool {
    // Darwin's IPv4 selector distinguishes dual-stack wildcard sockets from
    // IPv6-only ones. The unfiltered snapshot prints both as IPv6 "*".
    let Ok(bindings) = process::listeners_on_port(address.port(), address.is_ipv4()) else {
        return false;
    };
    let mut matching = bindings
        .iter()
        .filter(|binding| {
            if binding.port != address.port() {
                return false;
            }
            let host = binding.address.trim_matches(['[', ']']);
            if host == "*" {
                return address.is_ipv4() || binding.family == "IPv6";
            }
            host.parse::<IpAddr>().is_ok_and(|ip| {
                let ip = ip.to_canonical();
                ip == address.ip() || (ip.is_unspecified() && (address.is_ipv4() || ip.is_ipv6()))
            })
        })
        .peekable();
    let uid = unsafe { libc::geteuid() };
    matching.peek().is_some()
        && matching.all(|binding| {
            process::identity(binding.pid)
                .is_some_and(|p| p.alive && p.pgid == pgid && p.uid == uid)
        })
}

pub fn port_belongs(port: u16, pgid: u32) -> bool {
    let Ok(bindings) = process::list_ports() else {
        return false;
    };
    bindings.iter().filter(|b| b.port == port).any(|b| {
        process::identity(b.pid)
            .is_some_and(|p| p.pgid == pgid && p.uid == unsafe { libc::geteuid() })
    })
}

pub fn ready(
    definition: &ServiceDefinition,
    pgid: u32,
    elapsed: Duration,
    phrase_found: bool,
) -> bool {
    match definition.readiness.kind {
        ReadinessKind::Process => elapsed.as_millis() >= definition.readiness.grace_ms as u128,
        ReadinessKind::Port => definition
            .readiness
            .port
            .is_some_and(|port| port_belongs(port, pgid)),
        ReadinessKind::Http => http_ready_if(&definition.readiness.url, |address| {
            http_listener_belongs(address, pgid)
        }),
        ReadinessKind::LogPhrase => phrase_found,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn localhost_http_ready_accepts_ipv6_loopback_listener() {
        let Ok(listener) = TcpListener::bind("[::1]:0") else {
            return;
        };
        let port = listener.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 256];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                );
            }
        });
        assert!(http_ready(&format!("http://localhost:{port}/health")));
        handle.join().unwrap();
    }
}
