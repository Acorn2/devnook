use crate::{model::now, process, service_model::LogPage};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

pub struct LogSink {
    file: File,
    bytes: u64,
    limit: u64,
    secrets: Vec<String>,
    pub truncated: bool,
}
impl LogSink {
    pub fn new(path: &Path, limit_kb: u64, mut secrets: Vec<String>) -> Result<Self, String> {
        secrets.retain(|v| !v.is_empty());
        let fragments: Vec<_> = secrets
            .iter()
            .flat_map(|value| {
                value
                    .lines()
                    .filter(|line| !line.is_empty())
                    .map(str::to_string)
            })
            .collect();
        let escaped: Vec<_> = secrets
            .iter()
            .filter_map(|value| serde_json::to_string(value).ok())
            .map(|value| value[1..value.len() - 1].to_string())
            .collect();
        secrets.extend(fragments);
        secrets.extend(escaped);
        secrets.sort();
        secrets.dedup();
        secrets.sort_by_key(|v| std::cmp::Reverse(v.len()));
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .map_err(|e| e.to_string())?;
        Ok(Self {
            file,
            bytes: 0,
            limit: limit_kb * 1024,
            secrets,
            truncated: false,
        })
    }
    pub fn write(&mut self, source: &str, text: &str) -> Result<(), String> {
        if self.truncated {
            return Ok(());
        }
        let mut safe = text.to_string();
        for secret in &self.secrets {
            safe = safe.replace(secret, "[REDACTED]");
        }
        let safe = process::redact_command(&safe);
        let line = format!("[{}] [{source}] {safe}\n", now());
        if self.bytes + line.len() as u64 > self.limit {
            self.truncated = true;
            let marker = format!(
                "[{}] [system] 日志达到大小上限，后续输出继续排空但不再写入\n",
                now()
            );
            self.file
                .write_all(marker.as_bytes())
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
        self.file
            .write_all(line.as_bytes())
            .map_err(|e| e.to_string())?;
        self.bytes += line.len() as u64;
        Ok(())
    }
}

pub fn collect(
    mut input: impl Read,
    source: &str,
    sink: Arc<Mutex<LogSink>>,
    phrase: String,
    found: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
) {
    let mut buffer = [0u8; 4096];
    let mut line = Vec::new();
    let mut omitted = false;
    let mut tail = String::new();
    loop {
        let count = match input.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => n,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => {
                failed.store(true, Ordering::Relaxed);
                break;
            }
        };
        if !phrase.is_empty() && !found.load(Ordering::Relaxed) {
            tail.push_str(&String::from_utf8_lossy(&buffer[..count]));
            if tail.contains(&phrase) {
                found.store(true, Ordering::Relaxed);
            }
            if tail.len() > 8192 {
                let mut split = tail.len() - 4096;
                while !tail.is_char_boundary(split) {
                    split += 1;
                }
                tail.drain(..split);
            }
        }
        for byte in &buffer[..count] {
            if *byte == b'\n' {
                let text = if omitted {
                    "[单行超过 64 KiB，整行已省略]".into()
                } else {
                    String::from_utf8_lossy(&line).into_owned()
                };
                if sink
                    .lock()
                    .map_err(|_| ())
                    .and_then(|mut s| s.write(source, &text).map_err(|_| ()))
                    .is_err()
                {
                    failed.store(true, Ordering::Relaxed);
                }
                line.clear();
                omitted = false;
            } else if !omitted {
                line.push(*byte);
                if line.len() > 65_536 {
                    omitted = true;
                    line.clear();
                }
            }
        }
    }
    if !line.is_empty() || omitted {
        let text = if omitted {
            "[单行超过 64 KiB，整行已省略]".into()
        } else {
            String::from_utf8_lossy(&line).into_owned()
        };
        if sink
            .lock()
            .map_err(|_| ())
            .and_then(|mut s| s.write(source, &text).map_err(|_| ()))
            .is_err()
        {
            failed.store(true, Ordering::Relaxed);
        }
    }
}

pub fn read(
    path: &Path,
    run_id: String,
    limit: usize,
    query: &str,
    truncated: bool,
) -> Result<LogPage, String> {
    if !path.exists() {
        return Ok(LogPage {
            run_id,
            lines: Vec::new(),
            truncated,
        });
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|e| format!("无法读取日志：{e}"))?;
    let length = file.metadata().map_err(|e| e.to_string())?.len();
    let start = length.saturating_sub(512 * 1024);
    file.seek(SeekFrom::Start(start))
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(512 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&bytes);
    let mut lines: Vec<_> = text
        .lines()
        .skip(usize::from(start > 0))
        .filter(|line| query.is_empty() || line.to_lowercase().contains(&query.to_lowercase()))
        .map(str::to_string)
        .collect();
    if lines.len() > limit.min(1000) {
        lines.drain(..lines.len() - limit.min(1000));
    }
    Ok(LogPage {
        run_id,
        lines,
        truncated: truncated || start > 0,
    })
}
