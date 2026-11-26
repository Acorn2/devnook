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

pub fn strip_ansi(input: &str) -> String {
    if !input.as_bytes().contains(&0x1b) {
        return input.to_string();
    }
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b {
            if i + 1 >= bytes.len() {
                break;
            }
            match bytes[i + 1] {
                b'[' => {
                    // CSI sequence: ESC [ params intermediates final (@-~)
                    i += 2;
                    while i < bytes.len() && !(0x40..=0x7e).contains(&bytes[i]) {
                        i += 1;
                    }
                    if i < bytes.len() {
                        i += 1;
                    }
                }
                b']' => {
                    // OSC sequence: ESC ] ... (BEL or ST)
                    i += 2;
                    while i < bytes.len() {
                        if bytes[i] == 0x07 {
                            i += 1;
                            break;
                        }
                        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                }
                0x40..=0x5f => {
                    i += 2;
                }
                _ => {
                    i += 1;
                }
            }
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

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
            .read(true)
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
    fn compact_for(&mut self, incoming: &[u8]) -> Result<(), String> {
        let marker = format!(
            "[{}] [system] 日志达到大小上限，较早输出已省略，仅保留最新内容\n",
            now()
        );
        let marker_bytes = marker.as_bytes();
        let incoming_slice = if marker_bytes.len() as u64 + incoming.len() as u64 > self.limit {
            let fallback = format!("[{}] [system] 单条日志超过文件限制，已省略\n", now());
            let mut compacted = Vec::with_capacity(marker_bytes.len() + fallback.len());
            compacted.extend_from_slice(marker_bytes);
            compacted.extend_from_slice(fallback.as_bytes());
            self.file
                .seek(SeekFrom::Start(0))
                .map_err(|e| e.to_string())?;
            self.file.write_all(&compacted).map_err(|e| e.to_string())?;
            self.file
                .set_len(compacted.len() as u64)
                .map_err(|e| e.to_string())?;
            self.bytes = compacted.len() as u64;
            self.truncated = true;
            return Ok(());
        } else {
            incoming
        };
        let keep_budget = self
            .limit
            .saturating_sub(marker_bytes.len() as u64 + incoming_slice.len() as u64)
            .min(self.limit / 2);
        let mut tail = Vec::new();
        if keep_budget > 0 && self.bytes > 0 {
            let start = self.bytes.saturating_sub(keep_budget);
            self.file
                .seek(SeekFrom::Start(start))
                .map_err(|e| e.to_string())?;
            Read::by_ref(&mut self.file)
                .take(keep_budget)
                .read_to_end(&mut tail)
                .map_err(|e| e.to_string())?;
            if start > 0 {
                if let Some(pos) = tail.iter().position(|b| *b == b'\n') {
                    tail.drain(..=pos);
                } else {
                    tail.clear();
                }
            }
        }
        let mut compacted =
            Vec::with_capacity(marker_bytes.len() + tail.len() + incoming_slice.len());
        compacted.extend_from_slice(marker_bytes);
        compacted.extend_from_slice(&tail);
        compacted.extend_from_slice(incoming_slice);
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|e| e.to_string())?;
        self.file.write_all(&compacted).map_err(|e| e.to_string())?;
        self.file
            .set_len(compacted.len() as u64)
            .map_err(|e| e.to_string())?;
        self.bytes = compacted.len() as u64;
        self.truncated = true;
        Ok(())
    }
    pub fn write(&mut self, source: &str, text: &str) -> Result<(), String> {
        let mut safe = strip_ansi(text);
        for secret in &self.secrets {
            safe = safe.replace(secret, "[REDACTED]");
        }
        let safe = process::redact_command(&safe);
        let line = format!("[{}] [{source}] {safe}\n", now());
        if self.bytes + line.len() as u64 > self.limit {
            return self.compact_for(line.as_bytes());
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
            if strip_ansi(&tail).contains(&phrase) {
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
    let window = if query.is_empty() {
        512 * 1024
    } else {
        2 * 1024 * 1024
    };
    let start = length.saturating_sub(window);
    file.seek(SeekFrom::Start(start))
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(window)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&bytes);
    let needle = query.to_lowercase();
    let mut lines: Vec<_> = text
        .lines()
        .skip(usize::from(start > 0))
        .filter(|line| needle.is_empty() || line.to_lowercase().contains(&needle))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn strip_ansi_removes_color_and_osc_sequences_preserving_utf8() {
        let colored = "\x1b[32mReady\x1b[0m in \x1b[1;36m300ms\x1b[0m · 中文 \x1b]0;title\x07ok";
        assert_eq!(strip_ansi(colored), "Ready in 300ms · 中文 ok");
    }

    #[test]
    fn rolling_log_sink_keeps_latest_lines_within_limit_and_redacts() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("rolling.log");
        let mut sink = LogSink::new(&path, 4, vec!["super-secret-value".into()]).unwrap();
        for index in 0..80 {
            sink.write(
                "stdout",
                &format!(
                    "\x1b[33mline-{index:03}\x1b[0m {} secret=super-secret-value",
                    "a".repeat(80)
                ),
            )
            .unwrap();
        }
        assert!(sink.truncated);
        let size = fs::metadata(&path).unwrap().len();
        assert!(size <= 4 * 1024, "log size {size} exceeded 4 KiB limit");
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("line-079"));
        assert!(!content.contains("line-000"));
        assert!(!content.contains("\x1b["));
        assert!(!content.contains("super-secret-value"));
        assert!(content.contains("[REDACTED]"));
    }
}
