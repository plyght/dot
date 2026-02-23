use std::path::{Path, PathBuf};

use crate::config::ContextConfig;

pub struct AgentsContext {
    content: Option<String>,
}

impl AgentsContext {
    pub fn load(cwd: &str, cfg: &ContextConfig) -> Self {
        let mut parts: Vec<String> = Vec::new();

        if cfg.auto_load_global {
            let config_dir = dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("dot");
            for name in &["AGENTS.md", "AGENT.md"] {
                let path = config_dir.join(name);
                if path.exists() {
                    match std::fs::read_to_string(&path) {
                        Ok(content) => {
                            let trimmed = content.trim().to_string();
                            if !trimmed.is_empty() {
                                tracing::debug!(
                                    "Loaded global agents context from {}",
                                    path.display()
                                );
                                parts.push(trimmed);
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to read {}: {}", path.display(), e);
                        }
                    }
                    break;
                }
            }
        }

        if cfg.auto_load_project {
            let cwd_path = Path::new(cwd);
            let candidates = [
                cwd_path.join("AGENTS.md"),
                cwd_path.join("AGENT.md"),
                cwd_path.join(".dot").join("AGENTS.md"),
                cwd_path.join(".dot").join("AGENT.md"),
            ];
            for path in &candidates {
                if path.exists() {
                    match std::fs::read_to_string(path) {
                        Ok(content) => {
                            let trimmed = content.trim().to_string();
                            if !trimmed.is_empty() {
                                tracing::debug!(
                                    "Loaded project agents context from {}",
                                    path.display()
                                );
                                parts.push(trimmed);
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to read {}: {}", path.display(), e);
                        }
                    }
                    break;
                }
            }
        }

        let content = if parts.is_empty() {
            None
        } else {
            Some(parts.join("\n\n---\n\n"))
        };

        AgentsContext { content }
    }

    pub fn apply_to_system_prompt(&self, base: &str) -> String {
        match &self.content {
            None => base.to_string(),
            Some(ctx) => format!("{ctx}\n\n---\n\n{base}"),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_none()
    }
}
