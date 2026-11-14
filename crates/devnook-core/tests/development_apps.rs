use devnook_core::{
    model::{Project, ProjectEdit, Settings},
    Core,
};
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};
use tempfile::TempDir;

fn import(core: &Core, root: &Path) -> Vec<Project> {
    let scan = core.scan_start(root.to_string_lossy().into(), 2).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let result = core.scan_status(&scan).unwrap();
        if result.status == "completed" {
            return core
                .import_projects(
                    &scan,
                    result.candidates.iter().map(|c| c.id.clone()).collect(),
                )
                .unwrap();
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn java_and_other_imports_inherit_global_choice_and_reimport_keeps_project_choice() {
    let temp = TempDir::new().unwrap();
    fs::create_dir(temp.path().join("java")).unwrap();
    fs::create_dir(temp.path().join("web")).unwrap();
    fs::write(temp.path().join("java/pom.xml"), "<project/>").unwrap();
    fs::write(temp.path().join("web/package.json"), "{}").unwrap();
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    core.save_settings(Settings {
        default_ide: "CURSOR".into(),
        ..Settings::default()
    })
    .unwrap();
    let projects = import(&core, temp.path());
    assert_eq!(projects.len(), 2);
    assert!(projects.iter().all(|p| p.default_ide == "CURSOR"));
    let java = projects
        .iter()
        .find(|p| p.candidate.project_type == "JAVA")
        .unwrap();
    core.edit_project(ProjectEdit {
        id: java.candidate.id.clone(),
        name: java.candidate.name.clone(),
        default_ide: "IDEA".into(),
        category: "personal".into(),
        is_favorite: true,
    })
    .unwrap();
    core.save_settings(Settings::default()).unwrap();
    let again = import(&core, temp.path());
    let same = again
        .iter()
        .find(|p| p.candidate.id == java.candidate.id)
        .unwrap();
    assert_eq!(same.default_ide, "IDEA");
    assert!(same.is_favorite);
    assert_eq!(same.category, "personal");
}

#[cfg(target_os = "macos")]
fn bundle(root: &Path, name: &str, id: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let app = root.join(name);
    fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
    fs::write(app.join("Contents/Info.plist"), format!(r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundlePackageType</key><string>APPL</string><key>CFBundleIdentifier</key><string>{id}</string><key>CFBundleName</key><string>Custom Editor</string><key>CFBundleExecutable</key><string>editor</string><key>CFBundleShortVersionString</key><string>1.2.3</string></dict></plist>"#)).unwrap();
    fs::write(
        app.join("Contents/MacOS/editor"),
        "#!/bin/sh\ntouch SHOULD_NOT_EXECUTE\n",
    )
    .unwrap();
    fs::set_permissions(
        app.join("Contents/MacOS/editor"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    app
}

#[test]
#[cfg(target_os = "macos")]
fn custom_app_selection_persists_without_launching_and_uninstall_keeps_preference() {
    let temp = TempDir::new().unwrap();
    fs::create_dir(temp.path().join("source")).unwrap();
    fs::write(temp.path().join("source/package.json"), "{}").unwrap();
    let app = bundle(
        temp.path(),
        "Editor with spaces.app",
        "dev.devnook.test.editor",
    );
    let database = temp.path().join("registry.db");
    let core = Core::new(&database).unwrap();
    let project = import(&core, &temp.path().join("source")).remove(0);
    let inspected = core.inspect_development_app(app.to_str().unwrap()).unwrap();
    assert_eq!(inspected.name, "Custom Editor");
    assert_eq!(inspected.version, "1.2.3");
    assert_eq!(
        inspected.id,
        format!("APP:{}", app.canonicalize().unwrap().display())
    );
    let changed = core
        .set_project_app(&project.candidate.id, &inspected.id)
        .unwrap();
    assert_eq!(changed[0].default_ide, inspected.id);
    assert_eq!(changed[0].candidate.name, project.candidate.name);
    core.save_settings(Settings {
        default_ide: inspected.id.clone(),
        ..Settings::default()
    })
    .unwrap();
    let catalog = core.development_apps().unwrap();
    assert!(catalog.apps.iter().any(|app| app.id == inspected.id));
    assert!(catalog.project_defaults[&project.candidate.id].available);
    assert_eq!(catalog.global_default.id, inspected.id);
    assert!(!temp.path().join("SHOULD_NOT_EXECUTE").exists());
    assert!(!temp.path().join("source/SHOULD_NOT_EXECUTE").exists());
    drop(core);
    let core = Core::new(&database).unwrap();
    assert_eq!(core.projects().unwrap()[0].default_ide, inspected.id);
    // Simulate an uninstall by moving only the test bundle, without deleting it.
    fs::rename(&app, temp.path().join("Uninstalled.app")).unwrap();
    let catalog = core.development_apps().unwrap();
    assert!(!catalog.project_defaults[&project.candidate.id].available);
    assert!(!catalog.global_default.available);
    assert_eq!(core.projects().unwrap()[0].default_ide, inspected.id);
    assert!(core.open_project(&project.candidate.id, "DEFAULT").is_err());
    assert!(core
        .set_project_app(&project.candidate.id, &inspected.id)
        .is_err());
    // Unavailable preference does not prevent unrelated project edits.
    core.edit_project(ProjectEdit {
        id: project.candidate.id,
        name: "Renamed".into(),
        default_ide: inspected.id,
        category: String::new(),
        is_favorite: true,
    })
    .unwrap();
}

#[test]
#[cfg(target_os = "macos")]
fn manually_chosen_apps_survive_refresh_restart_and_stale_settings_without_becoming_defaults() {
    let temp = TempDir::new().unwrap();
    let app_path = bundle(
        temp.path(),
        "Remembered Editor.app",
        "dev.devnook.test.remembered",
    );
    let database = temp.path().join("registry.db");
    let core = Core::new(&database).unwrap();
    let stale_settings_form = core.settings().unwrap();
    let inspected = core
        .inspect_development_app(app_path.to_str().unwrap())
        .unwrap();
    // Inspection alone remains read-only. The app is outside automatic directories,
    // unrecognized, and is not referenced by any project or global default.
    assert!(!core
        .development_apps()
        .unwrap()
        .apps
        .iter()
        .any(|app| app.id == inspected.id));
    let remembered = core
        .remember_development_app(app_path.to_str().unwrap())
        .unwrap();
    core.remember_development_app(app_path.canonicalize().unwrap().to_str().unwrap())
        .unwrap();
    assert_eq!(
        core.development_apps()
            .unwrap()
            .apps
            .iter()
            .filter(|app| app.id == remembered.id)
            .count(),
        1
    );
    assert_eq!(
        core.settings().unwrap().default_ide,
        stale_settings_form.default_ide
    );
    assert!(core.projects().unwrap().is_empty());
    // Saving a form opened before the selection cannot erase the private registry.
    core.save_settings(stale_settings_form).unwrap();
    assert!(core
        .remember_development_app(temp.path().join("Missing.app").to_str().unwrap())
        .is_err());
    drop(core);
    let core = Core::new(&database).unwrap();
    assert!(core
        .development_apps()
        .unwrap()
        .apps
        .iter()
        .any(|app| app.id == remembered.id));
    let db = rusqlite::Connection::open(&database).unwrap();
    assert_eq!(
        devnook_core::storage::remembered_apps(&db).unwrap().len(),
        1
    );
    assert_eq!(
        db.query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
            .unwrap(),
        3
    );
    let moved = temp.path().join("Temporarily unavailable.app");
    fs::rename(&app_path, &moved).unwrap();
    assert!(!core
        .development_apps()
        .unwrap()
        .apps
        .iter()
        .any(|app| app.id == remembered.id));
    assert_eq!(
        devnook_core::storage::remembered_apps(&db).unwrap().len(),
        1
    );
    fs::rename(&moved, &app_path).unwrap();
    assert!(core
        .development_apps()
        .unwrap()
        .apps
        .iter()
        .any(|app| app.id == remembered.id));
    assert!(!temp.path().join("SHOULD_NOT_EXECUTE").exists());
}

#[test]
#[cfg(target_os = "macos")]
fn bundle_validation_rejects_symlinks_malformed_metadata_and_non_apps() {
    use std::os::unix::fs::symlink;
    let temp = TempDir::new().unwrap();
    let app = bundle(temp.path(), "Editor.app", "dev.devnook.test.editor");
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let inspect = |path: &Path| core.inspect_development_app(path.to_str().unwrap());
    assert!(inspect(&app).is_ok());
    assert!(std::process::Command::new("/usr/bin/plutil")
        .args(["-convert", "binary1"])
        .arg(app.join("Contents/Info.plist"))
        .status()
        .unwrap()
        .success());
    assert!(inspect(&app).is_ok());
    let alias = temp.path().join("Alias.app");
    symlink(&app, &alias).unwrap();
    assert!(inspect(&alias).is_err());
    assert!(inspect(Path::new("relative.app")).is_err());
    assert!(inspect(temp.path()).is_err());
    let original = app.join("Contents/Info.plist");
    fs::rename(&original, app.join("Contents/saved.plist")).unwrap();
    symlink("saved.plist", &original).unwrap();
    assert!(inspect(&app).is_err());
    let malformed = bundle(temp.path(), "Broken.app", "dev.devnook.test.broken");
    fs::write(malformed.join("Contents/Info.plist"), "INVALID").unwrap();
    assert!(inspect(&malformed).is_err());
    let missing = bundle(temp.path(), "Missing.app", "dev.devnook.test.missing");
    fs::rename(
        missing.join("Contents/MacOS/editor"),
        missing.join("Contents/MacOS/moved"),
    )
    .unwrap();
    assert!(inspect(&missing).is_err());
    for invalid in [
        "APP:relative.app",
        "APP:/Applications/../Bad.app",
        "VSCODE;echo injected",
        "APP:/Applications/Bad.app\0",
    ] {
        assert!(devnook_core::ide::validate_target(invalid).is_err());
    }
}

#[test]
#[cfg(target_os = "macos")]
fn discovery_checks_real_bundles_recommendations_and_legacy_overrides() {
    let temp = TempDir::new().unwrap();
    fs::write(temp.path().join("pom.xml"), "<project/>").unwrap();
    let app = bundle(temp.path(), "Custom IDEA.app", "com.jetbrains.intellij");
    let core = Core::new(&temp.path().join("registry.db")).unwrap();
    let project = import(&core, temp.path()).remove(0);
    core.save_settings(Settings {
        default_ide: "IDEA".into(),
        idea_path: app.to_string_lossy().into(),
        ..Settings::default()
    })
    .unwrap();
    let catalog = core.development_apps().unwrap();
    let preference = &catalog.global_default;
    assert!(preference.available);
    let installed = catalog.apps.iter().find(|a| a.id == preference.id).unwrap();
    assert_eq!(Path::new(&installed.path), app.canonicalize().unwrap());
    assert!(installed.recommended_for.contains(&project.candidate.id));
    let ids: std::collections::HashSet<_> = catalog.apps.iter().map(|a| &a.id).collect();
    assert_eq!(ids.len(), catalog.apps.len());
    assert!(catalog
        .apps
        .iter()
        .all(|app| Path::new(&app.path).join("Contents/Info.plist").is_file()));
    assert!(!temp.path().join("SHOULD_NOT_EXECUTE").exists());
}
