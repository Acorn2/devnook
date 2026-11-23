use devnook_core::{
    model::{ProjectEdit, ScanProgress},
    Core,
};
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};
use tempfile::TempDir;

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}
fn scan(core: &Core, root: &Path, depth: usize) -> ScanProgress {
    let id = core
        .scan_start(root.to_string_lossy().into(), depth)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        let p = core.scan_status(&id).unwrap();
        if p.status != "scanning" {
            return p;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn import(core: &Core, root: &Path) -> Vec<devnook_core::model::Project> {
    let p = scan(core, root, 0);
    assert!(!p.incomplete, "{:?}", p.warnings);
    core.import_projects(&p.id, p.candidates.iter().map(|c| c.id.clone()).collect())
        .unwrap()
}

#[test]
fn pure_native_clients_and_client_only_aggregators_are_excluded_in_both_modes() {
    let temp = TempDir::new().unwrap();
    for (file, text) in [
        ("ios/App.xcodeproj/project.pbxproj", "not read"),
        ("swift-mobile/Package.swift", "let p = Package(platforms: [.iOS(.v17)], products: [.library(name: \"Client\")])"),
        ("android/settings.gradle.kts", "include(\":app\",\":shared\")"),
        ("android/app/build.gradle.kts", "plugins { id(\"com.android.application\") }"),
        ("android/shared/build.gradle.kts", "plugins { id(\"java-library\") }"),
        ("android-alias/build.gradle.kts", "plugins { alias(libs.plugins.android.application) }"),
        ("javafx-maven/pom.xml", "<project><dependencies><dependency><artifactId>javafx-controls</artifactId></dependency></dependencies></project>"),
        ("flutter/pubspec.yaml", "name: client\ndependencies:\n  flutter:\n    sdk: 'flutter' # quoted SDK"),
        ("flutter/lib/main.dart", "void main() {}"),
        ("wpf/Client.slnx", "<Solution><Project Path=\"src/Gui/Gui.csproj\"/><Project Path=\"src/Shared/Shared.csproj\"/></Solution>"),
        ("wpf/src/Gui/Gui.csproj", "<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><UseWPF>true</UseWPF><OutputType>WinExe</OutputType></PropertyGroup></Project>"),
        ("wpf/src/Shared/Shared.csproj", "<Project Sdk=\"Microsoft.NET.Sdk\"/>"),
        ("winforms/App.csproj", "<Project><PropertyGroup><UseWindowsForms>true</UseWindowsForms><OutputType>Exe</OutputType></PropertyGroup></Project>"),
        ("maui/App.csproj", "<Project><PropertyGroup><UseMaui>true</UseMaui><TargetFrameworks>net9.0-ios;net9.0-android</TargetFrameworks></PropertyGroup></Project>"),
        ("javafx/build.gradle", "plugins { id 'application'; id 'org.openjfx.javafxplugin' }"),
        ("compose/build.gradle.kts", "plugins { id(\"org.jetbrains.compose\"); application }"),
        ("electron/package.json", r#"{"devDependencies":{"electron":"*"},"scripts":{"start":"electron ."}}"#),
        ("tauri/package.json", r#"{"devDependencies":{"@tauri-apps/cli":"*"},"scripts":{"dev":"tauri dev"}}"#),
        ("tauri/src-tauri/Cargo.toml", "[package]\nname='gui'\n[dependencies]\ntauri='2'"),
        ("tauri/src-tauri/src/main.rs", "fn main() {}"),
        ("rust-gui/Cargo.toml", "[package]\nname='gui'\n[dependencies]\neframe='0.33'"),
        ("rust-gui/src/main.rs", "fn main() {}"),
        ("qt/CMakeLists.txt", "project(gui)\nfind_package(Qt6 COMPONENTS Widgets REQUIRED)\nadd_executable(gui WIN32 main.cpp)\ntarget_link_libraries(gui PRIVATE Qt6::Widgets)"),
        ("python/requirements.txt", "PySide6==6.8"),
        ("python/main.py", "raise RuntimeError('SHOULD_NOT_RUN')"),
        ("unity/ProjectSettings/ProjectVersion.txt", "m_EditorVersion: 6000"),
        ("unity/App.csproj", "<Project><PropertyGroup><OutputType>Exe</OutputType></PropertyGroup></Project>"),
        ("unreal/Game.uproject", "{}"),
        ("unreal/CMakeLists.txt", "project(game)\nadd_executable(game main.cpp)"),
        ("godot/project.godot", "config_version=5"),
        ("godot/pyproject.toml", "[project]\nname='editor-helper'"),
        ("godot/main.py", "raise RuntimeError('SHOULD_NOT_RUN')"),
        ("go-gui/go.mod", "module example.com/gui\nrequire fyne.io/fyne/v2 v2.6.0"),
        ("go-gui/main.go", "package main\nfunc main() {}"),
    ] { write(temp.path(), file, text); }
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    for depth in [0, 10] {
        let p = scan(&core, temp.path(), depth);
        assert!(!p.incomplete, "{:?}", p.warnings);
        assert!(
            p.candidates.is_empty(),
            "depth={depth}: {:?}",
            p.candidates
                .iter()
                .map(|c| (&c.name, &c.project_type, &c.components))
                .collect::<Vec<_>>()
        );
    }
    assert!(core.projects().unwrap().is_empty() && core.services().unwrap().is_empty());
}

#[test]
fn swift_servers_override_native_metadata_and_client_review_does_not_hide_backend() {
    let temp = TempDir::new().unwrap();
    for dir in ["vapor", "hummingbird"] {
        write(
            temp.path(),
            &format!("{dir}/App.xcodeproj/project.pbxproj"),
            "not read",
        );
        let repo = if dir == "vapor" {
            "vapor/vapor"
        } else {
            "hummingbird-project/hummingbird"
        };
        write(temp.path(), &format!("{dir}/Package.swift"), &format!("let p = Package(dependencies: [.package(url: \"https://github.com/{repo}.git\", from: \"1.0.0\")], targets: [.executableTarget(name: \"Server\")])"));
    }
    write(temp.path(), "mixed/.git", "gitdir: not-followed");
    write(
        temp.path(),
        "mixed/mobile/App.xcodeproj/project.pbxproj",
        "not read",
    );
    write(temp.path(), "mixed/backend/requirements.txt", "fastapi");
    write(
        temp.path(),
        "same-directory/App.xcodeproj/project.pbxproj",
        "not read",
    );
    write(temp.path(), "same-directory/requirements.txt", "flask");
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let projects = import(&core, temp.path());
    assert_eq!(projects.len(), 4);
    for p in &projects {
        assert!(!p.candidate.needs_review);
    }
    assert_eq!(
        projects
            .iter()
            .find(|p| p.candidate.name == "same-directory")
            .unwrap()
            .candidate
            .project_type,
        "PYTHON"
    );
    let mixed = projects
        .iter()
        .find(|p| p.candidate.name == "mixed")
        .unwrap();
    assert_eq!(mixed.candidate.components.len(), 1);
    assert_eq!(mixed.candidate.components[0].name, "backend");
    for p in projects {
        assert_eq!(core.service_drafts(&p.candidate.id).unwrap().len(), 1);
    }
    assert!(core.services().unwrap().is_empty());
}

#[test]
fn mobile_desktop_and_flutter_packages_choose_only_the_development_server_script() {
    let temp = TempDir::new().unwrap();
    let mut expected_scripts = std::collections::BTreeMap::new();
    for (name, dependency, scripts, expected) in [
        (
            "expo",
            "expo",
            r#"{"ios":"expo run:ios","start":"npx expo start"}"#,
            "start",
        ),
        (
            "rn",
            "react-native",
            r#"{"dev":"react-native run-ios","start":"react-native start"}"#,
            "start",
        ),
        (
            "electron",
            "electron",
            r#"{"dev":"electron .","dev:web":"vite --host 127.0.0.1"}"#,
            "dev:web",
        ),
        (
            "tauri",
            "@tauri-apps/cli",
            r#"{"dev":"tauri dev","web":"vite"}"#,
            "web",
        ),
        (
            "capacitor",
            "@capacitor/core",
            r#"{"dev":"next dev"}"#,
            "dev",
        ),
        (
            "flutter-web",
            "",
            r#"{"dev":"flutter run -d web-server"}"#,
            "dev",
        ),
        (
            "electron-alias",
            "electron",
            r#"{"dev":"pnpm run web","web":"vite","desktop":"electron ."}"#,
            "dev",
        ),
    ] {
        write(
            temp.path(),
            &format!("{name}/package.json"),
            &format!(r#"{{"dependencies":{{"{dependency}":"*"}},"scripts":{scripts}}}"#),
        );
        if name == "flutter-web" {
            write(
                temp.path(),
                "flutter-web/pubspec.yaml",
                "name: app\ndependencies:\n  flutter:\n    sdk: flutter",
            );
            write(temp.path(), "flutter-web/lib/main.dart", "void main(){}");
        }
        expected_scripts.insert(name.to_owned(), expected);
    }
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let projects = import(&core, temp.path());
    assert_eq!(projects.len(), expected_scripts.len());
    for p in &projects {
        let expected = expected_scripts[&p.candidate.name];
        let drafts = core.service_drafts(&p.candidate.id).unwrap();
        assert_eq!(drafts.len(), 1, "{}", p.candidate.name);
        assert_eq!(drafts[0].command, format!("npm run {expected}"));
        assert!(!drafts[0].definition.shell_mode);
    }
    assert!(core.services().unwrap().is_empty());
}

#[test]
fn node_cli_packages_suggest_explicit_web_launches() {
    let temp = TempDir::new().unwrap();
    for &(name, manifest, _) in NODE_LAUNCH_CASES {
        write(temp.path(), &format!("{name}/package.json"), manifest);
    }
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let projects = import(&core, temp.path());
    assert_eq!(projects.len(), NODE_LAUNCH_CASES.len());
    for &(name, _, expected) in NODE_LAUNCH_CASES {
        let project = projects.iter().find(|p| p.candidate.name == name).unwrap();
        assert_eq!(project.candidate.runnable, expected.is_some(), "{name}");
        let drafts = core.service_drafts(&project.candidate.id).unwrap();
        assert_eq!(drafts.len(), usize::from(expected.is_some()), "{name}");
        if let Some(script) = expected {
            assert_eq!(drafts[0].definition.executable, "npm", "{name}");
            assert_eq!(drafts[0].definition.args, ["run", script], "{name}");
            assert_eq!(drafts[0].source, format!("package.json · scripts.{script}"));
            assert!(!drafts[0].definition.shell_mode);
        }
    }
    assert!(core.services().unwrap().is_empty());
}

const NODE_LAUNCH_CASES: &[(&str, &str, Option<&str>)] = &[
    (
        "cli-and-workbench",
        r#"{"bin":{"tool":"index.js"},"dependencies":{"commander":"*","@vue/runtime-dom":"*"},"scripts":{"start":"node index.js","workspace":"node index.js workspace","test":"node --test"}}"#,
        Some("workspace"),
    ),
    (
        "cli-and-vite",
        r#"{"bin":"./cli.mjs","scripts":{"dev":"npm start","start":"npm run cli","cli":"node cli.mjs","web":"vite"}}"#,
        Some("web"),
    ),
    (
        "cli-only",
        r#"{"bin":"index.js","scripts":{"start":"node index.js","workspace":"node index.js workspace"}}"#,
        None,
    ),
    (
        "workspace-manager",
        r#"{"bin":"index.js","dependencies":{"vue":"*"},"scripts":{"start":"node index.js","workspace":"node index.js workspace list"}}"#,
        None,
    ),
    (
        "workspace-help",
        r#"{"bin":"index.js","dependencies":{"vue":"*"},"scripts":{"workspace":"node ./index.js workspace --help"}}"#,
        None,
    ),
    (
        "workspace-server",
        r#"{"scripts":{"workspace":"vite"}}"#,
        Some("workspace"),
    ),
    (
        "ordinary-node-server",
        r#"{"scripts":{"start":"node index.js","workspace":"node tools/workspace.js"}}"#,
        Some("start"),
    ),
];

#[test]
fn native_launches_and_compound_gui_scripts_never_become_server_suggestions() {
    let temp = TempDir::new().unwrap();
    for (name, script) in [
        ("electron-compound", "electron . && node server.js"),
        ("expo-open-ios", "expo start --ios"),
        ("expo-run", "expo run:ios"),
        ("react-native-run", "react-native run-android"),
    ] {
        write(
            temp.path(),
            &format!("{name}/package.json"),
            &format!(r#"{{"dependencies":{{"expo":"*"}},"scripts":{{"start":"{script}"}}}}"#),
        );
    }
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    assert!(scan(&core, temp.path(), 0).candidates.is_empty());
}

#[test]
fn same_language_native_and_server_modules_keep_only_server_and_shared_components() {
    let temp = TempDir::new().unwrap();
    for (file,text) in [
        ("android/settings.gradle.kts","include(\":app\",\":backend\")"),
        ("android/app/build.gradle.kts","plugins { id(\"com.android.application\") }"),
        ("android/backend/build.gradle.kts","plugins { id(\"org.springframework.boot\") }"),
        ("dotnet/Root.slnx","<Solution><Project Path=\"src/Desktop/Desktop.csproj\"/><Project Path=\"src/Api/Api.csproj\"/><Project Path=\"src/Shared/Shared.csproj\"/></Solution>"),
        ("dotnet/src/Desktop/Desktop.csproj","<Project><PropertyGroup><UseWindowsForms>true</UseWindowsForms><OutputType>WinExe</OutputType></PropertyGroup></Project>"),
        ("dotnet/src/Api/Api.csproj","<Project Sdk=\"Microsoft.NET.Sdk.Web\"/>"),
        ("dotnet/src/Shared/Shared.csproj","<Project Sdk=\"Microsoft.NET.Sdk\"/>"),
        ("qt/CMakeLists.txt","project(gui)\nfind_package(Qt6 COMPONENTS Widgets)\nadd_executable(gui WIN32 main.cpp)\nadd_subdirectory(backend)"),
        ("qt/backend/CMakeLists.txt","project(server)\nadd_executable(server main.cpp)"),
    ]{write(temp.path(),file,text);}
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let projects = import(&core, temp.path());
    assert_eq!(projects.len(), 3);
    for p in &projects {
        assert!(p
            .candidate
            .components
            .iter()
            .all(|c| !c.name.contains("Desktop") && c.name != "app"));
        let drafts = core.service_drafts(&p.candidate.id).unwrap();
        assert_eq!(drafts.len(), 1, "{}", p.candidate.name);
        if p.candidate.name == "android" {
            assert_eq!(drafts[0].command, "gradle bootRun --no-daemon");
        }
    }
}

#[test]
fn cli_workers_qt_networking_optional_gui_and_comments_are_not_native_apps() {
    let temp = TempDir::new().unwrap();
    for (file,text) in [
        ("rust/Cargo.toml","[package]\nname='server'\n[dependencies]\naxum='0.8'\neframe={version='0.33',optional=true}"),
        ("rust/src/main.rs","fn main(){}"),
        ("dotnet/App.csproj","<Project Sdk=\"Microsoft.NET.Sdk\"><!-- <UseWPF>true</UseWPF> --><PropertyGroup><OutputType>Exe</OutputType></PropertyGroup></Project>"),
        ("worker/App.csproj","<Project Sdk=\"Microsoft.NET.Sdk.Worker\"/>"),
        ("qt/CMakeLists.txt","project(network)\nfind_package(Qt6 COMPONENTS Core Network REQUIRED)\nif(WIN32)\nendif()\n# target_link_libraries(gui Qt6::Widgets)\nadd_executable(server main.cpp)"),
        ("python/pyproject.toml","[project]\nname='worker'\n[project.scripts]\nworker='worker:main'\n[project.optional-dependencies]\ngui=['PySide6']"),
        ("go/go.mod","module example.com/worker\n// require fyne.io/fyne/v2 v2.6.0"),
        ("go/main.go","package main\nfunc main(){}"),
        ("node/package.json",r#"{"scripts":{"start":"node worker.js"}}"#),
        ("node/worker.js","throw new Error('SHOULD_NOT_RUN')"),
        ("swift/Package.swift","// .iOS(.v17)\nlet p=Package(targets:[.executableTarget(name:\"Worker\")])"),
    ]{write(temp.path(),file,text);}
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let p = scan(&core, temp.path(), 0);
    assert!(!p.incomplete, "{:?}", p.warnings);
    assert_eq!(
        p.candidates.len(),
        8,
        "{:?}",
        p.candidates.iter().map(|c| &c.name).collect::<Vec<_>>()
    );
    assert!(core.services().unwrap().is_empty());
}

#[test]
fn independent_nested_services_and_existing_archives_are_preserved() {
    let temp = TempDir::new().unwrap();
    write(temp.path(), "app/.git", "gitdir: never-follow");
    write(temp.path(), "app/App.xcodeproj/project.pbxproj", "not read");
    write(
        temp.path(),
        "app/tools/backend/.git",
        "gitdir: never-follow",
    );
    write(
        temp.path(),
        "app/tools/backend/package.json",
        r#"{"scripts":{"start":"node server.js"}}"#,
    );
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let projects = import(&core, temp.path());
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].candidate.name, "backend");
    let id = projects[0].candidate.id.clone();
    core.edit_project(ProjectEdit {
        id: id.clone(),
        name: "保留名称".into(),
        default_ide: "CURSOR".into(),
        is_favorite: true,
        category: "保留分类".into(),
    })
    .unwrap();
    let definition = core.service_drafts(&id).unwrap().remove(0).definition;
    let saved = core
        .save_service(devnook_core::service_model::ServiceSave {
            definition,
            secret_values: Default::default(),
        })
        .unwrap();
    // An archive that later becomes client-only is hidden from discovery, never deleted.
    write(
        temp.path(),
        "app/tools/backend/package.json",
        r#"{"dependencies":{"electron":"*"},"scripts":{"start":"electron ."}}"#,
    );
    let p = scan(&core, temp.path(), 0);
    assert!(p.candidates.is_empty());
    let stored = core.projects().unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].candidate.id, id);
    assert_eq!(stored[0].candidate.name, "保留名称");
    assert!(stored[0].is_favorite);
    assert_eq!(stored[0].default_ide, "CURSOR");
    assert!(core.service_drafts(&id).unwrap().is_empty());
    let services = core.services().unwrap();
    assert_eq!(services.len(), 1);
    assert_eq!(services[0].definition.revision, saved.revision);
    assert!(services[0].current_run.is_none());
}

#[test]
fn linked_client_markers_do_not_change_real_host_project_eligibility() {
    let temp = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    write(
        temp.path(),
        "worker/build.gradle.kts",
        "plugins { application }",
    );
    write(
        outside.path(),
        "src/main/AndroidManifest.xml",
        "not followed",
    );
    write(outside.path(), "ProjectVersion.txt", "not followed");
    std::os::unix::fs::symlink(outside.path(), temp.path().join("worker/app")).unwrap();
    std::os::unix::fs::symlink(outside.path(), temp.path().join("worker/App.xcodeproj")).unwrap();
    std::os::unix::fs::symlink(outside.path(), temp.path().join("worker/ProjectSettings")).unwrap();
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let projects = import(&core, temp.path());
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].candidate.project_type, "JAVA");
    assert_eq!(
        core.service_drafts(&projects[0].candidate.id).unwrap()[0].command,
        "gradle run --no-daemon"
    );
}
