use anyhow::Result;
use tokio::sync::mpsc::UnboundedSender;

use crate::config::Config;
use crate::db::Db;
use crate::provider::{ContentBlock, Message, Provider, Role, StreamEventType, Usage};
use crate::tools::ToolRegistry;

const SYSTEM_PROMPT: &str = "\
You are dot, a helpful AI coding assistant running in a terminal. \
You have access to tools for reading/writing files, running shell commands, and searching code. \
Be concise and direct. When asked to make changes, use the tools to implement them — \
don't just describe what to do.";

#[derive(Debug)]
pub enum AgentEvent {
    TextDelta(String),
    TextComplete(String),
    ToolCallStart { id: String, name: String },
    ToolCallInputDelta(String),
    ToolCallExecuting { id: String, name: String, input: String },
    ToolCallResult { id: String, name: String, output: String, is_error: bool },
    Done { usage: Usage },
    Error(String),
}

struct PendingToolCall {
    id: String,
    name: String,
    input: String,
}

pub struct Agent {
    provider: Box<dyn Provider>,
    tools: ToolRegistry,
    db: Db,
    conversation_id: String,
    messages: Vec<Message>,
    system_prompt: String,
}

impl Agent {
    pub fn new(provider: Box<dyn Provider>, db: Db, _config: &Config) -> Result<Self> {
        let conversation_id =
            db.create_conversation(provider.model(), provider.name())?;
        tracing::debug!("Agent created with conversation {}", conversation_id);
        Ok(Agent {
            provider,
            tools: ToolRegistry::default_tools(),
            db,
            conversation_id,
            messages: Vec::new(),
            system_prompt: SYSTEM_PROMPT.to_string(),
        })
    }

    pub fn conversation_id(&self) -> &str {
        &self.conversation_id
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }


    pub fn set_model(&mut self, model: String) {
        self.provider.set_model(model);
    }

    pub fn available_models(&self) -> Vec<String> {
        self.provider.available_models()
    }

    pub async fn fetch_models(&self) -> anyhow::Result<Vec<String>> {
        self.provider.fetch_models().await
    }

    pub fn current_model(&self) -> &str {
        self.provider.model()
    }

    pub async fn send_message(
        &mut self,
        content: &str,
        event_tx: UnboundedSender<AgentEvent>,
    ) -> Result<()> {
        self.db
            .add_message(&self.conversation_id, "user", content)?;

        self.messages.push(Message {
            role: Role::User,
            content: vec![ContentBlock::Text(content.to_string())],
        });

        if self.messages.len() == 1 {
            let title: String = content.chars().take(60).collect();
            let _ = self.db.update_conversation_title(&self.conversation_id, &title);
        }

        let mut final_usage: Option<Usage> = None;

        loop {
            let tool_defs = self.tools.definitions();

            let mut stream_rx = self
                .provider
                .stream(&self.messages, Some(&self.system_prompt), &tool_defs, 8192)
                .await?;

            let mut full_text = String::new();
            let mut tool_calls: Vec<PendingToolCall> = Vec::new();
            let mut current_tool_input = String::new();

            while let Some(event) = stream_rx.recv().await {
                match event.event_type {
                    StreamEventType::TextDelta(text) => {
                        full_text.push_str(&text);
                        let _ = event_tx.send(AgentEvent::TextDelta(text));
                    }

                    StreamEventType::ToolUseStart { id, name } => {
                        current_tool_input.clear();
                        let _ = event_tx.send(AgentEvent::ToolCallStart {
                            id: id.clone(),
                            name: name.clone(),
                        });
                        tool_calls.push(PendingToolCall {
                            id,
                            name,
                            input: String::new(),
                        });
                    }

                    StreamEventType::ToolUseInputDelta(delta) => {
                        current_tool_input.push_str(&delta);
                        let _ = event_tx.send(AgentEvent::ToolCallInputDelta(delta));
                    }

                    StreamEventType::ToolUseEnd => {
                        if let Some(tc) = tool_calls.last_mut() {
                            tc.input = current_tool_input.clone();
                        }
                        current_tool_input.clear();
                    }

                    StreamEventType::MessageEnd {
                        stop_reason: _,
                        usage,
                    } => {
                        final_usage = Some(usage);
                    }

                    _ => {}
                }
            }

            let mut content_blocks: Vec<ContentBlock> = Vec::new();

            if !full_text.is_empty() {
                content_blocks.push(ContentBlock::Text(full_text.clone()));
            }

            for tc in &tool_calls {
                let input_value: serde_json::Value =
                    serde_json::from_str(&tc.input).unwrap_or(serde_json::Value::Null);
                content_blocks.push(ContentBlock::ToolUse {
                    id: tc.id.clone(),
                    name: tc.name.clone(),
                    input: input_value,
                });
            }

            self.messages.push(Message {
                role: Role::Assistant,
                content: content_blocks,
            });

            let stored_text = if !full_text.is_empty() {
                full_text.clone()
            } else {
                String::from("[tool use]")
            };
            let assistant_msg_id = self.db.add_message(
                &self.conversation_id,
                "assistant",
                &stored_text,
            )?;

            for tc in &tool_calls {
                let _ = self.db.add_tool_call(
                    &assistant_msg_id,
                    &tc.id,
                    &tc.name,
                    &tc.input,
                );
            }

            if tool_calls.is_empty() {
                let _ = event_tx.send(AgentEvent::TextComplete(full_text));
                if let Some(usage) = final_usage {
                    let _ = event_tx.send(AgentEvent::Done { usage });
                }
                break;
            }

            let mut result_blocks: Vec<ContentBlock> = Vec::new();

            for tc in &tool_calls {
                let input_value: serde_json::Value =
                    serde_json::from_str(&tc.input).unwrap_or(serde_json::Value::Null);

                let _ = event_tx.send(AgentEvent::ToolCallExecuting {
                    id: tc.id.clone(),
                    name: tc.name.clone(),
                    input: tc.input.clone(),
                });

                let tool_name = tc.name.clone();
                let tool_input = input_value.clone();

                let exec_result = tokio::time::timeout(
                    std::time::Duration::from_secs(30),
                    async {
                        tokio::task::block_in_place(|| {
                            self.tools.execute(&tool_name, tool_input)
                        })
                    },
                )
                .await;

                let (output, is_error) = match exec_result {
                    Err(_elapsed) => (
                        format!("Tool '{}' timed out after 30 seconds.", tc.name),
                        true,
                    ),
                    Ok(Err(e)) => (e.to_string(), true),
                    Ok(Ok(out)) => (out, false),
                };

                tracing::debug!(
                    "Tool '{}' result (error={}): {}",
                    tc.name,
                    is_error,
                    &output[..output.len().min(200)]
                );

                let _ = self.db.update_tool_result(&tc.id, &output, is_error);

                let _ = event_tx.send(AgentEvent::ToolCallResult {
                    id: tc.id.clone(),
                    name: tc.name.clone(),
                    output: output.clone(),
                    is_error,
                });

                result_blocks.push(ContentBlock::ToolResult {
                    tool_use_id: tc.id.clone(),
                    content: output,
                    is_error,
                });
            }

            self.messages.push(Message {
                role: Role::User,
                content: result_blocks,
            });
        }

        Ok(())
    }
}
