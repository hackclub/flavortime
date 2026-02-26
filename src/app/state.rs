use crate::data::config::Config;
use crate::services::discord::DiscordPresenceManager;
use std::sync::Mutex;

pub struct AppState {
    pub config: Mutex<Config>,
    pub discord: Mutex<Option<DiscordPresenceManager>>,
    pub flavortime_session_id: Mutex<Option<String>>,
    pub last_sharing_tick: Mutex<Option<u64>>,
    pub shutdown_requested: Mutex<bool>,
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for AppState {
    fn default() -> Self {
        let mut config = Config::load();
        if config.sharing_active_seconds_total != 0 {
            config.sharing_active_seconds_total = 0;
            let _ = config.save();
        }

        Self {
            config: Mutex::new(config),
            discord: Mutex::new(None),
            flavortime_session_id: Mutex::new(None),
            last_sharing_tick: Mutex::new(None),
            shutdown_requested: Mutex::new(false),
        }
    }
}
