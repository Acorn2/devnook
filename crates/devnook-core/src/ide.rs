use crate::{
    model::{Project, Settings},
    process::command,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Component, Path, PathBuf},
};

#[cfg(target_os = "macos")]
mod macos;
const APP_PREFIX: &str = "APP:";
struct KnownApp {
    legacy: &'static str,
    name: &'static str,
    bundle_id: &'static str,
    kind: &'static str,
    project_types: &'static [&'static str],
}
macro_rules! app {
    ($legacy:literal, $name:literal, $bundle:literal, $kind:literal, $types:expr) => {
        KnownApp {
            legacy: $legacy,
            name: $name,
            bundle_id: $bundle,
            kind: $kind,
            project_types: $types,
        }
    };
}
const KNOWN: &[KnownApp] = &[
    app!(
        "VSCODE",
        "VS Code",
        "com.microsoft.VSCode",
        "EDITOR",
        &[
            "NODE", "JAVA", "PYTHON", "RUST", "GO", "DOTNET", "PHP", "RUBY", "DART", "SWIFT",
            "CPP", "MIXED"
        ]
    ),
    app!(
        "",
        "VS Code Insiders",
        "com.microsoft.VSCodeInsiders",
        "EDITOR",
        &[]
    ),
    app!(
        "CURSOR",
        "Cursor",
        "com.todesktop.230313mzl4w4u92",
        "EDITOR",
        &[
            "NODE", "JAVA", "PYTHON", "RUST", "GO", "DOTNET", "PHP", "RUBY", "DART", "SWIFT",
            "CPP", "MIXED"
        ]
    ),
    app!("", "Windsurf", "com.exafunction.windsurf", "EDITOR", &[]),
    app!("", "Trae", "com.trae.app", "EDITOR", &[]),
    app!("", "Qoder", "com.qoder.app", "EDITOR", &[]),
    app!("", "Zed", "dev.zed.Zed", "EDITOR", &[]),
    app!("", "Sublime Text", "com.sublimetext.4", "EDITOR", &[]),
    app!("", "Sublime Text 3", "com.sublimetext.3", "EDITOR", &[]),
    app!(
        "IDEA",
        "IntelliJ IDEA",
        "com.jetbrains.intellij",
        "IDE",
        &["JAVA"]
    ),
    app!(
        "IDEA",
        "IntelliJ IDEA Community",
        "com.jetbrains.intellij.ce",
        "IDE",
        &["JAVA"]
    ),
    app!("", "WebStorm", "com.jetbrains.WebStorm", "IDE", &["NODE"]),
    app!("", "PyCharm", "com.jetbrains.pycharm", "IDE", &["PYTHON"]),
    app!(
        "",
        "PyCharm Community",
        "com.jetbrains.pycharm.ce",
        "IDE",
        &["PYTHON"]
    ),
    app!("", "GoLand", "com.jetbrains.goland", "IDE", &["GO"]),
    app!("", "RustRover", "com.jetbrains.rustrover", "IDE", &["RUST"]),
    app!("", "PhpStorm", "com.jetbrains.PhpStorm", "IDE", &["PHP"]),
    app!("", "CLion", "com.jetbrains.CLion", "IDE", &["CPP"]),
    app!("", "Rider", "com.jetbrains.rider", "IDE", &["DOTNET"]),
    app!("", "Xcode", "com.apple.dt.Xcode", "IDE", &["SWIFT"]),
    app!(
        "",
        "Android Studio",
        "com.google.android.studio",
        "IDE",
        &[]
    ),
    app!(
        "TERMINAL",
        "Terminal",
        "com.apple.Terminal",
        "TERMINAL",
        &[]
    ),
];
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevelopmentApp {
    pub id: String,
    pub name: String,
    pub bundle_id: String,
    pub path: String,
    pub version: String,
    pub kind: String,
    pub recommended_for: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RememberedApp {
    pub path: String,
    pub bundle_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppPreference {
    pub id: String,
    pub name: String,
    pub available: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevelopmentApps {
    pub apps: Vec<DevelopmentApp>,
    pub project_defaults: HashMap<String, AppPreference>,
    pub global_default: AppPreference,
    pub warnings: Vec<String>,
}
pub(super) struct BundleMetadata {
    bundle_id: String,
    name: Option<String>,
    executable: String,
    version: String,
}
fn app_path(target: &str) -> Option<&Path> {
    target
        .strip_prefix(APP_PREFIX)
        .map(Path::new)
        .filter(|path| {
            path.is_absolute()
                && path.extension().is_some_and(|ext| ext == "app")
                && !path
                    .components()
                    .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
                && target.len() <= 4096
                && !target.contains('\0')
        })
}
pub fn validate_target(target: &str) -> Result<(), String> {
    if ["IDEA", "VSCODE", "CURSOR"].contains(&target) || app_path(target).is_some() {
        Ok(())
    } else {
        Err("不支持的开发应用，请选择已安装的应用".into())
    }
}
pub fn inspect_app(path: &Path) -> Result<DevelopmentApp, String> {
    if !cfg!(target_os = "macos") {
        return Err("应用识别目前仅支持 macOS".into());
    }
    let target = format!(
        "{APP_PREFIX}{}",
        path.to_str().ok_or("应用路径不是有效 UTF-8")?
    );
    if app_path(&target).is_none() {
        return Err("请选择绝对路径下的 .app 应用".into());
    }
    let directory = fs::symlink_metadata(path).map_err(|_| "应用不存在或不可访问")?;
    if !directory.is_dir() || directory.file_type().is_symlink() {
        return Err("应用必须是实际的 .app 目录，不能是符号链接".into());
    }
    for directory in [path.join("Contents"), path.join("Contents/MacOS")] {
        let metadata = fs::symlink_metadata(directory).map_err(|_| "应用目录结构不完整")?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err("应用内容目录不能是符号链接".into());
        }
    }
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path.join("Contents/Info.plist"))
        .map_err(|_| "无法读取应用 Info.plist")?;
    if !file
        .metadata()
        .map_err(|_| "无法读取应用清单属性")?
        .is_file()
    {
        return Err("应用清单不是普通文件".into());
    }
    let mut bytes = Vec::new();
    file.take(1_048_577)
        .read_to_end(&mut bytes)
        .map_err(|_| "无法读取应用清单")?;
    if bytes.len() > 1_048_576 {
        return Err("应用清单超过 1 MiB 上限".into());
    }
    #[cfg(target_os = "macos")]
    let metadata = macos::metadata(&bytes)?;
    #[cfg(not(target_os = "macos"))]
    let metadata = BundleMetadata {
        bundle_id: String::new(),
        name: None,
        executable: String::new(),
        version: String::new(),
    };
    if metadata.bundle_id.is_empty()
        || metadata.bundle_id.len() > 255
        || !metadata
            .bundle_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b))
        || metadata.executable.is_empty()
        || Path::new(&metadata.executable).components().count() != 1
        || !matches!(
            Path::new(&metadata.executable).components().next(),
            Some(Component::Normal(_))
        )
    {
        return Err("应用标识或可执行文件名无效".into());
    }
    let executable = fs::symlink_metadata(path.join("Contents/MacOS").join(&metadata.executable))
        .map_err(|_| "应用缺少可执行文件")?;
    if !executable.is_file()
        || executable.file_type().is_symlink()
        || executable.permissions().mode() & 0o111 == 0
    {
        return Err("应用可执行文件不可用".into());
    }
    let canonical = path.canonicalize().map_err(|_| "应用路径不可访问")?;
    let path = canonical
        .to_str()
        .ok_or("应用路径不是有效 UTF-8")?
        .to_owned();
    let known = KNOWN
        .iter()
        .find(|known| known.bundle_id == metadata.bundle_id);
    Ok(DevelopmentApp {
        id: format!("{APP_PREFIX}{path}"),
        name: known
            .map(|app| app.name.to_owned())
            .or(metadata.name)
            .unwrap_or_else(|| {
                canonical
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into()
            }),
        bundle_id: metadata.bundle_id,
        path,
        version: metadata.version,
        kind: known.map_or("EDITOR", |app| app.kind).into(),
        recommended_for: Vec::new(),
    })
}
fn custom_path<'a>(target: &str, settings: &'a Settings) -> Option<&'a str> {
    match target {
        "IDEA" => Some(settings.idea_path.as_str()),
        "VSCODE" => Some(settings.vscode_path.as_str()),
        "CURSOR" => Some(settings.cursor_path.as_str()),
        _ => None,
    }
    .filter(|path| !path.is_empty())
}
fn preference(target: &str, settings: &Settings, apps: &[DevelopmentApp]) -> AppPreference {
    let chosen = if let Some(path) = custom_path(target, settings) {
        let path = Path::new(path)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(path));
        apps.iter().find(|app| Path::new(&app.path) == path)
    } else if app_path(target).is_some() {
        apps.iter().find(|app| app.id == target)
    } else {
        KNOWN
            .iter()
            .filter(|known| known.legacy == target && !known.legacy.is_empty())
            .find_map(|known| apps.iter().find(|app| app.bundle_id == known.bundle_id))
    };
    AppPreference {
        id: chosen.map_or_else(|| target.into(), |app| app.id.clone()),
        name: chosen.map(|app| app.name.clone()).unwrap_or_else(|| {
            KNOWN
                .iter()
                .find(|known| known.legacy == target && !known.legacy.is_empty())
                .map(|known| known.name.to_owned())
                .or_else(|| {
                    app_path(target)
                        .and_then(|p| p.file_stem())
                        .map(|s| s.to_string_lossy().into())
                })
                .unwrap_or_else(|| "开发应用".into())
        }),
        available: chosen.is_some(),
    }
}
pub fn discover(settings: &Settings, projects: &[Project]) -> Result<DevelopmentApps, String> {
    discover_with_remembered(settings, projects, &[])
}
pub fn discover_with_remembered(
    settings: &Settings,
    projects: &[Project],
    remembered: &[RememberedApp],
) -> Result<DevelopmentApps, String> {
    if !cfg!(target_os = "macos") {
        return Err("应用识别目前仅支持 macOS".into());
    }
    let mut paths = Vec::new();
    #[cfg(target_os = "macos")]
    for known in KNOWN {
        paths.extend(macos::registered_paths(known.bundle_id));
    }
    let mut warnings = Vec::new();
    let mut roots = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications/Utilities"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        roots.push(PathBuf::from(home).join("Applications"));
    }
    let mut remaining = 4096;
    for root in roots {
        collect_bundles(&root, 0, &mut remaining, &mut paths, &mut warnings);
    }
    let explicit: HashSet<_> = projects
        .iter()
        .map(|p| p.default_ide.as_str())
        .chain(std::iter::once(settings.default_ide.as_str()))
        .filter_map(app_path)
        .map(Path::to_path_buf)
        .chain(
            [
                &settings.idea_path,
                &settings.vscode_path,
                &settings.cursor_path,
            ]
            .into_iter()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from),
        )
        .collect();
    paths.extend(explicit.iter().cloned());
    paths.extend(remembered.iter().map(|app| PathBuf::from(&app.path)));
    let mut apps = Vec::new();
    let mut seen = HashSet::new();
    for path in paths {
        if let Ok(app) = inspect_app(&path) {
            if (KNOWN.iter().any(|known| known.bundle_id == app.bundle_id)
                || explicit.contains(&path)
                || remembered
                    .iter()
                    .any(|saved| saved.path == app.path && saved.bundle_id == app.bundle_id))
                && seen.insert(app.id.clone())
            {
                apps.push(app);
            }
        }
    }
    for app in &mut apps {
        if let Some(known) = KNOWN.iter().find(|known| known.bundle_id == app.bundle_id) {
            app.recommended_for = projects
                .iter()
                .filter(|project| {
                    known
                        .project_types
                        .contains(&project.candidate.project_type.as_str())
                        || (known.bundle_id == "com.apple.dt.Xcode"
                            && has_xcode_project(Path::new(&project.candidate.canonical_path)))
                        || (known.bundle_id == "com.google.android.studio"
                            && Path::new(&project.candidate.canonical_path)
                                .join("app/src/main/AndroidManifest.xml")
                                .is_file())
                })
                .map(|p| p.candidate.id.clone())
                .collect();
        }
    }
    let global_default = preference(&settings.default_ide, settings, &apps);
    let project_defaults = projects
        .iter()
        .map(|p| {
            (
                p.candidate.id.clone(),
                preference(&p.default_ide, settings, &apps),
            )
        })
        .collect();
    apps.sort_by(|a, b| a.name.cmp(&b.name).then(a.path.cmp(&b.path)));
    Ok(DevelopmentApps {
        apps,
        global_default,
        project_defaults,
        warnings,
    })
}
fn collect_bundles(
    root: &Path,
    depth: usize,
    remaining: &mut usize,
    paths: &mut Vec<PathBuf>,
    warnings: &mut Vec<String>,
) {
    if *remaining == 0 {
        return;
    }
    let Ok(metadata) = fs::symlink_metadata(root) else {
        return;
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        warnings.push(format!("无法读取应用目录：{}", root.display()));
        return;
    };
    for entry in entries {
        if *remaining == 0 {
            warnings.push("应用目录达到 4096 条目上限，可手动选择遗漏的应用".into());
            break;
        }
        *remaining -= 1;
        let Ok(entry) = entry else {
            continue;
        };
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if !kind.is_dir() || kind.is_symlink() {
            continue;
        }
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "app") {
            paths.push(path);
        } else if depth < 2 && !entry.file_name().to_string_lossy().starts_with('.') {
            collect_bundles(&path, depth + 1, remaining, paths, warnings);
        }
    }
}
fn has_xcode_project(root: &Path) -> bool {
    fs::read_dir(root)
        .into_iter()
        .flatten()
        .take(256)
        .filter_map(Result::ok)
        .any(|entry| {
            entry
                .file_type()
                .is_ok_and(|kind| kind.is_dir() && !kind.is_symlink())
                && entry
                    .path()
                    .extension()
                    .is_some_and(|ext| ext == "xcodeproj" || ext == "xcworkspace")
        })
}
pub fn open(project: &Project, target: &str, settings: &Settings) -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Err("快捷打开目前仅支持 macOS".into());
    }
    let path = Path::new(&project.candidate.canonical_path);
    if !path.is_dir() {
        return Err("项目目录不存在，请检查磁盘或目录路径".into());
    }
    let path = path.to_str().ok_or("项目路径不是有效 UTF-8")?;
    if target == "FINDER" {
        command("/usr/bin/open", &["-R", path])?;
        return Ok(());
    }
    let app = if let Some(path) = app_path(target) {
        inspect_app(path)?
    } else if let Some(path) = custom_path(target, settings) {
        inspect_app(Path::new(path)).map_err(|e| format!("无法打开自定义应用：{e}"))?
    } else {
        if target != "TERMINAL" {
            validate_target(target)?;
        }
        let catalog = discover(settings, std::slice::from_ref(project))?;
        let preference = preference(target, settings, &catalog.apps);
        catalog
            .apps
            .into_iter()
            .find(|app| app.id == preference.id)
            .ok_or_else(|| {
                format!(
                    "无法打开 {}：应用未安装或已移除，请重新选择应用",
                    preference.name
                )
            })?
    };
    // Verified application path and project are separate arguments, never Shell input.
    command("/usr/bin/open", &["-a", &app.path, path])
        .map_err(|_| format!("无法打开 {}，请刷新应用列表并检查安装状态", app.name))?;
    Ok(())
}
