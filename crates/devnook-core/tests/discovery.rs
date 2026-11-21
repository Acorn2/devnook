use devnook_core::{
    model::{Project, ProjectEdit, ScanProgress},
    service_model::{ReadinessKind, ServiceSave},
    Core,
};
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};
use tempfile::TempDir;

fn write(root: &Path, path: &str, content: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
fn scan(core: &Core, root: &Path, depth: usize) -> ScanProgress {
    let id = core
        .scan_start(root.to_string_lossy().into(), depth)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let p = core.scan_status(&id).unwrap();
        if p.status != "scanning" {
            return p;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn fullstack(root: &Path) {
    write(root, "app/.git", "gitdir: do-not-follow");
    write(
        root,
        "app/frontend/package.json",
        r#"{"scripts":{"dev":"touch SHOULD_NOT_RUN"},"dependencies":{"react":"*"}}"#,
    );
    write(
        root,
        "app/backend/pom.xml",
        "<project>spring-boot</project>",
    );
    write(
        root,
        "app/packages/shared/package.json",
        r#"{"name":"shared"}"#,
    );
}

#[test]
fn launch_suggestions_inherit_package_manager_and_refresh_without_registering_services() {
    let temp = TempDir::new().unwrap();
    fullstack(temp.path());
    write(
        temp.path(),
        "app/package.json",
        r#"{"packageManager":"pnpm@10.0.0"}"#,
    );
    write(temp.path(), "app/python/requirements.txt", "fastapi");
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let progress = scan(&core, temp.path(), 0);
    let projects = core
        .import_projects(&progress.id, vec![progress.candidates[0].id.clone()])
        .unwrap();
    let id = &projects[0].candidate.id;
    let drafts = core.service_drafts(id).unwrap();
    let front = drafts
        .iter()
        .find(|d| d.definition.name == "frontend")
        .unwrap();
    assert_eq!(front.command, "pnpm run dev");
    assert!(!front.definition.shell_mode);
    assert!(drafts.iter().any(|d| d.command == "mvn spring-boot:run"));
    assert!(drafts
        .iter()
        .any(|d| d.definition.name == "python" && d.command.is_empty()));
    assert_eq!(drafts.len(), 3); // No shared-library or root service without launch evidence.
    assert!(core.services().unwrap().is_empty());
    assert!(!temp.path().join("app/frontend/SHOULD_NOT_RUN").exists());
    write(
        temp.path(),
        "app/frontend/package.json",
        r#"{"scripts":{"start":"node app.js"}}"#,
    );
    assert!(core
        .service_drafts(id)
        .unwrap()
        .iter()
        .any(|d| d.definition.name == "frontend" && d.command == "pnpm run start"));
    let front_path = temp.path().join("app/frontend");
    let renamed = temp.path().join("original-frontend");
    fs::rename(&front_path, &renamed).unwrap();
    std::os::unix::fs::symlink(&renamed, &front_path).unwrap();
    assert!(!core
        .service_drafts(id)
        .unwrap()
        .iter()
        .any(|d| d.definition.name == "frontend"));
}

#[test]
fn fastapi_module_and_vite_have_separate_drafts_even_after_saving_vite() {
    let temp = TempDir::new().unwrap();
    let project = temp.path().join("archive");
    write(&project, ".git", "gitdir: do-not-follow");
    write(
        &project,
        "requirements.txt",
        "fastapi>=0.115\nuvicorn[standard]>=0.30\n",
    );
    write(&project, "server/__init__.py", "");
    write(
        &project,
        "server/__main__.py",
        r#"import argparse
import uvicorn
from pathlib import Path

def main():
    Path('SHOULD_NOT_RUN').touch()
    parser = argparse.ArgumentParser(description="API")
    parser.add_argument("--root", default="posts")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8052)
    args = parser.parse_args()
    if args.host != '127.0.0.1':
        parser.error('loopback only')
    uvicorn.run(
        create_app(args.root, database_url=None),
        host=args.host,
        port=args.port,
    )

if __name__ == "__main__":
    main()
"#,
    );
    write(
        &project,
        "run.py",
        "from pathlib import Path\nPath('SHOULD_NOT_RUN').touch()\n",
    );
    write(
        &project,
        "web/package.json",
        r#"{"scripts":{"dev":"vite"}}"#,
    );
    let project = project.canonicalize().unwrap();
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let progress = scan(&core, &project, 0);
    assert_eq!(progress.candidates.len(), 1);
    let projects = core
        .import_projects(&progress.id, vec![progress.candidates[0].id.clone()])
        .unwrap();
    let id = &projects[0].candidate.id;
    let drafts = core.service_drafts(id).unwrap();
    assert_eq!(drafts.len(), 2);
    let frontend = drafts
        .iter()
        .find(|draft| draft.command == "npm run dev")
        .unwrap();
    let backend = drafts
        .iter()
        .find(|draft| draft.command == "python3 -m server")
        .unwrap();
    assert_eq!(backend.source, "server/__main__.py");
    assert!(backend.independent_entrypoint);
    assert!(!frontend.independent_entrypoint);
    assert_eq!(backend.definition.name, "FastAPI");
    assert_eq!(backend.definition.runtime_type, "PYTHON");
    assert_eq!(Path::new(&backend.definition.working_directory), project);
    assert_eq!(
        Path::new(&frontend.definition.working_directory),
        project.join("web")
    );
    assert_eq!(backend.definition.expected_ports, [8052]);
    assert_eq!(backend.definition.readiness.kind, ReadinessKind::Port);
    assert_eq!(backend.definition.readiness.port, Some(8052));
    assert!(backend.definition.dependencies.is_empty());
    assert!(backend.definition.environment.is_empty());
    assert!(!backend.definition.shell_mode);
    assert!(core.services().unwrap().is_empty());

    let mut definition = frontend.definition.clone();
    definition.name = "既有前端".into();
    let stored = core
        .save_service(ServiceSave {
            definition,
            secret_values: Default::default(),
        })
        .unwrap();
    assert!(core.service_drafts(id).unwrap().iter().any(|draft| {
        draft.command == "python3 -m server"
            && draft.independent_entrypoint
            && draft.definition.working_directory != stored.working_directory
    }));
    let services = core.services().unwrap();
    assert_eq!(services.len(), 1);
    assert_eq!(services[0].definition.revision, stored.revision);
    assert_eq!(services[0].definition.name, "既有前端");
    assert!(services[0].current_run.is_none());
    assert!(!project.join("SHOULD_NOT_RUN").exists());
}

#[test]
fn simple_command_input_preserves_argument_boundaries_without_implicit_shell_execution() {
    use devnook_core::service_drafts::{display_command, parse_command, CommandSpec};
    let spec = CommandSpec {
        executable: "/Applications/My Tools/python3".into(),
        args: vec![
            "app.py".into(),
            "".into(),
            "a b".into(),
            "it's literal $x; &|".into(),
        ],
    };
    assert_eq!(parse_command(&display_command(&spec)).unwrap(), spec);
    let parsed = parse_command("pnpm dev --host '127.0.0.1' --label \"my app\"").unwrap();
    assert_eq!(
        parsed.args,
        vec!["dev", "--host", "127.0.0.1", "--label", "my app"]
    );
    assert_eq!(
        parse_command(r#"python3 -c "print('a\q')""#).unwrap().args[1],
        r#"print('a\q')"#
    );
    for text in [
        "",
        "''",
        "pnpm dev && touch FILE",
        "node $(command)",
        "FOO=bar npm start",
        "node \"unfinished",
        "node app.js\ncommand",
    ] {
        assert!(parse_command(text).is_err(), "{text}");
    }
}

#[test]
fn repository_without_root_manifest_groups_services_and_keeps_independent_nested_repo() {
    let temp = TempDir::new().unwrap();
    fullstack(temp.path());
    write(
        temp.path(),
        "app/tools/independent/.git",
        "gitdir: outside-is-not-read",
    );
    write(
        temp.path(),
        "app/tools/independent/go.mod",
        "module example.com/tool",
    );
    write(temp.path(), "app/node_modules/ignored/package.json", "{}");
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let p = scan(&core, temp.path(), 0);
    assert!(!p.incomplete);
    assert_eq!(p.candidates.len(), 2);
    let app = p.candidates.iter().find(|c| c.name == "app").unwrap();
    assert_eq!(app.boundary, "GIT");
    assert_eq!(app.project_type, "MIXED");
    assert_eq!(app.components.len(), 3);
    assert_eq!(app.components.iter().filter(|c| c.runnable).count(), 2);
    assert!(
        !app.components
            .iter()
            .find(|c| c.name == "packages/shared")
            .unwrap()
            .runnable
    );
    let nested = p
        .candidates
        .iter()
        .find(|c| c.name == "independent")
        .unwrap();
    assert_eq!(
        nested.parent_path.as_deref(),
        Some(app.canonical_path.as_str())
    );
    core.import_projects(&p.id, vec![nested.id.clone()])
        .unwrap();
    assert_eq!(core.projects().unwrap().len(), 1);
    assert_eq!(core.projects().unwrap()[0].candidate.name, "independent");
    assert!(core.services().unwrap().is_empty());
    assert!(!temp.path().join("app/frontend/SHOULD_NOT_RUN").exists());
}

#[test]
fn automatic_discovery_reaches_deep_projects_and_reports_explicit_and_safety_limits() {
    let temp = TempDir::new().unwrap();
    let deep = format!("{}/app/package.json", vec!["level"; 14].join("/"));
    write(temp.path(), &deep, "{}");
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let automatic = scan(&core, temp.path(), 0);
    assert_eq!(automatic.candidates.len(), 1);
    assert!(!automatic.incomplete);
    let limited = scan(&core, temp.path(), 1);
    assert!(limited.candidates.is_empty() && limited.incomplete);
    assert!(!limited.warnings.is_empty());
    write(
        temp.path(),
        &format!("{}/package.json", vec!["deeper"; 34].join("/")),
        "{}",
    );
    let safety = scan(&core, temp.path(), 0);
    assert!(safety.incomplete);
    assert!(safety
        .warnings
        .iter()
        .any(|warning| warning.contains("安全深度")));
}

#[test]
fn explicit_workspaces_and_unversioned_frontend_backend_pairs_have_project_roots() {
    let temp = TempDir::new().unwrap();
    write(
        temp.path(),
        "workspace/pnpm-workspace.yaml",
        "packages:\n  - apps/*\n  - packages/*",
    );
    write(
        temp.path(),
        "workspace/apps/web/package.json",
        r#"{"scripts":{"dev":"vite"}}"#,
    );
    write(temp.path(), "workspace/packages/shared/package.json", "{}");
    write(
        temp.path(),
        "plain/frontend/package.json",
        r#"{"scripts":{"start":"node index.js"}}"#,
    );
    write(temp.path(), "plain/backend/requirements.txt", "fastapi");
    write(temp.path(), "collection/frontend/package.json", "{}");
    write(temp.path(), "collection/backend/pom.xml", "<project/>");
    write(temp.path(), "collection/unrelated/package.json", "{}");
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let p = scan(&core, temp.path(), 0);
    assert_eq!(p.candidates.len(), 5);
    assert!(!p
        .candidates
        .iter()
        .any(|candidate| candidate.name == "collection"));
    assert_eq!(
        p.candidates
            .iter()
            .find(|c| c.name == "workspace")
            .unwrap()
            .boundary,
        "WORKSPACE"
    );
    let plain = p.candidates.iter().find(|c| c.name == "plain").unwrap();
    assert_eq!(plain.boundary, "COMBINED");
    assert_eq!(plain.components.iter().filter(|c| c.runnable).count(), 2);
}

#[test]
fn component_evidence_survives_reopen_and_bounded_rescan_without_overwriting_preferences() {
    let temp = TempDir::new().unwrap();
    fullstack(temp.path());
    let database = temp.path().join("registry.db");
    let core = Core::new(&database).unwrap();
    let p = scan(&core, temp.path(), 0);
    let imported = core
        .import_projects(&p.id, vec![p.candidates[0].id.clone()])
        .unwrap();
    let id = imported[0].candidate.id.clone();
    core.edit_project(ProjectEdit {
        id: id.clone(),
        name: "保留名称".into(),
        category: "保留分类".into(),
        default_ide: "CURSOR".into(),
        is_favorite: true,
    })
    .unwrap();
    let p = scan(&core, temp.path().join("app").as_path(), 1);
    assert!(p.incomplete && p.candidates[0].imported);
    core.import_projects(&p.id, vec![p.candidates[0].id.clone()])
        .unwrap();
    drop(core);
    let core = Core::new(&database).unwrap();
    let projects = core.projects().unwrap();
    assert_eq!(projects.len(), 1);
    let project = &projects[0];
    assert_eq!(project.candidate.id, id);
    assert_eq!(project.candidate.name, "保留名称");
    assert_eq!(project.category, "保留分类");
    assert_eq!(project.default_ide, "CURSOR");
    assert!(project.is_favorite);
    assert_eq!(project.candidate.components.len(), 3);
    assert!(core.services().unwrap().is_empty());
    let mut legacy = serde_json::to_value(project).unwrap();
    legacy.as_object_mut().unwrap().remove("components");
    legacy.as_object_mut().unwrap().remove("boundary");
    assert!(serde_json::from_value::<Project>(legacy)
        .unwrap()
        .candidate
        .components
        .is_empty());
    write(
        temp.path(),
        "app/packages/shared/.git",
        "gitdir: newly-independent",
    );
    let refreshed = scan(&core, temp.path(), 0);
    let root = refreshed
        .candidates
        .iter()
        .find(|candidate| candidate.name == "app")
        .unwrap();
    let updated = core
        .import_projects(&refreshed.id, vec![root.id.clone()])
        .unwrap();
    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].candidate.components.len(), 2);
    assert_eq!(updated[0].candidate.id, id);
    assert_eq!(updated[0].candidate.name, "保留名称");
    assert!(core.services().unwrap().is_empty());
}

#[test]
fn import_rejects_component_paths_replaced_with_symlinks_and_preserves_existing_projects() {
    let temp = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    fullstack(temp.path());
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let child = scan(&core, &temp.path().join("app/frontend"), 0);
    core.import_projects(&child.id, vec![child.candidates[0].id.clone()])
        .unwrap();
    let p = scan(&core, temp.path(), 0);
    assert!(p.candidates[0]
        .components
        .iter()
        .any(|c| c.name == "frontend" && c.imported));
    fs::rename(
        temp.path().join("app/frontend"),
        temp.path().join("app/frontend-original"),
    )
    .unwrap();
    std::os::unix::fs::symlink(outside.path(), temp.path().join("app/frontend")).unwrap();
    assert!(core
        .import_projects(&p.id, vec![p.candidates[0].id.clone()])
        .is_err());
    assert_eq!(core.projects().unwrap().len(), 1);
    assert!(core.services().unwrap().is_empty());
}

#[test]
fn feature_scan_recognizes_eleven_families_without_executing_manifests() {
    let temp = TempDir::new().unwrap();
    for (file, content) in [
        (
            "node/package.json",
            r#"{"scripts":{"dev":"touch SHOULD_NOT_RUN"}}"#,
        ),
        (
            "jvm/build.gradle.kts",
            "plugins { id(\"org.springframework.boot\") }",
        ),
        ("python/setup.py", "open('SHOULD_NOT_RUN','w').write('bad')"),
        (
            "rust/Cargo.toml",
            "[package]\nname='library'\nversion='0.1.0'",
        ),
        ("go/go.mod", "module example.com/library"),
        (
            "dotnet/Library.csproj",
            "<Project Sdk=\"Microsoft.NET.Sdk\"/>",
        ),
        (
            "php/composer.json",
            r#"{"require":{"laravel/framework":"*"}}"#,
        ),
        ("ruby/Gemfile", "system('touch SHOULD_NOT_RUN')"),
        (
            "dart/pubspec.yaml",
            "name: library\nenvironment:\n  sdk: ^3.6.0",
        ),
        (
            "swift/Package.swift",
            "// swift-tools-version: 6.0\nlet package = Package(name: \"library\")",
        ),
        ("cpp/CMakeLists.txt", "project(library)"),
    ] {
        write(temp.path(), file, content);
    }
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let p = scan(&core, temp.path(), 0);
    assert!(!p.incomplete, "{:?}", p.warnings);
    assert_eq!(p.candidates.len(), 11);
    let types: std::collections::HashSet<_> = p
        .candidates
        .iter()
        .map(|c| c.project_type.as_str())
        .collect();
    assert_eq!(types.len(), 11);
    for kind in [
        "NODE", "JAVA", "PYTHON", "RUST", "GO", "DOTNET", "PHP", "RUBY", "DART", "SWIFT", "CPP",
    ] {
        assert!(types.contains(kind));
    }
    assert!(p
        .candidates
        .iter()
        .filter(|c| c.name != "node" && c.name != "jvm")
        .all(|c| !c.runnable));
    assert!(!temp.path().join("node/SHOULD_NOT_RUN").exists());
    assert!(!temp.path().join("python/SHOULD_NOT_RUN").exists());
    assert!(!temp.path().join("ruby/SHOULD_NOT_RUN").exists());
    assert!(core.services().unwrap().is_empty());
}

#[test]
fn feature_scan_prunes_source_assets_and_documents_while_deep_scan_recovers_custom_layouts() {
    let temp = TempDir::new().unwrap();
    write(temp.path(), "app/package.json", "{}");
    for bucket in ["src", "assets", "docs"] {
        for index in 0..100 {
            for file in 0..4 {
                write(
                    temp.path(),
                    &format!("app/{bucket}/{index}/{file}.txt"),
                    "not a manifest",
                );
            }
        }
    }
    write(
        temp.path(),
        "app/src/nonstandard/deeper/service/package.json",
        r#"{"scripts":{"dev":"never-executed"}}"#,
    );
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let feature = scan(&core, temp.path(), 0);
    assert!(!feature.incomplete);
    assert_eq!(feature.candidates.len(), 1);
    assert!(feature.candidates[0].components.is_empty());
    assert!(
        feature.visited <= 3 && feature.entries < 20,
        "{} / {}",
        feature.visited,
        feature.entries
    );
    let deep = scan(&core, temp.path(), 8);
    assert!(!deep.incomplete);
    assert!(deep.visited > 300 && deep.entries > 1500);
    assert!(deep.candidates[0]
        .components
        .iter()
        .any(|c| c.name == "src/nonstandard/deeper/service"));
}

#[test]
fn declared_modules_reach_source_locations_and_honor_exclusions_across_ecosystems() {
    let temp = TempDir::new().unwrap();
    for (file, content) in [
        ("node/pnpm-workspace.yaml", "packages: ['src/packages/*', '!src/packages/excluded'] # inline comment"),
        ("node/src/packages/web/package.json", r#"{"scripts":{"dev":"vite"}}"#),
        ("node/src/packages/excluded/package.json", "{}"),
        ("rust/Cargo.toml", "[workspace]\nmembers = [\n 'src/crates/*',\n]\nexclude=['src/crates/excluded']\n[workspace.dependencies]\nhelper={path='unusual/helper'}"),
        ("rust/src/crates/api/Cargo.toml", "[package]\nname='api'"),
        ("rust/src/crates/excluded/Cargo.toml", "[package]\nname='excluded'"),
        ("rust/unusual/helper/Cargo.toml", "[package]\nname='helper'"),
        ("python/pyproject.toml", "[tool.uv.workspace]\nmembers=['src/python/*']\nexclude=['src/python/excluded']"),
        ("python/src/python/api/pyproject.toml", "[project]\nname='api'"),
        ("python/src/python/excluded/pyproject.toml", "[project]\nname='excluded'"),
        ("go/go.work", "go 1.24\nuse (\n ./src/go/api // local module\n ./src/go/shared\n)"),
        ("go/src/go/api/go.mod", "module example.com/api"),
        ("go/src/go/shared/go.mod", "module example.com/shared"),
        ("dart/pubspec.yaml", "name: root\nworkspace:\n  - src/dart/api"),
        ("dart/src/dart/api/pubspec.yaml", "name: api"),
    ] { write(temp.path(), file, content); }
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let p = scan(&core, temp.path(), 0);
    assert!(!p.incomplete, "{:?}", p.warnings);
    assert_eq!(p.candidates.len(), 5);
    for (name, count) in [
        ("node", 1),
        ("rust", 2),
        ("python", 1),
        ("go", 2),
        ("dart", 1),
    ] {
        let project = p.candidates.iter().find(|c| c.name == name).unwrap();
        assert_eq!(project.boundary, "WORKSPACE");
        assert_eq!(
            project.components.len(),
            count,
            "{name}: {:?}",
            project.components
        );
        assert!(project
            .components
            .iter()
            .all(|c| !c.name.contains("excluded")));
    }
}

#[test]
fn maven_gradle_dotnet_and_native_modules_are_static_and_comment_aware() {
    let temp = TempDir::new().unwrap();
    for (file, content) in [
        ("maven/pom.xml", "<project xmlns='urn:test'><modules><!-- <module>src/ignored</module> --><module>src/java/api&amp;web</module></modules></project>"),
        ("maven/src/java/api&web/pom.xml", "<project>spring-boot</project>"),
        ("maven/src/ignored/pom.xml", "<project/>"),
        ("gradle/settings.gradle.kts", "// include(\"src:ignored\")\ninclude(\n\":api\",\n\":shared\"\n)\nproject(\":api\").projectDir = file(\"src/jvm/api\")\nincludeBuild(\"src/build-logic\")"),
        ("gradle/src/jvm/api/build.gradle.kts", "plugins { id(\"org.springframework.boot\") }"),
        ("gradle/shared/build.gradle", "plugins { id 'java-library' }"),
        ("gradle/src/build-logic/build.gradle.kts", "plugins {}"),
        ("gradle/src/ignored/build.gradle", "plugins {}"),
        ("dotnet/Root.sln", "Project(\"{type}\") = \"Api\", \"src\\Api\\Api.csproj\", \"{id}\"\nEndProject"),
        ("dotnet/src/Api/Api.csproj", "<Project Sdk=\"Microsoft.NET.Sdk.Web\"><ItemGroup><ProjectReference Include=\"../Shared/Shared.csproj\"/></ItemGroup></Project>"),
        ("dotnet/src/Shared/Shared.csproj", "<Project/>"),
        ("cmake/CMakeLists.txt", "# add_subdirectory(src/ignored)\nproject(root)\nadd_subdirectory(src/core)"),
        ("cmake/src/core/CMakeLists.txt", "add_library(core STATIC core.cpp)"),
        ("cmake/src/ignored/CMakeLists.txt", "project(ignored)"),
        ("meson/meson.build", "project('root', 'c')\nsubdir('src/engine')"),
        ("meson/src/engine/meson.build", "static_library('engine', 'engine.c')"),
    ] { write(temp.path(), file, content); }
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let p = scan(&core, temp.path(), 0);
    assert!(!p.incomplete, "{:?}", p.warnings);
    assert_eq!(p.candidates.len(), 5);
    for (name, count) in [
        ("maven", 1),
        ("gradle", 3),
        ("dotnet", 2),
        ("cmake", 1),
        ("meson", 1),
    ] {
        let project = p.candidates.iter().find(|c| c.name == name).unwrap();
        assert_eq!(
            project.components.len(),
            count,
            "{name}: {:?}",
            project.components
        );
        assert!(project
            .components
            .iter()
            .all(|c| !c.name.contains("ignored")));
    }
}

#[test]
fn declared_and_shallow_nested_repositories_stay_independent_and_known_components_survive() {
    let temp = TempDir::new().unwrap();
    fullstack(temp.path());
    write(
        temp.path(),
        "app/.gitmodules",
        "[submodule \"other\"]\n path = src/private/repository\n url = not-read-or-fetched",
    );
    for dir in ["app/src/private/repository", "app/experiments/tool"] {
        write(
            temp.path(),
            &format!("{dir}/.git"),
            "gitdir: /outside/never-follow",
        );
        write(temp.path(), &format!("{dir}/package.json"), "{}");
    }
    write(
        temp.path(),
        "app/src/previously-known/service/package.json",
        r#"{"scripts":{"start":"never-run"}}"#,
    );
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let deep = scan(&core, temp.path(), 8);
    let app = deep.candidates.iter().find(|c| c.name == "app").unwrap();
    core.import_projects(&deep.id, vec![app.id.clone()])
        .unwrap();
    let feature = scan(&core, temp.path(), 0);
    assert!(!feature.incomplete, "{:?}", feature.warnings);
    assert_eq!(feature.candidates.len(), 3);
    let app = feature.candidates.iter().find(|c| c.name == "app").unwrap();
    assert!(app
        .components
        .iter()
        .any(|c| c.name == "src/previously-known/service"));
    assert!(app
        .components
        .iter()
        .all(|c| !c.name.contains("repository") && !c.name.contains("experiments")));
    assert!(feature
        .candidates
        .iter()
        .filter(|c| c.name != "app")
        .all(|c| c.boundary == "GIT"
            && c.parent_path.as_deref() == Some(app.canonical_path.as_str())));
    core.import_projects(&feature.id, vec![app.id.clone()])
        .unwrap();
    assert_eq!(core.projects().unwrap()[0].candidate.components.len(), 4);
}

#[test]
fn module_paths_cannot_escape_follow_symlinks_or_enter_dependencies_and_dynamic_rules_warn() {
    let temp = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    write(outside.path(), "external/package.json", "{}");
    write(
        temp.path(),
        "app/package.json",
        r#"{"workspaces":["../outside","/tmp","alias/*","node_modules/*",".env/private","${HOME}/secret"]}"#,
    );
    write(
        temp.path(),
        "app/node_modules/dependency/package.json",
        "{}",
    );
    write(temp.path(), "app/.env/private/package.json", "{}");
    std::os::unix::fs::symlink(outside.path(), temp.path().join("app/alias")).unwrap();
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let p = scan(&core, &temp.path().join("app"), 0);
    assert!(p.incomplete && !p.warnings.is_empty());
    assert_eq!(p.candidates.len(), 1);
    assert!(p.candidates[0].components.is_empty());
    write(
        temp.path(),
        "dynamic/settings.gradle",
        "include(findModules())\nprintln('NEVER_EXECUTED')",
    );
    write(
        temp.path(),
        "dynamic/pnpm-workspace.yaml",
        "packages: *unresolved_anchor",
    );
    let dynamic = scan(&core, &temp.path().join("dynamic"), 0);
    assert!(dynamic.incomplete && dynamic.warnings.len() >= 2);
}
