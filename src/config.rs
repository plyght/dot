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
    pub context: ContextConfig,
    #[serde(default)]
    pub acp_agents: HashMap<String, AcpAgentConfig>,
    #[serde(default)]
    pub mcp: HashMap<String, McpServerConfig>,
    #[serde(default)]
    pub agents: HashMap<String, AgentConfig>,
    #[serde(default)]
    pub tui: TuiConfig,
    #[serde(default)]
    pub permissions: HashMap<String, String>,
    #[serde(default)]
    pub providers: HashMap<String, ProviderDefinition>,
    #[serde(default)]
    pub custom_tools: HashMap<String, CustomToolConfig>,
    #[serde(default)]
    pub commands: HashMap<String, CommandConfig>,
    #[serde(default)]
    pub hooks: HashMap<String, HookConfig>,
    #[serde(default)]
    pub subagents: SubagentSettings,
    #[serde(default)]
    pub memory: MemoryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    #[serde(default = "default_true")]
    pub auto_load_global: bool,
    #[serde(default = "default_true")]
    pub auto_load_project: bool,
}
impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            auto_load_global: true,
            auto_load_project: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AcpAgentConfig {
    #[serde(default)]
    pub command: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub description: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuiConfig {
    #[serde(default = "default_true")]
    pub vim_mode: bool,
    #[serde(default)]
    pub favorite_models: Vec<String>,
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            vim_mode: true,
            favorite_models: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderDefinition {
    pub api: String,
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub models: Vec<String>,
    pub default_model: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomToolConfig {
    pub description: String,
    pub command: String,
    #[serde(default = "default_schema")]
    pub schema: serde_json::Value,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandConfig {
    pub description: String,
    pub command: String,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookConfig {
    pub command: String,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_max_subagent_turns")]
    pub max_turns: usize,
}

impl Default for SubagentSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            max_turns: 20,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub auto_extract: bool,
    #[serde(default = "default_inject_count")]
    pub inject_count: usize,
    #[serde(default = "default_max_memories")]
    pub max_memories: usize,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_extract: true,
            inject_count: 15,
            max_memories: 2000,
        }
    }
}

fn default_inject_count() -> usize {
    15
}

fn default_max_memories() -> usize {
    2000
}

fn default_max_subagent_turns() -> usize {
    20
}

fn default_true() -> bool {
    true
}

fn default_timeout() -> u64 {
    30
}

fn default_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {},
        "required": []
    })
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_provider: "anthropic".to_string(),
            default_model: "claude-sonnet-4-20250514".to_string(),
            theme: ThemeConfig {
                name: "terminal".to_string(),
            },
            context: ContextConfig::default(),
            acp_agents: HashMap::new(),
            mcp: HashMap::new(),
            agents: HashMap::new(),
            tui: TuiConfig::default(),
            permissions: HashMap::new(),
            providers: HashMap::new(),
            custom_tools: HashMap::new(),
            commands: HashMap::new(),
            hooks: HashMap::new(),
            subagents: SubagentSettings::default(),
            memory: MemoryConfig::default(),
        }
    }
}

impl Config {
    pub fn config_dir() -> PathBuf {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME")
            && !xdg.is_empty()
        {
            return PathBuf::from(xdg).join("dot");
        }
        #[cfg(unix)]
        return dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config")
            .join("dot");
        #[cfg(not(unix))]
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("dot")
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    pub fn data_dir() -> PathBuf {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME")
            && !xdg.is_empty()
        {
            return PathBuf::from(xdg).join("dot");
        }
        #[cfg(unix)]
        return dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".local")
            .join("share")
            .join("dot");
        #[cfg(not(unix))]
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
            let config = Self::default();
            config.save()?;
            Ok(config)
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
