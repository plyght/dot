use anyhow::{Context, Result};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

use crate::tools::Tool;

#[derive(Debug, Clone)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
}

pub struct SkillRegistry {
    skills: Vec<SkillInfo>,
}

impl SkillRegistry {
    pub fn discover() -> Self {
        let mut skills = Vec::new();
        let mut seen_names = std::collections::HashSet::new();

        for base in Self::search_paths() {
            if !base.exists() {
                continue;
            }
            let entries = match fs::read_dir(&base) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let skill_dir = entry.path();
                if !skill_dir.is_dir() {
                    continue;
                }
                let skill_file = skill_dir.join("SKILL.md");
                if skill_file.exists() {
                    if let Some(info) = Self::parse_skill(&skill_file) {
                        if seen_names.insert(info.name.clone()) {
                            skills.push(info);
                        }
                    }
                }
            }
        }

        tracing::info!("Discovered {} skills", skills.len());
        SkillRegistry { skills }
    }

    fn search_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let config_dir = dirs::config_dir().unwrap_or_else(|| home.join(".config"));
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        paths.push(config_dir.join("dot").join("skills"));
        paths.push(home.join(".agents").join("skills"));
        paths.push(home.join(".claude").join("skills"));

        paths.push(cwd.join(".dot").join("skills"));
        paths.push(cwd.join(".agents").join("skills"));
        paths.push(cwd.join(".claude").join("skills"));

        paths
    }

    fn parse_skill(path: &Path) -> Option<SkillInfo> {
        let content = fs::read_to_string(path).ok()?;
        let name = path.parent()?.file_name()?.to_string_lossy().to_string();

        let description = if content.starts_with("---") {
            if let Some(end) = content[3..].find("---") {
                let frontmatter = &content[3..3 + end];
                Self::extract_field(frontmatter, "description")
                    .unwrap_or_else(|| Self::first_meaningful_line(&content, 3 + end + 3))
            } else {
                Self::first_meaningful_line(&content, 0)
            }
        } else {
            Self::first_meaningful_line(&content, 0)
        };

        Some(SkillInfo {
            name,
            description,
            path: path.to_path_buf(),
        })
    }

    fn extract_field(frontmatter: &str, field: &str) -> Option<String> {
        let prefix = format!("{}:", field);
        for line in frontmatter.lines() {
            let trimmed = line.trim();
            if let Some(value) = trimmed.strip_prefix(&prefix) {
                let value = value.trim().trim_matches('"').trim_matches('\'');
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
        None
    }

    fn first_meaningful_line(content: &str, skip: usize) -> String {
        content
            .get(skip..)
            .unwrap_or("")
            .lines()
            .find(|l| {
                let t = l.trim();
                !t.is_empty() && !t.starts_with('#') && !t.starts_with("---")
            })
            .unwrap_or("No description")
            .trim()
            .chars()
            .take(120)
            .collect()
    }

    pub fn skills(&self) -> &[SkillInfo] {
        &self.skills
    }

    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    pub fn into_tool(self) -> Option<SkillTool> {
        if self.skills.is_empty() {
            return None;
        }
        Some(SkillTool {
            skills: self.skills,
        })
    }
}

pub struct SkillTool {
    skills: Vec<SkillInfo>,
}

impl Tool for SkillTool {
    fn name(&self) -> &str {
        "skill"
    }

    fn description(&self) -> &str {
        "Load a skill by name for specialized domain guidance. Use this when the task matches an available skill."
    }

    fn input_schema(&self) -> Value {
        let skill_names: Vec<&str> = self.skills.iter().map(|s| s.name.as_str()).collect();
        let desc_list: Vec<String> = self
            .skills
            .iter()
            .map(|s| format!("{}: {}", s.name, s.description))
            .collect();

        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": format!("Skill to load. Available: {}", desc_list.join("; ")),
                    "enum": skill_names
                }
            },
            "required": ["name"]
        })
    }

    fn execute(&self, input: Value) -> Result<String> {
        let name = input["name"]
            .as_str()
            .context("Missing required parameter 'name'")?;

        let info = self.skills.iter().find(|s| s.name == name).with_context(|| {
            format!(
                "Unknown skill '{}'. Available: {}",
                name,
                self.skills
                    .iter()
                    .map(|s| s.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;

        fs::read_to_string(&info.path)
            .with_context(|| format!("Failed to read skill '{}'", name))
    }
}
