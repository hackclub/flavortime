#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod data;
mod services;
#[cfg(target_os = "macos")]
mod tray;

use app::commands::{
    check_for_update, close_flavortime_session, close_flavortime_session_for_shutdown,
    download_update, force_refresh_discord, get_discord_status, get_hackatime_data, get_status,
    init_discord, login_as_adult, login_with_flavortown_api_key, logout, open_external,
    refresh_referral_codes, restart_for_update, send_flavortown_heartbeat,
    set_adult_referral_code, set_app_enabled, set_custom_referral_code, set_launch_at_startup,
    set_selected_referral_code, set_show_referral_code, set_show_time_tracking,
    update_discord_presence,
};
use app::state::AppState;
use data::runtime::validate_startup_fields;
use std::time::Duration;
use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;

pub fn run() {
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
        if std::env::var_os("WEBKIT_DISABLE_COMPOSITING_MODE").is_none() {
            std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }
    }

    if let Err(err) = validate_startup_fields() {
        eprintln!("{err}");
        return;
    }

    #[cfg(target_os = "macos")]
    let start_hidden = std::env::args().any(|arg| arg == "--hidden");

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                app.set_dock_visibility(false);
            }

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .level_for(
                            "discord_presence::connection::os::unix",
                            log::LevelFilter::Off,
                        )
                        .level_for(
                            "discord_presence::connection::manager",
                            log::LevelFilter::Off,
                        )
                        .build(),
                )?;
            }

            let state = AppState::new();
            app.manage(state);

            #[cfg(target_os = "macos")]
            tray::setup(app)?;

            #[cfg(target_os = "macos")]
            {
                if start_hidden {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.hide();
                    }
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            login_with_flavortown_api_key,
            login_as_adult,
            logout,
            close_flavortime_session,
            set_selected_referral_code,
            set_custom_referral_code,
            set_show_referral_code,
            set_show_time_tracking,
            set_launch_at_startup,
            set_app_enabled,
            get_hackatime_data,
            refresh_referral_codes,
            init_discord,
            get_discord_status,
            force_refresh_discord,
            update_discord_presence,
            set_adult_referral_code,
            open_external,
            send_flavortown_heartbeat,
            check_for_update,
            download_update,
            restart_for_update,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(move |app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                let intercept_exit = app
                    .try_state::<AppState>()
                    .and_then(|state| {
                        state.shutdown_requested.lock().ok().map(|mut requested| {
                            if *requested {
                                false
                            } else {
                                *requested = true;
                                true
                            }
                        })
                    })
                    .unwrap_or(false);

                if intercept_exit {
                    api.prevent_exit();
                    let app_handle = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = tokio::time::timeout(
                            Duration::from_secs(3),
                            close_flavortime_session_for_shutdown(&app_handle),
                        )
                        .await;
                        app_handle.exit(0);
                    });
                }
            }
        });
}
