//! Read-only launch suggestions. Drafts never register or execute a service.
use crate::{
    scanner::{has_fastapi, identify, plain_file, read_manifest, uvicorn_entrypoint},
    service_model::*,
    Core,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashSet},
    path::Path,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandSpec {
    pub executable: String,
    pub args: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDraft {
    pub definition: ServiceDefinition,
    pub command: String,
    pub source: String,
    #[serde(default)]
    pub independent_entrypoint: bool,
}

/// A small command-line tokenizer, never a shell evaluator.
pub fn parse_command(command: &str) -> Result<CommandSpec, String> {
    if command.len() > 131_072 || command.contains(['\n', '\r', '\0']) {
        return Err("启动命令应为一行，且不超过 131072 字节".into());
    }
    let (mut words, mut word, mut quote, mut escaped, mut started) =
        (Vec::new(), String::new(), None, false, false);
    for ch in command.chars() {
        if escaped {
            if quote == Some('"') && !['"', '\\', '$', '`'].contains(&ch) {
                word.push('\\');
            }
            word.push(ch);
            escaped = false;
            continue;
        }
        match quote {
            Some('\'') => {
                if ch == '\'' {
                    quote = None
                } else {
                    word.push(ch)
                }
            }
            Some('"') => match ch {
                '"' => quote = None,
                '\\' => escaped = true,
                '$' | '`' => return Err("变量展开需要在高级设置中显式开启 Shell 模式".into()),
                _ => word.push(ch),
            },
            _ => match ch {
                '\'' | '"' => {
                    quote = Some(ch);
                    started = true;
                }
                '\\' => {
                    escaped = true;
                    started = true;
                }
                '|' | '&' | ';' | '<' | '>' | '$' | '`' | '(' | ')' => {
                    return Err("组合命令需要在高级设置中显式开启 Shell 模式".into())
                }
                ch if ch.is_whitespace() => {
                    if started {
                        words.push(std::mem::take(&mut word));
                        started = false;
                    }
                }
                _ => {
                    word.push(ch);
                    started = true;
                }
            },
        }
    }
    if escaped || quote.is_some() {
        return Err("启动命令的引号或转义未闭合".into());
    }
    if started {
        words.push(word);
    }
    if words.is_empty() || words[0].is_empty() {
        return Err("请填写启动命令".into());
    }
    if words[0].contains('=') {
        return Err("请在高级设置中填写环境变量".into());
    }
    if words.len() > 257 {
        return Err("启动参数不能超过 256 项".into());
    }
    Ok(CommandSpec {
        executable: words.remove(0),
        args: words,
    })
}

pub fn display_command(spec: &CommandSpec) -> String {
    std::iter::once(&spec.executable)
        .chain(spec.args.iter())
        .map(|word| {
            if !word.is_empty()
                && word
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || "_./:@%+=,-".contains(ch))
            {
                word.clone()
            } else {
                format!("'{}'", word.replace('\'', "'\\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Match a custom package script to a verified running Node entry. The command
/// comes from the manifest, never from replaying ps output or the old environment.
pub(crate) fn external_node_command(
    process: &crate::model::ProcessDetail,
    root: &Path,
) -> Option<(CommandSpec, String, String)> {
    let cwd = Path::new(process.cwd.as_deref()?);
    if root.canonicalize().ok()?.as_path() != root
        || cwd.canonicalize().ok()?.as_path() != cwd
        || !cwd.starts_with(root)
    {
        return None;
    }
    let mut checked = root.to_path_buf();
    for component in cwd.strip_prefix(root).ok()?.components() {
        let std::path::Component::Normal(name) = component else {
            return None;
        };
        if name.to_str()?.starts_with('.') || name == "node_modules" || name == "vendor" {
            return None;
        }
        checked.push(name);
        if std::fs::symlink_metadata(&checked)
            .ok()?
            .file_type()
            .is_symlink()
        {
            return None;
        }
    }
    let executable = process.executable.as_deref()?;
    let runtime = Path::new(executable).file_name()?.to_str()?;
    if !["node", "nodejs"].contains(&runtime) {
        return None;
    }
    // ps loses argv boundaries. Strip a known executable path (possibly with
    // spaces), then require exact argument equality with a simple manifest command.
    let observed = process
        .command
        .strip_prefix(executable)
        .filter(|rest| rest.starts_with(char::is_whitespace))
        .map(|rest| parse_command(&format!("{runtime} {rest}")))
        .unwrap_or_else(|| parse_command(&process.command))
        .ok()?;
    if Path::new(&observed.executable).file_name()?.to_str()? != runtime {
        return None;
    }
    let manifest: Value = serde_json::from_str(&read_manifest(&cwd.join("package.json"))?).ok()?;
    let scripts = manifest.get("scripts")?.as_object()?;
    let mut matches = Vec::new();
    for (name, text) in scripts {
        let Some(spec) = text.as_str().and_then(|text| parse_command(text).ok()) else {
            continue;
        };
        if ![runtime, "node", "nodejs"].contains(&spec.executable.as_str())
            || spec.args != observed.args
        {
            continue;
        }
        // No eval, loaders, debugger flags, arbitrary installed CLIs or guessed
        // shell semantics. File metadata proves the entry stays inside the project.
        let Some(entry) = spec.args.first().filter(|arg| !arg.starts_with('-')) else {
            continue;
        };
        let entry = Path::new(entry);
        if !matches!(
            entry.extension().and_then(|s| s.to_str()),
            Some("js" | "mjs" | "cjs" | "ts" | "mts" | "cts")
        ) {
            continue;
        }
        let path = if entry.is_absolute() {
            entry.to_path_buf()
        } else {
            cwd.join(entry)
        };
        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        let mut checked = root.to_path_buf();
        let safe = relative.components().all(|part| {
            let std::path::Component::Normal(name) = part else {
                return false;
            };
            if name.to_string_lossy().starts_with('.') || name == "node_modules" || name == "vendor"
            {
                return false;
            }
            checked.push(name);
            std::fs::symlink_metadata(&checked).is_ok_and(|m| !m.file_type().is_symlink())
        });
        let command = display_command(&spec);
        if !safe
            || !plain_file(&path)
            || spec
                .args
                .iter()
                .any(|arg| crate::process::redact_command(arg) != *arg)
            || crate::process::redact_command(&command) != command
        {
            continue;
        }
        let service_name = if ["start", "dev", "serve"].contains(&name.as_str()) {
            manifest
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(name)
                .to_string()
        } else {
            name.clone()
        };
        matches.push((spec, service_name, format!("package.json · scripts.{name}")));
    }
    // Aliases are safe only when they describe the exact same command; choose
    // a stable name rather than clearing an otherwise unambiguous launch entry.
    let first = matches.first()?;
    if matches.iter().any(|item| item.0 != first.0) {
        return None;
    }
    matches.into_iter().next()
}

fn suggestion(path: &Path, root: &Path) -> Option<(CommandSpec, String)> {
    if read_manifest(&path.join("package.json"))
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .is_some()
    {
        let script = crate::scanner::node_script(path)?;
        let mut manager = None;
        for dir in path.ancestors().take_while(|dir| dir.starts_with(root)) {
            let manifest = read_manifest(&dir.join("package.json"))
                .and_then(|s| serde_json::from_str::<Value>(&s).ok());
            if let Some(field) = manifest
                .as_ref()
                .and_then(|p| p.get("packageManager"))
                .and_then(Value::as_str)
            {
                let name = field.split('@').next().unwrap_or_default();
                if !["npm", "pnpm", "yarn", "bun"].contains(&name) {
                    return None;
                }
                manager = Some(name.to_string());
                break;
            }
            let found: Vec<_> = [
                ("pnpm-lock.yaml", "pnpm"),
                ("yarn.lock", "yarn"),
                ("bun.lock", "bun"),
                ("bun.lockb", "bun"),
                ("package-lock.json", "npm"),
            ]
            .into_iter()
            .filter_map(|(file, name)| plain_file(&dir.join(file)).then_some(name))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
            if found.len() > 1 {
                return None;
            }
            if let Some(name) = found.first() {
                manager = Some((*name).to_string());
                break;
            }
        }
        return Some((
            CommandSpec {
                executable: manager.unwrap_or_else(|| "npm".into()),
                args: vec!["run".into(), script.clone()],
            },
            format!("package.json · scripts.{script}"),
        ));
    }
    if read_manifest(&path.join("pom.xml")).is_some_and(|s| s.contains("spring-boot")) {
        let executable = if plain_file(&path.join("mvnw")) {
            path.join("mvnw").to_string_lossy().into()
        } else {
            "mvn".into()
        };
        return Some((
            CommandSpec {
                executable,
                args: vec!["spring-boot:run".into()],
            },
            "pom.xml · Spring Boot".into(),
        ));
    }
    for manifest in ["build.gradle", "build.gradle.kts"] {
        if let Some(text) = read_manifest(&path.join(manifest)) {
            let task = if text.contains("org.springframework.boot") {
                "bootRun"
            } else if text.contains("application") {
                "run"
            } else {
                continue;
            };
            let executable = if plain_file(&path.join("gradlew")) {
                path.join("gradlew").to_string_lossy().into()
            } else {
                "gradle".into()
            };
            return Some((
                CommandSpec {
                    executable,
                    args: vec![task.into(), "--no-daemon".into()],
                },
                format!("{manifest} · {task}"),
            ));
        }
    }
    if plain_file(&path.join("Cargo.toml")) && plain_file(&path.join("src/main.rs")) {
        return Some((
            CommandSpec {
                executable: "cargo".into(),
                args: vec!["run".into()],
            },
            "Cargo.toml · src/main.rs".into(),
        ));
    }
    None
}

impl Core {
    pub fn service_drafts(&self, project_id: &str) -> Result<Vec<ServiceDraft>, String> {
        let project = self
            .projects()?
            .into_iter()
            .find(|p| p.candidate.id == project_id)
            .ok_or("项目不存在")?;
        let root = Path::new(&project.candidate.canonical_path);
        let canonical = root.canonicalize().map_err(|_| "项目目录不可访问")?;
        if canonical != root {
            return Err("项目目录已改变，请重新扫描".into());
        }
        let mut paths = vec![project.candidate.canonical_path.clone()];
        paths.extend(
            project
                .candidate
                .components
                .iter()
                .map(|c| c.canonical_path.clone()),
        );
        let mut seen = HashSet::new();
        let mut drafts = Vec::new();
        for text in paths {
            if !seen.insert(text.clone()) {
                continue;
            }
            let path = Path::new(&text);
            if !path.starts_with(root) || path.canonicalize().ok().as_deref() != Some(path) {
                continue;
            }
            let Some(candidate) = identify(path, None).filter(|c| c.runnable) else {
                continue;
            };
            let mut name = if path == root {
                "根目录启动".to_string()
            } else {
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            };
            let mut command = suggestion(path, root);
            let mut runtime = candidate.project_type.as_str();
            let mut port = None;
            let mut independent_entrypoint = false;
            if command.is_none()
                && candidate
                    .frameworks
                    .iter()
                    .any(|framework| framework == "FastAPI")
                && has_fastapi(path)
            {
                if let Some((args, source, detected_port)) = uvicorn_entrypoint(path) {
                    command = Some((
                        CommandSpec {
                            executable: "python3".into(),
                            args,
                        },
                        source,
                    ));
                    runtime = "PYTHON";
                    port = detected_port;
                    independent_entrypoint = true;
                    if path == root {
                        name = "FastAPI".into();
                    }
                }
            }
            let definition = ServiceDefinition {
                id: String::new(),
                project_id: project_id.into(),
                name,
                runtime_type: if ["NODE", "JAVA", "PYTHON"].contains(&runtime) {
                    runtime.into()
                } else {
                    "OTHER".into()
                },
                executable: command
                    .as_ref()
                    .map(|c| c.0.executable.clone())
                    .unwrap_or_default(),
                args: command
                    .as_ref()
                    .map(|c| c.0.args.clone())
                    .unwrap_or_default(),
                shell_mode: false,
                working_directory: text,
                environment: BTreeMap::new(),
                secret_keys: vec![],
                expected_ports: port.into_iter().collect(),
                dependencies: vec![],
                readiness: match port {
                    Some(port) => Readiness {
                        kind: ReadinessKind::Port,
                        port: Some(port),
                        ..Readiness::default()
                    },
                    None => Readiness::default(),
                },
                stop_grace_seconds: 5,
                log_limit_kb: 1024,
                revision: String::new(),
                created_at: 0,
                updated_at: 0,
            };
            drafts.push(ServiceDraft {
                definition,
                command: command
                    .as_ref()
                    .map(|c| display_command(&c.0))
                    .unwrap_or_default(),
                source: command
                    .map(|c| c.1)
                    .unwrap_or_else(|| "需要补充启动命令".into()),
                independent_entrypoint,
            });
        }
        drafts.sort_by_key(|d| {
            (
                d.definition.working_directory == project.candidate.canonical_path,
                d.definition.name.clone(),
            )
        });
        Ok(drafts)
    }
}
