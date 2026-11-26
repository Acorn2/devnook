// Menu-bar access reuses the existing Core client. No service runs in this window.
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};

const TRAY: &str = "devnook-tray";
const PANEL_WIDTH: f64 = 356.0;
const PANEL_MIN_HEIGHT: f64 = 80.0;
const PANEL_MAX_HEIGHT: f64 = 520.0;

pub struct PanelHeight(Mutex<f64>);

#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Destination {
    Services,
    Ports,
    Managed {
        id: String,
        action: Option<ServiceAction>,
    },
    External {
        pid: u32,
    },
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceAction {
    Start,
    Restart,
}

#[derive(Clone, Serialize)]
pub struct Navigation {
    sequence: u64,
    destination: Destination,
}

#[derive(Default)]
pub struct PendingNavigation(Mutex<(u64, Option<Navigation>)>);

fn hide_panel(panel: &WebviewWindow) -> tauri::Result<()> {
    panel.hide()?;
    panel.emit_to(panel.label(), "devnook:window-hidden", ())
}

pub fn show_main(app: &AppHandle) -> Result<(), String> {
    let main = app
        .get_webview_window("main")
        .ok_or("main window unavailable")?;
    main.unminimize().map_err(|e| e.to_string())?;
    main.show().map_err(|e| e.to_string())?;
    main.set_focus().map_err(|e| e.to_string())?;
    main.emit_to("main", "devnook:window-shown", ())
        .map_err(|e| e.to_string())?;
    if let Some(panel) = app.get_webview_window("tray") {
        let _ = hide_panel(&panel);
    }
    Ok(())
}

#[tauri::command]
pub fn open_main_window(app: AppHandle, destination: Option<Destination>) -> Result<(), String> {
    if let Some(destination) = destination {
        let pending = app.state::<PendingNavigation>();
        let mut pending = pending.0.lock().map_err(|_| "navigation unavailable")?;
        pending.0 += 1;
        pending.1 = Some(Navigation {
            sequence: pending.0,
            destination,
        });
    }
    show_main(&app)?;
    // The request remains queued if the main WebView has not subscribed yet.
    app.get_webview_window("main")
        .ok_or("main window unavailable")?
        .emit_to("main", "devnook:navigate", ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn take_main_navigation(
    window: WebviewWindow,
    app: AppHandle,
) -> Result<Option<Navigation>, String> {
    if window.label() != "main" {
        return Err("navigation is only available to the main window".into());
    }
    app.state::<PendingNavigation>()
        .0
        .lock()
        .map(|mut pending| pending.1.take())
        .map_err(|_| "navigation unavailable".into())
}

#[tauri::command]
pub fn hide_tray(window: WebviewWindow) -> Result<(), String> {
    if window.label() != "tray" {
        return Err("only the menu-bar panel can be hidden here".into());
    }
    hide_panel(&window).map_err(|e| e.to_string())
}

fn requested_height(height: f64) -> Result<f64, String> {
    if !height.is_finite() {
        return Err("invalid panel height".into());
    }
    Ok(height.ceil().clamp(PANEL_MIN_HEIGHT, PANEL_MAX_HEIGHT))
}

#[tauri::command]
pub fn resize_tray(window: WebviewWindow, app: AppHandle, height: f64) -> Result<(), String> {
    if window.label() != "tray" {
        return Err("only the menu-bar panel can resize here".into());
    }
    let height = requested_height(height)?;
    *app.state::<PanelHeight>()
        .0
        .lock()
        .map_err(|_| "panel sizing unavailable")? = height;
    if window.is_visible().map_err(|e| e.to_string())? {
        if let Some(rect) = app
            .tray_by_id(TRAY)
            .ok_or("tray unavailable")?
            .rect()
            .map_err(|e| e.to_string())?
        {
            position_panel(&app, rect, height).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn quit_desktop(app: AppHandle) {
    // Core and runner processes have independent lifetimes. Do not stop them.
    app.exit(0);
}

#[derive(Clone, Copy, Debug)]
struct Bounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

fn panel_frame(icon: Bounds, area: Bounds, scale: f64, content_height: f64) -> Bounds {
    let margin = (8.0 * scale).min(((area.width.min(area.height) - 1.0) / 2.0).max(0.0));
    let width = (PANEL_WIDTH * scale)
        .min(area.width - margin * 2.0)
        .max(1.0);
    let height = (content_height.clamp(PANEL_MIN_HEIGHT, PANEL_MAX_HEIGHT) * scale)
        .min(area.height - margin * 2.0)
        .max(1.0);
    Bounds {
        x: (icon.x + icon.width / 2.0 - width / 2.0)
            .clamp(area.x + margin, area.x + area.width - width - margin),
        y: (icon.y + icon.height + 4.0 * scale)
            .clamp(area.y + margin, area.y + area.height - height - margin),
        width,
        height,
    }
}

fn position_panel(app: &AppHandle, rect: tauri::Rect, content_height: f64) -> tauri::Result<()> {
    let panel = app
        .get_webview_window("tray")
        .ok_or(tauri::Error::WindowNotFound)?;
    let position = rect.position.to_physical::<f64>(1.0);
    let size = rect.size.to_physical::<f64>(1.0);
    let monitor = app
        .available_monitors()?
        .into_iter()
        .find(|monitor| {
            let origin = monitor.position();
            let extent = monitor.size();
            position.x >= origin.x as f64
                && position.y >= origin.y as f64
                && position.x < origin.x as f64 + extent.width as f64
                && position.y < origin.y as f64 + extent.height as f64
        })
        .or(app.primary_monitor()?);
    if let Some(monitor) = monitor {
        let scale = monitor.scale_factor();
        let area = monitor.work_area();
        let frame = panel_frame(
            Bounds {
                x: position.x,
                y: position.y,
                width: size.width,
                height: size.height,
            },
            Bounds {
                x: area.position.x as f64,
                y: area.position.y as f64,
                width: area.size.width as f64,
                height: area.size.height as f64,
            },
            scale,
            content_height,
        );
        panel.set_size(PhysicalSize::new(
            frame.width.round() as u32,
            frame.height.round() as u32,
        ))?;
        panel.set_position(PhysicalPosition::new(
            frame.x.round() as i32,
            frame.y.round() as i32,
        ))?;
    }
    Ok(())
}

fn show_panel(app: &AppHandle, rect: tauri::Rect) -> tauri::Result<()> {
    let height = app
        .state::<PanelHeight>()
        .0
        .lock()
        .map(|value| *value)
        .unwrap_or(PANEL_MIN_HEIGHT);
    position_panel(app, rect, height)?;
    let panel = app
        .get_webview_window("tray")
        .ok_or(tauri::Error::WindowNotFound)?;
    panel.show()?;
    panel.set_focus()?;
    panel.emit_to("tray", "devnook:tray-opened", ())?;
    Ok(())
}

pub fn setup(app: &mut tauri::App) -> tauri::Result<()> {
    app.manage(PendingNavigation::default());
    app.manage(PanelHeight(Mutex::new(PANEL_MIN_HEIGHT)));
    let panel =
        WebviewWindowBuilder::new(app, "tray", WebviewUrl::App("index.html?panel=tray".into()))
            .title("DevNook")
            .inner_size(PANEL_WIDTH, PANEL_MIN_HEIGHT)
            .decorations(false)
            .resizable(false)
            .maximizable(false)
            .minimizable(false)
            .skip_taskbar(true)
            .always_on_top(true)
            .visible(false)
            .focused(false)
            .shadow(true)
            .build()?;
    let handle = panel.clone();
    panel.on_window_event(move |event| {
        if matches!(event, WindowEvent::Focused(false)) {
            let _ = hide_panel(&handle);
        }
    });
    TrayIconBuilder::with_id(TRAY)
        .icon(tauri::include_image!("icons/tray.png"))
        .icon_as_template(true)
        .tooltip("DevNook")
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                rect,
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(panel) = app.get_webview_window("tray") {
                    if panel.is_visible().unwrap_or(false) {
                        let _ = hide_panel(&panel);
                    } else if show_panel(app, rect).is_err() {
                        let _ = show_main(app);
                    }
                }
            }
        })
        .build(app)?;
    Ok(())
}

pub fn window_event(window: &tauri::Window, event: &WindowEvent) {
    if window.label() == "main" && window.app_handle().tray_by_id(TRAY).is_some() {
        if let WindowEvent::CloseRequested { api, .. } = event {
            if window.hide().is_ok() {
                let _ = window.emit_to("main", "devnook:window-hidden", ());
                api.prevent_close();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_stays_in_work_area_at_edges_and_retina_scales() {
        for scale in [1.0, 2.0] {
            for origin in [-1920.0, 0.0, 1920.0] {
                let area = Bounds {
                    x: origin,
                    y: 24.0 * scale,
                    width: 1440.0 * scale,
                    height: 760.0 * scale,
                };
                for x in [
                    area.x,
                    area.x + area.width / 2.0,
                    area.x + area.width - 20.0,
                ] {
                    let frame = panel_frame(
                        Bounds {
                            x,
                            y: 0.0,
                            width: 20.0 * scale,
                            height: 24.0 * scale,
                        },
                        area,
                        scale,
                        280.0,
                    );
                    assert_eq!(frame.width, PANEL_WIDTH * scale);
                    assert_eq!(frame.height, 280.0 * scale);
                    assert!(frame.x >= area.x && frame.y >= area.y);
                    assert!(frame.x + frame.width <= area.x + area.width);
                    assert!(frame.y + frame.height <= area.y + area.height);
                }
            }
        }
    }

    #[test]
    fn panel_shrinks_in_small_work_areas() {
        for (width, height) in [(320.0, 480.0), (10.0, 10.0)] {
            let frame = panel_frame(
                Bounds {
                    x: 300.0,
                    y: 0.0,
                    width: 20.0,
                    height: 24.0,
                },
                Bounds {
                    x: 0.0,
                    y: 0.0,
                    width,
                    height,
                },
                2.0,
                PANEL_MAX_HEIGHT,
            );
            assert!(frame.width > 0.0 && frame.height > 0.0);
            assert!(frame.x >= 0.0 && frame.y >= 0.0);
            assert!(frame.x + frame.width <= width && frame.y + frame.height <= height);
        }
    }

    #[test]
    fn content_height_grows_and_shrinks_with_bounds() {
        assert!(requested_height(f64::NAN).is_err());
        assert!(requested_height(f64::INFINITY).is_err());
        assert_eq!(requested_height(10.0).unwrap(), PANEL_MIN_HEIGHT);
        assert_eq!(requested_height(160.1).unwrap(), 161.0);
        assert_eq!(requested_height(1600.0).unwrap(), PANEL_MAX_HEIGHT);
        let area = Bounds {
            x: 0.0,
            y: 24.0,
            width: 1440.0,
            height: 800.0,
        };
        let icon = Bounds {
            x: 1000.0,
            y: 0.0,
            width: 20.0,
            height: 24.0,
        };
        let small = panel_frame(icon, area, 1.0, 144.0);
        let large = panel_frame(icon, area, 1.0, 490.0);
        assert_eq!(small.height, 144.0);
        assert_eq!(large.height, 490.0);
        assert_eq!(small.x, large.x);
        assert_eq!(small.y, large.y);
    }

    #[test]
    fn navigation_does_not_accept_shell_urls_or_stop_actions() {
        for value in [
            serde_json::json!({"kind":"managed", "id":"s", "action":"stop"}),
            serde_json::json!({"kind":"shell", "command":"echo test"}),
            serde_json::json!({"kind":"external", "pid":1, "url":"http://example.com"}),
        ] {
            assert!(serde_json::from_value::<Destination>(value).is_err());
        }
        assert!(serde_json::from_value::<Destination>(
            serde_json::json!({"kind":"managed", "id":"s", "action":"restart"})
        )
        .is_ok());
        assert!(serde_json::from_value::<Destination>(serde_json::json!({"kind":"ports"})).is_ok());
    }
}
