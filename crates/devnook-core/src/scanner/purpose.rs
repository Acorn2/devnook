//! Conservative, read-only separation of native clients and host development services.
use super::{
    manifests::{literal_calls, static_tokens},
    plain_file, read_manifest,
};
use quick_xml::{events::Event, Reader, XmlVersion};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Component, Path},
};

#[derive(Default)]
pub(super) struct Purpose {
    pub clients: HashSet<&'static str>,
    pub hosts: HashSet<&'static str>,
    pub node_script: Option<String>,
}
impl Purpose {
    pub fn suppresses(&self, kind: &str) -> bool {
        self.clients.contains(kind) && !self.hosts.contains(kind)
    }
}

fn at(root: &Path, relative: &str) -> bool {
    let mut path = root.to_path_buf();
    let parts: Vec<_> = Path::new(relative).components().collect();
    for (index, part) in parts.iter().enumerate() {
        let Component::Normal(name) = part else {
            return false;
        };
        path.push(name);
        let Ok(meta) = fs::symlink_metadata(&path) else {
            return false;
        };
        if meta.file_type().is_symlink() || (index + 1 != parts.len() && !meta.is_dir()) {
            return false;
        }
    }
    plain_file(&path)
}
fn dependency(pkg: &Value, names: &[&str]) -> bool {
    ["dependencies", "devDependencies"].iter().any(|key| {
        pkg.get(key)
            .is_some_and(|v| names.iter().any(|name| v.get(name).is_some()))
    })
}
fn command_words(text: &str) -> Option<Vec<String>> {
    let mut text = text.trim();
    // Support literal leading assignments without interpreting expansion or shell expressions.
    while let Some((first, rest)) = text.split_once(char::is_whitespace) {
        let Some((key, value)) = first.split_once('=') else {
            break;
        };
        if key.is_empty()
            || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            || value.contains(['"', '\'', '$', '`'])
        {
            return None;
        }
        text = rest.trim_start();
    }
    let spec = crate::service_drafts::parse_command(text).ok()?;
    let mut words: Vec<_> = std::iter::once(spec.executable).chain(spec.args).collect();
    while words.first().is_some_and(|w| {
        ["env", "cross-env", "npx", "pnpm", "yarn", "bunx"].contains(
            &Path::new(w)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .as_ref(),
        )
    }) {
        words.remove(0);
        while words
            .first()
            .is_some_and(|w| w.starts_with('-') || w == "exec" || w == "dlx" || w.contains('='))
        {
            words.remove(0);
        }
    }
    (!words.is_empty()).then_some(words)
}
fn program(words: &[String]) -> &str {
    Path::new(&words[0])
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
}
fn native_command(text: &str) -> bool {
    if command_words(text).is_none()
        && static_tokens(text).first().is_some_and(|s| {
            [
                "electron",
                "tauri",
                "xcodebuild",
                "electron-forge",
                "electron-vite",
            ]
            .contains(&s.as_str())
        })
    {
        return true;
    }
    command_words(text).is_some_and(|words| {
        let cmd = program(&words);
        matches!(
            cmd,
            "electron"
                | "electron-forge"
                | "electron-builder"
                | "electron-vite"
                | "tauri"
                | "xcodebuild"
                | "unity"
                | "godot"
                | "ns"
        ) || (cmd == "react-native" && words.get(1).is_some_and(|v| v.starts_with("run-")))
            || (cmd == "expo"
                && words
                    .get(1)
                    .is_some_and(|v| v.starts_with("run:") || v == "prebuild"))
            || (cmd == "flutter"
                && !words
                    .iter()
                    .any(|v| v == "web-server" || v == "--device-id=web-server"))
    })
}
fn node_client(pkg: &Value) -> bool {
    dependency(
        pkg,
        &[
            "expo",
            "react-native",
            "electron",
            "electron-vite",
            "@electron-forge/cli",
            "@tauri-apps/cli",
            "@tauri-apps/api",
            "@capacitor/core",
            "@nativescript/core",
        ],
    ) || pkg
        .get("scripts")
        .and_then(Value::as_object)
        .is_some_and(|scripts| {
            scripts
                .values()
                .filter_map(Value::as_str)
                .any(native_command)
        })
}
fn host_command(pkg: &Value, text: &str, seen: &mut HashSet<String>) -> bool {
    let Some(words) = command_words(text) else {
        return false;
    };
    let cmd = program(&words);
    let sub = words.get(1).map(String::as_str).unwrap_or("");
    let alias = if cmd == "npm" && sub == "run" {
        words.get(2)
    } else if cmd == "run" {
        words.get(1)
    } else if words.len() == 1 && pkg.get("scripts").and_then(|v| v.get(cmd)).is_some() {
        words.first()
    } else {
        None
    };
    if let Some(script) = alias {
        if words
            .iter()
            .any(|v| ["--ios", "--android", "-i", "-a"].contains(&v.as_str()))
        {
            return false;
        }
        if seen.len() >= 8 || !seen.insert(script.clone()) {
            return false;
        }
        return pkg
            .get("scripts")
            .and_then(|v| v.get(script))
            .and_then(Value::as_str)
            .is_some_and(|v| host_command(pkg, v, seen));
    }
    match cmd {
        "expo" => {
            sub == "start"
                && !words
                    .iter()
                    .any(|v| ["--ios", "--android", "-i", "-a"].contains(&v.as_str()))
        }
        "react-native" => sub == "start",
        "flutter" => {
            sub == "run"
                && words
                    .iter()
                    .any(|v| v == "web-server" || v == "--device-id=web-server")
        }
        "vite" => !words.iter().any(|v| v == "build"),
        "next" | "nuxt" | "nuxi" | "astro" => ["dev", "start", "preview"].contains(&sub),
        "react-scripts" => sub == "start",
        "ng" | "vue-cli-service" => sub == "serve",
        "webpack" => sub == "serve",
        "http-server" | "serve" | "parcel" | "webpack-dev-server" => true,
        "node" | "nodemon" | "tsx" | "ts-node" | "ts-node-dev" | "bun" => {
            words.iter().skip(1).any(|v| {
                let path = Path::new(v);
                let script_file = path.extension().is_some_and(|v| {
                    ["js", "mjs", "cjs", "ts", "tsx"].contains(&v.to_string_lossy().as_ref())
                });
                let service =
                    |s: &str| ["server", "backend", "api", "worker", "metro"].contains(&s);
                script_file
                    && !v.starts_with('-')
                    && (path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .is_some_and(service)
                        || path
                            .parent()
                            .is_some_and(|p| p.iter().any(|s| service(&s.to_string_lossy()))))
            })
        }
        _ => false,
    }
}
fn choose_node_script(pkg: &Value, strict: bool) -> Option<String> {
    [
        "dev",
        "start",
        "serve",
        "dev:web",
        "start:web",
        "web",
        "dev:server",
        "start:server",
        "server",
    ]
    .into_iter()
    .find(|key| {
        pkg.get("scripts")
            .and_then(|v| v.get(key))
            .and_then(Value::as_str)
            .is_some_and(|text| {
                !text.trim().is_empty()
                    && !native_command(text)
                    && (!strict || host_command(pkg, text, &mut HashSet::new()))
            })
    })
    .map(str::to_owned)
}
fn required_toml_deps(value: &toml::Value) -> Vec<String> {
    value
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .into_iter()
        .flat_map(|v| v.iter())
        .filter(|(_, value)| {
            !value
                .get("optional")
                .and_then(toml::Value::as_bool)
                .unwrap_or(false)
        })
        .map(|(name, value)| {
            value
                .get("package")
                .and_then(toml::Value::as_str)
                .unwrap_or(name)
                .to_owned()
        })
        .collect()
}
fn python_deps(root: &Path) -> (Vec<String>, bool, bool) {
    let value = read_manifest(&root.join("pyproject.toml"))
        .and_then(|s| toml::from_str::<toml::Value>(&s).ok());
    let project = value.as_ref().and_then(|v| v.get("project"));
    let mut deps: Vec<_> = project
        .and_then(|v| v.get("dependencies"))
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_str)
        .map(str::to_owned)
        .collect();
    if let Some(text) = read_manifest(&root.join("requirements.txt")) {
        deps.extend(
            text.lines()
                .filter(|l| !l.trim().starts_with('#'))
                .map(str::to_owned),
        );
    }
    let deps = deps
        .into_iter()
        .map(|v| {
            v.trim()
                .split(['=', '<', '>', '~', '!', '[', ';', ' ', '@'])
                .next()
                .unwrap_or("")
                .to_ascii_lowercase()
                .replace('_', "-")
        })
        .collect();
    let scripts = project
        .and_then(|v| v.get("scripts"))
        .and_then(toml::Value::as_table)
        .is_some_and(|v| !v.is_empty());
    let gui = project
        .and_then(|v| v.get("gui-scripts"))
        .and_then(toml::Value::as_table)
        .is_some_and(|v| !v.is_empty());
    (deps, scripts, gui)
}
pub(crate) fn has_fastapi(root: &Path) -> bool {
    python_deps(root).0.iter().any(|dep| dep == "fastapi")
}
fn xml_properties(text: &str) -> Option<(String, BTreeMap<String, String>)> {
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut sdk = String::new();
    let mut properties = BTreeMap::new();
    let mut name = String::new();
    loop {
        match reader.read_event().ok()? {
            Event::Start(e) | Event::Empty(e) => {
                name = e.local_name().as_ref().to_ascii_lowercase();
                if name == "project" {
                    for attr in e.attributes() {
                        let attr = attr.ok()?;
                        if attr.key.as_ref() == "Sdk" {
                            sdk = attr
                                .normalized_value(XmlVersion::Implicit1_0)
                                .ok()?
                                .to_ascii_lowercase();
                        }
                    }
                }
            }
            Event::Text(e) => {
                properties
                    .entry(name.clone())
                    .or_insert_with(String::new)
                    .push_str(&format!(
                        "{}\n",
                        e.xml_content(XmlVersion::Implicit1_0).to_ascii_lowercase()
                    ));
            }
            Event::End(_) => name.clear(),
            Event::DocType(_) => return None,
            Event::Eof => break,
            _ => {}
        }
    }
    Some((sdk, properties))
}

pub(super) fn analyze(root: &Path, names: &[String]) -> Purpose {
    let mut result = Purpose::default();
    let pkg = read_manifest(&root.join("package.json"))
        .and_then(|s| serde_json::from_str::<Value>(&s).ok());
    let xcode = names.iter().any(|v| {
        (v.ends_with(".xcodeproj") || v.ends_with(".xcworkspace"))
            && fs::symlink_metadata(root.join(v))
                .is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
    });
    if xcode {
        result.clients.insert("SWIFT");
    }
    let game = at(root, "ProjectSettings/ProjectVersion.txt")
        || at(root, "project.godot")
        || names
            .iter()
            .any(|v| v.ends_with(".uproject") && at(root, v));
    if game {
        result.clients.extend([
            "DOTNET", "CPP", "NODE", "PYTHON", "JAVA", "RUST", "SWIFT", "DART", "GO",
        ]);
    }
    for file in ["build.gradle", "build.gradle.kts", "pom.xml"] {
        let Some(text) = read_manifest(&root.join(file)) else {
            continue;
        };
        let ts = static_tokens(&text);
        let has = |needle: &str| ts.iter().any(|s| s == needle);
        let xml = (file == "pom.xml").then(|| xml_properties(&text)).flatten();
        let artifacts: Vec<_> = xml
            .as_ref()
            .and_then(|(_, p)| p.get("artifactid"))
            .map(|v| v.split_whitespace().collect())
            .unwrap_or_default();
        if artifacts
            .iter()
            .any(|v| ["javafx-controls", "javafx-fxml", "javafx-graphics"].contains(v))
            || [
                "com.android.application",
                "com.android.library",
                "org.jetbrains.kotlin.android",
                "org.jetbrains.compose",
                "org.openjfx.javafxplugin",
                "javafx-controls",
                "libs.plugins.android.application",
                "libs.plugins.android.library",
            ]
            .iter()
            .any(|s| has(s))
        {
            result.clients.insert("JAVA");
        }
        if has("org.springframework.boot")
            || artifacts.iter().any(|v| v.starts_with("spring-boot"))
            || has("io.ktor.plugin")
        {
            result.hosts.insert("JAVA");
        }
    }
    if at(root, "src/main/AndroidManifest.xml") || at(root, "app/src/main/AndroidManifest.xml") {
        result.clients.insert("JAVA");
    }
    if let Some(text) = read_manifest(&root.join("Package.swift")) {
        let ts = static_tokens(&text);
        let executable = ts
            .iter()
            .any(|s| [".executableTarget", ".executable"].contains(&s.as_str()));
        let server = ts.iter().any(|s| {
            [
                "github.com/vapor/vapor",
                "github.com/hummingbird-project/hummingbird",
                "github.com/kitura/kitura",
            ]
            .iter()
            .any(|repo| s.to_ascii_lowercase().contains(repo))
        });
        if ts
            .iter()
            .any(|s| [".iOS", ".tvOS", ".watchOS", ".visionOS"].contains(&s.as_str()))
        {
            result.clients.insert("SWIFT");
        }
        if executable && (server || !result.clients.contains("SWIFT")) {
            result.hosts.insert("SWIFT");
        }
    }
    if let Some(value) =
        read_manifest(&root.join("Cargo.toml")).and_then(|s| toml::from_str::<toml::Value>(&s).ok())
    {
        let deps = required_toml_deps(&value);
        let gui = deps.iter().any(|v| {
            ["tauri", "eframe", "iced", "slint", "gtk", "gtk4", "druid"].contains(&v.as_str())
        });
        if gui {
            result.clients.insert("RUST");
        }
        let server = deps
            .iter()
            .any(|v| ["axum", "actix-web", "rocket", "warp", "tonic"].contains(&v.as_str()));
        if !gui && (!game || server) && (at(root, "src/main.rs") || value.get("bin").is_some()) {
            result.hosts.insert("RUST");
        }
    }
    if let Some(text) = read_manifest(&root.join("go.mod")) {
        let ts = static_tokens(&text);
        let gui = ts.iter().any(|s| {
            s.starts_with("fyne.io/fyne")
                || s == "gioui.org"
                || s.starts_with("github.com/hajimehoshi/ebiten")
        });
        let server = ts.iter().any(|s| {
            [
                "github.com/gin-gonic/gin",
                "github.com/labstack/echo/v4",
                "github.com/gofiber/fiber/v2",
                "github.com/go-chi/chi/v5",
            ]
            .contains(&s.as_str())
        });
        if gui {
            result.clients.insert("GO");
        }
        if !gui && (!game || server) && at(root, "main.go") {
            result.hosts.insert("GO");
        }
    }
    let (deps, scripts, gui_scripts) = python_deps(root);
    let gui = gui_scripts
        || deps.iter().any(|v| {
            [
                "pyside6",
                "pyside2",
                "pyqt6",
                "pyqt5",
                "kivy",
                "wxpython",
                "dearpygui",
            ]
            .contains(&v.as_str())
        });
    if gui {
        result.clients.insert("PYTHON");
    }
    let server = deps.iter().any(|v| {
        [
            "fastapi", "django", "flask", "litestar", "sanic", "aiohttp", "tornado", "celery",
            "rq", "dramatiq",
        ]
        .contains(&v.as_str())
    });
    if server || at(root, "manage.py") || (!gui && !game && (scripts || at(root, "main.py"))) {
        result.hosts.insert("PYTHON");
    }
    if let Some(text) = read_manifest(&root.join("pubspec.yaml")) {
        let flutter = text
            .lines()
            .any(|l| static_tokens(l) == ["sdk:", "flutter"]);
        if flutter {
            result.clients.insert("DART");
        }
        let server = text.lines().any(|l| {
            ["shelf:", "dart_frog:", "serverpod:"]
                .iter()
                .any(|key| l.trim().starts_with(key))
        });
        if (server || (!flutter && !game))
            && (at(root, "bin/main.dart") || at(root, "bin/server.dart"))
        {
            result.hosts.insert("DART");
        }
    }
    for name in names.iter().filter(|s| {
        [".csproj", ".fsproj", ".vbproj"]
            .iter()
            .any(|suffix| s.ends_with(suffix))
    }) {
        let Some((sdk, props)) = read_manifest(&root.join(name)).and_then(|s| xml_properties(&s))
        else {
            continue;
        };
        let client = ["usewpf", "usewindowsforms", "usemaui", "usewinui"]
            .iter()
            .any(|v| props.get(*v).is_some_and(|v| v.trim() == "true"))
            || props
                .get("outputtype")
                .is_some_and(|v| v.trim() == "winexe")
            || ["targetframework", "targetframeworks"].iter().any(|key| {
                props.get(*key).is_some_and(|v| {
                    ["-ios", "-android", "-maccatalyst", "-tvos"]
                        .iter()
                        .any(|s| v.contains(s))
                })
            });
        if client {
            result.clients.insert("DOTNET");
        }
        if sdk.contains("microsoft.net.sdk.web")
            || sdk.contains("microsoft.net.sdk.worker")
            || (!client && !game && props.get("outputtype").is_some_and(|v| v.trim() == "exe"))
        {
            result.hosts.insert("DOTNET");
        }
    }
    for file in ["CMakeLists.txt", "meson.build"] {
        let Some(text) = read_manifest(&root.join(file)) else {
            continue;
        };
        let ts = static_tokens(&text);
        let qt = ts.iter().any(|s| s.contains("Qt5") || s.contains("Qt6"))
            && ts.iter().any(|s| {
                [
                    "Widgets",
                    "Quick",
                    "Qml",
                    "Qt6::Widgets",
                    "Qt5::Widgets",
                    "Qt6::Quick",
                    "Qt5::Quick",
                    "Qt6::Qml",
                    "Qt5::Qml",
                ]
                .contains(&s.as_str())
            });
        let window_executable = literal_calls(&text, "add_executable")
            .iter()
            .flatten()
            .any(|s| ["WIN32", "MACOSX_BUNDLE"].contains(&s.as_str()));
        let gui = qt
            || window_executable
            || ts.iter().any(|s| {
                [
                    "Qt6::Widgets",
                    "Qt5::Widgets",
                    "Qt6::Quick",
                    "Qt5::Quick",
                    "Qt6::Qml",
                    "Qt5::Qml",
                    "SDL2",
                    "SFML",
                    "raylib",
                    "wxWidgets",
                ]
                .contains(&s.as_str())
            });
        if gui {
            result.clients.insert("CPP");
        }
        if !gui
            && !game
            && ts
                .iter()
                .any(|s| ["add_executable", "executable"].contains(&s.as_str()))
        {
            result.hosts.insert("CPP");
        }
    }
    if pkg.as_ref().is_some_and(node_client) {
        result.clients.insert("NODE");
    }
    if let Some(pkg) = &pkg {
        result.node_script = choose_node_script(pkg, !result.clients.is_empty());
        if result.node_script.is_some() {
            result.hosts.insert("NODE");
        }
    }
    if at(root, "Gemfile") && at(root, "config/application.rb") {
        result.hosts.insert("RUBY");
    }
    if at(root, "artisan") || at(root, "bin/console") {
        result.hosts.insert("PHP");
    }
    result
}

pub(crate) fn node_script(path: &Path) -> Option<String> {
    let names: Vec<_> = fs::read_dir(path)
        .into_iter()
        .flatten()
        .take(4096)
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| !t.is_symlink()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    analyze(path, &names).node_script
}
