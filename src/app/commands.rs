use super::state::AppState;
use crate::data::{
    config::{Config, Mode, Referral},
    runtime::runtime,
};
use crate::services::{
    discord::DiscordPresenceManager,
    flavortown,
    hackatime::{latest_project, rolling_24h_window, Hackatime},
    pyramid::fetch_codes,
};
use serde::Serialize;
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_updater::UpdaterExt;

#[derive(Serialize)]
pub struct Status {
    pub auth_mode: String,
    pub slack_id: Option<String>,
    pub referral_codes: Vec<Referral>,
    pub selected_referral_code: Option<String>,
    pub custom_referral_code: Option<String>,
    pub show_referral_code: bool,
    pub show_time_tracking: bool,
    pub launch_at_startup: bool,
    pub app_enabled: bool,
}

#[derive(Serialize)]
pub struct Project {
    pub name: String,
    pub hours: f64,
}

#[derive(Serialize)]
pub struct HackatimeData {
    pub current_project: Option<Project>,
    pub total_hours: f64,
    pub heartbeat_idle: bool,
    pub sharing_active_seconds_total: u64,
}

#[derive(Serialize)]
pub struct DiscordStatus {
    pub connected: bool,
    pub enabled: bool,
    pub active: bool,
    pub flatpak_discord_detected: bool,
}

#[derive(Serialize)]
pub struct UpdaterStatus {
    pub update_available: bool,
    pub dev_mode: bool,
}

struct HackatimeSnapshot {
    current_project: Option<Project>,
    total_hours: f64,
    heartbeat_idle: bool,
}

#[tauri::command]
pub fn open_external(url: String) -> Result<(), String> {
    let url = url.trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("Invalid URL".to_string());
    }
    webbrowser::open(url).map_err(stringify)?;
    Ok(())
}

#[tauri::command]
pub fn get_discord_status(state: State<AppState>) -> Result<DiscordStatus, String> {
    let enabled = lock(&state.config)?.app_enabled;
    maybe_ensure_discord_client(&state.discord, enabled)?;
    let (connected, active) = poll_discord_status(&state.discord, enabled, false)?;
    Ok(discord_status(enabled, connected, active))
}

#[tauri::command]
pub fn force_refresh_discord(state: State<AppState>) -> Result<DiscordStatus, String> {
    let enabled = lock(&state.config)?.app_enabled;
    maybe_ensure_discord_client(&state.discord, enabled)?;
    let (connected, active) = poll_discord_status(&state.discord, enabled, true)?;
    Ok(discord_status(enabled, connected, active))
}

#[tauri::command]
pub fn get_status(app: AppHandle, state: State<AppState>) -> Result<Status, String> {
    let mut cfg = lock(&state.config)?;
    let mut changed = false;

    match app.autolaunch().is_enabled() {
        Ok(autostart) => {
            if cfg.launch_at_startup != autostart {
                cfg.launch_at_startup = autostart;
                changed = true;
            }
        }
        Err(err) => {
            log::warn!(
                "Autostart status check failed in get_status, keeping saved value: {}",
                err
            );
        }
    }

    if cfg.ensure_selected_code() {
        changed = true;
    }

    if changed {
        cfg.save()?;
    }

    Ok(Status {
        auth_mode: mode_name(&cfg.auth_mode).to_string(),
        slack_id: cfg.slack_id.clone(),
        referral_codes: cfg.available_referral_codes.clone(),
        selected_referral_code: cfg.selected_referral_code.clone(),
        custom_referral_code: cfg.custom_referral_code.clone(),
        show_referral_code: cfg.show_referral_code,
        show_time_tracking: cfg.show_time_tracking,
        launch_at_startup: cfg.launch_at_startup,
        app_enabled: cfg.app_enabled,
    })
}

#[tauri::command]
pub async fn login_with_flavortown_api_key(
    state: State<'_, AppState>,
    api_key: String,
) -> Result<bool, String> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err("API key is required".to_string());
    }

    let user = flavortown::current_user(api_key).await?;
    let slack_id = user.slack_id;

    let window = rolling_24h_window();
    let _ = Hackatime::user_projects_details(
        &slack_id,
        Some(window.start_rfc3339.as_str()),
        Some(window.end_rfc3339.as_str()),
    )
    .await?;

    let codes = fetch_codes(&slack_id).await.unwrap_or_else(|err| {
        log::warn!("Failed to fetch referral codes after login: {err}");
        Vec::new()
    });

    let should_reconnect = {
        let mut cfg = lock(&state.config)?;
        cfg.auth_mode = Mode::Hackatime;
        cfg.flavortown_api_key = Some(api_key.to_string());
        cfg.slack_id = Some(slack_id);
        cfg.available_referral_codes = codes;
        cfg.show_time_tracking = true;
        cfg.ensure_selected_code();
        cfg.save()?;
        cfg.app_enabled
    };

    *lock(&state.flavortime_fingerprint)? = None;
    reset_sharing_session(&state)?;
    ensure_flavortime_fingerprint(&state, api_key).await?;

    if should_reconnect {
        if let Err(err) = (|| -> Result<(), String> {
            ensure_discord_client(&state.discord)?;
            retry_discord_connection(&state.discord)?;
            let cfg = lock(&state.config)?;
            sync_discord(&cfg, &state.discord)?;
            Ok(())
        })() {
            log::warn!("Discord reconnection after login failed (non-fatal): {err}");
        }
    }
    Ok(true)
}

#[tauri::command]
pub fn login_as_adult(state: State<AppState>) -> Result<(), String> {
    let should_reconnect = {
        let mut cfg = lock(&state.config)?;
        cfg.auth_mode = Mode::Adult;
        cfg.flavortown_api_key = None;
        cfg.slack_id = None;
        cfg.available_referral_codes.clear();
        cfg.selected_referral_code = None;
        cfg.show_time_tracking = false;
        cfg.save()?;
        cfg.app_enabled
    };

    *lock(&state.flavortime_fingerprint)? = None;
    reset_sharing_session(&state)?;

    if should_reconnect {
        ensure_discord_client(&state.discord)?;
        retry_discord_connection(&state.discord)?;
    }

    let cfg = lock(&state.config)?;
    sync_discord(&cfg, &state.discord)
}

#[tauri::command]
pub fn logout(state: State<AppState>) -> Result<(), String> {
    {
        let mut cfg = lock(&state.config)?;
        cfg.reset();
        cfg.save()?;
    }

    let mut rpc = lock(&state.discord)?;
    if let Some(client) = rpc.as_mut() {
        client.stop();
    }
    *rpc = None;
    *lock(&state.flavortime_fingerprint)? = None;
    *lock(&state.last_sharing_tick)? = None;
    Ok(())
}

#[tauri::command]
pub fn set_selected_referral_code(
    state: State<AppState>,
    code: Option<String>,
) -> Result<(), String> {
    let mut cfg = lock(&state.config)?;
    cfg.selected_referral_code = trimmed(code);
    cfg.save()?;
    sync_discord(&cfg, &state.discord)
}

#[tauri::command]
pub fn set_custom_referral_code(
    state: State<AppState>,
    code: Option<String>,
) -> Result<(), String> {
    let mut cfg = lock(&state.config)?;
    cfg.custom_referral_code = trimmed(code);
    cfg.save()?;
    sync_discord(&cfg, &state.discord)
}

#[tauri::command]
pub fn set_show_referral_code(state: State<AppState>, show: bool) -> Result<(), String> {
    let mut cfg = lock(&state.config)?;
    cfg.show_referral_code = show;
    cfg.save()?;
    sync_discord(&cfg, &state.discord)
}

#[tauri::command]
pub fn set_show_time_tracking(state: State<AppState>, show: bool) -> Result<(), String> {
    let mut cfg = lock(&state.config)?;
    cfg.show_time_tracking = show;
    cfg.save()
}

#[tauri::command]
pub fn set_launch_at_startup(
    app: AppHandle,
    state: State<AppState>,
    enabled: bool,
) -> Result<(), String> {
    let autolaunch = app.autolaunch();
    if enabled {
        autolaunch.enable().map_err(stringify)?;
    } else {
        autolaunch.disable().map_err(stringify)?;
    }

    let mut cfg = lock(&state.config)?;
    cfg.launch_at_startup = enabled;
    cfg.save()
}

#[tauri::command]
pub fn set_app_enabled(state: State<AppState>, enabled: bool) -> Result<(), String> {
    {
        let mut cfg = lock(&state.config)?;
        cfg.app_enabled = enabled;
        cfg.save()?;
    }

    if enabled {
        ensure_discord_client(&state.discord)?;
    }

    let mut rpc = lock(&state.discord)?;
    if let Some(client) = rpc.as_mut() {
        client.set_enabled(enabled);
        client.maybe_recover();
    }
    Ok(())
}

#[tauri::command]
pub async fn get_hackatime_data(state: State<'_, AppState>) -> Result<HackatimeData, String> {
    let (auth_mode, slack_id, app_enabled, show_time_tracking, show_referral_code) = {
        let cfg = lock(&state.config)?;
        (
            cfg.auth_mode.clone(),
            required(cfg.slack_id.clone(), "Not logged in with Flavortime")?,
            cfg.app_enabled,
            cfg.show_time_tracking,
            cfg.show_referral_code,
        )
    };

    if !matches!(auth_mode, Mode::Hackatime) {
        return Err("Not logged in with Flavortime".to_string());
    }

    let snapshot = fetch_hackatime_snapshot(&slack_id).await?;

    let sharing_enabled = app_enabled && (show_time_tracking || show_referral_code);
    let discord_connected = {
        let mut rpc = lock(&state.discord)?;
        if let Some(client) = rpc.as_mut() {
            client.maybe_recover();
            client.refresh_activity();
            client.is_ready()
        } else {
            false
        }
    };

    let should_accumulate = sharing_enabled && discord_connected && !snapshot.heartbeat_idle;
    let sharing_active_seconds_total = accumulate_sharing_seconds(&state, should_accumulate)?;

    Ok(HackatimeData {
        current_project: snapshot.current_project,
        total_hours: snapshot.total_hours,
        heartbeat_idle: snapshot.heartbeat_idle,
        sharing_active_seconds_total,
    })
}

#[tauri::command]
pub async fn send_flavortown_heartbeat(state: State<'_, AppState>) -> Result<u64, String> {
    let (auth_mode, api_key, sharing_active_seconds_total) = {
        let cfg = lock(&state.config)?;
        (
            cfg.auth_mode.clone(),
            cfg.flavortown_api_key.clone(),
            cfg.sharing_active_seconds_total,
        )
    };

    if !matches!(auth_mode, Mode::Hackatime) {
        return Ok(0);
    }

    let api_key = match api_key {
        Some(value) if !value.trim().is_empty() => value,
        _ => return Ok(0),
    };

    let fingerprint = ensure_flavortime_fingerprint(&state, &api_key).await?;

    match flavortown::send_heartbeat(&api_key, &fingerprint, sharing_active_seconds_total).await? {
        flavortown::HeartbeatOutcome::ActiveUsers(count) => Ok(count),
        flavortown::HeartbeatOutcome::InvalidFingerprint => {
            let fingerprint = rotate_flavortime_fingerprint(&state, &api_key).await?;
            let sharing_total_after_rotate = lock(&state.config)?.sharing_active_seconds_total;

            match flavortown::send_heartbeat(&api_key, &fingerprint, sharing_total_after_rotate)
                .await?
            {
                flavortown::HeartbeatOutcome::ActiveUsers(count) => Ok(count),
                flavortown::HeartbeatOutcome::InvalidFingerprint => {
                    Err("Flavortown rejected fingerprint after rotation".to_string())
                }
            }
        }
    }
}

#[tauri::command]
pub async fn refresh_referral_codes(state: State<'_, AppState>) -> Result<Vec<Referral>, String> {
    let slack_id = {
        let cfg = lock(&state.config)?;
        required(cfg.slack_id.clone(), "No Slack ID available")?
    };
    let codes = fetch_codes(&slack_id).await?;

    let mut cfg = lock(&state.config)?;
    cfg.available_referral_codes = codes.clone();
    cfg.ensure_selected_code();
    cfg.save()?;
    Ok(codes)
}

#[tauri::command]
pub fn init_discord(state: State<AppState>) -> Result<(), String> {
    let (enabled, referral) = {
        let cfg = lock(&state.config)?;
        (cfg.app_enabled, cfg.display_code())
    };

    ensure_discord_client(&state.discord)?;

    let mut rpc = lock(&state.discord)?;
    if let Some(client) = rpc.as_mut() {
        client.maybe_recover();
        client.set_enabled(enabled);
        client.update(None, None, referral);
    }
    Ok(())
}

#[tauri::command]
pub fn update_discord_presence(
    state: State<AppState>,
    project: Option<String>,
    hours: Option<f64>,
) -> Result<(), String> {
    let (show_time, referral, enabled) = {
        let cfg = lock(&state.config)?;
        (cfg.show_time_tracking, cfg.display_code(), cfg.app_enabled)
    };
    let hours = if show_time { hours } else { None };

    if enabled {
        ensure_discord_client(&state.discord)?;
    }

    let mut rpc = lock(&state.discord)?;
    if let Some(client) = rpc.as_mut() {
        client.maybe_recover();
        client.update(project, hours, referral);
    }
    Ok(())
}

#[tauri::command]
pub fn set_adult_referral_code(state: State<AppState>, code: String) -> Result<(), String> {
    let mut cfg = lock(&state.config)?;
    cfg.custom_referral_code = trimmed(Some(code));
    cfg.save()?;
    sync_discord(&cfg, &state.discord)
}

#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> Result<UpdaterStatus, String> {
    let updater = app.updater().map_err(stringify)?;
    let update_available = updater.check().await.map_err(stringify)?.is_some();

    Ok(UpdaterStatus {
        update_available,
        dev_mode: cfg!(debug_assertions),
    })
}

#[tauri::command]
pub async fn download_update(app: AppHandle) -> Result<(), String> {
    let updater = app.updater().map_err(stringify)?;
    let update = updater
        .check()
        .await
        .map_err(stringify)?
        .ok_or_else(|| "No update is currently available".to_string())?;

    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(stringify)
}

#[tauri::command]
pub fn restart_for_update(app: AppHandle) -> Result<(), String> {
    app.restart()
}

fn accumulate_sharing_seconds(state: &AppState, session_active: bool) -> Result<u64, String> {
    let now = unix_now_secs();
    let mut last_tick = lock(&state.last_sharing_tick)?;
    let mut cfg = lock(&state.config)?;

    if let Some(previous) = *last_tick {
        let elapsed = now.saturating_sub(previous).min(120);
        if elapsed > 0 && session_active {
            cfg.sharing_active_seconds_total =
                cfg.sharing_active_seconds_total.saturating_add(elapsed);
            cfg.save()?;
        }
    }

    *last_tick = Some(now);
    Ok(cfg.sharing_active_seconds_total)
}

async fn fetch_hackatime_snapshot(slack_id: &str) -> Result<HackatimeSnapshot, String> {
    let window = rolling_24h_window();
    let projects = Hackatime::user_projects_details(
        slack_id,
        Some(window.start_rfc3339.as_str()),
        Some(window.end_rfc3339.as_str()),
    )
    .await?;

    let total_seconds = projects
        .iter()
        .map(|project| project.total_seconds.max(0.0))
        .sum::<f64>();
    let total_hours = total_seconds / 3600.0;

    let latest = latest_project(&projects, Some(window.start_unix));
    let heartbeat_idle = latest
        .as_ref()
        .map(|(_, unix_time)| unix_now_secs().saturating_sub(*unix_time) > 180)
        .unwrap_or(true);

    let current_project = latest.map(|(name, _)| Project {
        name: name.to_string(),
        hours: total_hours,
    });

    Ok(HackatimeSnapshot {
        current_project,
        total_hours,
        heartbeat_idle,
    })
}

async fn ensure_flavortime_fingerprint(state: &AppState, api_key: &str) -> Result<String, String> {
    if let Some(existing) = lock(&state.flavortime_fingerprint)?.clone() {
        return Ok(existing);
    }

    rotate_flavortime_fingerprint(state, api_key).await
}

async fn rotate_flavortime_fingerprint(state: &AppState, api_key: &str) -> Result<String, String> {
    let fingerprint = flavortown::create_fingerprint(api_key).await?;
    *lock(&state.flavortime_fingerprint)? = Some(fingerprint.clone());
    reset_sharing_session(state)?;
    Ok(fingerprint)
}

fn reset_sharing_session(state: &AppState) -> Result<(), String> {
    {
        let mut cfg = lock(&state.config)?;
        cfg.sharing_active_seconds_total = 0;
        cfg.save()?;
    }

    *lock(&state.last_sharing_tick)? = None;
    Ok(())
}

fn sync_discord(cfg: &Config, rpc: &Mutex<Option<DiscordPresenceManager>>) -> Result<(), String> {
    if cfg.app_enabled {
        ensure_discord_client(rpc)?;
    }

    let mut rpc = lock(rpc)?;
    if let Some(client) = rpc.as_mut() {
        client.set_enabled(cfg.app_enabled);
        client.maybe_recover();
        client.update(None, None, cfg.display_code());
    }
    Ok(())
}

fn ensure_discord_client(rpc: &Mutex<Option<DiscordPresenceManager>>) -> Result<(), String> {
    let mut rpc = lock(rpc)?;
    if rpc.is_none() {
        let mut client = DiscordPresenceManager::new(runtime().discord_client_id);
        client.start();
        *rpc = Some(client);
    }
    Ok(())
}

fn maybe_ensure_discord_client(
    rpc: &Mutex<Option<DiscordPresenceManager>>,
    enabled: bool,
) -> Result<(), String> {
    if enabled {
        ensure_discord_client(rpc)?;
    }
    Ok(())
}

fn poll_discord_status(
    rpc: &Mutex<Option<DiscordPresenceManager>>,
    enabled: bool,
    force_refresh: bool,
) -> Result<(bool, bool), String> {
    let mut rpc = lock(rpc)?;
    if let Some(client) = rpc.as_mut() {
        if enabled && force_refresh {
            client.force_refresh();
        }
        client.maybe_recover();
        client.refresh_activity();
        Ok((client.is_ready(), client.is_active()))
    } else {
        Ok((false, false))
    }
}

fn retry_discord_connection(rpc: &Mutex<Option<DiscordPresenceManager>>) -> Result<(), String> {
    let mut rpc = lock(rpc)?;
    if let Some(client) = rpc.as_mut() {
        client.reconnect_now();
    }
    Ok(())
}

fn discord_status(enabled: bool, connected: bool, active: bool) -> DiscordStatus {
    DiscordStatus {
        connected,
        enabled,
        active: active && enabled,
        flatpak_discord_detected: flatpak_discord_detected(),
    }
}

#[cfg(target_os = "linux")]
fn flatpak_discord_detected() -> bool {
    static DETECTED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *DETECTED.get_or_init(detect_flatpak_discord)
}

#[cfg(target_os = "linux")]
fn detect_flatpak_discord() -> bool {
    use std::path::PathBuf;
    use std::process::Command;

    let mut candidates = vec![
        PathBuf::from("/var/lib/flatpak/app/com.discordapp.Discord"),
        PathBuf::from("/var/lib/flatpak/exports/bin/com.discordapp.Discord"),
    ];

    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".local/share/flatpak/app/com.discordapp.Discord"));
        candidates.push(home.join(".local/share/flatpak/exports/bin/com.discordapp.Discord"));
    }

    if candidates.iter().any(|path| path.exists()) {
        return true;
    }

    Command::new("flatpak")
        .args(["info", "com.discordapp.Discord"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(not(target_os = "linux"))]
fn flatpak_discord_detected() -> bool {
    false
}

fn lock<T>(mutex: &Mutex<T>) -> Result<MutexGuard<'_, T>, String> {
    mutex
        .lock()
        .map_err(|_| "Internal state lock failed".to_string())
}

fn mode_name(mode: &Mode) -> &'static str {
    match mode {
        Mode::None => "none",
        Mode::Hackatime => "hackatime",
        Mode::Adult => "adult",
    }
}

fn trimmed(value: Option<String>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn stringify(err: impl ToString) -> String {
    err.to_string()
}

fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn required<T>(value: Option<T>, message: &str) -> Result<T, String> {
    value.ok_or_else(|| message.to_string())
}
