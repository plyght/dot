pub mod copilot;
mod login;
pub mod oauth;
mod ui;

pub use login::login_flow;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

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
