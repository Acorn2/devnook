use crate::{process, service_model::*};
use std::{
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream},
    time::Duration,
};

pub fn http_target(text: &str) -> Result<(SocketAddr, String, String), String> {
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
    let ip = if host == "localhost" {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    } else {
        host.trim_matches(['[', ']'])
            .parse::<IpAddr>()
            .map_err(|_| "健康检查只能访问 loopback 地址")?
    };
    if !ip.is_loopback() {
        return Err("健康检查只能访问 loopback 地址".into());
    }
    let port = url.port_or_known_default().ok_or("健康检查缺少端口")?;
    let mut path = url.path().to_string();
    if let Some(query) = url.query() {
        path.push('?');
        path.push_str(query);
    }
    Ok((SocketAddr::new(ip, port), path, format!("{host}:{port}")))
}

pub fn http_ready(text: &str) -> bool {
    let Ok((address, path, host)) = http_target(text) else {
        return false;
    };
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
        ReadinessKind::Http => {
            http_target(&definition.readiness.url)
                .is_ok_and(|(address, _, _)| port_belongs(address.port(), pgid))
                && http_ready(&definition.readiness.url)
        }
        ReadinessKind::LogPhrase => phrase_found,
    }
}
