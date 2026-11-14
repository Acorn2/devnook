use super::{git_boundary, identify_with_hints, manifests, warning, ScanJob, AUTO_DEPTH, SKIP};
use glob::{MatchOptions, Pattern};
use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    fs,
    path::{Component, Path, PathBuf},
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};

const MAX_ENTRIES: usize = 100_000;
const MAX_EXPANSIONS: usize = 100_000;
const PRUNE: &[&str] = &[
    "src",
    "source",
    "sources",
    "lib",
    "test",
    "tests",
    "__tests__",
    "spec",
    "specs",
    "docs",
    "doc",
    "documentation",
    "assets",
    "static",
    "public",
    "resources",
    "fixtures",
    "images",
    "media",
    "data",
    "datasets",
    "logs",
    "log",
    "tmp",
    "temp",
    ".swiftpm",
    ".build",
    "bin",
    "obj",
    "Pods",
    "Carthage",
    ".dart_tool",
    ".pub-cache",
    "CMakeFiles",
];
const CONTAINERS: &[&str] = &[
    "frontend",
    "front-end",
    "backend",
    "back-end",
    "web",
    "client",
    "server",
    "api",
    "apps",
    "applications",
    "packages",
    "services",
    "modules",
    "crates",
    "libs",
    "shared",
    "common",
    "workers",
    "tools",
    ".tools",
    "plugins",
    "components",
    "examples",
    "samples",
    "projects",
    "subprojects",
    "cmd",
];

#[derive(Clone)]
struct Entry {
    name: String,
    directory: bool,
}
struct Walker<'a> {
    root: &'a Path,
    job: &'a ScanJob,
    started: Instant,
    entries: usize,
    expansions: usize,
    depth_limited: bool,
    cache: HashMap<PathBuf, Vec<Entry>>,
}
impl Walker<'_> {
    fn stopped(&self) -> bool {
        self.job.cancel.load(Ordering::Relaxed)
            || self.entries >= MAX_ENTRIES
            || self.expansions >= MAX_EXPANSIONS
            || self.started.elapsed() > Duration::from_secs(120)
    }
    fn list(&mut self, path: &Path) -> Vec<Entry> {
        if let Some(values) = self.cache.get(path) {
            return values.clone();
        }
        let mut values = Vec::new();
        match fs::read_dir(path) {
            Ok(entries) => {
                for entry in entries {
                    if self.stopped() {
                        break;
                    }
                    self.entries += 1;
                    if let Ok(entry) = entry {
                        let name = entry.file_name().to_string_lossy().into_owned();
                        if hard_skip(&name) {
                            continue;
                        }
                        if let Ok(kind) = entry.file_type() {
                            if !kind.is_symlink() {
                                values.push(Entry {
                                    name,
                                    directory: kind.is_dir(),
                                });
                            }
                        }
                    }
                }
            }
            Err(e) => warning(self.job, format!("无法访问 {}：{e}", path.display())),
        }
        values.sort_by(|a, b| a.name.cmp(&b.name));
        self.job.progress.lock().unwrap().entries = self.entries;
        self.cache.insert(path.into(), values.clone());
        values
    }
    fn safe_dir(&self, path: &Path) -> bool {
        let Ok(relative) = path.strip_prefix(self.root) else {
            return false;
        };
        let mut cursor = self.root.to_path_buf();
        if !fs::symlink_metadata(&cursor).is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink()) {
            return false;
        }
        for part in relative.components() {
            let Component::Normal(name) = part else {
                return false;
            };
            if hard_skip(&name.to_string_lossy()) {
                return false;
            }
            cursor.push(name);
            if !fs::symlink_metadata(&cursor)
                .is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
            {
                return false;
            }
        }
        true
    }
    fn normalized(&self, base: &Path, text: &str) -> Option<PathBuf> {
        if text.len() > 4096 || text.contains(['$', '{', '}', '\0', '\n', ':']) {
            return None;
        }
        let text = text.replace('\\', "/");
        if Path::new(&text).is_absolute() {
            return None;
        }
        let mut result = base.to_path_buf();
        for part in Path::new(&text).components() {
            match part {
                Component::CurDir => {}
                Component::ParentDir => {
                    if result == self.root || !result.pop() {
                        return None;
                    }
                }
                Component::Normal(name) => {
                    if hard_skip(&name.to_string_lossy()) {
                        return None;
                    }
                    result.push(name);
                }
                _ => return None,
            }
        }
        result.starts_with(self.root).then_some(result)
    }
    fn expand(&mut self, base: &Path, text: &str, max_depth: usize) -> Vec<PathBuf> {
        let Some(pattern) = self.normalized(base, text) else {
            warning(
                self.job,
                format!("忽略不安全或动态的模块路径：{} / {text}", base.display()),
            );
            return vec![];
        };
        let parts: Vec<_> = pattern
            .strip_prefix(self.root)
            .unwrap()
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        if parts.len() > max_depth {
            self.depth_limited = true;
            return vec![];
        }
        let mut queue = VecDeque::from([(self.root.to_path_buf(), 0)]);
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        while let Some((path, index)) = queue.pop_front() {
            if self.stopped() {
                break;
            }
            if !seen.insert((path.clone(), index)) {
                continue;
            }
            self.expansions += 1;
            if !self.safe_dir(&path) {
                continue;
            }
            if index == parts.len() {
                result.push(path);
                continue;
            }
            let segment = &parts[index];
            if segment == "**" {
                queue.push_back((path.clone(), index + 1));
                if path.strip_prefix(self.root).unwrap().components().count() >= max_depth {
                    self.depth_limited = true;
                    continue;
                }
                for entry in self
                    .list(&path)
                    .into_iter()
                    .filter(|e| e.directory && !prune(&e.name))
                {
                    queue.push_back((path.join(entry.name), index));
                }
            } else if segment.contains(['*', '?', '[']) {
                let Ok(pattern) = Pattern::new(segment) else {
                    warning(self.job, format!("无法解析模块路径模式：{text}"));
                    continue;
                };
                for entry in self
                    .list(&path)
                    .into_iter()
                    .filter(|e| e.directory && pattern.matches_with(&e.name, options()))
                {
                    queue.push_back((path.join(entry.name), index + 1));
                }
            } else {
                queue.push_back((path.join(segment), index + 1));
            }
        }
        result
    }
}
fn options() -> MatchOptions {
    MatchOptions {
        case_sensitive: true,
        require_literal_separator: true,
        require_literal_leading_dot: true,
    }
}
fn hard_skip(name: &str) -> bool {
    name == ".env"
        || name.starts_with(".env.")
        || SKIP.iter().any(|v| name.eq_ignore_ascii_case(v))
        || [
            ".dart_tool",
            ".pub-cache",
            ".build",
            ".swiftpm",
            "Pods",
            "Carthage",
            "obj",
            "CMakeFiles",
        ]
        .iter()
        .any(|v| name.eq_ignore_ascii_case(v))
}
fn prune(name: &str) -> bool {
    PRUNE.iter().any(|v| name.eq_ignore_ascii_case(v))
        || (name.starts_with('.') && name != ".tools")
}
fn excluded(path: &Path, exclusions: &[(PathBuf, Pattern)]) -> bool {
    !git_boundary(path)
        && exclusions.iter().any(|(base, pattern)| {
            path.strip_prefix(base)
                .ok()
                .is_some_and(|p| pattern.matches_path_with(p, options()))
        })
}

/// depth=0 follows project features. Positive depths retain bounded recursive supplementation.
pub(super) fn run(
    root: PathBuf,
    depth: usize,
    imported: HashSet<String>,
    known: Vec<PathBuf>,
    job: Arc<ScanJob>,
) {
    let mut walker = Walker {
        root: &root,
        job: &job,
        started: Instant::now(),
        entries: 0,
        expansions: 0,
        depth_limited: false,
        cache: HashMap::new(),
    };
    // None is a project collection; Some(n) is the remaining shallow probe inside a project.
    let mut pending = VecDeque::from([(root.clone(), None::<u8>, None::<String>)]);
    let max_depth = if depth == 0 { AUTO_DEPTH } else { depth };
    if depth == 0 {
        for path in known {
            if path.starts_with(&root) && walker.safe_dir(&path) {
                pending.push_back((path, Some(0), None));
            }
        }
    }
    let mut visited = HashSet::new();
    let mut boundaries = BTreeMap::new();
    let mut clients = HashSet::new();
    let mut exclusions = Vec::new();
    while let Some((path, probe, parent)) = pending.pop_front() {
        if walker.stopped() {
            break;
        }
        if !walker.safe_dir(&path)
            || (depth == 0 && excluded(&path, &exclusions))
            || !visited.insert(path.clone())
        {
            continue;
        }
        let level = path.strip_prefix(&root).unwrap().components().count();
        if level > max_depth {
            walker.depth_limited = true;
            continue;
        }
        let entries = walker.list(&path);
        let names: Vec<_> = entries.iter().map(|e| e.name.clone()).collect();
        let hints = manifests::analyze(&path, &names);
        if !hints.purpose.clients.is_empty() {
            clients.insert(path.clone());
        }
        let candidate = identify_with_hints(&path, parent.clone(), &hints);
        let boundary = git_boundary(&path)
            || hints.workspace
            || candidate.as_ref().is_some_and(|c| c.is_monorepo);
        if git_boundary(&path) {
            boundaries.insert(path.clone(), "GIT");
        } else if boundary {
            boundaries.insert(path.clone(), "WORKSPACE");
        } else if candidate.is_none() && !hints.purpose.clients.is_empty() {
            boundaries.insert(path.clone(), "CLIENT");
        }
        let recognized = candidate.is_some() || boundary || !hints.purpose.clients.is_empty();
        let next_parent = candidate
            .as_ref()
            .map(|c| c.canonical_path.clone())
            .or(parent);
        {
            let mut p = job.progress.lock().unwrap();
            p.visited += 1;
            p.current_path = path.to_string_lossy().into();
            if let Some(mut candidate) = candidate {
                candidate.imported = imported.contains(&candidate.canonical_path);
                p.candidates.push(candidate);
                if p.candidates.len() >= 5000 {
                    p.incomplete = true;
                    p.warnings
                        .push("候选项目达到 5000 个上限，请缩小扫描范围".into());
                    break;
                }
            }
        }
        if depth == 0 {
            for message in &hints.warnings {
                warning(&job, format!("{} / {message}", path.display()));
            }
            for text in &hints.excludes {
                if let Ok(pattern) = Pattern::new(&text.replace('\\', "/").trim_start_matches("./"))
                {
                    exclusions.push((path.clone(), pattern));
                } else {
                    warning(
                        &job,
                        format!("无法解析排除模式：{} / {text}", path.display()),
                    );
                }
            }
            for text in &hints.modules {
                for module in walker.expand(&path, text, max_depth) {
                    if module != path && !excluded(&module, &exclusions) {
                        pending.push_back((module, Some(0), next_parent.clone()));
                    }
                }
            }
        }
        let current_probe = if recognized { Some(2) } else { probe };
        for entry in entries.into_iter().filter(|e| e.directory) {
            let child = path.join(&entry.name);
            if level >= max_depth {
                walker.depth_limited = true;
                continue;
            }
            if depth > 0 {
                pending.push_back((child, None, next_parent.clone()));
                continue;
            }
            if prune(&entry.name) || excluded(&child, &exclusions) {
                continue;
            }
            let next_probe = match current_probe {
                None => None,
                Some(_)
                    if CONTAINERS
                        .iter()
                        .any(|v| entry.name.eq_ignore_ascii_case(v)) =>
                {
                    Some(2)
                }
                Some(n) if n > 0 => Some(n - 1),
                Some(_) => continue,
            };
            pending.push_back((child, next_probe, next_parent.clone()));
        }
    }
    if walker.depth_limited {
        warning(
            &job,
            if depth == 0 {
                "特征扫描已达到安全深度边界，请选择更具体的目录"
            } else {
                "已达到深度补扫边界，结果可能不完整；可扩大深度或选择更具体的目录"
            }
            .into(),
        );
    }
    if walker.entries >= MAX_ENTRIES {
        warning(
            &job,
            "已达到扫描条目上限（100000），结果可能不完整，请缩小扫描范围".into(),
        );
    }
    if walker.expansions >= MAX_EXPANSIONS {
        warning(&job, "模块路径展开达到安全上限，请缩小扫描范围".into());
    }
    if walker.started.elapsed() > Duration::from_secs(120) {
        warning(
            &job,
            "已达到扫描时间上限（120 秒），结果可能不完整，请缩小扫描范围".into(),
        );
    }
    let mut p = job.progress.lock().unwrap();
    p.entries = walker.entries;
    p.candidates = super::discovery::group(
        &root,
        std::mem::take(&mut p.candidates),
        &boundaries,
        &imported,
        &clients,
    );
    p.status = if job.cancel.load(Ordering::Relaxed) {
        "cancelled"
    } else {
        "completed"
    }
    .into();
}
