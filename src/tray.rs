use crate::app::state::AppState;
use crate::data::config::Mode;
use crate::services::hackatime::{latest_project, rolling_24h_window, Hackatime};
use std::time::Duration;
use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Manager, WebviewWindow,
};

#[cfg(target_os = "macos")]
const MACOS_TRAY_ICON_TEMPLATE: &[u8] = include_bytes!("../icons/trayTemplate.png");
const NO_DATA_TEXT: &str = "No data yet";
const NO_PROJECT_TEXT: &str = "No active project";

fn format_hours(total_seconds: f64) -> String {
    let total_minutes = (total_seconds.max(0.0) / 60.0).floor() as u32;
    let hours = total_minutes / 60;
    let minutes = total_minutes % 60;
    match hours {
        0 => format!("{minutes}m today"),
        _ => format!("{hours}h {minutes}m today"),
    }
}

fn restore_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn hide_main_window(window: &WebviewWindow) {
    let _ = window.unminimize();
    let _ = window.hide();
}

fn reset_status_texts(last_time_text: &mut String, last_project_text: &mut String) {
    *last_time_text = NO_DATA_TEXT.to_string();
    *last_project_text = NO_PROJECT_TEXT.to_string();
}

pub fn setup(app: &App) -> tauri::Result<()> {
    let handle = app.handle();

    let time_item = MenuItemBuilder::with_id("time", "No data yet")
        .enabled(false)
        .build(handle)?;
    let project_item = MenuItemBuilder::with_id("project", "No active project")
        .enabled(false)
        .build(handle)?;
    let separator = PredefinedMenuItem::separator(handle)?;
    let show_item = MenuItemBuilder::with_id("show", "Show Flavortime").build(handle)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(handle)?;

    let menu = MenuBuilder::new(handle)
        .items(&[
            &time_item,
            &project_item,
            &separator,
            &show_item,
            &quit_item,
        ])
        .build()?;

    #[cfg(target_os = "macos")]
    let icon = Image::from_bytes(MACOS_TRAY_ICON_TEMPLATE)
        .or_else(|_| Image::from_path("icons/32x32.png"))
        .or_else(|_| {
            app.default_window_icon()
                .cloned()
                .ok_or(tauri::Error::AssetNotFound("tray icon".into()))
        })?;
    #[cfg(not(target_os = "macos"))]
    let icon = Image::from_path("icons/32x32.png").or_else(|_| {
        app.default_window_icon()
            .cloned()
            .ok_or(tauri::Error::AssetNotFound("tray icon".into()))
    })?;

    let tray_builder = TrayIconBuilder::new()
        .icon(icon)
        .tooltip("Flavortime")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => restore_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { .. } = event {
                if let Some(window) = tray.app_handle().get_webview_window("main") {
                    let is_visible = window.is_visible().unwrap_or(false);
                    let is_minimized = window.is_minimized().unwrap_or(false);

                    if is_visible && !is_minimized {
                        hide_main_window(&window);
                    } else {
                        restore_main_window(tray.app_handle());
                    }
                }
            }
        });
    #[cfg(target_os = "macos")]
    let tray_builder = tray_builder.icon_as_template(true);
    let tray = tray_builder.build(app)?;

    if let Some(window) = app.get_webview_window("main") {
        #[cfg(not(target_os = "macos"))]
        {
            let _ = window.set_skip_taskbar(true);
        }

        let w = window.clone();
        window.on_window_event(move |event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                hide_main_window(&w);
            }
            tauri::WindowEvent::Resized(_) => {
                if w.is_minimized().unwrap_or(false) {
                    hide_main_window(&w);
                }
            }
            _ => {}
        });
    }

    let app_handle = handle.clone();
    tauri::async_runtime::spawn(async move {
        let mut last_time_text = NO_DATA_TEXT.to_string();
        let mut last_project_text = NO_PROJECT_TEXT.to_string();

        loop {
            let auth = app_handle.try_state::<AppState>().and_then(|state| {
                let cfg = state.config.lock().ok()?;
                Some((cfg.auth_mode.clone(), cfg.slack_id.clone()))
            });

            match auth {
                Some((Mode::Hackatime, Some(slack_id))) => {
                    let window = rolling_24h_window();

                    if let Ok(projects) = Hackatime::user_projects_details(
                        &slack_id,
                        Some(window.start_rfc3339.as_str()),
                        Some(window.end_rfc3339.as_str()),
                    )
                    .await
                    {
                        let total_seconds = projects
                            .iter()
                            .map(|project| project.total_seconds.max(0.0))
                            .sum::<f64>();
                        last_time_text = format_hours(total_seconds);
                        last_project_text = latest_project(&projects, None)
                            .map(|(project, _)| format!("Working on {project}"))
                            .unwrap_or_else(|| "No active project".into());
                    }

                    let _ = time_item.set_text(&last_time_text);
                    let _ = project_item.set_text(&last_project_text);
                    let _ = tray.set_tooltip(Some(&format!("Flavortime — {}", last_time_text)));
                }
                Some((Mode::Adult, _)) => {
                    reset_status_texts(&mut last_time_text, &mut last_project_text);
                    let _ = time_item.set_text("Adult mode (no Hackatime)");
                    let _ = project_item.set_text(NO_PROJECT_TEXT);
                    let _ = tray.set_tooltip(Some("Flavortime"));
                }
                Some((Mode::Hackatime, None)) => {
                    reset_status_texts(&mut last_time_text, &mut last_project_text);
                    let _ = time_item.set_text("Flavortime disconnected");
                    let _ = project_item.set_text("Open Flavortime to reconnect");
                    let _ = tray.set_tooltip(Some("Flavortime"));
                }
                Some((Mode::None, _)) => {
                    reset_status_texts(&mut last_time_text, &mut last_project_text);
                    let _ = time_item.set_text("Not signed in");
                    let _ = project_item.set_text(NO_PROJECT_TEXT);
                    let _ = tray.set_tooltip(Some("Flavortime"));
                }
                None => {
                    let _ = time_item.set_text("Starting Flavortime...");
                    let _ = project_item.set_text("Loading status...");
                    let _ = tray.set_tooltip(Some("Flavortime"));
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    continue;
                }
            }

            tokio::time::sleep(Duration::from_secs(20)).await;
        }
    });

    Ok(())
}
