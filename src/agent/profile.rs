use std::collections::HashMap;

use crate::config::AgentConfig;

pub(super) const DEFAULT_SYSTEM_PROMPT: &str = include_str!("prompt.txt");

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

    pub fn plan_profile() -> Self {
        let mut filter = HashMap::new();
        filter.insert("write_file".to_string(), false);
        filter.insert("run_command".to_string(), false);
        filter.insert("apply_patch".to_string(), false);
        filter.insert("web_fetch".to_string(), false);
        filter.insert("multiedit".to_string(), false);
        filter.insert("batch".to_string(), false);
        AgentProfile {
            name: "plan".to_string(),
            description: "Read-only planning assistant".to_string(),
            system_prompt: "You are a planning assistant. You can read and analyze code but cannot make changes. Help the user understand codebases, plan approaches, and think through problems. You have access to file reading, search, and grep tools only.".to_string(),
            model_spec: None,
            tool_filter: filter,
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
