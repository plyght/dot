use serde::Serialize;

use crate::provider::{ContentBlock, Message, Role, StopReason, ToolDefinition};

#[derive(Serialize)]
pub(super) struct AnthropicRequest<'a> {
    pub model: &'a str,
    pub messages: Vec<serde_json::Value>,
    pub max_tokens: u32,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<serde_json::Value>,
    pub temperature: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<serde_json::Value>,
}

fn convert_content_block(block: &ContentBlock) -> serde_json::Value {
    match block {
        ContentBlock::Text(text) => serde_json::json!({
            "type": "text",
            "text": text,
        }),
        ContentBlock::ToolUse { id, name, input } => serde_json::json!({
            "type": "tool_use",
            "id": id,
            "name": name,
            "input": input,
        }),
        ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => serde_json::json!({
            "type": "tool_result",
            "tool_use_id": tool_use_id,
            "content": content,
            "is_error": is_error,
        }),
        ContentBlock::Thinking {
            thinking,
            signature,
        } => serde_json::json!({
            "type": "thinking",
            "thinking": thinking,
            "signature": signature,
        }),
    }
}

pub(super) fn convert_messages(messages: &[Message]) -> Vec<serde_json::Value> {
    messages
        .iter()
        .filter(|m| m.role != Role::System)
        .map(|m| {
            let role = match m.role {
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::System => "user",
            };
            let content: Vec<serde_json::Value> =
                m.content.iter().map(convert_content_block).collect();
            serde_json::json!({ "role": role, "content": content })
        })
        .collect()
}

pub(super) fn convert_tools(tools: &[ToolDefinition]) -> Vec<serde_json::Value> {
    tools
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.input_schema,
            })
        })
        .collect()
}

pub(super) fn stop_reason_from_str(s: &str) -> StopReason {
    match s {
        "end_turn" => StopReason::EndTurn,
        "max_tokens" => StopReason::MaxTokens,
        "tool_use" => StopReason::ToolUse,
        "stop_sequence" => StopReason::StopSequence,
        _ => StopReason::EndTurn,
    }
}
