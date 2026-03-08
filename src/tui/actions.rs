use std::sync::Arc;

use tokio::sync::{Mutex, mpsc};

use crate::agent::Agent;
use crate::tui::app::{self, App, ChatMessage};
use crate::tui::input::InputAction;
use crate::tui::tools::StreamSegment;
use crate::tui::widgets::{AgentEntry, SessionEntry, time_ago};

pub async fn dispatch_acp_action(
    app: &mut App,
    acp: &Arc<Mutex<crate::acp::AcpClient>>,
    action: InputAction,
    agent_rx: &mut Option<mpsc::UnboundedReceiver<crate::agent::AgentEvent>>,
    agent_task: &mut Option<tokio::task::JoinHandle<()>>,
) -> LoopSignal {
    match action {
        InputAction::Quit => return LoopSignal::Quit,
        InputAction::CancelStream => {
            if let Some(handle) = agent_task.take() {
                handle.abort();
            }
            *agent_rx = None;
            let acp_clone = Arc::clone(acp);
            tokio::spawn(async move {
                let mut c = acp_clone.lock().await;
                let _ = c.cancel().await;
            });
            app.is_streaming = false;
            app.streaming_started = None;
            if !app.current_response.is_empty()
                || !app.current_tool_calls.is_empty()
                || !app.streaming_segments.is_empty()
            {
                if !app.current_response.is_empty() {
                    app.streaming_segments
                        .push(StreamSegment::Text(std::mem::take(
                            &mut app.current_response,
                        )));
                }
                let content: String = app
                    .streaming_segments
                    .iter()
                    .filter_map(|s| {
                        if let StreamSegment::Text(t) = s {
                            Some(t.as_str())
                        } else {
                            None
                        }
                    })
                    .collect();
                let thinking = if app.current_thinking.is_empty() {
                    None
                } else {
                    Some(std::mem::take(&mut app.current_thinking))
                };
                app.messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content,
                    tool_calls: std::mem::take(&mut app.current_tool_calls),
                    thinking,
                    model: Some(app.model_name.clone()),
                    segments: Some(std::mem::take(&mut app.streaming_segments)),
                });
            } else {
                app.current_response.clear();
                app.current_thinking.clear();
                app.current_tool_calls.clear();
                app.streaming_segments.clear();
            }
            app.pending_tool_name = None;
            app.pending_question = None;
            app.pending_permission = None;
            app.status_message = Some(app::StatusMessage::info("cancelled"));
            return LoopSignal::CancelStream;
        }
        InputAction::SendMessage(msg) => {
            let (tx, rx) = mpsc::unbounded_channel();
            *agent_rx = Some(rx);
            let acp_clone = Arc::clone(acp);
            *agent_task = Some(tokio::spawn(async move {
                let mut client = acp_clone.lock().await;
                if let Err(e) = client.send_prompt(&msg).await {
                    let _ = tx.send(crate::agent::AgentEvent::Error(format!("{e}")));
                    return;
                }
                drop(client);
                loop {
                    let mut client = acp_clone.lock().await;
                    match client.read_next().await {
                        Ok(acp_msg) => {
                            drop(client);
                            match acp_msg {
                                crate::acp::AcpMessage::Notification(n) => {
                                    use crate::acp::types::SessionUpdate;
                                    match n.update {
                                        SessionUpdate::AgentMessageChunk { content } => {
                                            if let crate::acp::ContentBlock::Text { text } = content {
                                                let _ = tx.send(crate::agent::AgentEvent::TextDelta(text));
                                            }
                                        }
                                        SessionUpdate::ThoughtChunk { content } => {
                                            if let crate::acp::ContentBlock::Text { text } = content {
                                                let _ = tx.send(crate::agent::AgentEvent::ThinkingDelta(text));
                                            }
                                        }
                                                SessionUpdate::ToolCall { tool_call_id, title, status, content, raw_input, .. } => {
                                                            let _ = tx.send(crate::agent::AgentEvent::ToolCallStart {
                                                                id: tool_call_id.clone(),
                                                                name: title.clone(),
                                                            });
                                                            if status == crate::acp::ToolCallStatus::InProgress {
                                                                let input = raw_input
                                                                    .as_ref()
                                                                    .map(|v| serde_json::to_string_pretty(v).unwrap_or_default())
                                                                    .unwrap_or_default();
                                                                let _ = tx.send(crate::agent::AgentEvent::ToolCallExecuting {
                                                                    id: tool_call_id.clone(),
                                                                    name: title.clone(),
                                                                    input,
                                                                });
                                                            }
                                                            if status == crate::acp::ToolCallStatus::Completed
                                                                || status == crate::acp::ToolCallStatus::Failed
                                                            {
                                                let output = content.as_ref().map(|c| {
                                                    c.iter().filter_map(|tc| {
                                                        if let crate::acp::ToolCallContent::Content { content } = tc {
                                                            if let crate::acp::ContentBlock::Text { text } = content {
                                                                return Some(text.clone());
                                                            }
                                                        }
                                                        None
                                                    }).collect::<Vec<_>>().join("\n")
                                                }).unwrap_or_default();
                                                let _ = tx.send(crate::agent::AgentEvent::ToolCallResult {
                                                    id: tool_call_id,
                                                    name: title,
                                                    output,
                                                    is_error: status == crate::acp::ToolCallStatus::Failed,
                                                });
                                            }
                                        }
                                        SessionUpdate::ToolCallUpdate { tool_call_id, title, status, content, .. } => {
                                            if let Some(s) = status {
                                                if s == crate::acp::ToolCallStatus::Completed
                                                    || s == crate::acp::ToolCallStatus::Failed
                                                {
                                                    let output = content.as_ref().map(|c| {
                                                        c.iter().filter_map(|tc| {
                                                            if let crate::acp::ToolCallContent::Content { content } = tc {
                                                                if let crate::acp::ContentBlock::Text { text } = content {
                                                                    return Some(text.clone());
                                                                }
                                                            }
                                                            None
                                                        }).collect::<Vec<_>>().join("\n")
                                                    }).unwrap_or_default();
                                                    let _ = tx.send(crate::agent::AgentEvent::ToolCallResult {
                                                        id: tool_call_id,
                                                        name: title.unwrap_or_default(),
                                                        output,
                                                        is_error: s == crate::acp::ToolCallStatus::Failed,
                                                    });
                                                }
                                            }
                                        }
                                                        SessionUpdate::Plan { entries } => {
                                                            let todos: Vec<crate::agent::TodoItem> = entries.iter().map(|e| {
                                                                crate::agent::TodoItem {
                                                                    content: e.content.clone(),
                                                                    status: match e.status {
                                                                        crate::acp::PlanEntryStatus::Pending => crate::agent::TodoStatus::Pending,
                                                                        crate::acp::PlanEntryStatus::InProgress => crate::agent::TodoStatus::InProgress,
                                                                        crate::acp::PlanEntryStatus::Completed => crate::agent::TodoStatus::Completed,
                                                                    },
                                                                }
                                                            }).collect();
                                                            let _ = tx.send(crate::agent::AgentEvent::TodoUpdate(todos));
                                                        }
                                                        SessionUpdate::CurrentModeUpdate { mode_id } => {
                                                            let mut c = acp_clone.lock().await;
                                                            c.set_current_mode(&mode_id);
                                                        }
                                                        SessionUpdate::ConfigOptionsUpdate { config_options } => {
                                                            let mut c = acp_clone.lock().await;
                                                            c.set_config_options(config_options);
                                                        }
                                                        _ => {}
                                    }
                                }
                                crate::acp::AcpMessage::PromptComplete(_) => {
                                    let _ = tx.send(crate::agent::AgentEvent::TextComplete(String::new()));
                                    let _ = tx.send(crate::agent::AgentEvent::Done {
                                        usage: crate::provider::Usage::default(),
                                    });
                                    break;
                                }
                                crate::acp::AcpMessage::IncomingRequest { id, method, params } => {
                                    let mut client = acp_clone.lock().await;
                                    if handle_acp_extension_method(&tx, &method, &params) {
                                        let _ = client.respond(id, serde_json::json!({})).await;
                                    } else {
                                        handle_acp_incoming_request(&mut client, id, &method, params).await;
                                    }
                                }
                                crate::acp::AcpMessage::Response { .. } => {}
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(crate::agent::AgentEvent::Error(format!("{e}")));
                            break;
                        }
                    }
                }
            }));
        }
        InputAction::OpenExternalEditor => return LoopSignal::OpenEditor,
        InputAction::ScrollUp(n) => app.scroll_up(n),
        InputAction::ScrollDown(n) => app.scroll_down(n),
        InputAction::ScrollToTop => app.scroll_to_top(),
        InputAction::ScrollToBottom => app.scroll_to_bottom(),
        InputAction::ClearConversation => app.clear_conversation(),
        InputAction::ToggleThinking => {
            app.thinking_expanded = !app.thinking_expanded;
            app.mark_dirty();
        }
        InputAction::CopyMessage(idx) => {
            if idx < app.messages.len() {
                app::copy_to_clipboard(&app.messages[idx].content);
                app.status_message = Some(app::StatusMessage::info("copied to clipboard"));
            }
        }
        InputAction::OpenRenamePopup => {
            app.rename_input = app.conversation_title.clone().unwrap_or_default();
            app.rename_visible = true;
        }
        InputAction::OpenAgentSelector => {
            let acp_lock = acp.lock().await;
            let modes = acp_lock.available_modes();
            let current = acp_lock.current_mode().unwrap_or("").to_string();
            let entries: Vec<AgentEntry> = modes
                .iter()
                .map(|m| AgentEntry {
                    name: m.id.clone(),
                    description: m.description.clone().unwrap_or_else(|| m.name.clone()),
                })
                .collect();
            drop(acp_lock);
            if entries.is_empty() {
                app.status_message =
                    Some(app::StatusMessage::info("no modes available"));
            } else {
                app.agent_selector.open(entries, &current);
            }
        }
        InputAction::SelectAgent { name } => {
            let acp_clone = Arc::clone(acp);
            let mode_id = name.clone();
            tokio::spawn(async move {
                let mut c = acp_clone.lock().await;
                let _ = c.set_mode(&mode_id).await;
            });
            app.model_name = name.clone();
            app.mark_dirty();
        }
        InputAction::ToggleAgent => {
            let mut acp_lock = acp.lock().await;
            let modes = acp_lock.available_modes().to_vec();
            let current = acp_lock.current_mode().unwrap_or("").to_string();
            if !modes.is_empty() {
                let idx = modes.iter().position(|m| m.id == current).unwrap_or(0);
                let next = &modes[(idx + 1) % modes.len()];
                let next_id = next.id.clone();
                let _ = acp_lock.set_mode(&next_id).await;
                acp_lock.set_current_mode(&next_id);
                drop(acp_lock);
                app.model_name = next_id;
                app.mark_dirty();
            }
        }
        InputAction::NewConversation
        | InputAction::OpenModelSelector
        | InputAction::OpenSessionSelector
        | InputAction::ResumeSession { .. }
        | InputAction::SelectModel { .. }
        | InputAction::OpenThinkingSelector
        | InputAction::SetThinkingLevel(_)
        | InputAction::CycleThinkingLevel
        | InputAction::TruncateToMessage(_)
        | InputAction::RevertToMessage(_)
        | InputAction::ForkFromMessage(_)
        | InputAction::AnswerQuestion(_)
        | InputAction::LoadSkill { .. }
        | InputAction::RunCustomCommand { .. }
        | InputAction::ExportSession(_)
        | InputAction::RenameSession(_)
        | InputAction::AnswerPermission(_)
        | InputAction::None => {
            app.status_message = Some(app::StatusMessage::info("not available in ACP mode"));
        }
    }
    LoopSignal::Continue
}

fn handle_acp_extension_method(
    tx: &mpsc::UnboundedSender<crate::agent::AgentEvent>,
    method: &str,
    params: &serde_json::Value,
) -> bool {
    match method {
        "cursor/update_todos" => {
            if let Some(items) = params["todos"].as_array() {
                let todos: Vec<crate::agent::TodoItem> = items
                    .iter()
                    .filter_map(|t| {
                        Some(crate::agent::TodoItem {
                            content: t["content"].as_str()?.to_string(),
                            status: match t["status"].as_str().unwrap_or("pending") {
                                "in_progress" => crate::agent::TodoStatus::InProgress,
                                "completed" => crate::agent::TodoStatus::Completed,
                                _ => crate::agent::TodoStatus::Pending,
                            },
                        })
                    })
                    .collect();
                let _ = tx.send(crate::agent::AgentEvent::TodoUpdate(todos));
            }
            true
        }
        "cursor/ask_question" => {
            let question = params["question"].as_str().unwrap_or("").to_string();
            let options: Vec<String> = params["options"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let (resp_tx, _) = tokio::sync::oneshot::channel();
            let _ = tx.send(crate::agent::AgentEvent::Question {
                id: uuid::Uuid::new_v4().to_string(),
                question,
                options,
                responder: crate::agent::QuestionResponder(resp_tx),
            });
            true
        }
        "cursor/create_plan" | "cursor/task" | "cursor/generate_image" => true,
        _ => false,
    }
}

async fn handle_acp_incoming_request(
    client: &mut crate::acp::AcpClient,
    id: u64,
    method: &str,
    params: serde_json::Value,
) {
    match method {
        "fs/read_text_file" => {
            let path = params["path"].as_str().unwrap_or("");
            match std::fs::read_to_string(path) {
                Ok(content) => {
                    let _ = client.respond(id, serde_json::json!({"content": content})).await;
                }
                Err(e) => {
                    let _ = client.respond_error(id, -32603, &e.to_string()).await;
                }
            }
        }
        "fs/write_text_file" => {
            let path = params["path"].as_str().unwrap_or("");
            let content = params["content"].as_str().unwrap_or("");
            match std::fs::write(path, content) {
                Ok(()) => {
                    let _ = client.respond(id, serde_json::json!({})).await;
                }
                Err(e) => {
                    let _ = client.respond_error(id, -32603, &e.to_string()).await;
                }
            }
        }
        "terminal/create" => {
            let command = params["command"].as_str().unwrap_or("sh");
            let args: Vec<String> = params["args"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let cwd = params["cwd"].as_str();
            let mut cmd = tokio::process::Command::new(command);
            cmd.args(&args);
            if let Some(d) = cwd {
                cmd.current_dir(d);
            }
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());
            match cmd.spawn() {
                Ok(_child) => {
                    let tid = uuid::Uuid::new_v4().to_string();
                    let _ = client.respond(id, serde_json::json!({"terminalId": tid})).await;
                }
                Err(e) => {
                    let _ = client.respond_error(id, -32603, &e.to_string()).await;
                }
            }
        }
        "session/request_permission" => {
            let options = params["options"].as_array();
            let allow_id = options
                .and_then(|opts| {
                    opts.iter().find(|o| {
                        o["kind"].as_str() == Some("allow_once")
                            || o["kind"].as_str() == Some("allow-once")
                    })
                })
                .and_then(|o| o["optionId"].as_str())
                .unwrap_or("allow-once");
            let _ = client
                .respond(
                    id,
                    serde_json::json!({
                        "outcome": { "outcome": "selected", "optionId": allow_id }
                    }),
                )
                .await;
        }
        _ => {
            let _ = client
                .respond_error(id, -32601, &format!("unsupported: {}", method))
                .await;
        }
    }
}

pub enum LoopSignal {
    Continue,
    Quit,
    CancelStream,
    OpenEditor,
}

pub async fn dispatch_action(
    app: &mut App,
    agent: &Arc<Mutex<Agent>>,
    action: InputAction,
    agent_rx: &mut Option<mpsc::UnboundedReceiver<crate::agent::AgentEvent>>,
    agent_task: &mut Option<tokio::task::JoinHandle<()>>,
) -> LoopSignal {
    match action {
        InputAction::Quit => return LoopSignal::Quit,
        InputAction::CancelStream => {
            if let Some(handle) = agent_task.take() {
                handle.abort();
            }
            *agent_rx = None;
            app.is_streaming = false;
            app.streaming_started = None;
            if !app.current_response.is_empty()
                || !app.current_tool_calls.is_empty()
                || !app.streaming_segments.is_empty()
            {
                if !app.current_response.is_empty() {
                    app.streaming_segments
                        .push(StreamSegment::Text(std::mem::take(
                            &mut app.current_response,
                        )));
                }
                let content: String = app
                    .streaming_segments
                    .iter()
                    .filter_map(|s| {
                        if let StreamSegment::Text(t) = s {
                            Some(t.as_str())
                        } else {
                            None
                        }
                    })
                    .collect();
                let thinking = if app.current_thinking.is_empty() {
                    None
                } else {
                    Some(std::mem::take(&mut app.current_thinking))
                };
                app.messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content,
                    tool_calls: std::mem::take(&mut app.current_tool_calls),
                    thinking,
                    model: Some(app.model_name.clone()),
                    segments: Some(std::mem::take(&mut app.streaming_segments)),
                });
            } else {
                app.current_response.clear();
                app.current_thinking.clear();
                app.current_tool_calls.clear();
                app.streaming_segments.clear();
            }
            app.pending_tool_name = None;
            app.pending_question = None;
            app.pending_permission = None;
            app.status_message = Some(app::StatusMessage::info("cancelled"));
            return LoopSignal::CancelStream;
        }
        InputAction::SendMessage(msg) => {
            let images: Vec<(String, String)> = app
                .take_attachments()
                .into_iter()
                .map(|a| (a.media_type, a.data))
                .collect();

            let (tx, rx) = mpsc::unbounded_channel();
            *agent_rx = Some(rx);

            let agent_clone = Arc::clone(agent);
            let err_tx = tx.clone();
            *agent_task = Some(tokio::spawn(async move {
                let mut agent = agent_clone.lock().await;
                let result = if images.is_empty() {
                    agent.send_message(&msg, tx).await
                } else {
                    agent.send_message_with_images(&msg, images, tx).await
                };
                if let Err(e) = result {
                    tracing::error!("Agent send_message error: {}", e);
                    let _ = err_tx.send(crate::agent::AgentEvent::Error(format!("{e}")));
                }
            }));
        }
        InputAction::NewConversation => {
            let mut agent_lock = agent.lock().await;
            match agent_lock.new_conversation() {
                Ok(()) => app.clear_conversation(),
                Err(e) => {
                    app.status_message = Some(app::StatusMessage::error(format!(
                        "failed to start new conversation: {e}"
                    )))
                }
            }
        }
        InputAction::OpenModelSelector => {
            let agent_lock = agent.lock().await;
            let grouped = agent_lock.fetch_all_models().await;
            let current_provider = agent_lock.current_provider_name().to_string();
            let current_model = agent_lock.current_model().to_string();
            drop(agent_lock);
            app.model_selector.favorites = app.favorite_models.clone();
            app.model_selector
                .open(grouped, &current_provider, &current_model);
        }
        InputAction::OpenAgentSelector => {
            let agent_lock = agent.lock().await;
            let entries: Vec<AgentEntry> = agent_lock
                .agent_profiles()
                .iter()
                .map(|p| AgentEntry {
                    name: p.name.clone(),
                    description: p.description.clone(),
                })
                .collect();
            let current = agent_lock.current_agent_name().to_string();
            drop(agent_lock);
            app.agent_selector.open(entries, &current);
        }
        InputAction::OpenSessionSelector => {
            let agent_lock = agent.lock().await;
            let current_id = agent_lock.conversation_id().to_string();
            let sessions = agent_lock.list_sessions().unwrap_or_default();
            drop(agent_lock);
            let entries: Vec<SessionEntry> = sessions
                .into_iter()
                .map(|s| {
                    let title = if let Some(t) = &s.title {
                        t.clone()
                    } else if s.id == current_id {
                        app.conversation_title
                            .clone()
                            .unwrap_or_else(|| "new conversation".to_string())
                    } else {
                        "untitled".to_string()
                    };
                    SessionEntry {
                        id: s.id.clone(),
                        title,
                        subtitle: format!("{} · {}", time_ago(&s.updated_at), s.provider),
                    }
                })
                .collect();
            app.session_selector.open(entries);
        }
        InputAction::ResumeSession { id } => {
            let mut agent_lock = agent.lock().await;
            match agent_lock.get_session(&id) {
                Ok(conv) => {
                    let title = conv.title.clone();
                    let conv_model = conv.model.clone();
                    let messages_for_ui: Vec<(String, String)> = conv
                        .messages
                        .iter()
                        .map(|m| (m.role.clone(), m.content.clone()))
                        .collect();
                    match agent_lock.resume_conversation(&conv) {
                        Ok(()) => {
                            drop(agent_lock);
                            app.clear_conversation();
                            app.conversation_title = title;
                            for (role, content) in messages_for_ui {
                                let model = if role == "assistant" {
                                    Some(conv_model.clone())
                                } else {
                                    None
                                };
                                app.messages.push(ChatMessage {
                                    role,
                                    content,
                                    tool_calls: Vec::new(),
                                    thinking: None,
                                    model,
                                    segments: None,
                                });
                            }
                            app.scroll_to_bottom();
                        }
                        Err(e) => {
                            drop(agent_lock);
                            app.status_message = Some(app::StatusMessage::error(format!(
                                "failed to resume session: {e}"
                            )));
                        }
                    }
                }
                Err(e) => {
                    drop(agent_lock);
                    app.status_message =
                        Some(app::StatusMessage::error(format!("session not found: {e}")));
                }
            }
        }
        InputAction::SelectModel { provider, model } => {
            let mut agent_lock = agent.lock().await;
            agent_lock.set_active_provider(&provider, &model);
            let cw = agent_lock.context_window();
            if cw > 0 {
                app.context_window = cw;
            } else {
                app.context_window = agent_lock.fetch_context_window().await;
            }
        }
        InputAction::SelectAgent { name } => {
            let mut agent_lock = agent.lock().await;
            agent_lock.switch_agent(&name);
            app.model_name = agent_lock.current_model().to_string();
            app.provider_name = agent_lock.current_provider_name().to_string();
            let cw = agent_lock.context_window();
            if cw > 0 {
                app.context_window = cw;
            } else {
                app.context_window = agent_lock.fetch_context_window().await;
            }
        }
        InputAction::ScrollUp(n) => app.scroll_up(n),
        InputAction::ScrollDown(n) => app.scroll_down(n),
        InputAction::ScrollToTop => app.scroll_to_top(),
        InputAction::ScrollToBottom => app.scroll_to_bottom(),
        InputAction::ClearConversation => app.clear_conversation(),
        InputAction::ToggleThinking => {
            app.thinking_expanded = !app.thinking_expanded;
            app.mark_dirty();
        }
        InputAction::OpenThinkingSelector => {
            let level = app.thinking_level();
            app.thinking_selector.open(level);
        }
        InputAction::SetThinkingLevel(budget) => {
            let mut agent_lock = agent.lock().await;
            agent_lock.set_thinking_budget(budget);
        }
        InputAction::CycleThinkingLevel => {
            let next = app.thinking_level().next();
            let budget = next.budget_tokens();
            app.thinking_budget = budget;
            let mut agent_lock = agent.lock().await;
            agent_lock.set_thinking_budget(budget);
        }
        InputAction::TruncateToMessage(idx) => {
            app.messages.truncate(idx + 1);
            app.current_response.clear();
            app.current_thinking.clear();
            app.current_tool_calls.clear();
            app.streaming_segments.clear();
            app.scroll_to_bottom();
            let mut agent_lock = agent.lock().await;
            agent_lock.truncate_messages(idx + 1);
        }
        InputAction::RevertToMessage(idx) => {
            let prompt = if idx < app.messages.len() && app.messages[idx].role == "user" {
                app.messages[idx].content.clone()
            } else if idx > 0 && app.messages[idx - 1].role == "user" {
                app.messages[idx - 1].content.clone()
            } else {
                String::new()
            };
            app.current_response.clear();
            app.current_thinking.clear();
            app.current_tool_calls.clear();
            app.streaming_segments.clear();
            let mut agent_lock = agent.lock().await;
            match agent_lock.revert_to_message(idx) {
                Ok(restored) => {
                    drop(agent_lock);
                    app.messages.truncate(idx);
                    app.input = prompt;
                    app.cursor_pos = app.input.len();
                    app.chips.clear();
                    app.mark_dirty();
                    app.scroll_to_bottom();
                    let count = restored.len();
                    if count > 0 {
                        app.status_message = Some(app::StatusMessage::info(format!(
                            "reverted {count} file{}",
                            if count == 1 { "" } else { "s" }
                        )));
                    }
                }
                Err(e) => {
                    drop(agent_lock);
                    app.status_message =
                        Some(app::StatusMessage::error(format!("revert failed: {e}")));
                }
            }
        }
        InputAction::CopyMessage(idx) => {
            if idx < app.messages.len() {
                app::copy_to_clipboard(&app.messages[idx].content);
                app.status_message = Some(app::StatusMessage::info("copied to clipboard"));
            }
        }
        InputAction::ForkFromMessage(idx) => {
            let fork_messages: Vec<(String, String, Option<String>)> = app.messages[..=idx]
                .iter()
                .map(|m| (m.role.clone(), m.content.clone(), m.model.clone()))
                .collect();
            let prompt = fork_messages
                .iter()
                .rev()
                .find(|(role, _, _)| role == "user")
                .map(|(_, content, _)| content.clone())
                .unwrap_or_default();
            let mut agent_lock = agent.lock().await;
            match agent_lock.fork_conversation(idx + 1) {
                Ok(()) => {
                    drop(agent_lock);
                    app.clear_conversation();
                    for (role, content, model) in fork_messages {
                        app.messages.push(ChatMessage {
                            role,
                            content,
                            tool_calls: Vec::new(),
                            thinking: None,
                            model,
                            segments: None,
                        });
                    }
                    app.input = prompt;
                    app.cursor_pos = app.input.len();
                    app.chips.clear();
                    app.scroll_to_bottom();
                }
                Err(e) => {
                    drop(agent_lock);
                    app.status_message =
                        Some(app::StatusMessage::error(format!("fork failed: {e}")));
                }
            }
        }
        InputAction::AnswerQuestion(answer) => {
            app.messages.push(ChatMessage {
                role: "user".to_string(),
                content: answer,
                tool_calls: Vec::new(),
                thinking: None,
                model: None,
                segments: None,
            });
            app.scroll_to_bottom();
        }
        InputAction::LoadSkill { name } => {
            let display = format!("/{}", name);
            app.messages.push(ChatMessage {
                role: "user".to_string(),
                content: display,
                tool_calls: Vec::new(),
                thinking: None,
                model: None,
                segments: None,
            });
            app.scroll_to_bottom();
            let msg = format!("Load and use the {} skill", name);
            let (tx, rx) = mpsc::unbounded_channel();
            *agent_rx = Some(rx);
            let agent_clone = Arc::clone(agent);
            *agent_task = Some(tokio::spawn(async move {
                let mut agent = agent_clone.lock().await;
                if let Err(e) = agent.send_message(&msg, tx).await {
                    tracing::error!("Agent send_message error: {}", e);
                }
            }));
        }
        InputAction::RunCustomCommand { name, args } => {
            let display = format!("/{} {}", name, args).trim_end().to_string();
            app.messages.push(ChatMessage {
                role: "user".to_string(),
                content: display,
                tool_calls: Vec::new(),
                thinking: None,
                model: None,
                segments: None,
            });
            let agent_lock = agent.lock().await;
            match agent_lock.execute_command(&name, &args) {
                Ok(output) => {
                    app.messages.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: output,
                        tool_calls: Vec::new(),
                        thinking: None,
                        model: None,
                        segments: None,
                    });
                }
                Err(e) => {
                    app.status_message =
                        Some(app::StatusMessage::error(format!("command error: {e}")));
                }
            }
            drop(agent_lock);
            app.scroll_to_bottom();
        }
        InputAction::ToggleAgent => {
            let mut agent_lock = agent.lock().await;
            let current = agent_lock.current_agent_name().to_string();
            let names: Vec<String> = agent_lock
                .agent_profiles()
                .iter()
                .map(|p| p.name.clone())
                .collect();
            let idx = names.iter().position(|n| n == &current).unwrap_or(0);
            let next = names[(idx + 1) % names.len()].clone();
            agent_lock.switch_agent(&next);
            app.agent_name = agent_lock.current_agent_name().to_string();
            app.model_name = agent_lock.current_model().to_string();
            app.provider_name = agent_lock.current_provider_name().to_string();
        }
        InputAction::ExportSession(path_opt) => {
            let agent_lock = agent.lock().await;
            let cwd = agent_lock.cwd().to_string();
            drop(agent_lock);
            let title = app
                .conversation_title
                .as_deref()
                .unwrap_or("session")
                .to_string();
            let path = match path_opt {
                Some(p) => p,
                None => {
                    let slug: String = title
                        .chars()
                        .map(|c| {
                            if c.is_alphanumeric() {
                                c.to_ascii_lowercase()
                            } else {
                                '-'
                            }
                        })
                        .collect();
                    format!("{}/session-{}.md", cwd, slug)
                }
            };
            let mut md = format!("# Session: {}\n\n", title);
            for msg in &app.messages {
                match msg.role.as_str() {
                    "user" => {
                        md.push_str("---\n\n## User\n\n");
                        md.push_str(&msg.content);
                        md.push_str("\n\n");
                    }
                    "assistant" => {
                        md.push_str("---\n\n## Assistant\n\n");
                        md.push_str(&msg.content);
                        md.push_str("\n\n");
                        for tc in &msg.tool_calls {
                            let status = if tc.is_error { "error" } else { "done" };
                            md.push_str(&format!("- `{}` ({})\n", tc.name, status));
                        }
                    }
                    _ => {}
                }
            }
            match std::fs::write(&path, &md) {
                Ok(()) => {
                    app.status_message =
                        Some(app::StatusMessage::success(format!("exported to {}", path)))
                }
                Err(e) => {
                    app.status_message =
                        Some(app::StatusMessage::error(format!("export failed: {e}")))
                }
            }
        }
        InputAction::OpenExternalEditor => return LoopSignal::OpenEditor,
        InputAction::AnswerPermission(_) | InputAction::None => {}
        InputAction::OpenRenamePopup => {
            app.rename_input = app.conversation_title.clone().unwrap_or_default();
            app.rename_visible = true;
        }
        InputAction::RenameSession(title) => {
            let agent_lock = agent.lock().await;
            if let Err(e) = agent_lock.rename_session(&title) {
                app.status_message = Some(app::StatusMessage::error(format!("rename failed: {e}")));
            } else {
                app.conversation_title = Some(title);
            }
            app.rename_visible = false;
        }
    }
    LoopSignal::Continue
}
