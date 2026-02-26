use crate::data::runtime::runtime;
use chrono::{DateTime, Duration as ChronoDuration, SecondsFormat, Utc};
use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct ProjectDetails {
    pub name: String,
    #[serde(default)]
    pub total_seconds: f64,
    #[serde(default)]
    pub last_heartbeat: Option<String>,
    #[serde(default)]
    pub most_recent_heartbeat: Option<String>,
}

#[derive(Deserialize)]
struct ProjectsDetailsResponse {
    #[serde(default)]
    projects: Vec<ProjectDetails>,
}

pub struct Hackatime;

pub struct RollingWindow {
    pub start_rfc3339: String,
    pub end_rfc3339: String,
    pub start_unix: u64,
}

pub fn rolling_24h_window() -> RollingWindow {
    let end = Utc::now();
    let start = end - ChronoDuration::hours(24);
    let start_unix = u64::try_from(start.timestamp()).unwrap_or_default();

    RollingWindow {
        start_rfc3339: start.to_rfc3339_opts(SecondsFormat::Secs, true),
        end_rfc3339: end.to_rfc3339_opts(SecondsFormat::Secs, true),
        start_unix,
    }
}

pub fn latest_project(
    projects: &[ProjectDetails],
    min_timestamp: Option<u64>,
) -> Option<(&str, u64)> {
    projects
        .iter()
        .filter_map(|project| {
            project_timestamp(project).and_then(|timestamp| {
                if min_timestamp.is_some_and(|min| timestamp < min) {
                    None
                } else {
                    Some((project.name.as_str(), timestamp))
                }
            })
        })
        .max_by_key(|(_, timestamp)| *timestamp)
}

fn project_timestamp(project: &ProjectDetails) -> Option<u64> {
    project
        .most_recent_heartbeat
        .as_deref()
        .and_then(parse_iso_timestamp)
        .or_else(|| {
            project
                .last_heartbeat
                .as_deref()
                .and_then(parse_iso_timestamp)
        })
}

fn parse_iso_timestamp(value: &str) -> Option<u64> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .and_then(|parsed| u64::try_from(parsed.timestamp()).ok())
}

impl Hackatime {
    pub async fn user_projects_details(
        username: &str,
        start_date: Option<&str>,
        end_date: Option<&str>,
    ) -> Result<Vec<ProjectDetails>, String> {
        let username = username.trim();
        if username.is_empty() {
            return Err("Missing Slack ID".to_string());
        }

        let mut url = format!(
            "{}/api/v1/users/{}/projects/details",
            runtime().hackatime_base_url,
            urlencoding::encode(username)
        );
        let client = reqwest::Client::new();

        let mut query_parts = Vec::<String>::new();
        if let Some(value) = start_date.filter(|value| !value.trim().is_empty()) {
            query_parts.push(format!("start_date={}", urlencoding::encode(value)));
            query_parts.push(format!("since={}", urlencoding::encode(value)));
        }
        if let Some(value) = end_date.filter(|value| !value.trim().is_empty()) {
            query_parts.push(format!("end_date={}", urlencoding::encode(value)));
            query_parts.push(format!("until={}", urlencoding::encode(value)));
        }
        if !query_parts.is_empty() {
            url.push('?');
            url.push_str(&query_parts.join("&"));
        }

        let response = client
            .get(url)
            .send()
            .await
            .map_err(|err| err.to_string())?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Failed to get project details: {status} {body}"));
        }

        let body = response
            .json::<ProjectsDetailsResponse>()
            .await
            .map_err(|err| err.to_string())?;
        Ok(body.projects)
    }
}
