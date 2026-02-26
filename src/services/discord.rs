use crate::data::locale::{rich_presence_text, RichPresenceText};
use discord_presence::{
    models::{ActivityType, DisplayType},
    Client, Event,
};
use std::{
    mem,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    thread::Builder,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Default)]
struct Presence {
    project: Option<String>,
    hours: Option<f64>,
    referral: Option<String>,
    show_referral_button: bool,
    enabled: bool,
    session_start: Option<u64>,
    activity_published: bool,
}

pub struct DiscordPresenceManager {
    client_id: u64,
    client: Client,
    ready: Arc<AtomicBool>,
    stopping: Arc<AtomicBool>,
    unready_since: Arc<AtomicU64>,
    last_restart: Arc<AtomicU64>,
    state: Presence,
}

impl DiscordPresenceManager {
    pub fn new(client_id: u64) -> Self {
        Self {
            client_id,
            client: Client::new(client_id),
            ready: Arc::new(AtomicBool::new(false)),
            stopping: Arc::new(AtomicBool::new(false)),
            unready_since: Arc::new(AtomicU64::new(0)),
            last_restart: Arc::new(AtomicU64::new(0)),
            state: Presence {
                enabled: true,
                ..Presence::default()
            },
        }
    }

    pub fn start(&mut self) {
        self.ready.store(false, Ordering::Relaxed);
        self.unready_since.store(unix_secs(), Ordering::Relaxed);
        let ready = Arc::clone(&self.ready);
        let stopping = Arc::clone(&self.stopping);
        let unready_since = Arc::clone(&self.unready_since);
        self.client
            .on_ready(move |_| {
                ready.store(true, Ordering::Relaxed);
                stopping.store(false, Ordering::Relaxed);
                unready_since.store(0, Ordering::Relaxed);
                log::info!("Discord Rich Presence connected");
            })
            .persist();

        let ready = Arc::clone(&self.ready);
        let stopping = Arc::clone(&self.stopping);
        let unready_since = Arc::clone(&self.unready_since);
        self.client
            .on_disconnected(move |_| {
                let was_ready = ready.swap(false, Ordering::Relaxed);
                if was_ready {
                    unready_since.store(unix_secs(), Ordering::Relaxed);
                }
                if !stopping.load(Ordering::Relaxed) {
                    log::debug!("Discord Rich Presence disconnected");
                }
            })
            .persist();

        let ready = Arc::clone(&self.ready);
        let stopping = Arc::clone(&self.stopping);
        let unready_since = Arc::clone(&self.unready_since);
        self.client
            .on_event(Event::Error, move |ctx| {
                let was_ready = ready.swap(false, Ordering::Relaxed);
                if was_ready {
                    unready_since.store(unix_secs(), Ordering::Relaxed);
                }
                if stopping.load(Ordering::Relaxed) {
                    return;
                }

                let event = format!("{:?}", ctx.event);
                if event.contains("Io Error") || is_transient_rpc_error(&event) {
                    log::debug!("Ignoring transient Discord RPC error: {}", event);
                    return;
                }
                log::error!("Discord RPC error: {}", event);
            })
            .persist();

        self.client.start();
    }

    pub fn update(
        &mut self,
        project: Option<String>,
        hours: Option<f64>,
        referral: Option<String>,
        show_referral_button: bool,
    ) {
        self.state.project = project;
        self.state.hours = hours;
        self.state.referral = referral;
        self.state.show_referral_button = show_referral_button;
        if self.has_activity_payload() {
            self.state.session_start.get_or_insert_with(unix_secs);
        } else {
            self.state.session_start = None;
        }
        self.sync();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.state.enabled = enabled;
        if enabled {
            self.maybe_recover();
            self.sync();
        } else {
            self.state.session_start = None;
            if self.state.activity_published {
                self.clear();
                self.state.activity_published = false;
            }
        }
    }

    pub fn stop(&mut self) {
        if self.stopping.swap(true, Ordering::Relaxed) {
            return;
        }

        self.ready.store(false, Ordering::Relaxed);
        self.unready_since.store(0, Ordering::Relaxed);
        let client = mem::replace(&mut self.client, Client::new(self.client_id));
        let stopping = Arc::clone(&self.stopping);
        spawn_named("discord-rpc-shutdown", move || {
            let _ = client.shutdown();
            stopping.store(false, Ordering::Relaxed);
        });
    }

    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }

    pub fn is_active(&self) -> bool {
        self.state.enabled && self.is_ready() && self.has_activity_payload()
    }

    pub fn refresh_activity(&mut self) {
        self.sync();
    }

    pub fn maybe_recover(&mut self) {
        if !self.state.enabled || self.stopping.load(Ordering::Relaxed) || self.is_ready() {
            return;
        }

        let now = unix_secs();
        let unready_since = self.unready_since.load(Ordering::Relaxed);
        if unready_since == 0 {
            return;
        }

        if now.saturating_sub(unready_since) < 18 {
            return;
        }

        let last_restart = self.last_restart.load(Ordering::Relaxed);
        if now.saturating_sub(last_restart) < 25 {
            return;
        }
        self.last_restart.store(now, Ordering::Relaxed);
        self.unready_since.store(0, Ordering::Relaxed);

        log::info!("Discord RPC stuck waiting, restarting RPC client");
        self.stop();
        self.start();
        self.sync();
    }

    pub fn reconnect_now(&mut self) {
        if self.stopping.load(Ordering::Relaxed) {
            return;
        }

        let now = unix_secs();
        self.last_restart.store(now, Ordering::Relaxed);
        self.unready_since.store(now, Ordering::Relaxed);
        self.stop();
        self.start();
    }

    pub fn force_refresh(&mut self) {
        if !self.state.enabled || self.stopping.load(Ordering::Relaxed) {
            return;
        }

        self.reconnect_now();
        self.sync();
    }

    fn sync(&mut self) {
        self.maybe_recover();
        if !self.state.enabled || !self.ready.load(Ordering::Relaxed) {
            return;
        }
        if !self.has_activity_payload() {
            if self.state.activity_published {
                self.clear();
                self.state.activity_published = false;
            }
            return;
        }

        let text = rich_presence_text();
        let referral_host = non_empty_trimmed(Some(text.referral_host.as_str()))
            .unwrap_or("flavortown.hackclub.com");
        let referral_code = non_empty_trimmed(self.state.referral.as_deref());
        let referral_url = match referral_code {
            Some(code) => format!("https://{referral_host}/{code}"),
            None => format!("https://{referral_host}"),
        };
        let project_line = if let Some(name) = self
            .state
            .project
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if let Some(hours) = self.state.hours.filter(|hours| *hours > 0.0) {
                let time = fmt_hours_short(hours);
                format!("{}{name} ({time} spent)", text.details_project_prefix)
            } else {
                format!("{}{name}", text.details_project_prefix)
            }
        } else {
            text.details_idle.clone()
        };

        let time_line = self
            .state
            .hours
            .filter(|hours| *hours > 0.0)
            .map(|hours| fmt_hours(hours, text));
        let status_tagline = text.status_tagline.trim();
        let details = if status_tagline.is_empty() {
            text.details_idle.clone()
        } else {
            status_tagline.to_string()
        };
        let state_line = Some(project_line.clone());
        let session_start = self.state.session_start;
        let show_referral_button = self.state.show_referral_button;
        let brand_label = text.brand_label.clone();
        let referral_button = text.referral_button.clone();
        let small_text = if status_tagline.is_empty() {
            None
        } else {
            time_line
        };

        let mut client = self.client.clone();
        spawn_named("discord-rpc-activity", move || {
            let result = client.set_activity(|activity| {
                let mut next = activity
                    .details(details)
                    .activity_type(ActivityType::Playing)
                    .status_display(DisplayType::Name)
                    .assets(|assets| {
                        let mut next = assets
                            .large_image("flavortown_logo")
                            .large_text(brand_label.clone());

                        if let Some(line) = small_text.as_ref() {
                            next = next.small_image("flavortown_logo").small_text(line.clone());
                        }

                        next
                    });

                if let Some(line) = state_line.as_ref() {
                    next = next.state(line.clone());
                }

                if let Some(start) = session_start {
                    next = next.timestamps(|ts| ts.start(start));
                }

                if show_referral_button {
                    next = next.append_buttons(|button| {
                        button
                            .label(referral_button.clone())
                            .url(referral_url.clone())
                    });
                }

                next
            });

            if let Err(err) = result {
                let message = err.to_string();
                if !message.contains("Io Error") && !message.contains("not started") {
                    log::error!("Failed to set Discord activity: {}", message);
                }
            }
        });
        self.state.activity_published = true;
    }

    fn clear(&mut self) {
        let mut client = self.client.clone();
        spawn_named("discord-rpc-clear", move || {
            let _ = client.clear_activity();
        });
    }

    fn has_activity_payload(&self) -> bool {
        let has_tagline = !rich_presence_text().status_tagline.trim().is_empty();
        let has_project = non_empty_trimmed(self.state.project.as_deref()).is_some();
        let has_hours = self.state.hours.is_some_and(|value| value > 0.0);
        let has_referral = self.state.show_referral_button;

        has_tagline || has_project || has_hours || has_referral
    }
}

impl Drop for DiscordPresenceManager {
    fn drop(&mut self) {
        self.stop();
    }
}

fn fmt_hours_short(hours: f64) -> String {
    let (whole, rem) = split_hours_minutes(hours);
    if whole > 0 {
        format!("{whole}h {rem}m")
    } else {
        format!("{rem}m")
    }
}

fn fmt_hours(hours: f64, text: &RichPresenceText) -> String {
    let (whole, rem) = split_hours_minutes(hours);
    if whole > 0 {
        format!(
            "{}{whole}h {rem}m{}",
            text.time_today_prefix, text.time_logged_suffix
        )
    } else {
        format!(
            "{}{rem}m{}",
            text.time_today_prefix, text.time_logged_suffix
        )
    }
}

fn split_hours_minutes(hours: f64) -> (u32, u32) {
    let mins = (hours * 60.0).round().max(0.0) as u32;
    (mins / 60, mins % 60)
}

fn spawn_named(name: &str, task: impl FnOnce() + Send + 'static) {
    if let Err(err) = Builder::new().name(name.to_string()).spawn(task) {
        log::error!("Failed to spawn thread `{name}`: {}", err);
    }
}

fn is_transient_rpc_error(event: &str) -> bool {
    event.contains("Error parsing Json")
        || event.contains("JsonError")
        || event.contains("missing field `cmd`")
}

fn non_empty_trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |delta| delta.as_secs())
}
