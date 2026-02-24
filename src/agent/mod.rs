mod events;
mod profile;

pub use events::{AgentEvent, TodoItem, TodoStatus};
pub use profile::AgentProfile;

use events::PendingToolCall;

use crate::config::Config;
use crate::db::Db;
use crate::provider::{ContentBlock, Message, Provider, Role, StreamEventType, Usage};
use crate::tools::ToolRegistry;
use anyhow::Result;
use tokio::sync::mpsc::UnboundedSender;

const COMPACT_THRESHOLD: f32 = 0.8;
const COMPACT_KEEP_MESSAGES: usize = 10;

const TITLE_SYSTEM_PROMPT: &str = "\
You are a title generator. You output ONLY a thread title. Nothing else.

Generate a brief title that would help the user find this conversation later.

Rules:
- A single line, 50 characters or fewer
- No explanations, no quotes, no punctuation wrapping
- Use the same language as the user message
- Title must be grammatically correct and read naturally
- Never include tool names (e.g. read tool, bash tool, edit tool)
- Focus on the main topic or question the user wants to retrieve
- Vary your phrasing — avoid repetitive patterns like always starting with \"Analyzing\"
- When a file is mentioned, focus on WHAT the user wants to do WITH the file
- Keep exact: technical terms, numbers, filenames, HTTP codes
- Remove filler words: the, this, my, a, an
- If the user message is short or conversational (e.g. \"hello\", \"hey\"): \
  create a title reflecting the user's tone (Greeting, Quick check-in, etc.)";

pub struct Agent {
    providers: Vec<Box<dyn Provider>>,
    active: usize,
    tools: ToolRegistry,
    db: Db,
    conversation_id: String,
    messages: Vec<Message>,
    profiles: Vec<AgentProfile>,
    active_profile: usize,
    pub thinking_budget: u32,
    cwd: String,
    agents_context: crate::context::AgentsContext,
    last_input_tokens: u32,
}

impl Agent {
    pub fn new(
        providers: Vec<Box<dyn Provider>>,
        db: Db,
        _config: &Config,
        tools: ToolRegistry,
        profiles: Vec<AgentProfile>,
        cwd: String,
        agents_context: crate::context::AgentsContext,
    ) -> Result<Self> {
        assert!(!providers.is_empty(), "at least one provider required");
        let conversation_id =
            db.create_conversation(providers[0].model(), providers[0].name(), &cwd)?;
        tracing::debug!("Agent created with conversation {}", conversation_id);
        let profiles = if profiles.is_empty() {
            vec![AgentProfile::default_profile()]
        } else {
            profiles
        };
        Ok(Agent {
            providers,
            active: 0,
            tools,
            db,
            conversation_id,
            messages: Vec::new(),
            profiles,
            active_profile: 0,
            thinking_budget: 0,
            cwd,
            agents_context,
            last_input_tokens: 0,
        })
    }
    fn provider(&self) -> &dyn Provider {
        &*self.providers[self.active]
    }
    fn provider_mut(&mut self) -> &mut dyn Provider {
        &mut *self.providers[self.active]
    }
    fn profile(&self) -> &AgentProfile {
        &self.profiles[self.active_profile]
    }
    pub fn conversation_id(&self) -> &str {
        &self.conversation_id
    }
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }
    pub fn set_model(&mut self, model: String) {
        self.provider_mut().set_model(model);
    }
    pub fn set_active_provider(&mut self, provider_name: &str, model: &str) {
        if let Some(idx) = self
            .providers
            .iter()
            .position(|p| p.name() == provider_name)
        {
            self.active = idx;
            self.providers[idx].set_model(model.to_string());
        }
    }
    pub fn set_thinking_budget(&mut self, budget: u32) {
        self.thinking_budget = budget;
    }
    pub fn available_models(&self) -> Vec<String> {
        self.provider().available_models()
    }
    pub async fn fetch_all_models(&self) -> Vec<(String, Vec<String>)> {
        let mut result = Vec::new();
        for p in &self.providers {
            let models = match p.fetch_models().await {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!("Failed to fetch models for {}: {e}", p.name());
                    Vec::new()
                }
            };
            result.push((p.name().to_string(), models));
        }
        result
    }
    pub fn current_model(&self) -> &str {
        self.provider().model()
    }
    pub fn current_provider_name(&self) -> &str {
        self.provider().name()
    }
    pub fn current_agent_name(&self) -> &str {
        &self.profile().name
    }
    pub fn context_window(&self) -> u32 {
        self.provider().context_window()
    }
    pub async fn fetch_context_window(&self) -> u32 {
        match self.provider().fetch_context_window().await {
            Ok(cw) => cw,
            Err(e) => {
                tracing::warn!("Failed to fetch context window: {e}");
                0
            }
        }
    }
    pub fn agent_profiles(&self) -> &[AgentProfile] {
        &self.profiles
    }
    pub fn switch_agent(&mut self, name: &str) -> bool {
        if let Some(idx) = self.profiles.iter().position(|p| p.name == name) {
            self.active_profile = idx;
            let model_spec = self.profiles[idx].model_spec.clone();

            if let Some(spec) = model_spec {
                let (provider, model) = Config::parse_model_spec(&spec);
                if let Some(prov) = provider {
                    self.set_active_provider(prov, model);
                } else {
                    self.set_model(model.to_string());
                }
            }
            tracing::info!("Switched to agent '{}'", name);
            true
        } else {
            false
        }
    }
    pub fn cleanup_if_empty(&mut self) {
        if self.messages.is_empty() {
            let _ = self.db.delete_conversation(&self.conversation_id);
        }
    }
    pub fn new_conversation(&mut self) -> Result<()> {
        self.cleanup_if_empty();
        let conversation_id = self.db.create_conversation(
            self.provider().model(),
            self.provider().name(),
            &self.cwd,
        )?;
        self.conversation_id = conversation_id;
        self.messages.clear();
        Ok(())
    }
    pub fn resume_conversation(&mut self, conversation: &crate::db::Conversation) -> Result<()> {
        self.conversation_id = conversation.id.clone();
        self.messages = conversation
            .messages
            .iter()
            .map(|m| Message {
                role: if m.role == "user" {
                    Role::User
                } else {
                    Role::Assistant
                },
                content: vec![ContentBlock::Text(m.content.clone())],
            })
            .collect();
        tracing::debug!("Resumed conversation {}", conversation.id);
        Ok(())
    }
    pub fn list_sessions(&self) -> Result<Vec<crate::db::ConversationSummary>> {
        self.db.list_conversations_for_cwd(&self.cwd, 50)
    }
    pub fn get_session(&self, id: &str) -> Result<crate::db::Conversation> {
        self.db.get_conversation(id)
    }
    pub fn conversation_title(&self) -> Option<String> {
        self.db
            .get_conversation(&self.conversation_id)
            .ok()
            .and_then(|c| c.title)
    }
    pub fn cwd(&self) -> &str {
        &self.cwd
    }

    pub fn truncate_messages(&mut self, count: usize) {
        let target = count.min(self.messages.len());
        self.messages.truncate(target);
    }

    pub fn fork_conversation(&mut self, msg_count: usize) -> Result<()> {
        let kept = self.messages[..msg_count.min(self.messages.len())].to_vec();
        self.cleanup_if_empty();
        let conversation_id = self.db.create_conversation(
            self.provider().model(),
            self.provider().name(),
            &self.cwd,
        )?;
        self.conversation_id = conversation_id;
        self.messages = kept;
        for msg in &self.messages {
            let role = match msg.role {
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::System => "system",
            };
            let text: String = msg
                .content
                .iter()
                .filter_map(|b| {
                    if let ContentBlock::Text(t) = b {
                        Some(t.as_str())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            if !text.is_empty() {
                let _ = self.db.add_message(&self.conversation_id, role, &text);
            }
        }
        Ok(())
    }

    fn title_model(&self) -> &str {
        match self.provider().name() {
            "anthropic" => "claude-3-5-haiku-20241022",
            "openai" => "gpt-5-nano",
            _ => self.provider().model(),
        }
    }

    fn should_compact(&self) -> bool {
        let limit = self.provider().context_window();
        let threshold = (limit as f32 * COMPACT_THRESHOLD) as u32;
        self.last_input_tokens >= threshold
    }
    async fn compact(&mut self, event_tx: &UnboundedSender<AgentEvent>) -> Result<()> {
        let keep = COMPACT_KEEP_MESSAGES;
        if self.messages.len() <= keep + 2 {
            return Ok(());
        }
        let cutoff = self.messages.len() - keep;
        let old_messages = self.messages[..cutoff].to_vec();
        let kept = self.messages[cutoff..].to_vec();

        let mut summary_text = String::new();
        for msg in &old_messages {
            let role = match msg.role {
                Role::User => "User",
                Role::Assistant => "Assistant",
                Role::System => "System",
            };
            for block in &msg.content {
                if let ContentBlock::Text(t) = block {
                    summary_text.push_str(&format!("{}:\n{}\n\n", role, t));
                }
            }
        }
        let summary_request = vec![Message {
            role: Role::User,
            content: vec![ContentBlock::Text(format!(
                "Summarize the following conversation history concisely, preserving all key decisions, facts, code changes, and context that would be needed to continue the work:\n\n{}",
                summary_text
            ))],
        }];

        let mut stream_rx = self
            .provider()
            .stream(
                &summary_request,
                Some("You are a concise summarizer. Produce a dense, factual summary."),
                &[],
                4096,
                0,
            )
            .await?;
        let mut full_summary = String::new();
        while let Some(event) = stream_rx.recv().await {
            if let StreamEventType::TextDelta(text) = event.event_type {
                full_summary.push_str(&text);
            }
        }
        self.messages = vec![
            Message {
                role: Role::User,
                content: vec![ContentBlock::Text(
                    "[Previous conversation summarized below]".to_string(),
                )],
            },
            Message {
                role: Role::Assistant,
                content: vec![ContentBlock::Text(format!(
                    "Summary of prior context:\n\n{}",
                    full_summary
                ))],
            },
        ];
        self.messages.extend(kept);

        let _ = self.db.add_message(
            &self.conversation_id,
            "assistant",
            &format!("[Compacted {} messages into summary]", cutoff),
        );
        self.last_input_tokens = 0;
        let _ = event_tx.send(AgentEvent::Compacted {
            messages_removed: cutoff,
        });
        Ok(())
    }
    pub async fn send_message(
        &mut self,
        content: &str,
        event_tx: UnboundedSender<AgentEvent>,
    ) -> Result<()> {
        self.send_message_with_images(content, Vec::new(), event_tx)
            .await
    }

    pub async fn send_message_with_images(
        &mut self,
        content: &str,
        images: Vec<(String, String)>,
        event_tx: UnboundedSender<AgentEvent>,
    ) -> Result<()> {
        if self.should_compact() {
            self.compact(&event_tx).await?;
        }
        self.db
            .add_message(&self.conversation_id, "user", content)?;
        let mut blocks: Vec<ContentBlock> = Vec::new();
        for (media_type, data) in images {
            blocks.push(ContentBlock::Image { media_type, data });
        }
        blocks.push(ContentBlock::Text(content.to_string()));
        self.messages.push(Message {
            role: Role::User,
            content: blocks,
        });
        let title_rx = if self.messages.len() == 1 {
            let title_messages = vec![Message {
                role: Role::User,
                content: vec![ContentBlock::Text(format!(
                    "Generate a title for this conversation:\n\n{}",
                    content
                ))],
            }];
            self.provider()
                .stream_with_model(
                    self.title_model(),
                    &title_messages,
                    Some(TITLE_SYSTEM_PROMPT),
                    &[],
                    100,
                    0,
                )
                .await
                .ok()
        } else {
            None
        };
        let mut final_usage: Option<Usage> = None;
        let system_prompt = self
            .agents_context
            .apply_to_system_prompt(&self.profile().system_prompt);
        let tool_filter = self.profile().tool_filter.clone();
        let thinking_budget = self.thinking_budget;
        loop {
            let mut tool_defs = self.tools.definitions_filtered(&tool_filter);
            tool_defs.push(crate::provider::ToolDefinition {
                name: "todo_write".to_string(),
                description: "Create or update the task list for the current session. Use to track progress on multi-step tasks.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "todos": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "content": { "type": "string", "description": "Brief description of the task" },
                                    "status": { "type": "string", "enum": ["pending", "in_progress", "completed"], "description": "Current status" }
                                },
                                "required": ["content", "status"]
                            }
                        }
                    },
                    "required": ["todos"]
                }),
            });
            let mut stream_rx = self
                .provider()
                .stream(
                    &self.messages,
                    Some(&system_prompt),
                    &tool_defs,
                    8192,
                    thinking_budget,
                )
                .await?;
            let mut full_text = String::new();
            let mut full_thinking = String::new();
            let mut full_thinking_signature = String::new();
            let mut tool_calls: Vec<PendingToolCall> = Vec::new();
            let mut current_tool_input = String::new();
            while let Some(event) = stream_rx.recv().await {
                match event.event_type {
                    StreamEventType::TextDelta(text) => {
                        full_text.push_str(&text);
                        let _ = event_tx.send(AgentEvent::TextDelta(text));
                    }
                    StreamEventType::ThinkingDelta(text) => {
                        full_thinking.push_str(&text);
                        let _ = event_tx.send(AgentEvent::ThinkingDelta(text));
                    }
                    StreamEventType::ThinkingComplete {
                        thinking,
                        signature,
                    } => {
                        full_thinking = thinking;
                        full_thinking_signature = signature;
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
                        self.last_input_tokens = usage.input_tokens;
                        final_usage = Some(usage);
                    }

                    _ => {}
                }
            }

            let mut content_blocks: Vec<ContentBlock> = Vec::new();
            if !full_thinking.is_empty() {
                content_blocks.push(ContentBlock::Thinking {
                    thinking: full_thinking.clone(),
                    signature: full_thinking_signature.clone(),
                });
            }
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
            let assistant_msg_id =
                self.db
                    .add_message(&self.conversation_id, "assistant", &stored_text)?;
            for tc in &tool_calls {
                let _ = self
                    .db
                    .add_tool_call(&assistant_msg_id, &tc.id, &tc.name, &tc.input);
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
                if tc.name == "todo_write" {
                    if let Some(todos_arr) = input_value.get("todos").and_then(|v| v.as_array()) {
                        let items: Vec<TodoItem> = todos_arr
                            .iter()
                            .filter_map(|t| {
                                let content = t.get("content")?.as_str()?.to_string();
                                let status = match t
                                    .get("status")
                                    .and_then(|s| s.as_str())
                                    .unwrap_or("pending")
                                {
                                    "in_progress" => TodoStatus::InProgress,
                                    "completed" => TodoStatus::Completed,
                                    _ => TodoStatus::Pending,
                                };
                                Some(TodoItem { content, status })
                            })
                            .collect();
                        let _ = event_tx.send(AgentEvent::TodoUpdate(items));
                    }
                    let _ = event_tx.send(AgentEvent::ToolCallResult {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        output: "ok".to_string(),
                        is_error: false,
                    });
                    result_blocks.push(ContentBlock::ToolResult {
                        tool_use_id: tc.id.clone(),
                        content: "ok".to_string(),
                        is_error: false,
                    });
                    continue;
                }
                let _ = event_tx.send(AgentEvent::ToolCallExecuting {
                    id: tc.id.clone(),
                    name: tc.name.clone(),
                    input: tc.input.clone(),
                });
                let tool_name = tc.name.clone();
                let tool_input = input_value.clone();

                let exec_result = tokio::time::timeout(std::time::Duration::from_secs(30), async {
                    tokio::task::block_in_place(|| self.tools.execute(&tool_name, tool_input))
                })
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

        if let Some(mut rx) = title_rx {
            let mut raw = String::new();
            while let Some(event) = rx.recv().await {
                if let StreamEventType::TextDelta(text) = event.event_type {
                    raw.push_str(&text);
                }
            }
            let title = raw
                .trim()
                .trim_matches('"')
                .trim_matches('`')
                .trim_matches('*')
                .replace('\n', " ");
            let title: String = title.chars().take(50).collect();
            if !title.is_empty() {
                let _ = self
                    .db
                    .update_conversation_title(&self.conversation_id, &title);
                let _ = event_tx.send(AgentEvent::TitleGenerated(title));
            }
        }

        Ok(())
    }
}
