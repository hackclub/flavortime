use crate::data::runtime::runtime;
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct HeartbeatResponse {
    active_users: Option<u64>,
}

#[derive(Deserialize)]
struct CloseResponse {
    active_users: Option<u64>,
}

#[derive(Deserialize)]
struct SessionIdResponse {
    #[serde(default, alias = "sessionId", alias = "session_id")]
    session_id: Option<String>,
}

#[derive(Deserialize)]
struct MeResponse {
    #[serde(default, alias = "slackId")]
    slack_id: Option<String>,
}

pub struct FlavortownUser {
    pub slack_id: String,
}

pub struct SessionMetadata {
    pub platform: &'static str,
    pub app_version: &'static str,
}

pub enum HeartbeatOutcome {
    ActiveUsers(u64),
    InvalidSessionId,
}

pub enum CloseOutcome {
    ActiveUsers(u64),
    InvalidSessionId,
}

pub fn session_metadata() -> SessionMetadata {
    SessionMetadata {
        platform: std::env::consts::OS,
        app_version: env!("CARGO_PKG_VERSION"),
    }
}

pub async fn current_user(api_key: &str) -> Result<FlavortownUser, String> {
    let api_key = api_key.trim();
    let url = format!("{}/api/v1/users/me", runtime().flavortown_base_url);
    let response = reqwest::Client::new()
        .get(url)
        .header("Authorization", format!("Bearer {api_key}"))
        .send()
        .await
        .map_err(|err| err.to_string())?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Flavortown login failed: {status} {body}"));
    }

    let body = response
        .json::<MeResponse>()
        .await
        .map_err(|err| err.to_string())?;
    let slack_id = body
        .slack_id
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "No Slack ID associated with this Flavortown account".to_string())?;

    Ok(FlavortownUser { slack_id })
}

pub async fn create_session(
    api_key: &str,
    platform: &str,
    app_version: &str,
) -> Result<String, String> {
    let api_key = api_key.trim();
    let url = format!(
        "{}/api/v1/flavortime/session",
        runtime().flavortown_base_url
    );
    let payload = json!({
        "platform": platform,
        "app_version": app_version
    });

    let response = reqwest::Client::new()
        .post(url)
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&payload)
        .send()
        .await
        .map_err(|err| err.to_string())?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Session creation failed: {status} {body}"));
    }

    let body = response
        .json::<SessionIdResponse>()
        .await
        .map_err(|err| err.to_string())?;

    body.session_id
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Flavortown did not return a session ID".to_string())
}

pub async fn send_heartbeat(
    api_key: &str,
    session_id: &str,
    sharing_active_seconds_total: u64,
    platform: &str,
    app_version: &str,
) -> Result<HeartbeatOutcome, String> {
    let api_key = api_key.trim();
    let url = format!(
        "{}/api/v1/flavortime/heartbeat",
        runtime().flavortown_base_url
    );
    let payload = json!({
        "session_id": session_id,
        "sharing_active_seconds_total": sharing_active_seconds_total,
        "platform": platform,
        "app_version": app_version
    });

    let response = reqwest::Client::new()
        .post(url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("X-Flavortime-Session-Id", session_id)
        .json(&payload)
        .send()
        .await
        .map_err(|err| err.to_string())?;

    if !response.status().is_success() {
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(HeartbeatOutcome::InvalidSessionId);
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Heartbeat failed: {status} {body}"));
    }

    let body = response
        .json::<HeartbeatResponse>()
        .await
        .map_err(|err| err.to_string())?;

    Ok(HeartbeatOutcome::ActiveUsers(
        body.active_users.unwrap_or(0),
    ))
}

pub async fn close_session(
    api_key: &str,
    session_id: &str,
    sharing_active_seconds_total: u64,
    platform: &str,
    app_version: &str,
) -> Result<CloseOutcome, String> {
    let api_key = api_key.trim();
    let url = format!("{}/api/v1/flavortime/close", runtime().flavortown_base_url);
    let payload = json!({
        "session_id": session_id,
        "sharing_active_seconds_total": sharing_active_seconds_total,
        "platform": platform,
        "app_version": app_version
    });

    let response = reqwest::Client::new()
        .post(url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("X-Flavortime-Session-Id", session_id)
        .json(&payload)
        .send()
        .await
        .map_err(|err| err.to_string())?;

    if !response.status().is_success() {
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(CloseOutcome::InvalidSessionId);
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Close session failed: {status} {body}"));
    }

    let body = response
        .json::<CloseResponse>()
        .await
        .map_err(|err| err.to_string())?;

    Ok(CloseOutcome::ActiveUsers(body.active_users.unwrap_or(0)))
}
