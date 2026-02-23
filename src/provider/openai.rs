use std::{collections::HashMap, future::Future, pin::Pin};

use anyhow::Context;
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{
        ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessage,
        ChatCompletionRequestAssistantMessageContent, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessage, ChatCompletionRequestSystemMessageContent,
        ChatCompletionRequestToolMessage, ChatCompletionRequestToolMessageContent,
        ChatCompletionRequestUserMessage, ChatCompletionRequestUserMessageContent,
        ChatCompletionTool, ChatCompletionToolType, CreateChatCompletionRequest, FinishReason,
        FunctionCall, FunctionObject,
    },
};
use futures::StreamExt;
use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::provider::{
    ContentBlock, Message, Provider, Role, StopReason, StreamEvent, StreamEventType,
    ToolDefinition, Usage,
};

pub struct OpenAIProvider {
    client: Client<OpenAIConfig>,
    model: String,
    cached_models: tokio::sync::Mutex<Option<Vec<String>>>,
}

impl OpenAIProvider {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            model: model.into(),
            cached_models: tokio::sync::Mutex::new(None),
        }
    }
    pub fn new_with_config(config: OpenAIConfig, model: impl Into<String>) -> Self {
        Self {
            client: Client::with_config(config),
            model: model.into(),
            cached_models: tokio::sync::Mutex::new(None),
        }
    }
}

#[derive(Default)]
struct ToolCallAccum {
    id: String,
    name: String,
    arguments: String,
    started: bool,
}

fn convert_messages(
    messages: &[Message],
    system: Option<&str>,
) -> anyhow::Result<Vec<ChatCompletionRequestMessage>> {
    let mut result: Vec<ChatCompletionRequestMessage> = Vec::new();

    if let Some(sys) = system {
        result.push(ChatCompletionRequestMessage::System(
            ChatCompletionRequestSystemMessage {
                content: ChatCompletionRequestSystemMessageContent::Text(sys.to_string()),
                name: None,
            },
        ));
    }

    for msg in messages {
        match msg.role {
            Role::System => {
                let text = extract_text_content(&msg.content);
                result.push(ChatCompletionRequestMessage::System(
                    ChatCompletionRequestSystemMessage {
                        content: ChatCompletionRequestSystemMessageContent::Text(text),
                        name: None,
                    },
                ));
            }

            Role::User => {
                let mut tool_results: Vec<(String, String)> = Vec::new();
                let mut texts: Vec<String> = Vec::new();

                for block in &msg.content {
                    match block {
                        ContentBlock::Text(t) => texts.push(t.clone()),
                        ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            ..
                        } => {
                            tool_results.push((tool_use_id.clone(), content.clone()));
                        }
                        _ => {}
                    }
                }

                for (id, content) in tool_results {
                    result.push(ChatCompletionRequestMessage::Tool(
                        ChatCompletionRequestToolMessage {
                            content: ChatCompletionRequestToolMessageContent::Text(content),
                            tool_call_id: id,
                        },
                    ));
                }

                if !texts.is_empty() {
                    result.push(ChatCompletionRequestMessage::User(
                        ChatCompletionRequestUserMessage {
                            content: ChatCompletionRequestUserMessageContent::Text(
                                texts.join("\n"),
                            ),
                            name: None,
                        },
                    ));
                }
            }

            Role::Assistant => {
                let mut text_parts: Vec<String> = Vec::new();
                let mut tool_calls: Vec<ChatCompletionMessageToolCall> = Vec::new();

                for block in &msg.content {
                    match block {
                        ContentBlock::Text(t) => text_parts.push(t.clone()),
                        ContentBlock::ToolUse { id, name, input } => {
                            tool_calls.push(ChatCompletionMessageToolCall {
                                id: id.clone(),
                                r#type: ChatCompletionToolType::Function,
                                function: FunctionCall {
                                    name: name.clone(),
                                    arguments: serde_json::to_string(input).unwrap_or_default(),
                                },
                            });
                        }
                        _ => {}
                    }
                }

                let content = if text_parts.is_empty() {
                    None
                } else {
                    Some(ChatCompletionRequestAssistantMessageContent::Text(
                        text_parts.join("\n"),
                    ))
                };

                result.push(ChatCompletionRequestMessage::Assistant(
                    ChatCompletionRequestAssistantMessage {
                        content,
                        name: None,
                        tool_calls: if tool_calls.is_empty() {
                            None
                        } else {
                            Some(tool_calls)
                        },
                        refusal: None,
                        ..Default::default()
                    },
                ));
            }
        }
    }

    Ok(result)
}

fn extract_text_content(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .filter_map(|b| {
            if let ContentBlock::Text(t) = b {
                Some(t.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn convert_tools(tools: &[ToolDefinition]) -> Vec<ChatCompletionTool> {
    tools
        .iter()
        .map(|t| ChatCompletionTool {
            r#type: ChatCompletionToolType::Function,
            function: FunctionObject {
                name: t.name.clone(),
                description: Some(t.description.clone()),
                parameters: Some(t.input_schema.clone()),
                strict: None,
            },
        })
        .collect()
}

fn map_finish_reason(reason: &FinishReason) -> StopReason {
    match reason {
        FinishReason::Stop => StopReason::EndTurn,
        FinishReason::Length => StopReason::MaxTokens,
        FinishReason::ToolCalls | FinishReason::FunctionCall => StopReason::ToolUse,
        FinishReason::ContentFilter => StopReason::StopSequence,
    }
}

impl Provider for OpenAIProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn set_model(&mut self, model: String) {
        self.model = model;
    }

    fn available_models(&self) -> Vec<String> {
        let cache = self.cached_models.blocking_lock();
        cache.clone().unwrap_or_else(|| {
            vec![
                "gpt-4o".to_string(),
                "gpt-4o-mini".to_string(),
                "gpt-4-turbo".to_string(),
                "o1".to_string(),
                "o1-mini".to_string(),
                "o3-mini".to_string(),
            ]
        })
    }

    fn fetch_models(
        &self,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<String>>> + Send + '_>> {
        let client = self.client.clone();
        Box::pin(async move {
            {
                let cache = self.cached_models.lock().await;
                if let Some(ref models) = *cache {
                    return Ok(models.clone());
                }
            }

            let resp = client.models().list().await;

            match resp {
                Ok(list) => {
                    let mut models: Vec<String> = list
                        .data
                        .into_iter()
                        .map(|m| m.id)
                        .filter(|id| {
                            id.starts_with("gpt-")
                                || id.starts_with("o1")
                                || id.starts_with("o3")
                                || id.starts_with("o4")
                        })
                        .collect();
                    models.sort();
                    models.dedup();

                    if models.is_empty() {
                        return Ok(self.available_models());
                    }

                    let mut cache = self.cached_models.lock().await;
                    *cache = Some(models.clone());
                    Ok(models)
                }
                Err(e) => {
                    warn!("Failed to fetch OpenAI models: {e}");
                    Ok(self.available_models())
                }
            }
        })
    }

    fn stream(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        max_tokens: u32,
    ) -> Pin<
        Box<dyn Future<Output = anyhow::Result<mpsc::UnboundedReceiver<StreamEvent>>> + Send + '_>,
    > {
        let messages = messages.to_vec();
        let system = system.map(String::from);
        let tools = tools.to_vec();
        let model = self.model.clone();
        let client = self.client.clone();

        Box::pin(async move {
            let converted_messages = convert_messages(&messages, system.as_deref())
                .context("Failed to convert messages")?;
            let converted_tools = convert_tools(&tools);

            let request = CreateChatCompletionRequest {
                model: model.clone(),
                messages: converted_messages,
                max_completion_tokens: Some(max_tokens),
                stream: Some(true),
                tools: if converted_tools.is_empty() {
                    None
                } else {
                    Some(converted_tools)
                },
                temperature: Some(1.0),
                ..Default::default()
            };

            let mut oai_stream = client
                .chat()
                .create_stream(request)
                .await
                .context("Failed to create OpenAI stream")?;

            let (tx, rx) = mpsc::unbounded_channel::<StreamEvent>();
            let tx_clone = tx.clone();

            tokio::spawn(async move {
                let mut tool_accum: HashMap<u32, ToolCallAccum> = HashMap::new();
                let mut total_output_tokens: u32 = 0;
                let mut final_stop_reason: Option<StopReason> = None;

                let _ = tx_clone.send(StreamEvent {
                    event_type: StreamEventType::MessageStart,
                });

                while let Some(result) = oai_stream.next().await {
                    match result {
                        Err(e) => {
                            warn!("OpenAI stream error: {e}");
                            let _ = tx_clone.send(StreamEvent {
                                event_type: StreamEventType::Error(e.to_string()),
                            });
                            return;
                        }
                        Ok(response) => {
                            if let Some(usage) = response.usage {
                                total_output_tokens = usage.completion_tokens;
                            }

                            for choice in response.choices {
                                if let Some(reason) = &choice.finish_reason {
                                    final_stop_reason = Some(map_finish_reason(reason));

                                    if matches!(
                                        reason,
                                        FinishReason::ToolCalls | FinishReason::FunctionCall
                                    ) {
                                        for (_, accum) in &tool_accum {
                                            if accum.started {
                                                let _ = tx_clone.send(StreamEvent {
                                                    event_type: StreamEventType::ToolUseEnd,
                                                });
                                            }
                                        }
                                        tool_accum.clear();
                                    }
                                }

                                let delta = choice.delta;

                                if let Some(content) = delta.content {
                                    if !content.is_empty() {
                                        let _ = tx_clone.send(StreamEvent {
                                            event_type: StreamEventType::TextDelta(content),
                                        });
                                    }
                                }

                                if let Some(tool_call_chunks) = delta.tool_calls {
                                    for chunk in tool_call_chunks {
                                        let idx = chunk.index;
                                        let entry = tool_accum.entry(idx).or_default();

                                        if let Some(id) = chunk.id {
                                            if !id.is_empty() {
                                                entry.id = id;
                                            }
                                        }

                                        if let Some(func) = chunk.function {
                                            if let Some(name) = func.name {
                                                if !name.is_empty() {
                                                    entry.name = name;
                                                }
                                            }

                                            if !entry.started
                                                && !entry.id.is_empty()
                                                && !entry.name.is_empty()
                                            {
                                                let _ = tx_clone.send(StreamEvent {
                                                    event_type: StreamEventType::ToolUseStart {
                                                        id: entry.id.clone(),
                                                        name: entry.name.clone(),
                                                    },
                                                });
                                                entry.started = true;
                                                debug!(
                                                    "OpenAI tool use start: id={} name={}",
                                                    entry.id, entry.name
                                                );
                                            }

                                            if let Some(args) = func.arguments {
                                                if !args.is_empty() {
                                                    entry.arguments.push_str(&args);
                                                    let _ =
                                                        tx_clone.send(StreamEvent {
                                                            event_type:
                                                                StreamEventType::ToolUseInputDelta(
                                                                    args,
                                                                ),
                                                        });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                for (_, accum) in &tool_accum {
                    if accum.started {
                        let _ = tx_clone.send(StreamEvent {
                            event_type: StreamEventType::ToolUseEnd,
                        });
                    }
                }

                let stop = final_stop_reason.unwrap_or(StopReason::EndTurn);
                let _ = tx_clone.send(StreamEvent {
                    event_type: StreamEventType::MessageEnd {
                        stop_reason: stop,
                        usage: Usage {
                            input_tokens: 0,
                            output_tokens: total_output_tokens,
                            cache_read_tokens: 0,
                            cache_write_tokens: 0,
                        },
                    },
                });
            });

            Ok(rx)
        })
    }
}
