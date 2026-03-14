use anyhow::{Context, Result, bail};
use crossterm::{
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
};
use serde::Deserialize;
use std::io::{self, Write};
use std::path::PathBuf;

use super::ProviderCredential;

const COPILOT_CLIENT_ID: &str = "Iv1.b507a08c87ecfe98";
const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const TOKEN_URL: &str = "https://github.com/login/oauth/access_token";

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[allow(dead_code)]
    expires_in: u64,
    interval: u64,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    #[serde(default)]
    access_token: String,
    #[serde(default)]
    error: String,
    #[serde(default)]
    interval: Option<u64>,
}

fn apps_json_path() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME")
        && !xdg.is_empty()
    {
        return PathBuf::from(xdg).join("github-copilot").join("apps.json");
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("github-copilot")
        .join("apps.json")
}

/// Try to read an existing Copilot OAuth token from ~/.config/github-copilot/apps.json
pub fn read_existing_token() -> Option<String> {
    let path = apps_json_path();
    let content = std::fs::read_to_string(&path).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&content).ok()?;
    let obj = parsed.as_object()?;
    obj.iter().find_map(|(key, value)| {
        if key.starts_with("github.com") {
            value["oauth_token"].as_str().map(|v| v.to_string())
        } else {
            None
        }
    })
}

fn save_to_apps_json(token: &str) -> Result<()> {
    let path = apps_json_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }

    let key = format!("github.com:{}", COPILOT_CLIENT_ID);
    let existing: serde_json::Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    let mut obj = existing
        .as_object()
        .cloned()
        .unwrap_or_else(serde_json::Map::new);
    obj.insert(
        key,
        serde_json::json!({
            "oauth_token": token,
            "githubAppId": COPILOT_CLIENT_ID,
        }),
    );

    std::fs::write(&path, serde_json::to_string_pretty(&obj)?)
        .with_context(|| format!("writing {}", path.display()))
}

pub(super) async fn device_flow() -> Result<ProviderCredential> {
    let mut stdout = io::stdout();

    let client = reqwest::Client::new();
    let resp = client
        .post(DEVICE_CODE_URL)
        .header("Accept", "application/json")
        .form(&[("client_id", COPILOT_CLIENT_ID), ("scope", "read:user")])
        .send()
        .await
        .context("requesting device code")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        bail!("device code request failed ({}): {}", status, body);
    }

    let device: DeviceCodeResponse = resp.json().await.context("parsing device code response")?;

    execute!(
        stdout,
        Print("\r\n"),
        SetForegroundColor(Color::Yellow),
        Print("  Enter this code at GitHub:\r\n\r\n"),
        ResetColor,
        SetForegroundColor(Color::White),
        Print(format!("    {}\r\n\r\n", device.user_code)),
        ResetColor,
        SetForegroundColor(Color::DarkGrey),
        Print(format!("  URL: {}\r\n\r\n", device.verification_uri)),
        ResetColor,
        SetForegroundColor(Color::Yellow),
        Print("  Waiting for authorization...\r\n"),
        ResetColor,
    )?;
    stdout.flush()?;

    if let Err(e) = open::that(&device.verification_uri) {
        execute!(
            stdout,
            SetForegroundColor(Color::Red),
            Print(format!("  Could not open browser: {}\r\n", e)),
            ResetColor,
        )?;
    }

    let mut interval = device.interval;
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(interval)).await;

        let resp = client
            .post(TOKEN_URL)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", COPILOT_CLIENT_ID),
                ("device_code", &device.device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await
            .context("polling for token")?;

        let token: TokenResponse = resp.json().await.context("parsing token response")?;

        if !token.access_token.is_empty() {
            let _ = save_to_apps_json(&token.access_token);
            return Ok(ProviderCredential::OAuth {
                access_token: token.access_token,
                refresh_token: None,
                expires_at: None,
                api_key: None,
            });
        }

        match token.error.as_str() {
            "authorization_pending" => continue,
            "slow_down" => {
                interval = token.interval.unwrap_or(interval + 5);
                continue;
            }
            "expired_token" => bail!("device code expired — please try again"),
            "access_denied" => bail!("authorization was denied"),
            other => bail!("token exchange error: {}", other),
        }
    }
}

pub async fn copilot_login() -> Result<ProviderCredential> {
    let mut stdout = io::stdout();

    if let Some(token) = read_existing_token() {
        execute!(
            stdout,
            Print("\r\n"),
            SetForegroundColor(Color::Green),
            Print("  Found existing Copilot token.\r\n"),
            ResetColor,
        )?;
        stdout.flush()?;
        return Ok(ProviderCredential::OAuth {
            access_token: token,
            refresh_token: None,
            expires_at: None,
            api_key: None,
        });
    }

    device_flow().await
}
