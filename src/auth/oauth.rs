use anyhow::{Context, Result, anyhow};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use crossterm::{execute, style::{Color, Print, ResetColor, SetForegroundColor}};
use rand::RngCore;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::{self, Write};

use super::ProviderCredential;
use super::ui::read_input_raw;

const ANTHROPIC_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const ANTHROPIC_AUTH_URL_MAX: &str = "https://claude.ai/oauth/authorize";
const ANTHROPIC_AUTH_URL_CONSOLE: &str = "https://console.anthropic.com/oauth/authorize";
const ANTHROPIC_TOKEN_URL: &str = "https://console.anthropic.com/v1/oauth/token";
const ANTHROPIC_CREATE_KEY_URL: &str =
    "https://api.anthropic.com/api/oauth/claude_cli/create_api_key";
const REDIRECT_URI: &str = "https://console.anthropic.com/oauth/code/callback";
const ANTHROPIC_SCOPES: &str = "org:create_api_key user:profile user:inference";

#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
}

fn generate_pkce() -> (String, String) {
    let mut rng = rand::thread_rng();
    let mut bytes = [0u8; 32];
    rng.fill_bytes(&mut bytes);
    let verifier = URL_SAFE_NO_PAD.encode(bytes);

    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let digest = hasher.finalize();
    let challenge = URL_SAFE_NO_PAD.encode(digest);

    (verifier, challenge)
}

async fn exchange_code_for_token(code: &str, verifier: &str) -> Result<OAuthTokenResponse> {
    let (actual_code, state) = code.split_once('#').unwrap_or((code, ""));

    let body = serde_json::json!({
        "code": actual_code,
        "state": state,
        "grant_type": "authorization_code",
        "client_id": ANTHROPIC_CLIENT_ID,
        "redirect_uri": REDIRECT_URI,
        "code_verifier": verifier
    });
    let client = reqwest::Client::new();
    let response = client
        .post(ANTHROPIC_TOKEN_URL)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .context("sending token exchange request")?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("token exchange failed ({}): {}", status, body));
    }
    response
        .json::<OAuthTokenResponse>()
        .await
        .context("parsing token response")
}

async fn create_api_key_from_token(access_token: &str) -> Result<String> {
    let client = reqwest::Client::new();
    let response = client
        .post(ANTHROPIC_CREATE_KEY_URL)
        .bearer_auth(access_token)
        .header("Content-Type", "application/json")
        .body("{}")
        .send()
        .await
        .context("sending create-api-key request")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("API key creation failed ({}): {}", status, body));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .context("parsing create-api-key response")?;

    let key = body["raw_key"]
        .as_str()
        .or_else(|| body["api_key"]["secret_key"].as_str())
        .or_else(|| body["secret_key"].as_str())
        .or_else(|| body["key"].as_str())
        .ok_or_else(|| anyhow!("could not find API key in response: {}", body))?;

    Ok(key.to_string())
}

pub(super) async fn oauth_pkce_flow(create_key: bool) -> Result<ProviderCredential> {
    let (verifier, challenge) = generate_pkce();
    let auth_base = if create_key {
        ANTHROPIC_AUTH_URL_CONSOLE
    } else {
        ANTHROPIC_AUTH_URL_MAX
    };
    let auth_url = {
        let mut u = url::Url::parse(auth_base).context("parsing auth URL")?;
        u.query_pairs_mut()
            .append_pair("code", "true")
            .append_pair("client_id", ANTHROPIC_CLIENT_ID)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", REDIRECT_URI)
            .append_pair("scope", ANTHROPIC_SCOPES)
            .append_pair("code_challenge", &challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("state", &verifier);
        u.to_string()
    };

    let mut stdout = io::stdout();
    execute!(
        stdout,
        Print("\r\n"),
        SetForegroundColor(Color::Yellow),
        Print("  Opening browser for authentication...\r\n\r\n"),
        ResetColor,
        SetForegroundColor(Color::DarkGrey),
        Print("  If your browser doesn't open, visit:\r\n  "),
        ResetColor,
        SetForegroundColor(Color::Cyan),
        Print(format!("{}\r\n\r\n", auth_url)),
        ResetColor,
        SetForegroundColor(Color::White),
        Print("  After authorizing, copy the full URL or code and paste it below.\r\n"),
        Print("  (The code may contain a '#' — include everything)\r\n\r\n"),
        ResetColor,
    )?;
    stdout.flush()?;

    if let Err(e) = open::that(&auth_url) {
        execute!(
            stdout,
            SetForegroundColor(Color::Red),
            Print(format!("  Could not open browser: {}\r\n", e)),
            ResetColor,
        )?;
    }

    let code = read_input_raw("Authorization code", false)?;
    let code = code.trim().to_string();
    if code.is_empty() {
        return Err(anyhow!("authorization code cannot be empty"));
    }

    execute!(
        stdout,
        Print("\r\n"),
        SetForegroundColor(Color::Yellow),
        Print("  Exchanging code for tokens...\r\n"),
        ResetColor,
    )?;
    stdout.flush()?;

    let token = exchange_code_for_token(&code, &verifier).await?;

    if create_key {
        execute!(
            stdout,
            SetForegroundColor(Color::Yellow),
            Print("  Creating API key...\r\n"),
            ResetColor,
        )?;
        stdout.flush()?;

        let api_key = create_api_key_from_token(&token.access_token).await?;
        Ok(ProviderCredential::ApiKey { key: api_key })
    } else {
        let expires_at = token
            .expires_in
            .map(|e| chrono::Utc::now().timestamp() + (e as i64));
        Ok(ProviderCredential::OAuth {
            access_token: token.access_token,
            refresh_token: token.refresh_token,
            expires_at,
            api_key: None,
        })
    }
}
