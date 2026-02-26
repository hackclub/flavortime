use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    None,
    Hackatime,
    Adult,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Referral {
    pub code: String,
    pub code_type: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub auth_mode: Mode,
    pub slack_id: Option<String>,
    pub flavortown_api_key: Option<String>,
    pub available_referral_codes: Vec<Referral>,
    pub selected_referral_code: Option<String>,
    pub custom_referral_code: Option<String>,
    pub show_referral_code: bool,
    pub show_time_tracking: bool,
    pub launch_at_startup: bool,
    pub app_enabled: bool,
    pub sharing_active_seconds_total: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auth_mode: Mode::None,
            slack_id: None,
            flavortown_api_key: None,
            available_referral_codes: Vec::new(),
            selected_referral_code: None,
            custom_referral_code: None,
            show_referral_code: true,
            show_time_tracking: true,
            launch_at_startup: false,
            app_enabled: true,
            sharing_active_seconds_total: 0,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        for path in [Self::path(), Self::backup_path()] {
            if let Some(config) = Self::load_from_path(&path) {
                return config;
            }
        }

        Self::default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::path();
        let backup = Self::backup_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }

        if path.exists() {
            let _ = fs::copy(&path, &backup);
        }

        let raw = serde_json::to_string_pretty(self).map_err(|err| err.to_string())?;
        fs::write(&path, &raw).map_err(|err| err.to_string())?;
        let _ = fs::write(&backup, &raw);
        Ok(())
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    fn path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("flavortime")
            .join("config.json")
    }

    fn backup_path() -> PathBuf {
        let mut path = Self::path();
        path.set_file_name("config.backup.json");
        path
    }

    fn load_from_path(path: &Path) -> Option<Self> {
        let raw = fs::read_to_string(path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    fn preferred_code(&self) -> Option<String> {
        self.available_referral_codes
            .iter()
            .find(|code| code.code_type == "custom")
            .or_else(|| self.available_referral_codes.first())
            .map(|code| code.code.clone())
    }

    pub fn ensure_selected_code(&mut self) -> bool {
        let is_selected_valid = self
            .selected_referral_code
            .as_ref()
            .is_some_and(|selected| {
                self.available_referral_codes
                    .iter()
                    .any(|code| code.code == *selected)
            });

        if is_selected_valid {
            return false;
        }

        let preferred = self.preferred_code();
        if preferred != self.selected_referral_code {
            self.selected_referral_code = preferred;
            return true;
        }
        false
    }

    pub fn display_code(&self) -> Option<String> {
        if !self.show_referral_code {
            return None;
        }

        self.custom_referral_code
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| self.selected_referral_code.clone())
            .or_else(|| self.preferred_code())
    }
}
