use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub default_provider: String,
    pub default_model: String,
    pub theme: ThemeConfig,
    #[serde(default)]
    pub mcp: HashMap<String, McpServerConfig>,
    #[serde(default)]
    pub agents: HashMap<String, AgentConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    #[serde(default)]
    pub command: Vec<String>,
    pub url: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub description: String,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub tools: HashMap<String, bool>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

fn default_timeout() -> u64 {
    30
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_provider: "anthropic".to_string(),
            default_model: "claude-sonnet-4-20250514".to_string(),
            theme: ThemeConfig {
                name: "dark".to_string(),
            },
            mcp: HashMap::new(),
            agents: HashMap::new(),
        }
    }
}

impl Config {
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("dot")
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    pub fn data_dir() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("dot")
    }

    pub fn db_path() -> PathBuf {
        Self::data_dir().join("dot.db")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("reading config from {}", path.display()))?;
            toml::from_str(&content).context("parsing config.toml")
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let dir = Self::config_dir();
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("creating config dir {}", dir.display()))?;
        let content = toml::to_string_pretty(self).context("serializing config")?;
        std::fs::write(Self::config_path(), content).context("writing config.toml")
    }

    pub fn ensure_dirs() -> Result<()> {
        std::fs::create_dir_all(Self::config_dir()).context("creating config directory")?;
        std::fs::create_dir_all(Self::data_dir()).context("creating data directory")?;
        Ok(())
    }

    pub fn enabled_mcp_servers(&self) -> Vec<(&str, &McpServerConfig)> {
        self.mcp
            .iter()
            .filter(|(_, cfg)| cfg.enabled && !cfg.command.is_empty())
            .map(|(name, cfg)| (name.as_str(), cfg))
            .collect()
    }

    pub fn enabled_agents(&self) -> Vec<(&str, &AgentConfig)> {
        self.agents
            .iter()
            .filter(|(_, cfg)| cfg.enabled)
            .map(|(name, cfg)| (name.as_str(), cfg))
            .collect()
    }

    /// Parse a `provider/model` spec. Returns `(provider, model)` if `/` present,
    /// otherwise `(None, spec)`.
    pub fn parse_model_spec(spec: &str) -> (Option<&str>, &str) {
        if let Some((provider, model)) = spec.split_once('/') {
            (Some(provider), model)
        } else {
            (None, spec)
        }
    }
}
