use crate::data::runtime::runtime;
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct HeartbeatResponse {
    active_users: Option<u64>,
}

#[derive(Deserialize)]
struct FingerprintResponse {
    fingerprint: Option<String>,
}

#[derive(Deserialize)]
struct MeResponse {
    #[serde(default, alias = "slackId")]
    slack_id: Option<String>,
}

pub struct FlavortownUser {
    pub slack_id: String,
}

pub enum HeartbeatOutcome {
    ActiveUsers(u64),
    InvalidFingerprint,
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

pub async fn create_fingerprint(api_key: &str) -> Result<String, String> {
    let api_key = api_key.trim();
    let url = format!(
        "{}/api/v1/flavortime/fingerprint",
        runtime().flavortown_base_url
    );
    let response = reqwest::Client::new()
        .post(url)
        .header("Authorization", format!("Bearer {api_key}"))
        .send()
        .await
        .map_err(|err| err.to_string())?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Fingerprint creation failed: {status} {body}"));
    }

    let body = response
        .json::<FingerprintResponse>()
        .await
        .map_err(|err| err.to_string())?;

    body.fingerprint
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Flavortown did not return a fingerprint".to_string())
}

pub async fn send_heartbeat(
    api_key: &str,
    fingerprint: &str,
    sharing_active_seconds_total: u64,
) -> Result<HeartbeatOutcome, String> {
    let api_key = api_key.trim();
    let url = format!(
        "{}/api/v1/flavortime/heartbeat",
        runtime().flavortown_base_url
    );
    let payload = json!({
        "fingerprint": fingerprint,
        "sharing_active_seconds_total": sharing_active_seconds_total
    });

    let response = reqwest::Client::new()
        .post(url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("X-Flavortime-Fingerprint", fingerprint)
        .json(&payload)
        .send()
        .await
        .map_err(|err| err.to_string())?;

    if !response.status().is_success() {
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(HeartbeatOutcome::InvalidFingerprint);
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
