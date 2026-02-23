use anyhow::Result;
use crossterm::{execute, style::{Color, Print, ResetColor, SetForegroundColor}};
use std::io::{self};
use super::{Credentials, ProviderCredential};
use super::oauth::oauth_pkce_flow;
use super::ui::{select_from_menu, read_api_key};
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
