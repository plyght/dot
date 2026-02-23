use anyhow::{Context, Result, anyhow};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, ClearType},
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::PathBuf;

const ANTHROPIC_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const ANTHROPIC_AUTH_URL_MAX: &str = "https://claude.ai/oauth/authorize";
const ANTHROPIC_AUTH_URL_CONSOLE: &str = "https://console.anthropic.com/oauth/authorize";
const ANTHROPIC_TOKEN_URL: &str = "https://console.anthropic.com/v1/oauth/token";
const ANTHROPIC_CREATE_KEY_URL: &str =
    "https://api.anthropic.com/api/oauth/claude_cli/create_api_key";
const REDIRECT_URI: &str = "https://console.anthropic.com/oauth/code/callback";
const ANTHROPIC_SCOPES: &str = "org:create_api_key user:profile user:inference";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Credentials {
    pub providers: HashMap<String, ProviderCredential>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderCredential {
    ApiKey {
        key: String,
    },
    OAuth {
        access_token: String,
        refresh_token: Option<String>,
        expires_at: Option<i64>,
        api_key: Option<String>,
    },
}

impl ProviderCredential {
    pub fn api_key(&self) -> Option<&str> {
        match self {
            ProviderCredential::ApiKey { key } => Some(key.as_str()),
            ProviderCredential::OAuth {
                api_key: Some(k), ..
            } => Some(k.as_str()),
            ProviderCredential::OAuth { access_token, .. } => Some(access_token.as_str()),
        }
    }
}

impl Credentials {
    fn path() -> PathBuf {
        crate::config::Config::config_dir().join("credentials.json")
    }

    pub fn load() -> Result<Self> {
        let path = Self::path();
        if path.exists() {
            let content = std::fs::read_to_string(&path).context("reading credentials file")?;
            serde_json::from_str(&content).context("parsing credentials file")
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, serde_json::to_string_pretty(self)?)
            .context("writing credentials file")
    }

    pub fn get(&self, provider: &str) -> Option<&ProviderCredential> {
        self.providers.get(provider)
    }

    pub fn set(&mut self, provider: &str, cred: ProviderCredential) {
        self.providers.insert(provider.to_string(), cred);
    }
}

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

fn clear_and_header(stdout: &mut impl Write) -> Result<()> {
    execute!(
        stdout,
        terminal::Clear(ClearType::All),
        cursor::MoveTo(0, 0),
        SetForegroundColor(Color::Cyan),
        Print(" ● dot — minimal ai agent\r\n\r\n"),
        ResetColor,
    )?;
    Ok(())
}

fn select_from_menu(prompt: &str, items: &[&str]) -> Result<Option<usize>> {
    let mut stdout = io::stdout();
    let mut selected = 0usize;

    terminal::enable_raw_mode()?;
    execute!(stdout, cursor::Hide)?;

    let result: Result<Option<usize>> = (|| loop {
        clear_and_header(&mut stdout)?;

        execute!(
            stdout,
            SetForegroundColor(Color::White),
            Print(format!(" {}\r\n\r\n", prompt)),
            ResetColor,
        )?;

        for (i, item) in items.iter().enumerate() {
            if i == selected {
                execute!(
                    stdout,
                    SetForegroundColor(Color::Green),
                    Print(format!("  ❯ {}\r\n", item)),
                    ResetColor,
                )?;
            } else {
                execute!(
                    stdout,
                    SetForegroundColor(Color::DarkGrey),
                    Print(format!("    {}\r\n", item)),
                    ResetColor,
                )?;
            }
        }

        execute!(
            stdout,
            Print("\r\n"),
            SetForegroundColor(Color::DarkGrey),
            Print("  ↑↓ navigate   enter select   q quit"),
            ResetColor,
        )?;
        stdout.flush()?;

        match event::read()? {
            Event::Key(KeyEvent {
                code,
                kind: KeyEventKind::Press,
                ..
            }) => match code {
                KeyCode::Up => {
                    if selected > 0 {
                        selected -= 1;
                    }
                }
                KeyCode::Down => {
                    if selected < items.len() - 1 {
                        selected += 1;
                    }
                }
                KeyCode::Enter => return Ok(Some(selected)),
                KeyCode::Char('q') | KeyCode::Esc => return Ok(None),
                _ => {}
            },
            _ => {}
        }
    })();

    let _ = terminal::disable_raw_mode();
    execute!(stdout, cursor::Show, Print("\r\n"))?;

    result
}

fn read_input_raw(prompt: &str, mask: bool) -> Result<String> {
    let mut stdout = io::stdout();

    execute!(
        stdout,
        SetForegroundColor(Color::Cyan),
        Print(format!("  {} ", prompt)),
        SetForegroundColor(Color::DarkGrey),
        Print("› "),
        ResetColor,
    )?;
    stdout.flush()?;

    terminal::enable_raw_mode()?;
    execute!(stdout, cursor::Show)?;

    let mut input = String::new();

    let result: Result<String> = (|| loop {
        match event::read()? {
            Event::Key(KeyEvent {
                code,
                kind: KeyEventKind::Press,
                ..
            }) => match code {
                KeyCode::Enter => {
                    execute!(stdout, Print("\r\n"))?;
                    return Ok(input.clone());
                }
                KeyCode::Backspace => {
                    if input.pop().is_some() {
                        execute!(stdout, Print("\x08 \x08"))?;
                    }
                }
                KeyCode::Esc => {
                    execute!(stdout, Print("\r\n"))?;
                    return Err(anyhow!("Cancelled"));
                }
                KeyCode::Char(c) => {
                    input.push(c);
                    if mask {
                        execute!(stdout, Print("*"))?;
                    } else {
                        execute!(stdout, Print(c))?;
                    }
                }
                _ => {}
            },
            _ => {}
        }
        stdout.flush()?;
    })();

    let _ = terminal::disable_raw_mode();
    result
}

fn read_api_key(prompt: &str) -> Result<String> {
    let mut stdout = io::stdout();
    let _ = terminal::disable_raw_mode();
    execute!(stdout, cursor::Show)?;

    clear_and_header(&mut stdout)?;

    execute!(
        stdout,
        SetForegroundColor(Color::White),
        Print(format!(" {}\r\n\r\n", prompt)),
        ResetColor,
    )?;

    let key = read_input_raw("API key", true)?;
    let trimmed = key.trim().to_string();
    if trimmed.is_empty() {
        return Err(anyhow!("API key cannot be empty"));
    }
    Ok(trimmed)
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

async fn oauth_pkce_flow(create_key: bool) -> Result<ProviderCredential> {
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
            .map(|e| chrono::Utc::now().timestamp_millis() + (e as i64) * 1000);
        Ok(ProviderCredential::OAuth {
            access_token: token.access_token,
            refresh_token: token.refresh_token,
            expires_at,
            api_key: None,
        })
    }
}

pub async fn login_flow() -> Result<()> {
    let mut stdout = io::stdout();

    let providers = &["Anthropic", "OpenAI"];
    let provider_idx = match select_from_menu("Select a provider", providers)? {
        Some(i) => i,
        None => {
            execute!(stdout, Print("  Cancelled.\r\n"))?;
            return Ok(());
        }
    };

    match provider_idx {
        0 => {
            let methods = &[
                "Claude Pro/Max  (OAuth — browser login)",
                "Create API Key  (OAuth + auto-generate key)",
                "Enter API Key   (paste key manually)",
            ];
            let method_idx = match select_from_menu("Anthropic — authentication method", methods)?
            {
                Some(i) => i,
                None => {
                    execute!(stdout, Print("  Cancelled.\r\n"))?;
                    return Ok(());
                }
            };

            let cred = match method_idx {
                0 => oauth_pkce_flow(false).await?,
                1 => oauth_pkce_flow(true).await?,
                2 => {
                    let key = read_api_key("Enter your Anthropic API key")?;
                    ProviderCredential::ApiKey { key }
                }
                _ => unreachable!(),
            };

            let mut creds = Credentials::load()?;
            creds.set("anthropic", cred);
            creds.save()?;

            execute!(
                stdout,
                SetForegroundColor(Color::Green),
                Print("\r\n  ✓ Anthropic credentials saved.\r\n\r\n"),
                ResetColor,
            )?;
        }
        1 => {
            let key = read_api_key("Enter your OpenAI API key")?;
            let cred = ProviderCredential::ApiKey { key };
            let mut creds = Credentials::load()?;
            creds.set("openai", cred);
            creds.save()?;

            execute!(
                stdout,
                SetForegroundColor(Color::Green),
                Print("\r\n  ✓ OpenAI credentials saved.\r\n\r\n"),
                ResetColor,
            )?;
        }
        _ => unreachable!(),
    }

    Ok(())
}
