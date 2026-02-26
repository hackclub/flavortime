#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod data;
mod services;
#[cfg(target_os = "macos")]
mod tray;

use app::commands::{
    check_for_update, download_update, force_refresh_discord, get_discord_status,
    get_hackatime_data, get_status, init_discord, login_as_adult, login_with_flavortown_api_key,
    logout, open_external, refresh_referral_codes, restart_for_update, send_flavortown_heartbeat,
    set_adult_referral_code, set_app_enabled, set_custom_referral_code, set_launch_at_startup,
    set_selected_referral_code, set_show_referral_code, set_show_time_tracking,
    update_discord_presence,
};
use app::state::AppState;
use data::runtime::validate_startup_fields;
use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;

pub fn run() {
    if let Err(err) = validate_startup_fields() {
        eprintln!("{err}");
        return;
    }

    #[cfg(target_os = "macos")]
    let start_hidden = std::env::args().any(|arg| arg == "--hidden");

    tauri::Builder::default()
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
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
