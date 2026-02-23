pub mod file;
pub mod shell;

use crate::provider::ToolDefinition;

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;
    fn execute(&self, input: serde_json::Value) -> anyhow::Result<String>;
}

pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        ToolRegistry { tools: Vec::new() }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.push(tool);
    }

    pub fn register_many(&mut self, tools: Vec<Box<dyn Tool>>) {
        self.tools.extend(tools);
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .iter()
            .map(|t| ToolDefinition {
                name: t.name().to_string(),
                description: t.description().to_string(),
                input_schema: t.input_schema(),
            })
            .collect()
    }

    /// Return tool definitions filtered by an allow/deny map.
    /// If the map is empty, all tools are returned. Otherwise, tools
    /// explicitly set to `false` are excluded.
    pub fn definitions_filtered(
        &self,
        filter: &std::collections::HashMap<String, bool>,
    ) -> Vec<ToolDefinition> {
        if filter.is_empty() {
            return self.definitions();
        }
        self.tools
            .iter()
            .filter(|t| {
                filter.get(t.name()).copied().unwrap_or(true)
            })
            .map(|t| ToolDefinition {
                name: t.name().to_string(),
                description: t.description().to_string(),
                input_schema: t.input_schema(),
            })
            .collect()
    }

    pub fn execute(&self, name: &str, input: serde_json::Value) -> anyhow::Result<String> {
        for tool in &self.tools {
            if tool.name() == name {
                tracing::debug!("Executing tool: {}", name);
                return tool.execute(input);
            }
        }
        anyhow::bail!("Unknown tool: {}", name)
    }

    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    pub fn default_tools() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(file::ReadFileTool));
        registry.register(Box::new(file::WriteFileTool));
        registry.register(Box::new(file::ListDirectoryTool));
        registry.register(Box::new(file::SearchFilesTool));
        registry.register(Box::new(shell::RunCommandTool));
        registry
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
