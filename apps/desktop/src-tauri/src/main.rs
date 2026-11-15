#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use devnook_core::browser::{BrowserAddress, BrowserShortcut, BrowserTarget};
use devnook_core::external_service::{ExternalServiceConfiguration, ExternalServiceHandoffPreview};
use devnook_core::{access::*, ipc::CoreClient as Core, model::*, service_model::*, workspace::*};
use std::sync::Arc;
use tauri::{Manager, State};

mod tray;

async fn run<T: Send + 'static>(
    core: State<'_, Arc<Core>>,
    action: impl FnOnce(Arc<Core>) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || action(core))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn list_projects(core: State<'_, Arc<Core>>) -> Result<Vec<Project>, String> {
    run(core, |c| c.projects()).await
}
#[tauri::command]
async fn get_settings(core: State<'_, Arc<Core>>) -> Result<Settings, String> {
    run(core, |c| c.settings()).await
}
#[tauri::command]
async fn list_development_apps(
    core: State<'_, Arc<Core>>,
) -> Result<devnook_core::ide::DevelopmentApps, String> {
    run(core, |c| {
        c.invoke("list_development_apps", serde_json::json!({}))
    })
    .await
}
#[tauri::command]
async fn inspect_development_app(
    core: State<'_, Arc<Core>>,
    path: String,
) -> Result<devnook_core::ide::DevelopmentApp, String> {
    run(core, move |c| {
        c.invoke("inspect_development_app", serde_json::json!({"path":path}))
    })
    .await
}
#[tauri::command]
async fn remember_development_app(
    core: State<'_, Arc<Core>>,
    path: String,
) -> Result<devnook_core::ide::DevelopmentApp, String> {
    run(core, move |c| {
        c.invoke("remember_development_app", serde_json::json!({"path":path}))
    })
    .await
}
#[tauri::command]
async fn set_project_app(
    core: State<'_, Arc<Core>>,
    id: String,
    target: String,
) -> Result<Vec<Project>, String> {
    run(core, move |c| {
        c.invoke(
            "set_project_app",
            serde_json::json!({"id":id,"target":target}),
        )
    })
    .await
}
#[tauri::command]
async fn save_settings(core: State<'_, Arc<Core>>, settings: Settings) -> Result<(), String> {
    run(core, move |c| c.save_settings(settings)).await
}
#[tauri::command]
async fn edit_project(
    core: State<'_, Arc<Core>>,
    edit: ProjectEdit,
) -> Result<Vec<Project>, String> {
    run(core, move |c| c.edit_project(edit)).await
}
#[tauri::command]
async fn remove_project(core: State<'_, Arc<Core>>, id: String) -> Result<(), String> {
    run(core, move |c| c.remove_project(&id)).await
}
#[tauri::command]
async fn scan_start(
    core: State<'_, Arc<Core>>,
    root: String,
    depth: usize,
) -> Result<String, String> {
    run(core, move |c| c.scan_start(root, depth)).await
}
#[tauri::command]
async fn scan_status(core: State<'_, Arc<Core>>, id: String) -> Result<ScanProgress, String> {
    run(core, move |c| c.scan_status(&id)).await
}
#[tauri::command]
async fn scan_cancel(core: State<'_, Arc<Core>>, id: String) -> Result<(), String> {
    run(core, move |c| c.scan_cancel(&id)).await
}
#[tauri::command]
async fn import_projects(
    core: State<'_, Arc<Core>>,
    scan_id: String,
    ids: Vec<String>,
) -> Result<Vec<Project>, String> {
    run(core, move |c| c.import_projects(&scan_id, ids)).await
}
#[tauri::command]
async fn list_ports(core: State<'_, Arc<Core>>) -> Result<Vec<PortBinding>, String> {
    run(core, |c| c.ports()).await
}
#[tauri::command]
async fn inspect_process(core: State<'_, Arc<Core>>, pid: u32) -> Result<ProcessDetail, String> {
    run(core, move |c| c.inspect(pid)).await
}
#[tauri::command]
async fn assign_process(
    core: State<'_, Arc<Core>>,
    pid: u32,
    fingerprint: String,
    project_id: String,
) -> Result<ProcessDetail, String> {
    run(core, move |c| {
        c.assign_process(pid, &fingerprint, &project_id)
    })
    .await
}
#[tauri::command]
async fn prepare_stop(core: State<'_, Arc<Core>>, pid: u32) -> Result<StopPreview, String> {
    run(core, move |c| c.prepare_stop(pid)).await
}
#[tauri::command]
async fn confirm_stop(
    core: State<'_, Arc<Core>>,
    token: String,
    confirmed_pid: u32,
) -> Result<(), String> {
    run(core, move |c| c.confirm_stop(&token, confirmed_pid)).await
}
#[tauri::command]
async fn open_project(
    core: State<'_, Arc<Core>>,
    id: String,
    target: String,
) -> Result<(), String> {
    run(core, move |c| c.open_project(&id, &target)).await
}

#[tauri::command]
async fn service_browser_addresses(
    core: State<'_, Arc<Core>>,
    target: BrowserTarget,
) -> Result<Vec<BrowserAddress>, String> {
    run(core, move |c| c.service_browser_addresses(&target)).await
}

#[tauri::command]
async fn tray_browser_shortcuts(
    core: State<'_, Arc<Core>>,
) -> Result<Vec<BrowserShortcut>, String> {
    run(core, |c| c.tray_browser_shortcuts()).await
}

#[tauri::command]
async fn open_service_browser(
    core: State<'_, Arc<Core>>,
    target: BrowserTarget,
    address: BrowserAddress,
) -> Result<(), String> {
    run(core, move |c| c.open_service_browser(&target, &address)).await
}

#[tauri::command]
async fn list_services(core: State<'_, Arc<Core>>) -> Result<Vec<ServiceView>, String> {
    run(core, |c| c.services()).await
}
#[tauri::command]
async fn service_drafts(
    core: State<'_, Arc<Core>>,
    project_id: String,
) -> Result<Vec<devnook_core::service_drafts::ServiceDraft>, String> {
    run(core, move |c| {
        c.invoke(
            "service_drafts",
            serde_json::json!({"project_id":project_id}),
        )
    })
    .await
}
#[tauri::command]
async fn external_service_configuration(
    core: State<'_, Arc<Core>>,
    pid: u32,
) -> Result<ExternalServiceConfiguration, String> {
    run(core, move |c| c.external_service_configuration(pid)).await
}
#[tauri::command]
async fn prepare_external_service_stop(
    core: State<'_, Arc<Core>>,
    pid: u32,
) -> Result<StopPreview, String> {
    run(core, move |c| {
        c.invoke(
            "prepare_external_service_stop",
            serde_json::json!({"pid":pid}),
        )
    })
    .await
}
#[tauri::command]
async fn prepare_external_service_handoff(
    core: State<'_, Arc<Core>>,
    id: String,
    revision: String,
    pid: u32,
    fingerprint: String,
) -> Result<ExternalServiceHandoffPreview, String> {
    run(core, move |c| {
        c.invoke(
            "prepare_external_service_handoff",
            serde_json::json!({"id":id,"revision":revision,"pid":pid,"fingerprint":fingerprint}),
        )
    })
    .await
}
#[tauri::command]
async fn confirm_external_service_handoff(
    core: State<'_, Arc<Core>>,
    token: String,
    confirmed_pid: u32,
) -> Result<ServiceRun, String> {
    run(core, move |c| {
        c.invoke(
            "confirm_external_service_handoff",
            serde_json::json!({"token":token,"confirmed_pid":confirmed_pid}),
        )
    })
    .await
}
#[tauri::command]
async fn parse_service_command(
    core: State<'_, Arc<Core>>,
    command: String,
) -> Result<devnook_core::service_drafts::CommandSpec, String> {
    run(core, move |c| {
        c.invoke(
            "parse_service_command",
            serde_json::json!({"command":command}),
        )
    })
    .await
}
#[tauri::command]
async fn save_service(
    core: State<'_, Arc<Core>>,
    input: ServiceSave,
) -> Result<ServiceDefinition, String> {
    run(core, move |c| c.save_service(input)).await
}
#[tauri::command]
async fn archive_service(core: State<'_, Arc<Core>>, id: String) -> Result<(), String> {
    run(core, move |c| c.archive_service(&id)).await
}
#[tauri::command]
async fn service_history(
    core: State<'_, Arc<Core>>,
    id: String,
) -> Result<Vec<ServiceRun>, String> {
    run(core, move |c| c.service_history(&id)).await
}
#[tauri::command]
async fn service_logs(
    core: State<'_, Arc<Core>>,
    id: String,
    limit: usize,
    query: String,
) -> Result<LogPage, String> {
    run(core, move |c| c.service_logs(&id, limit, &query)).await
}
#[tauri::command]
async fn clear_service_log(core: State<'_, Arc<Core>>, id: String) -> Result<(), String> {
    run(core, move |c| c.clear_service_log(&id)).await
}
#[tauri::command]
async fn prepare_service_action(
    core: State<'_, Arc<Core>>,
    id: String,
    action: String,
) -> Result<ServiceActionPreview, String> {
    run(core, move |c| c.prepare_service_action(&id, &action)).await
}
#[tauri::command]
async fn confirm_service_action(
    core: State<'_, Arc<Core>>,
    token: String,
) -> Result<ServiceRun, String> {
    run(core, move |c| c.confirm_service_action(&token)).await
}
#[tauri::command]
async fn stop_service(core: State<'_, Arc<Core>>, id: String) -> Result<ServiceRun, String> {
    run(core, move |c| c.stop_service(&id)).await
}

#[tauri::command]
async fn list_workspaces(core: State<'_, Arc<Core>>) -> Result<Vec<WorkspaceGroup>, String> {
    run(core, |c| c.workspaces()).await
}
#[tauri::command]
async fn save_workspace(
    core: State<'_, Arc<Core>>,
    group: WorkspaceGroup,
) -> Result<WorkspaceGroup, String> {
    run(core, move |c| c.save_workspace(group)).await
}
#[tauri::command]
async fn remove_workspace(core: State<'_, Arc<Core>>, id: String) -> Result<(), String> {
    run(core, move |c| {
        c.invoke("remove_workspace", serde_json::json!({"id":id}))
    })
    .await
}
#[tauri::command]
async fn workspace_history(
    core: State<'_, Arc<Core>>,
    id: String,
) -> Result<Vec<WorkspaceRun>, String> {
    run(core, move |c| c.workspace_history(&id)).await
}
#[tauri::command]
async fn prepare_workspace_action(
    core: State<'_, Arc<Core>>,
    id: String,
    action: String,
) -> Result<WorkspacePreview, String> {
    run(core, move |c| c.prepare_workspace_action(&id, &action)).await
}
#[tauri::command]
async fn confirm_workspace_action(
    core: State<'_, Arc<Core>>,
    token: String,
) -> Result<WorkspaceRun, String> {
    run(core, move |c| c.confirm_workspace_action(&token)).await
}
#[tauri::command]
async fn agent_grants(core: State<'_, Arc<Core>>) -> Result<Vec<AgentGrant>, String> {
    run(core, move |c| {
        c.invoke("agent_grants", serde_json::json!({}))
    })
    .await
}
#[tauri::command]
async fn create_agent_grant(
    core: State<'_, Arc<Core>>,
    input: GrantInput,
) -> Result<GrantCreated, String> {
    run(core, move |c| {
        c.invoke("create_agent_grant", serde_json::json!({"input":input}))
    })
    .await
}
#[tauri::command]
async fn revoke_agent_grant(core: State<'_, Arc<Core>>, id: String) -> Result<(), String> {
    run(core, move |c| {
        c.invoke("revoke_agent_grant", serde_json::json!({"id":id}))
    })
    .await
}
#[tauri::command]
async fn access_audits(core: State<'_, Arc<Core>>) -> Result<Vec<AccessAudit>, String> {
    run(core, move |c| {
        c.invoke("access_audits", serde_json::json!({}))
    })
    .await
}

fn main() {
    let arguments: Vec<_> = std::env::args_os().collect();
    if arguments.get(1).is_some_and(|v| v == "--devnook-worker") {
        if devnook_core::service_worker::from_stdin().is_err() {
            std::process::exit(1);
        }
        return;
    }
    if arguments.get(1).is_some_and(|v| v == "--devnook-core") {
        let result = arguments
            .get(2)
            .ok_or("missing data directory")
            .and_then(|directory| {
                let launcher = devnook_core::supervisor::RunnerLauncher::current()
                    .map_err(|_| "runner unavailable")?;
                devnook_core::ipc::serve(std::path::Path::new(directory), launcher)
                    .map_err(|_| "core failed")
            });
        if result.is_err() {
            std::process::exit(1);
        }
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let directory = if cfg!(debug_assertions) {
                std::env::var_os("DEVNOOK_DATA_DIR")
                    .map(std::path::PathBuf::from)
                    .unwrap_or(app.path().app_data_dir()?)
            } else {
                app.path().app_data_dir()?
            };
            let core = Core::new_deferred(&directory, std::env::current_exe()?)
                .map_err(std::io::Error::other)?;
            app.manage(Arc::new(core));
            // Failure must leave the ordinary main window usable.
            if tray::setup(app).is_err() {
                eprintln!("DevNook menu-bar access could not be initialized");
            }
            Ok(())
        })
        .on_window_event(tray::window_event)
        .invoke_handler(tauri::generate_handler![
            tray::open_main_window,
            tray::take_main_navigation,
            tray::hide_tray,
            tray::resize_tray,
            tray::quit_desktop,
            system_language,
            list_projects,
            get_settings,
            list_development_apps,
            inspect_development_app,
            remember_development_app,
            set_project_app,
            save_settings,
            edit_project,
            remove_project,
            scan_start,
            scan_status,
            scan_cancel,
            import_projects,
            list_ports,
            inspect_process,
            assign_process,
            prepare_stop,
            confirm_stop,
            open_project,
            service_browser_addresses,
            tray_browser_shortcuts,
            open_service_browser,
            list_services,
            service_drafts,
            external_service_configuration,
            prepare_external_service_stop,
            prepare_external_service_handoff,
            confirm_external_service_handoff,
            parse_service_command,
            save_service,
            archive_service,
            service_history,
            service_logs,
            clear_service_log,
            prepare_service_action,
            confirm_service_action,
            stop_service,
            list_workspaces,
            save_workspace,
            remove_workspace,
            workspace_history,
            prepare_workspace_action,
            confirm_workspace_action,
            agent_grants,
            create_agent_grant,
            revoke_agent_grant,
            access_audits
        ])
        .build(tauri::generate_context!())
        .expect("DevNook 桌面运行失败")
        .run(|app, event| {
            if let tauri::RunEvent::Reopen {
                has_visible_windows: false,
                ..
            } = event
            {
                let _ = tray::show_main(app);
            }
        });
}

// Display preference only: this does not contact Core or alter system settings.
#[tauri::command]
fn system_language() -> String {
    #[cfg(target_os = "macos")]
    {
        objc2_foundation::NSLocale::preferredLanguages()
            .firstObject()
            .map(|language| language.to_string())
            .unwrap_or_else(|| "en".into())
    }
    #[cfg(not(target_os = "macos"))]
    {
        "en".into()
    }
}
