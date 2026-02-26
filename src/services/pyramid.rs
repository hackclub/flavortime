use crate::data::{config::Referral, runtime::runtime};
use serde::Deserialize;

#[derive(Deserialize)]
struct ReferralCode {
    code: Option<String>,
    #[serde(rename = "type")]
    code_type: Option<String>,
}

#[derive(Deserialize)]
struct LookupResponse {
    error: Option<String>,
    #[serde(default)]
    codes: Vec<ReferralCode>,
}

pub async fn fetch_codes(slack_id: &str) -> Result<Vec<Referral>, String> {
    let rt = runtime();
    let url = format!(
        "{}/api/v1/codes/lookup?slack_id={}&campaign_slug={}",
        rt.pyramid_base_url,
        urlencoding::encode(slack_id),
        rt.flavortown_campaign_slug
    );

    let res = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .map_err(|err| err.to_string())?;

    if !res.status().is_success() {
        return Err(format!("Referral lookup failed: {}", res.status()));
    }

    let body = res
        .json::<LookupResponse>()
        .await
        .map_err(|err| err.to_string())?;

    if let Some(error) = body.error.as_deref() {
        return if error == "User not found" {
            Ok(Vec::new())
        } else {
            Err(error.to_string())
        };
    }

    Ok(body
        .codes
        .into_iter()
        .filter_map(|item| {
            Some(Referral {
                code: item.code?,
                code_type: item.code_type?,
            })
        })
        .collect())
}
