use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;

pub enum AnthropicAuth {
    ApiKey(String),
    OAuth {
        access_token: String,
        refresh_token: String,
        expires_at: i64,
    },
}

pub(super) struct AuthResolved {
    pub header_name: String,
    pub header_value: String,
    pub is_oauth: bool,
}

pub(super) async fn refresh_oauth_token(
    client: &reqwest::Client,
    refresh_token: &str,
) -> anyhow::Result<(String, i64)> {
    let resp = client
        .post("https://console.anthropic.com/v1/oauth/token")
        .json(&serde_json::json!({
            "grant_type": "refresh_token",
            "refresh_token": refresh_token,
            "client_id": "9d1c250a-e61b-44d9-88ed-5944d1962f5e",
        }))
        .send()
        .await
        .context("Failed to send OAuth refresh request")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("OAuth refresh failed {status}: {body}"));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .context("Failed to parse OAuth refresh response")?;
    let access_token = data["access_token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No access_token in refresh response"))?
        .to_string();
    let expires_in = data["expires_in"].as_i64().unwrap_or(3600);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let expires_at = now + expires_in;

    Ok((access_token, expires_at))
}
