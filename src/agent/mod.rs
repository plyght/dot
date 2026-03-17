mod events;
mod profile;
mod subagent;

pub use events::{AgentEvent, QuestionResponder, TodoItem, TodoStatus};
pub use profile::AgentProfile;

use events::PendingToolCall;

use crate::command::CommandRegistry;
use crate::config::Config;
use crate::db::Db;
use crate::extension::{Event, EventContext, HookRegistry, HookResult};
use crate::memory::MemoryStore;
use crate::provider::{ContentBlock, Message, Provider, Role, StreamEventType, Usage};
use crate::tools::ToolRegistry;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

const COMPACT_THRESHOLD: f32 = 0.8;
const COMPACT_KEEP_MESSAGES: usize = 10;

const MEMORY_INSTRUCTIONS: &str = "\n\n\
# Memory

You have persistent memory across conversations. **Core blocks** (above) are always visible — update them via `core_memory_update` for essential user/agent facts. **Archival memory** is searched per turn — use `memory_add`/`memory_search`/`memory_list`/`memory_delete` to manage it.

When the user says \"remember\"/\"forget\"/\"what do you know about me\", use the appropriate memory tool. Memories are also auto-extracted in the background, so focus on explicit requests.";

/// Tool call data for persisting an interrupted assistant message.
#[derive(Debug, Clone)]
pub struct InterruptedToolCall {
    pub name: String,
    pub input: String,
    pub output: Option<String>,
    pub is_error: bool,
}

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
    providers: Vec<Arc<dyn Provider>>,
    active: usize,
    tools: Arc<ToolRegistry>,
    db: Db,
    memory: Option<Arc<MemoryStore>>,
    memory_auto_extract: bool,
    memory_inject_count: usize,
    conversation_id: String,
    persisted: bool,
    messages: Vec<Message>,
    profiles: Vec<AgentProfile>,
    active_profile: usize,
    pub thinking_budget: u32,
    cwd: String,
    agents_context: crate::context::AgentsContext,
    last_input_tokens: u32,
    permissions: HashMap<String, String>,
    snapshots: crate::snapshot::SnapshotManager,
    hooks: HookRegistry,
    commands: CommandRegistry,
    subagent_enabled: bool,
    subagent_max_turns: usize,
    background_results: Arc<std::sync::Mutex<HashMap<String, String>>>,
    background_handles: HashMap<String, tokio::task::JoinHandle<()>>,
    background_tx: Option<UnboundedSender<AgentEvent>>,
}

impl Agent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        providers: Vec<Box<dyn Provider>>,
        db: Db,
        config: &Config,
        memory: Option<Arc<MemoryStore>>,
        tools: ToolRegistry,
        profiles: Vec<AgentProfile>,
        cwd: String,
        agents_context: crate::context::AgentsContext,
        hooks: HookRegistry,
        commands: CommandRegistry,
    ) -> Result<Self> {
        assert!(!providers.is_empty(), "at least one provider required");
        let providers: Vec<Arc<dyn Provider>> = providers.into_iter().map(Arc::from).collect();
        let conversation_id = uuid::Uuid::new_v4().to_string();
        tracing::debug!("Agent created with conversation {}", conversation_id);
        let mut profiles = if profiles.is_empty() {
            vec![AgentProfile::default_profile()]
        } else {
            profiles
        };
        if !profiles.iter().any(|p| p.name == "plan") {
            let at = 1.min(profiles.len());
            profiles.insert(at, AgentProfile::plan_profile());
        }
        Ok(Agent {
            providers,
            active: 0,
            tools: Arc::new(tools),
            db,
            memory,
            memory_auto_extract: config.memory.auto_extract,
            memory_inject_count: config.memory.inject_count,
            conversation_id,
            persisted: false,
            messages: Vec::new(),
            profiles,
            active_profile: 0,
            thinking_budget: 0,
            cwd,
            agents_context,
            last_input_tokens: 0,
            permissions: config.permissions.clone(),
            snapshots: crate::snapshot::SnapshotManager::new(),
            hooks,
            commands,
            subagent_enabled: config.subagents.enabled,
            subagent_max_turns: config.subagents.max_turns,
            background_results: Arc::new(std::sync::Mutex::new(HashMap::new())),
            background_handles: HashMap::new(),
            background_tx: None,
        })
    }
    fn ensure_persisted(&mut self) -> Result<()> {
        if !self.persisted {
            self.db.create_conversation_with_id(
                &self.conversation_id,
                self.provider().model(),
                self.provider().name(),
                &self.cwd,
            )?;
            self.persisted = true;
        }
        Ok(())
    }
    fn provider(&self) -> &dyn Provider {
        &*self.providers[self.active]
    }
    fn provider_arc(&self) -> Arc<dyn Provider> {
        Arc::clone(&self.providers[self.active])
    }
    pub fn aside_provider(&self) -> Arc<dyn Provider> {
        Arc::clone(&self.providers[self.active])
    }
    pub fn set_background_tx(&mut self, tx: UnboundedSender<AgentEvent>) {
        self.background_tx = Some(tx);
    }
    pub fn background_tx(&self) -> Option<UnboundedSender<AgentEvent>> {
        self.background_tx.clone()
    }
    fn event_context(&self, event: &Event) -> EventContext {
        EventContext {
            event: event.as_str().to_string(),
            model: self.provider().model().to_string(),
            provider: self.provider().name().to_string(),
            cwd: self.cwd.clone(),
            session_id: self.conversation_id.clone(),
            ..Default::default()
        }
    }
    pub fn execute_command(&self, name: &str, args: &str) -> Result<String> {
        self.commands.execute(name, args, &self.cwd)
    }
    pub fn list_commands(&self) -> Vec<(&str, &str)> {
        self.commands.list()
    }
    pub fn has_command(&self, name: &str) -> bool {
        self.commands.has(name)
    }
    pub fn hooks(&self) -> &HookRegistry {
        &self.hooks
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
        if let Some(p) = Arc::get_mut(&mut self.providers[self.active]) {
            p.set_model(model);
        } else {
            tracing::warn!("cannot change model while background subagent is active");
        }
    }
    pub fn set_active_provider(&mut self, provider_name: &str, model: &str) {
        if let Some(idx) = self
            .providers
            .iter()
            .position(|p| p.name() == provider_name)
        {
            self.active = idx;
            if let Some(p) = Arc::get_mut(&mut self.providers[idx]) {
                p.set_model(model.to_string());
            } else {
                tracing::warn!("cannot change model while background subagent is active");
            }
        }
    }
    pub fn set_thinking_budget(&mut self, budget: u32) {
        self.thinking_budget = budget;
    }
    pub fn available_models(&self) -> Vec<String> {
        self.provider().available_models()
    }
    pub fn cached_all_models(&self) -> Vec<(String, Vec<String>)> {
        self.providers
            .iter()
            .map(|p| (p.name().to_string(), p.available_models()))
            .collect()
    }
    pub async fn fetch_all_models(&mut self) -> Vec<(String, Vec<String>)> {
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
        if self.messages.is_empty() && self.persisted {
            let _ = self.db.delete_conversation(&self.conversation_id);
        }
    }
    pub fn new_conversation(&mut self) -> Result<()> {
        self.cleanup_if_empty();
        self.conversation_id = uuid::Uuid::new_v4().to_string();
        self.persisted = false;
        self.messages.clear();
        Ok(())
    }
    pub fn resume_conversation(&mut self, conversation: &crate::db::Conversation) -> Result<()> {
        self.conversation_id = conversation.id.clone();
        self.persisted = true;
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
        {
            let ctx = self.event_context(&Event::OnResume);
            self.hooks.emit(&Event::OnResume, &ctx);
        }
        Ok(())
    }

    /// Add an interrupted (cancelled) assistant message to context and DB so the
    /// model sees it on the next send and can continue from where it stopped.
    pub fn add_interrupted_message(
        &mut self,
        content: String,
        tool_calls: Vec<InterruptedToolCall>,
        thinking: Option<String>,
    ) -> Result<()> {
        let mut blocks: Vec<ContentBlock> = Vec::new();
        if let Some(t) = thinking
            && !t.is_empty()
        {
            blocks.push(ContentBlock::Thinking {
                thinking: t,
                signature: String::new(),
            });
        }
        if !content.is_empty() {
            blocks.push(ContentBlock::Text(content.clone()));
        }
        let mut tool_ids: Vec<String> = Vec::new();
        for tc in &tool_calls {
            let id = Uuid::new_v4().to_string();
            tool_ids.push(id.clone());
            let input_value: serde_json::Value =
                serde_json::from_str(&tc.input).unwrap_or_else(|_| serde_json::json!({}));
            blocks.push(ContentBlock::ToolUse {
                id: id.clone(),
                name: tc.name.clone(),
                input: input_value,
            });
        }
        for (tc, id) in tool_calls.iter().zip(tool_ids.iter()) {
            blocks.push(ContentBlock::ToolResult {
                tool_use_id: id.clone(),
                content: tc.output.clone().unwrap_or_default(),
                is_error: tc.is_error,
            });
        }
        self.messages.push(Message {
            role: Role::Assistant,
            content: blocks,
        });
        let stored_text = if content.is_empty() {
            String::from("[tool use]")
        } else {
            content
        };
        self.ensure_persisted()?;
        let assistant_msg_id =
            self.db
                .add_message(&self.conversation_id, "assistant", &stored_text)?;
        for (tc, id) in tool_calls.iter().zip(tool_ids.iter()) {
            let _ = self
                .db
                .add_tool_call(&assistant_msg_id, id, &tc.name, &tc.input);
            if let Some(ref output) = tc.output {
                let _ = self.db.update_tool_result(id, output, tc.is_error);
            }
        }
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
    pub fn rename_session(&self, title: &str) -> Result<()> {
        self.db
            .update_conversation_title(&self.conversation_id, title)
            .context("failed to rename session")
    }
    pub fn cwd(&self) -> &str {
        &self.cwd
    }

    pub fn truncate_messages(&mut self, count: usize) {
        let target = count.min(self.messages.len());
        self.messages.truncate(target);
    }

    pub fn revert_to_message(&mut self, keep: usize) -> Result<Vec<String>> {
        let keep = keep.min(self.messages.len());
        let checkpoint_idx = self.messages[..keep]
            .iter()
            .filter(|m| m.role == Role::Assistant)
            .count();
        self.messages.truncate(keep);
        self.db
            .truncate_messages(&self.conversation_id, keep)
            .context("truncating db messages")?;
        let restored = if checkpoint_idx > 0 {
            let res = self.snapshots.restore_to_checkpoint(checkpoint_idx - 1)?;
            self.snapshots.truncate_checkpoints(checkpoint_idx);
            res
        } else {
            let res = self.snapshots.restore_all()?;
            self.snapshots.truncate_checkpoints(0);
            res
        };
        Ok(restored)
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
        self.persisted = true;
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
        self.provider().model()
    }

    fn should_compact(&self) -> bool {
        let limit = self.provider().context_window();
        let threshold = (limit as f32 * COMPACT_THRESHOLD) as u32;
        self.last_input_tokens >= threshold
    }
    fn emit_compact_hooks(&self, phase: &Event) {
        let ctx = self.event_context(phase);
        self.hooks.emit(phase, &ctx);
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

        self.emit_compact_hooks(&Event::BeforeCompact);
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
        self.emit_compact_hooks(&Event::AfterCompact);
        Ok(())
    }
    /// Remove orphaned tool-call cycles and trailing user messages from the
    /// end of the conversation. This handles cases where a previous stream was
    /// cancelled or errored mid-execution, leaving ToolResult messages without
    /// a subsequent assistant response, assistant ToolUse messages without
    /// corresponding ToolResult messages, or plain user messages that never
    /// received a response (which would cause consecutive user messages on the
    /// next send).
    fn sanitize(&mut self) {
        loop {
            let dominated = match self.messages.last() {
                None => false,
                Some(msg) if msg.role == Role::User => {
                    !msg.content.is_empty()
                        && msg
                            .content
                            .iter()
                            .all(|b| matches!(b, ContentBlock::ToolResult { .. }))
                }
                Some(msg) if msg.role == Role::Assistant => msg
                    .content
                    .iter()
                    .any(|b| matches!(b, ContentBlock::ToolUse { .. })),
                _ => false,
            };
            if dominated {
                self.messages.pop();
            } else {
                break;
            }
        }
        // Drop any trailing user message to prevent consecutive user messages.
        // This happens when a previous send_message was cancelled after pushing
        // the user message but before the assistant could respond.
        if matches!(self.messages.last(), Some(msg) if msg.role == Role::User) {
            self.messages.pop();
        }
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
        self.sanitize();
        {
            let mut ctx = self.event_context(&Event::OnUserInput);
            ctx.prompt = Some(content.to_string());
            self.hooks.emit(&Event::OnUserInput, &ctx);
        }
        if !self.provider().supports_server_compaction() && self.should_compact() {
            self.compact(&event_tx).await?;
        }
        self.ensure_persisted()?;
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
            let preview: String = content.chars().take(50).collect();
            let preview = preview.trim().to_string();
            if !preview.is_empty() {
                let _ = self
                    .db
                    .update_conversation_title(&self.conversation_id, &preview);
                let _ = event_tx.send(AgentEvent::TitleGenerated(preview));
            }
            let title_messages = vec![Message {
                role: Role::User,
                content: vec![ContentBlock::Text(format!(
                    "Generate a title for this conversation:\n\n{}",
                    content
                ))],
            }];
            match self
                .provider()
                .stream_with_model(
                    self.title_model(),
                    &title_messages,
                    Some(TITLE_SYSTEM_PROMPT),
                    &[],
                    100,
                    0,
                )
                .await
            {
                Ok(rx) => Some(rx),
                Err(e) => {
                    tracing::warn!("title generation stream failed: {e}");
                    None
                }
            }
        } else {
            None
        };
        let mut final_usage: Option<Usage> = None;
        let mut system_prompt = self
            .agents_context
            .apply_to_system_prompt(&self.profile().system_prompt);
        if let Some(ref store) = self.memory {
            let query: String = content.chars().take(200).collect();
            match store.inject_context(&query, self.memory_inject_count) {
                Ok(ctx) if !ctx.is_empty() => {
                    system_prompt.push_str("\n\n");
                    system_prompt.push_str(&ctx);
                }
                Err(e) => tracing::warn!("memory injection failed: {e}"),
                _ => {}
            }
            system_prompt.push_str(MEMORY_INSTRUCTIONS);
        }
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
            tool_defs.push(crate::provider::ToolDefinition {
                name: "question".to_string(),
                description: "Ask the user a question and wait for their response. Use when you need clarification or a decision from the user.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "question": { "type": "string", "description": "The question to ask the user" },
                        "options": { "type": "array", "items": { "type": "string" }, "description": "Optional list of choices" }
                    },
                    "required": ["question"]
                }),
            });
            tool_defs.push(crate::provider::ToolDefinition {
                name: "snapshot_list".to_string(),
                description: "List all files that have been created or modified in this session."
                    .to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {},
                }),
            });
            tool_defs.push(crate::provider::ToolDefinition {
                name: "snapshot_restore".to_string(),
                description: "Restore a file to its original state before this session modified it. Pass a path or omit to restore all files.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "File path to restore (omit to restore all)" }
                    },
                }),
            });
            if self.subagent_enabled {
                let profile_names: Vec<String> =
                    self.profiles.iter().map(|p| p.name.clone()).collect();
                let profiles_desc = if profile_names.is_empty() {
                    String::new()
                } else {
                    format!(" Available profiles: {}.", profile_names.join(", "))
                };
                tool_defs.push(crate::provider::ToolDefinition {
                    name: "subagent".to_string(),
                    description: format!(
                        "Delegate a focused task to a subagent that runs in isolated context with its own conversation. \
                         The subagent has access to tools and works autonomously without user interaction. \
                         Use for complex subtasks that benefit from separate context (research, code analysis, multi-file changes). \
                         Set background=true to run non-blocking (returns immediately with an ID; retrieve results later with subagent_result).{}",
                        profiles_desc
                    ),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "description": {
                                "type": "string",
                                "description": "What the subagent should do (used as system prompt context)"
                            },
                            "task": {
                                "type": "string",
                                "description": "The specific task prompt for the subagent"
                            },
                            "profile": {
                                "type": "string",
                                "description": "Agent profile to use (affects available tools and system prompt)"
                            },
                            "background": {
                                "type": "boolean",
                                "description": "Run in background (non-blocking). Returns an ID to check later with subagent_result."
                            }
                        },
                        "required": ["description", "task"]
                    }),
                });
                tool_defs.push(crate::provider::ToolDefinition {
                    name: "subagent_result".to_string(),
                    description: "Retrieve the result of a background subagent by ID. Returns the output if complete, or a status message if still running.".to_string(),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "id": {
                                "type": "string",
                                "description": "The subagent ID returned when it was launched in background mode"
                            }
                        },
                        "required": ["id"]
                    }),
                });
            }
            if self.memory.is_some() {
                tool_defs.extend(crate::memory::tools::definitions());
            }
            {
                let mut ctx = self.event_context(&Event::BeforePrompt);
                ctx.prompt = Some(content.to_string());
                match self.hooks.emit_blocking(&Event::BeforePrompt, &ctx) {
                    HookResult::Block(reason) => {
                        let _ = event_tx.send(AgentEvent::TextComplete(format!(
                            "[blocked by hook: {}]",
                            reason.trim()
                        )));
                        return Ok(());
                    }
                    HookResult::Modify(_modified) => {}
                    HookResult::Allow => {}
                }
            }
            self.hooks.emit(
                &Event::OnStreamStart,
                &self.event_context(&Event::OnStreamStart),
            );
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
            self.hooks.emit(
                &Event::OnStreamEnd,
                &self.event_context(&Event::OnStreamEnd),
            );
            let mut full_text = String::new();
            let mut full_thinking = String::new();
            let mut full_thinking_signature = String::new();
            let mut compaction_content: Option<String> = None;
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
                    StreamEventType::CompactionComplete(content) => {
                        let _ = event_tx.send(AgentEvent::Compacting);
                        compaction_content = Some(content);
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
                        let _ = self
                            .db
                            .update_last_input_tokens(&self.conversation_id, usage.input_tokens);
                        final_usage = Some(usage);
                    }

                    _ => {}
                }
            }

            let mut content_blocks: Vec<ContentBlock> = Vec::new();
            if let Some(ref summary) = compaction_content {
                content_blocks.push(ContentBlock::Compaction {
                    content: summary.clone(),
                });
            }
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
                    serde_json::from_str(&tc.input).unwrap_or_else(|_| serde_json::json!({}));
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
            {
                let mut ctx = self.event_context(&Event::AfterPrompt);
                ctx.prompt = Some(full_text.clone());
                self.hooks.emit(&Event::AfterPrompt, &ctx);
            }
            self.snapshots.checkpoint();
            if tool_calls.is_empty() {
                let _ = event_tx.send(AgentEvent::TextComplete(full_text));
                if let Some(usage) = final_usage {
                    let _ = event_tx.send(AgentEvent::Done { usage });
                }
                if self.memory_auto_extract
                    && let Some(ref store) = self.memory
                {
                    let msgs = self.messages.clone();
                    let provider = self.provider_arc();
                    let store = Arc::clone(store);
                    let conv_id = self.conversation_id.clone();
                    let etx = event_tx.clone();
                    tokio::spawn(async move {
                        match crate::memory::extract::extract(&msgs, &*provider, &store, &conv_id)
                            .await
                        {
                            Ok(result)
                                if result.added > 0 || result.updated > 0 || result.deleted > 0 =>
                            {
                                let _ = etx.send(AgentEvent::MemoryExtracted {
                                    added: result.added,
                                    updated: result.updated,
                                    deleted: result.deleted,
                                });
                            }
                            Err(e) => tracing::warn!("memory extraction failed: {e}"),
                            _ => {}
                        }
                    });
                }
                break;
            }

            let mut result_blocks: Vec<ContentBlock> = Vec::new();

            for tc in &tool_calls {
                let input_value: serde_json::Value =
                    serde_json::from_str(&tc.input).unwrap_or_else(|_| serde_json::json!({}));
                // Virtual tool: todo_write
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
                // Virtual tool: question
                if tc.name == "question" {
                    let question = input_value
                        .get("question")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?")
                        .to_string();
                    let options: Vec<String> = input_value
                        .get("options")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default();
                    let (tx, rx) = tokio::sync::oneshot::channel();
                    let _ = event_tx.send(AgentEvent::Question {
                        id: tc.id.clone(),
                        question: question.clone(),
                        options,
                        responder: QuestionResponder(tx),
                    });
                    let answer = match rx.await {
                        Ok(a) => a,
                        Err(_) => "[cancelled]".to_string(),
                    };
                    let _ = event_tx.send(AgentEvent::ToolCallResult {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        output: answer.clone(),
                        is_error: false,
                    });
                    result_blocks.push(ContentBlock::ToolResult {
                        tool_use_id: tc.id.clone(),
                        content: answer,
                        is_error: false,
                    });
                    continue;
                }
                // Virtual tool: snapshot_list
                if tc.name == "snapshot_list" {
                    let changes = self.snapshots.list_changes();
                    let output = if changes.is_empty() {
                        "No file changes in this session.".to_string()
                    } else {
                        changes
                            .iter()
                            .map(|(p, k)| format!("{} {}", k.icon(), p))
                            .collect::<Vec<_>>()
                            .join("\n")
                    };
                    let _ = event_tx.send(AgentEvent::ToolCallResult {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        output: output.clone(),
                        is_error: false,
                    });
                    result_blocks.push(ContentBlock::ToolResult {
                        tool_use_id: tc.id.clone(),
                        content: output,
                        is_error: false,
                    });
                    continue;
                }
                // Virtual tool: snapshot_restore
                if tc.name == "snapshot_restore" {
                    let output =
                        if let Some(path) = input_value.get("path").and_then(|v| v.as_str()) {
                            match self.snapshots.restore(path) {
                                Ok(msg) => msg,
                                Err(e) => e.to_string(),
                            }
                        } else {
                            match self.snapshots.restore_all() {
                                Ok(msgs) => {
                                    if msgs.is_empty() {
                                        "Nothing to restore.".to_string()
                                    } else {
                                        msgs.join("\n")
                                    }
                                }
                                Err(e) => e.to_string(),
                            }
                        };
                    let _ = event_tx.send(AgentEvent::ToolCallResult {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        output: output.clone(),
                        is_error: false,
                    });
                    result_blocks.push(ContentBlock::ToolResult {
                        tool_use_id: tc.id.clone(),
                        content: output,
                        is_error: false,
                    });
                    continue;
                }
                // Virtual tool: batch
                if tc.name == "batch" {
                    let invocations = input_value
                        .get("invocations")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    tracing::debug!("batch: {} invocations", invocations.len());
                    let results: Vec<serde_json::Value> = invocations
                        .iter()
                        .map(|inv| {
                            let name = inv.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
                            let input = inv.get("input").cloned().unwrap_or(serde_json::Value::Null);
                            match self.tools.execute(name, input) {
                                Ok(out) => serde_json::json!({ "tool_name": name, "result": out, "is_error": false }),
                                Err(e) => serde_json::json!({ "tool_name": name, "result": e.to_string(), "is_error": true }),
                            }
                        })
                        .collect();
                    let output = serde_json::to_string(&results).unwrap_or_else(|e| e.to_string());
                    let _ = event_tx.send(AgentEvent::ToolCallResult {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        output: output.clone(),
                        is_error: false,
                    });
                    result_blocks.push(ContentBlock::ToolResult {
                        tool_use_id: tc.id.clone(),
                        content: output,
                        is_error: false,
                    });
                    continue;
                }
                // Virtual tool: subagent
                if tc.name == "subagent" {
                    let description = input_value
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("subtask")
                        .to_string();
                    let task = input_value
                        .get("task")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let profile = input_value
                        .get("profile")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let background = input_value
                        .get("background")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    let output = if background {
                        match self.spawn_background_subagent(
                            &description,
                            &task,
                            profile.as_deref(),
                        ) {
                            Ok(id) => format!("Background subagent launched with id: {id}"),
                            Err(e) => {
                                tracing::error!("background subagent error: {e}");
                                format!("[subagent error: {e}]")
                            }
                        }
                    } else {
                        match self
                            .run_subagent(&description, &task, profile.as_deref(), &event_tx)
                            .await
                        {
                            Ok(text) => text,
                            Err(e) => {
                                tracing::error!("subagent error: {e}");
                                format!("[subagent error: {e}]")
                            }
                        }
                    };
                    let is_error = output.starts_with("[subagent error:");
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
                    continue;
                }
                // Virtual tool: subagent_result
                if tc.name == "subagent_result" {
                    let id = input_value.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    let output = {
                        let results = self
                            .background_results
                            .lock()
                            .unwrap_or_else(|e| e.into_inner());
                        if let Some(result) = results.get(id) {
                            result.clone()
                        } else if self.background_handles.contains_key(id) {
                            format!("Subagent '{id}' is still running.")
                        } else {
                            format!("No subagent found with id '{id}'.")
                        }
                    };
                    let _ = event_tx.send(AgentEvent::ToolCallResult {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        output: output.clone(),
                        is_error: false,
                    });
                    result_blocks.push(ContentBlock::ToolResult {
                        tool_use_id: tc.id.clone(),
                        content: output,
                        is_error: false,
                    });
                    continue;
                }
                // Virtual tools: memory
                if let Some(ref store) = self.memory
                    && let Some((output, is_error)) = crate::memory::tools::handle(
                        &tc.name,
                        &input_value,
                        store,
                        &self.conversation_id,
                    )
                {
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
                    continue;
                }
                // Permission check
                let perm = self
                    .permissions
                    .get(&tc.name)
                    .map(|s| s.as_str())
                    .unwrap_or("allow");
                if perm == "deny" {
                    let output = format!("Tool '{}' is denied by permissions config.", tc.name);
                    let _ = event_tx.send(AgentEvent::ToolCallResult {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        output: output.clone(),
                        is_error: true,
                    });
                    result_blocks.push(ContentBlock::ToolResult {
                        tool_use_id: tc.id.clone(),
                        content: output,
                        is_error: true,
                    });
                    continue;
                }
                if perm == "ask" {
                    let summary = format!("{}: {}", tc.name, &tc.input[..tc.input.len().min(100)]);
                    let (ptx, prx) = tokio::sync::oneshot::channel();
                    let _ = event_tx.send(AgentEvent::PermissionRequest {
                        tool_name: tc.name.clone(),
                        input_summary: summary,
                        responder: QuestionResponder(ptx),
                    });
                    let answer = match prx.await {
                        Ok(a) => a,
                        Err(_) => "deny".to_string(),
                    };
                    if answer != "allow" {
                        let output = format!("Tool '{}' denied by user.", tc.name);
                        let _ = event_tx.send(AgentEvent::ToolCallResult {
                            id: tc.id.clone(),
                            name: tc.name.clone(),
                            output: output.clone(),
                            is_error: true,
                        });
                        result_blocks.push(ContentBlock::ToolResult {
                            tool_use_id: tc.id.clone(),
                            content: output,
                            is_error: true,
                        });
                        continue;
                    }
                }
                // Snapshot before file writes
                if tc.name == "write_file" || tc.name == "apply_patch" {
                    if tc.name == "write_file" {
                        if let Some(path) = input_value.get("path").and_then(|v| v.as_str()) {
                            self.snapshots.before_write(path);
                        }
                    } else if let Some(patches) =
                        input_value.get("patches").and_then(|v| v.as_array())
                    {
                        for patch in patches {
                            if let Some(path) = patch.get("path").and_then(|v| v.as_str()) {
                                self.snapshots.before_write(path);
                            }
                        }
                    }
                }
                {
                    let mut ctx = self.event_context(&Event::BeforeToolCall);
                    ctx.tool_name = Some(tc.name.clone());
                    ctx.tool_input = Some(tc.input.clone());
                    match self.hooks.emit_blocking(&Event::BeforeToolCall, &ctx) {
                        HookResult::Block(reason) => {
                            let output = format!("[blocked by hook: {}]", reason.trim());
                            let _ = event_tx.send(AgentEvent::ToolCallResult {
                                id: tc.id.clone(),
                                name: tc.name.clone(),
                                output: output.clone(),
                                is_error: true,
                            });
                            result_blocks.push(ContentBlock::ToolResult {
                                tool_use_id: tc.id.clone(),
                                content: output,
                                is_error: true,
                            });
                            continue;
                        }
                        HookResult::Modify(_modified) => {}
                        HookResult::Allow => {}
                    }
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
                    Err(_elapsed) => {
                        let msg = format!("Tool '{}' timed out after 30 seconds.", tc.name);
                        let mut ctx = self.event_context(&Event::OnToolError);
                        ctx.tool_name = Some(tc.name.clone());
                        ctx.error = Some(msg.clone());
                        self.hooks.emit(&Event::OnToolError, &ctx);
                        (msg, true)
                    }
                    Ok(Err(e)) => {
                        let msg = e.to_string();
                        let mut ctx = self.event_context(&Event::OnToolError);
                        ctx.tool_name = Some(tc.name.clone());
                        ctx.error = Some(msg.clone());
                        self.hooks.emit(&Event::OnToolError, &ctx);
                        (msg, true)
                    }
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
                {
                    let mut ctx = self.event_context(&Event::AfterToolCall);
                    ctx.tool_name = Some(tc.name.clone());
                    ctx.tool_output = Some(output.clone());
                    self.hooks.emit(&Event::AfterToolCall, &ctx);
                }
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

        let title = if let Some(mut rx) = title_rx {
            let mut raw = String::new();
            while let Some(event) = rx.recv().await {
                match event.event_type {
                    StreamEventType::TextDelta(text) => raw.push_str(&text),
                    StreamEventType::Error(e) => {
                        tracing::warn!("title stream error: {e}");
                    }
                    _ => {}
                }
            }
            let t = raw
                .trim()
                .trim_matches('"')
                .trim_matches('`')
                .trim_matches('*')
                .replace('\n', " ");
            let t: String = t.chars().take(50).collect();
            if t.is_empty() {
                tracing::warn!("title stream returned empty text");
                None
            } else {
                Some(t)
            }
        } else {
            None
        };
        let fallback = || -> String {
            self.messages
                .first()
                .and_then(|m| {
                    m.content.iter().find_map(|b| {
                        if let ContentBlock::Text(t) = b {
                            let s: String = t.chars().take(50).collect();
                            let s = s.trim().to_string();
                            if s.is_empty() { None } else { Some(s) }
                        } else {
                            None
                        }
                    })
                })
                .unwrap_or_else(|| "Chat".to_string())
        };
        let title = title.unwrap_or_else(fallback);
        let _ = self
            .db
            .update_conversation_title(&self.conversation_id, &title);
        let _ = event_tx.send(AgentEvent::TitleGenerated(title.clone()));
        {
            let mut ctx = self.event_context(&Event::OnTitleGenerated);
            ctx.title = Some(title);
            self.hooks.emit(&Event::OnTitleGenerated, &ctx);
        }

        Ok(())
    }
}
