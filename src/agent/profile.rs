use std::collections::HashMap;

use crate::config::AgentConfig;

pub(super) const DEFAULT_SYSTEM_PROMPT: &str = "You are dot, a helpful AI coding assistant running in a terminal. \
You have access to tools for reading/writing files, running shell commands, and searching code. \
Be concise and direct. When asked to make changes, use the tools to implement them. \
Don't just describe what to do.";

#[derive(Debug, Clone)]
pub struct AgentProfile {
    pub name: String,
    pub description: String,
    pub system_prompt: String,
    pub model_spec: Option<String>,
    pub tool_filter: HashMap<String, bool>,
}

impl AgentProfile {
    pub fn default_profile() -> Self {
        AgentProfile {
            name: "dot".to_string(),
            description: "Default coding assistant".to_string(),
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            model_spec: None,
            tool_filter: HashMap::new(),
        }
    }

    pub fn from_config(name: &str, cfg: &AgentConfig) -> Self {
        let system_prompt = cfg
            .system_prompt
            .clone()
            .unwrap_or_else(|| DEFAULT_SYSTEM_PROMPT.to_string());
        AgentProfile {
            name: name.to_string(),
            description: cfg.description.clone(),
            system_prompt,
            model_spec: cfg.model.clone(),
            tool_filter: cfg.tools.clone(),
        }
    }
}
