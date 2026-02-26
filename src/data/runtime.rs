use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct RuntimeToml {
    hackatime_base_url: Option<String>,
    pyramid_base_url: Option<String>,
    flavortown_base_url: Option<String>,
    flavortown_campaign_slug: Option<String>,
    discord_client_id: Option<u64>,
}

pub struct Runtime {
    pub hackatime_base_url: String,
    pub pyramid_base_url: String,
    pub flavortown_base_url: String,
    pub flavortown_campaign_slug: String,
    pub discord_client_id: u64,
}

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

pub fn runtime() -> &'static Runtime {
    RUNTIME.get().expect("runtime not initialized")
}

pub fn validate_startup_fields() -> Result<(), String> {
    if RUNTIME.get().is_some() {
        return Ok(());
    }

    let parsed = toml::from_str::<RuntimeToml>(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/config.toml"
    )))
    .map_err(|err| format!("Failed to parse config.toml: {err}"))?;
    let runtime = Runtime {
        hackatime_base_url: required_text(parsed.hackatime_base_url, "hackatime_base_url")?,
        pyramid_base_url: required_text(parsed.pyramid_base_url, "pyramid_base_url")?,
        flavortown_base_url: required_text(parsed.flavortown_base_url, "flavortown_base_url")?,
        flavortown_campaign_slug: required_text(
            parsed.flavortown_campaign_slug,
            "flavortown_campaign_slug",
        )?,
        discord_client_id: required(parsed.discord_client_id, "discord_client_id")?,
    };

    let _ = RUNTIME.set(runtime);
    Ok(())
}

fn required<T>(value: Option<T>, name: &str) -> Result<T, String> {
    value.ok_or_else(|| missing_field(name))
}

fn required_text(value: Option<String>, name: &str) -> Result<String, String> {
    let value = required(value, name)?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(missing_field(name));
    }
    Ok(trimmed.to_string())
}

fn missing_field(name: &str) -> String {
    format!("Missing required field: {name}")
}
