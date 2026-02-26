use anyhow::{Context, Result};
use serde_json::Value;
use std::fs;

use super::Tool;

pub struct MultiEditTool;

impl Tool for MultiEditTool {
    fn name(&self) -> &str {
        "multiedit"
    }

    fn description(&self) -> &str {
        "Edit multiple sections of a single file in one operation. Each edit specifies an old_text to find and new_text to replace it with. Edits are applied in reverse position order to preserve offsets."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path to edit"
                },
                "edits": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "old_text": {
                                "type": "string",
                                "description": "Exact text to find in the file"
                            },
                            "new_text": {
                                "type": "string",
                                "description": "Replacement text"
                            }
                        },
                        "required": ["old_text", "new_text"]
                    },
                    "description": "Array of edits to apply"
                }
            },
            "required": ["path", "edits"]
        })
    }

    fn execute(&self, input: Value) -> Result<String> {
        let path = input["path"]
            .as_str()
            .context("Missing required parameter 'path'")?;
        let edits = input["edits"]
            .as_array()
            .context("Missing required parameter 'edits'")?;

        if edits.is_empty() {
            return Ok("No edits to apply.".to_string());
        }

        tracing::debug!("multiedit: {} edits on {}", edits.len(), path);

        let mut content =
            fs::read_to_string(path).with_context(|| format!("Failed to read file: {}", path))?;

        let mut missing: Vec<String> = edits
            .iter()
            .filter_map(|e| {
                let old = e["old_text"].as_str()?;
                if !content.contains(old) {
                    Some(old.to_string())
                } else {
                    None
                }
            })
            .collect();

        if !missing.is_empty() {
            missing.dedup();
            anyhow::bail!(
                "old_text not found in {}: {}",
                path,
                missing
                    .iter()
                    .map(|s| format!("{:?}", s))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }

        let mut positioned: Vec<(usize, &str, &str)> = edits
            .iter()
            .filter_map(|e| {
                let old = e["old_text"].as_str()?;
                let new = e["new_text"].as_str()?;
                let pos = content.find(old)?;
                Some((pos, old, new))
            })
            .collect();

        positioned.sort_by(|a, b| b.0.cmp(&a.0));

        for (_, old, new) in &positioned {
            let pos = content
                .find(old)
                .with_context(|| format!("old_text {:?} no longer found after prior edits", old))?;
            content.replace_range(pos..pos + old.len(), new);
        }

        fs::write(path, &content).with_context(|| format!("Failed to write file: {}", path))?;

        Ok(format!("Applied {} edit(s) to {}", edits.len(), path))
    }
}
