mod discovery;
mod manifests;
mod purpose;
mod walk;

pub(crate) use purpose::node_script;

use crate::model::{Candidate, ScanProgress};
use serde_json::Value;
use std::{
    collections::HashSet,
    fs,
    io::Read,
    os::unix::fs::OpenOptionsExt,
    path::Path,
    sync::{atomic::AtomicBool, Arc, Mutex},
};
use uuid::Uuid;

pub const AUTO_DEPTH: usize = 32;

pub(crate) fn plain_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
}

fn git_boundary(path: &Path) -> bool {
    fs::symlink_metadata(path.join(".git"))
        .is_ok_and(|m| !m.file_type().is_symlink() && (m.is_file() || m.is_dir()))
}

pub struct ScanJob {
    pub cancel: AtomicBool,
    pub progress: Mutex<ScanProgress>,
}

const SKIP: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    "coverage",
    ".next",
    ".nuxt",
    ".gradle",
    ".venv",
    "venv",
    "__pycache__",
    ".cache",
    "cache",
    "caches",
    "backup",
    "backups",
    ".tmp",
    ".idea",
    ".vscode",
    "vendor",
    ".tox",
    ".mypy_cache",
    ".pytest_cache",
    ".turbo",
];

pub(crate) fn read_manifest(path: &Path) -> Option<String> {
    // Only known, non-secret manifests are passed here. Never read arbitrary project files.
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 1_048_576 {
        return None;
    }
    let mut text = String::new();
    fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .ok()?
        .take(1_048_577)
        .read_to_string(&mut text)
        .ok()?;
    Some(text)
}

pub fn identify(path: &Path, parent: Option<String>) -> Option<Candidate> {
    let names: Vec<_> = fs::read_dir(path)
        .into_iter()
        .flatten()
        .take(4096)
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| !t.is_symlink()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    identify_with_hints(path, parent, &manifests::analyze(path, &names))
}

fn identify_with_hints(
    path: &Path,
    parent: Option<String>,
    hints: &manifests::Hints,
) -> Option<Candidate> {
    let mut types = Vec::new();
    let mut tools = Vec::new();
    let mut frameworks = Vec::new();
    let mut evidence = Vec::new();
    let mut runnable = false;
    let mut review = false;
    let mut monorepo = plain_file(&path.join("pnpm-workspace.yaml"));
    if let Some(text) = read_manifest(&path.join("package.json")) {
        types.push("NODE");
        evidence.push("package.json".into());
        match serde_json::from_str::<Value>(&text) {
            Ok(pkg) => {
                monorepo |= pkg.get("workspaces").is_some();
                for (key, label) in [
                    ("react", "React"),
                    ("next", "Next.js"),
                    ("vue", "Vue"),
                    ("vite", "Vite"),
                    ("nuxt", "Nuxt"),
                    ("express", "Express"),
                    ("@nestjs/core", "NestJS"),
                    ("expo", "Expo"),
                    ("react-native", "React Native"),
                ] {
                    if ["dependencies", "devDependencies"]
                        .iter()
                        .any(|field| pkg.get(field).and_then(|d| d.get(key)).is_some())
                    {
                        frameworks.push(label.into());
                    }
                }
                runnable |= hints.purpose.node_script.is_some();
            }
            Err(_) => review = true,
        }
        tools.push(
            if plain_file(&path.join("pnpm-lock.yaml"))
                || plain_file(&path.join("pnpm-workspace.yaml"))
            {
                "pnpm"
            } else if plain_file(&path.join("yarn.lock")) {
                "yarn"
            } else if plain_file(&path.join("bun.lock")) || plain_file(&path.join("bun.lockb")) {
                "bun"
            } else {
                "npm"
            }
            .into(),
        );
    }
    for (manifest, tool) in [
        ("pom.xml", "Maven"),
        ("build.gradle", "Gradle"),
        ("build.gradle.kts", "Gradle"),
    ] {
        if let Some(text) = read_manifest(&path.join(manifest)) {
            if !types.contains(&"JAVA") {
                types.push("JAVA");
            }
            evidence.push(manifest.into());
            tools.push(tool.into());
            if text.contains("spring-boot") || text.contains("org.springframework.boot") {
                frameworks.push("Spring Boot".into());
                runnable = true;
            }
            monorepo |= text.contains("<modules>");
            runnable |= text.contains("application");
        }
    }
    monorepo |=
        plain_file(&path.join("settings.gradle")) || plain_file(&path.join("settings.gradle.kts"));
    for manifest in ["pyproject.toml", "requirements.txt", "setup.py"] {
        if let Some(text) = read_manifest(&path.join(manifest)) {
            if !types.contains(&"PYTHON") {
                types.push("PYTHON");
            }
            evidence.push(manifest.into());
            let lower = text.to_lowercase();
            for (key, label) in [
                ("fastapi", "FastAPI"),
                ("django", "Django"),
                ("flask", "Flask"),
            ] {
                if lower.contains(key) {
                    frameworks.push(label.into());
                    runnable = true;
                }
            }
        }
    }
    if types.contains(&"PYTHON") {
        tools.push(
            if plain_file(&path.join("uv.lock")) {
                "uv"
            } else if plain_file(&path.join("poetry.lock")) {
                "Poetry"
            } else {
                "pip"
            }
            .into(),
        );
    }
    if let Some(text) = read_manifest(&path.join("Cargo.toml")) {
        types.push("RUST");
        tools.push("Cargo".into());
        evidence.push("Cargo.toml".into());
        monorepo |= text.contains("[workspace]");
        runnable |= plain_file(&path.join("src/main.rs"));
    }
    if read_manifest(&path.join("go.mod")).is_some() {
        types.push("GO");
        tools.push("Go".into());
        evidence.push("go.mod".into());
        runnable |= plain_file(&path.join("main.go"));
    }
    for kind in &hints.types {
        if !types.contains(kind) {
            types.push(kind);
        }
    }
    tools.extend(hints.tools.iter().cloned());
    frameworks.extend(hints.frameworks.iter().cloned());
    evidence.extend(hints.evidence.iter().cloned());
    evidence.sort();
    evidence.dedup();
    monorepo |= hints.workspace;
    runnable |= hints.runnable;
    runnable |= types.iter().any(|kind| hints.purpose.hosts.contains(kind));
    review |= hints.review;
    types.retain(|kind| !hints.purpose.suppresses(kind));
    if !hints.purpose.clients.is_empty() {
        runnable = types.iter().any(|kind| hints.purpose.hosts.contains(kind));
    }
    if types.is_empty() {
        return None;
    }
    frameworks.sort();
    frameworks.dedup();
    tools.sort();
    tools.dedup();
    Some(Candidate {
        id: Uuid::new_v4().to_string(),
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into(),
        root_path: path.to_string_lossy().into(),
        canonical_path: path.to_string_lossy().into(),
        project_type: if types.len() > 1 {
            "MIXED".into()
        } else {
            types[0].into()
        },
        frameworks,
        build_tools: tools,
        evidence,
        is_git: git_boundary(path),
        is_monorepo: monorepo,
        parent_path: parent,
        runnable,
        needs_review: review,
        imported: false,
        components: vec![],
        boundary: String::new(),
    })
}

fn warning(job: &ScanJob, message: String) {
    let mut p = job.progress.lock().unwrap();
    p.incomplete = true;
    if p.warnings.len() < 30 {
        p.warnings.push(message);
    }
}

pub fn run(root: std::path::PathBuf, depth: usize, imported: HashSet<String>, job: Arc<ScanJob>) {
    walk::run(root, depth, imported, vec![], job)
}

pub(crate) fn run_with_known(
    root: std::path::PathBuf,
    depth: usize,
    imported: HashSet<String>,
    known: Vec<std::path::PathBuf>,
    job: Arc<ScanJob>,
) {
    walk::run(root, depth, imported, known, job)
}
