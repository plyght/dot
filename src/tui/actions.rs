use std::sync::Arc;

use tokio::sync::{Mutex, mpsc};

use crate::agent::Agent;
use crate::tui::app::{self, App, ChatMessage};
use crate::tui::input::InputAction;
use crate::tui::tools::StreamSegment;
use crate::tui::widgets::{AgentEntry, SessionEntry, time_ago};

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
        InputAction::ForkFromMessage(idx) => {
            let fork_messages: Vec<(String, String, Option<String>)> = app.messages[..=idx]
                .iter()
                .map(|m| (m.role.clone(), m.content.clone(), m.model.clone()))
                .collect();
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
