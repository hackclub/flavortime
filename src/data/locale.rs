use serde::Deserialize;
use std::sync::OnceLock;

pub struct RichPresenceText {
    pub details_project_prefix: String,
    pub details_idle: String,
    pub brand_label: String,
    pub referral_button: String,
    pub referral_host: String,
    pub time_today_prefix: String,
    pub time_logged_suffix: String,
    pub status_tagline: String,
}

#[derive(Deserialize)]
struct EnLocale {
    rich_presence: Option<RichPresenceLocale>,
}

#[derive(Deserialize, Default)]
struct RichPresenceLocale {
    details_project_prefix: Option<String>,
    details_idle: Option<String>,
    brand_label: Option<String>,
    referral_button: Option<String>,
    referral_host: Option<String>,
    time_today_prefix: Option<String>,
    time_logged_suffix: Option<String>,
    status_tagline: Option<String>,
}

static RICH_PRESENCE_TEXT: OnceLock<RichPresenceText> = OnceLock::new();

pub fn rich_presence_text() -> &'static RichPresenceText {
    RICH_PRESENCE_TEXT.get_or_init(load_rich_presence_text)
}

fn load_rich_presence_text() -> RichPresenceText {
    let locale = serde_json::from_str::<EnLocale>(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/web/locales/en.json"
    )))
    .unwrap_or(EnLocale {
        rich_presence: None,
    });
    let rich_presence = locale.rich_presence.unwrap_or_default();

    RichPresenceText {
        details_project_prefix: text_or_default(rich_presence.details_project_prefix, "Project: "),
        details_idle: text_or_default(rich_presence.details_idle, "Flavortown"),
        brand_label: text_or_default(rich_presence.brand_label, "Flavortown"),
        referral_button: text_or_default(rich_presence.referral_button, "Sign up"),
        referral_host: text_or_default(rich_presence.referral_host, "flavortown.hackclub.com"),
        time_today_prefix: text_or_default(rich_presence.time_today_prefix, "Today: "),
        time_logged_suffix: text_or_default(rich_presence.time_logged_suffix, " logged"),
        status_tagline: text_or_default(
            rich_presence.status_tagline,
            "Work on your personal projects, get rewarded with prizes. For teens ages <19",
        ),
    }
}

fn text_or_default(value: Option<String>, default: &str) -> String {
    value.unwrap_or_else(|| default.to_string())
}
