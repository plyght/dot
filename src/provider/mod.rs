pub mod anthropic;
pub mod openai;

use std::{future::Future, pin::Pin};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedReceiver;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text(String),
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct StreamEvent {
    pub event_type: StreamEventType,
}

#[derive(Debug, Clone)]
pub enum StreamEventType {
    TextDelta(String),
    ToolUseStart {
        id: String,
        name: String,
    },
    ToolUseInputDelta(String),
    ToolUseEnd,
    MessageStart,
    MessageEnd {
        stop_reason: StopReason,
        usage: Usage,
    },
    Error(String),
}

#[derive(Debug, Clone)]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    ToolUse,
    StopSequence,
}

#[derive(Debug, Clone, Default)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_read_tokens: u32,
    pub cache_write_tokens: u32,
}

pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    fn model(&self) -> &str;
    fn set_model(&mut self, model: String);
    fn available_models(&self) -> Vec<String>;
    fn fetch_models(
        &self,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<String>>> + Send + '_>>;
    fn stream(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        max_tokens: u32,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<UnboundedReceiver<StreamEvent>>> + Send + '_>>;
}
