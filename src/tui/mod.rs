pub mod app;
pub mod event;
pub mod input;
pub mod markdown;
pub mod theme;
pub mod tools;
pub mod ui;
pub mod ui_popups;
pub mod ui_tools;
pub mod widgets;

use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use crossterm::{execute, terminal};
use tokio::sync::{Mutex, mpsc};

use crate::agent::{Agent, AgentProfile};
use crate::command::CommandRegistry;
use crate::config::Config;
use crate::db::Db;
use crate::extension::HookRegistry;
use crate::provider::Provider;
use crate::tools::ToolRegistry;

use app::{App, ChatMessage};
use event::{AppEvent, EventHandler};
use input::InputAction;
use widgets::{AgentEntry, SessionEntry, time_ago};

pub struct ExitInfo {
    pub conversation_id: String,
    pub title: Option<String>,
}

pub async fn run(
    config: Config,
    providers: Vec<Box<dyn Provider>>,
    db: Db,
    tools: ToolRegistry,
    profiles: Vec<AgentProfile>,
    cwd: String,
    resume_id: Option<String>,
    skill_names: Vec<(String, String)>,
    hooks: HookRegistry,
    commands: CommandRegistry,
) -> Result<()> {
    terminal::enable_raw_mode()?;
    let mut stdout = std::io::stderr();
    execute!(
        stdout,
        terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture,
        crossterm::event::EnableBracketedPaste
    )?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let result = run_app(
        &mut terminal,
        config,
        providers,
        db,
        tools,
        profiles,
        cwd,
        resume_id,
        skill_names,
        hooks,
        commands,
    )
    .await;

    terminal::disable_raw_mode()?;
    execute!(
        std::io::stderr(),
        terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture,
        crossterm::event::DisableBracketedPaste
    )?;
    terminal.show_cursor()?;

    if let Ok(ref info) = result {
        print_exit_screen(info);
    }

    result.map(|_| ())
}

fn print_exit_screen(info: &ExitInfo) {
    let title = info.title.as_deref().unwrap_or("untitled session");
    let id = &info.conversation_id;
    println!();
    println!("  \x1b[2mSession\x1b[0m   {}", title);
    println!("  \x1b[2mResume\x1b[0m    dot -s {}", id);
    println!();
}

#[allow(clippy::too_many_arguments)]
async fn run_app(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stderr>>,
    config: Config,
    providers: Vec<Box<dyn Provider>>,
    db: Db,
    tools: ToolRegistry,
    profiles: Vec<AgentProfile>,
    cwd: String,
    resume_id: Option<String>,
    skill_names: Vec<(String, String)>,
    hooks: HookRegistry,
    commands: CommandRegistry,
) -> Result<ExitInfo> {
    let model_name = providers[0].model().to_string();
    let provider_name = providers[0].name().to_string();
    let agent_name = profiles
        .first()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "dot".to_string());

    let history = db.get_user_message_history(500).unwrap_or_default();

    let agents_context = crate::context::AgentsContext::load(&cwd, &config.context);
    let agent = Arc::new(Mutex::new(Agent::new(
        providers,
        db,
        &config,
        tools,
        profiles,
        cwd,
        agents_context,
        hooks,
        commands,
    )?));

    let context_window = {
        let agent_lock = agent.lock().await;
        let cw = agent_lock.fetch_context_window().await;
        if cw == 0 {
            tracing::warn!("Failed to fetch context window from API");
        }
        cw
    };

    if let Some(ref id) = resume_id {
        let mut agent_lock = agent.lock().await;
        match agent_lock.get_session(id) {
            Ok(conv) => {
                let _ = agent_lock.resume_conversation(&conv);
            }
            Err(e) => {
                tracing::warn!("Failed to resume session {}: {}", id, e);
            }
        }
    }

    let mut app = App::new(
        model_name,
        provider_name,
        agent_name,
        &config.theme.name,
        config.tui.vim_mode,
        context_window,
    );
    app.history = history;
    app.favorite_models = config.tui.favorite_models.clone();
    app.skill_entries = skill_names;
    {
        let agent_lock = agent.lock().await;
        let cmds = agent_lock.list_commands();
        app.custom_command_names = cmds.iter().map(|(n, _)| n.to_string()).collect();
        app.command_palette.set_skills(&app.skill_entries);
        app.command_palette.add_custom_commands(&cmds);
    }

    if let Some(ref id) = resume_id {
        let agent_lock = agent.lock().await;
        if let Ok(conv) = agent_lock.get_session(id) {
            app.conversation_title = conv.title.clone();
            for m in &conv.messages {
                let model = if m.role == "assistant" {
                    Some(conv.model.clone())
                } else {
                    None
                };
                app.messages.push(ChatMessage {
                    role: m.role.clone(),
                    content: m.content.clone(),
                    tool_calls: Vec::new(),
                    thinking: None,
                    model,
                });
            }
            app.scroll_to_bottom();
        }
        drop(agent_lock);
    }

    let mut events = EventHandler::new();
    let mut agent_rx: Option<mpsc::UnboundedReceiver<crate::agent::AgentEvent>> = None;

    loop {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        let event = if let Some(ref mut rx) = agent_rx {
            tokio::select! {
                biased;
                agent_event = rx.recv() => {
                    match agent_event {
                        Some(ev) => {
                            app.handle_agent_event(ev);
                        }
                        None => {
                            if app.is_streaming {
                                app.is_streaming = false;
                            }
                            agent_rx = None;
                            if let Some(queued) = app.message_queue.pop_front() {
                                let (tx, rx) = mpsc::unbounded_channel();
                                agent_rx = Some(rx);
                                app.is_streaming = true;
                                app.streaming_started = Some(Instant::now());
                                app.current_response.clear();
                                app.current_thinking.clear();
                                app.current_tool_calls.clear();
                                app.error_message = None;
                                let agent_clone = Arc::clone(&agent);
                                tokio::spawn(async move {
                                    let mut agent = agent_clone.lock().await;
                                    let result = if queued.images.is_empty() {
                                        agent.send_message(&queued.text, tx).await
                                    } else {
                                        agent.send_message_with_images(&queued.text, queued.images, tx).await
                                    };
                                    if let Err(e) = result {
                                        tracing::error!("Agent send_message error: {}", e);
                                    }
                                });
                            }
                        }
                    }
                    continue;
                }
                ui_event = events.next() => {
                    match ui_event {
                        Some(ev) => ev,
                        None => break,
                    }
                }
            }
        } else {
            match events.next().await {
                Some(ev) => ev,
                None => break,
            }
        };

        match handle_event(&mut app, &agent, event, &mut agent_rx).await {
            LoopSignal::Quit => break,
            LoopSignal::OpenEditor => {
                let editor = std::env::var("VISUAL")
                    .or_else(|_| std::env::var("EDITOR"))
                    .unwrap_or_else(|_| "vi".to_string());
                let tmp = std::env::temp_dir().join("dot_input.md");
                let _ = std::fs::write(&tmp, &app.input);
                terminal::disable_raw_mode()?;
                execute!(
                    std::io::stderr(),
                    terminal::LeaveAlternateScreen,
                    crossterm::event::DisableMouseCapture
                )?;
                let status = std::process::Command::new(&editor).arg(&tmp).status();
                execute!(
                    std::io::stderr(),
                    terminal::EnterAlternateScreen,
                    crossterm::event::EnableMouseCapture
                )?;
                terminal::enable_raw_mode()?;
                terminal.clear()?;
                if status.is_ok() {
                    if let Ok(contents) = std::fs::read_to_string(&tmp) {
                        let trimmed = contents.trim_end().to_string();
                        if !trimmed.is_empty() {
                            app.cursor_pos = trimmed.len();
                            app.input = trimmed;
                        }
                    }
                }
                let _ = std::fs::remove_file(&tmp);
            }
            _ => {}
        }
    }

    let mut agent_lock = agent.lock().await;
    {
        let event = crate::extension::Event::BeforeExit;
        let ctx = crate::extension::EventContext {
            event: event.as_str().to_string(),
            cwd: agent_lock.cwd().to_string(),
            session_id: agent_lock.conversation_id().to_string(),
            ..Default::default()
        };
        agent_lock.hooks().emit(&event, &ctx);
    }
    let conversation_id = agent_lock.conversation_id().to_string();
    let title = agent_lock.conversation_title();
    agent_lock.cleanup_if_empty();
    drop(agent_lock);

    Ok(ExitInfo {
        conversation_id,
        title,
    })
}

enum LoopSignal {
    Continue,
    Quit,
    CancelStream,
    OpenEditor,
}

async fn dispatch_action(
    app: &mut App,
    agent: &Arc<Mutex<Agent>>,
    action: InputAction,
    agent_rx: &mut Option<mpsc::UnboundedReceiver<crate::agent::AgentEvent>>,
) -> LoopSignal {
    match action {
        InputAction::Quit => return LoopSignal::Quit,
        InputAction::CancelStream => {
            *agent_rx = None;
            app.is_streaming = false;
            app.streaming_started = None;
            if !app.current_response.is_empty() || !app.current_tool_calls.is_empty() {
                let thinking = if app.current_thinking.is_empty() {
                    None
                } else {
                    Some(std::mem::take(&mut app.current_thinking))
                };
                app.messages.push(app::ChatMessage {
                    role: "assistant".to_string(),
                    content: std::mem::take(&mut app.current_response),
                    tool_calls: std::mem::take(&mut app.current_tool_calls),
                    thinking,
                    model: Some(app.model_name.clone()),
                });
            } else {
                app.current_response.clear();
                app.current_thinking.clear();
                app.current_tool_calls.clear();
            }
            app.pending_tool_name = None;
            // Drop pending question/permission to unblock agent
            app.pending_question = None;
            app.pending_permission = None;
            app.error_message = Some("cancelled".to_string());
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
            tokio::spawn(async move {
                let mut agent = agent_clone.lock().await;
                let result = if images.is_empty() {
                    agent.send_message(&msg, tx).await
                } else {
                    agent.send_message_with_images(&msg, images, tx).await
                };
                if let Err(e) = result {
                    tracing::error!("Agent send_message error: {}", e);
                }
            });
        }
        InputAction::NewConversation => {
            let mut agent_lock = agent.lock().await;
            match agent_lock.new_conversation() {
                Ok(()) => app.clear_conversation(),
                Err(e) => {
                    app.error_message = Some(format!("failed to start new conversation: {e}"))
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
                                });
                            }
                            app.scroll_to_bottom();
                        }
                        Err(e) => {
                            drop(agent_lock);
                            app.error_message = Some(format!("failed to resume session: {e}"));
                        }
                    }
                }
                Err(e) => {
                    drop(agent_lock);
                    app.error_message = Some(format!("session not found: {e}"));
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
                        app.messages.push(app::ChatMessage {
                            role,
                            content,
                            tool_calls: Vec::new(),
                            thinking: None,
                            model,
                        });
                    }
                    app.scroll_to_bottom();
                }
                Err(e) => {
                    drop(agent_lock);
                    app.error_message = Some(format!("fork failed: {e}"));
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
            });
            app.scroll_to_bottom();
            let msg = format!("Load and use the {} skill", name);
            let (tx, rx) = mpsc::unbounded_channel();
            *agent_rx = Some(rx);
            let agent_clone = Arc::clone(agent);
            tokio::spawn(async move {
                let mut agent = agent_clone.lock().await;
                if let Err(e) = agent.send_message(&msg, tx).await {
                    tracing::error!("Agent send_message error: {}", e);
                }
            });
        }
        InputAction::RunCustomCommand { name, args } => {
            let display = format!("/{} {}", name, args).trim_end().to_string();
            app.messages.push(ChatMessage {
                role: "user".to_string(),
                content: display,
                tool_calls: Vec::new(),
                thinking: None,
                model: None,
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
                    });
                }
                Err(e) => {
                    app.error_message = Some(format!("command error: {}", e));
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
                Ok(()) => app.error_message = Some(format!("exported to {}", path)),
                Err(e) => app.error_message = Some(format!("export failed: {}", e)),
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
                app.error_message = Some(format!("rename failed: {e}"));
            } else {
                app.conversation_title = Some(title);
            }
            app.rename_visible = false;
        }
    }
    LoopSignal::Continue
}

async fn handle_event(
    app: &mut App,
    agent: &Arc<Mutex<Agent>>,
    event: AppEvent,
    agent_rx: &mut Option<mpsc::UnboundedReceiver<crate::agent::AgentEvent>>,
) -> LoopSignal {
    let action = match event {
        AppEvent::Key(key) => input::handle_key(app, key),
        AppEvent::Mouse(mouse) => input::handle_mouse(app, mouse),
        AppEvent::Paste(text) => input::handle_paste(app, text),
        AppEvent::Tick => {
            app.tick_count = app.tick_count.wrapping_add(1);
            app.animate_scroll();
            return LoopSignal::Continue;
        }
        AppEvent::Agent(ev) => {
            app.handle_agent_event(ev);
            return LoopSignal::Continue;
        }
        AppEvent::Resize(_, _) => return LoopSignal::Continue,
    };
    dispatch_action(app, agent, action, agent_rx).await
}
