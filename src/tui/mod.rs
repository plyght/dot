pub mod actions;
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
use crossterm::cursor::SetCursorStyle;
use crossterm::{execute, terminal};
use tokio::sync::{Mutex, mpsc};

use crate::agent::{Agent, AgentProfile};
use crate::command::CommandRegistry;
use crate::config::{Config, CursorShape};
use crate::db::Db;
use crate::extension::HookRegistry;
use crate::memory::MemoryStore;
use crate::provider::Provider;
use crate::tools::ToolRegistry;

use app::{App, ChatMessage};
use event::{AppEvent, EventHandler};

pub struct ExitInfo {
    pub conversation_id: String,
    pub title: Option<String>,
}

fn cursor_style(shape: &CursorShape, blink: bool) -> SetCursorStyle {
    match (shape, blink) {
        (CursorShape::Block, true) => SetCursorStyle::BlinkingBlock,
        (CursorShape::Block, false) => SetCursorStyle::SteadyBlock,
        (CursorShape::Underline, true) => SetCursorStyle::BlinkingUnderScore,
        (CursorShape::Underline, false) => SetCursorStyle::SteadyUnderScore,
        (CursorShape::Line, true) => SetCursorStyle::BlinkingBar,
        (CursorShape::Line, false) => SetCursorStyle::SteadyBar,
    }
}

fn apply_cursor_style(app: &App) -> Result<()> {
    let (shape, blink) = if app.vim_mode && app.mode == app::AppMode::Normal {
        let s = app
            .cursor_shape_normal
            .as_ref()
            .unwrap_or(&app.cursor_shape);
        let b = app.cursor_blink_normal.unwrap_or(app.cursor_blink);
        (s, b)
    } else {
        (&app.cursor_shape, app.cursor_blink)
    };
    execute!(std::io::stderr(), cursor_style(shape, blink))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn run(
    config: Config,
    providers: Vec<Box<dyn Provider>>,
    db: Db,
    memory: Option<Arc<MemoryStore>>,
    tools: ToolRegistry,
    profiles: Vec<AgentProfile>,
    cwd: String,
    resume_id: Option<String>,
    skill_names: Vec<(String, String)>,
    hooks: HookRegistry,
    commands: CommandRegistry,
    first_run: bool,
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
        memory,
        tools,
        profiles,
        cwd,
        resume_id,
        skill_names,
        hooks,
        commands,
        first_run,
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
    execute!(std::io::stderr(), SetCursorStyle::DefaultUserShape)?;

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
    memory: Option<Arc<MemoryStore>>,
    tools: ToolRegistry,
    profiles: Vec<AgentProfile>,
    cwd: String,
    resume_id: Option<String>,
    skill_names: Vec<(String, String)>,
    hooks: HookRegistry,
    commands: CommandRegistry,
    first_run: bool,
) -> Result<ExitInfo> {
    let model_name = providers[0].model().to_string();
    let provider_name = providers[0].name().to_string();
    let agent_name = profiles
        .first()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "dot".to_string());

    let history = db.get_user_message_history(500).unwrap_or_default();

    let agents_context = crate::context::AgentsContext::load(&cwd, &config.context);
    let (bg_tx, mut bg_rx) = mpsc::unbounded_channel();
    let mut agent_inner = Agent::new(
        providers,
        db,
        &config,
        memory,
        tools,
        profiles,
        cwd,
        agents_context,
        hooks,
        commands,
    )?;
    agent_inner.set_background_tx(bg_tx);
    let agent = Arc::new(Mutex::new(agent_inner));

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
        config.tui.cursor_shape.clone(),
        config.tui.cursor_blink,
        config.tui.cursor_shape_normal.clone(),
        config.tui.cursor_blink_normal,
    );
    app.history = history;
    app.favorite_models = config.tui.favorite_models.clone();
    app.skill_entries = skill_names;

    if first_run || resume_id.is_none() && {
        let creds = crate::auth::Credentials::load().unwrap_or_default();
        let has_creds = !creds.providers.is_empty();
        let has_env = std::env::var("ANTHROPIC_API_KEY")
            .ok()
            .filter(|k| !k.is_empty())
            .is_some()
            || std::env::var("OPENAI_API_KEY")
                .ok()
                .filter(|k| !k.is_empty())
                .is_some();
        !has_creds && !has_env
    } {
        app.welcome_screen.open();
    }
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
                    segments: None,
                    chips: None,
                });
            }
            if !conv.messages.is_empty() {
                let cw = agent_lock.context_window();
                app.context_window = if cw > 0 {
                    cw
                } else {
                    agent_lock.fetch_context_window().await
                };
                app.last_input_tokens = conv.last_input_tokens;
            }
            app.scroll_to_bottom();
        }
        drop(agent_lock);
    }

    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let agent_clone = Arc::clone(&agent);
        tokio::spawn(async move {
            let mut lock = agent_clone.lock().await;
            let result = lock.fetch_all_models().await;
            let provider = lock.current_provider_name().to_string();
            let model = lock.current_model().to_string();
            let _ = tx.send((result, provider, model));
        });
        app.model_fetch_rx = Some(rx);
    }

    let mut events = EventHandler::new();
    let mut agent_rx: Option<mpsc::UnboundedReceiver<crate::agent::AgentEvent>> = None;
    let mut agent_task: Option<tokio::task::JoinHandle<()>> = None;

    loop {
        terminal.draw(|f| ui::draw(f, &mut app))?;
        apply_cursor_style(&app)?;

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
                            if app.context_window == 0 {
                                let agent_lock = agent.lock().await;
                                let cw = agent_lock.context_window();
                                app.context_window = if cw > 0 {
                                    cw
                                } else {
                                    agent_lock.fetch_context_window().await
                                };
                            }
                            if let Some(queued) = app.message_queue.pop_front() {
                                let (tx, rx) = mpsc::unbounded_channel();
                                agent_rx = Some(rx);
                                app.is_streaming = true;
                                app.streaming_started = Some(Instant::now());
                                app.current_response.clear();
                                app.current_thinking.clear();
                                app.current_tool_calls.clear();
                                app.streaming_segments.clear();
                                app.status_message = None;
                                let agent_clone = Arc::clone(&agent);
                                agent_task = Some(tokio::spawn(async move {
                                    let mut agent = agent_clone.lock().await;
                                    let result = if queued.images.is_empty() {
                                        agent.send_message(&queued.text, tx).await
                                    } else {
                                        agent.send_message_with_images(&queued.text, queued.images, tx).await
                                    };
                                    if let Err(e) = result {
                                        tracing::error!("Agent send_message error: {}", e);
                                    }
                                }));
                            }
                        }
                    }
                    continue;
                }
                bg_event = bg_rx.recv() => {
                    if let Some(ev) = bg_event {
                        app.handle_agent_event(ev);
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
            tokio::select! {
                biased;
                bg_event = bg_rx.recv() => {
                    if let Some(ev) = bg_event {
                        app.handle_agent_event(ev);
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
        };

        match handle_event(&mut app, &agent, event, &mut agent_rx, &mut agent_task).await {
            actions::LoopSignal::Quit => break,
            actions::LoopSignal::OpenEditor => {
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
                if status.is_ok()
                    && let Ok(contents) = std::fs::read_to_string(&tmp)
                {
                    let trimmed = contents.trim_end().to_string();
                    if !trimmed.is_empty() {
                        app.cursor_pos = trimmed.len();
                        app.input = trimmed;
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

async fn handle_event(
    app: &mut App,
    agent: &Arc<Mutex<Agent>>,
    event: AppEvent,
    agent_rx: &mut Option<mpsc::UnboundedReceiver<crate::agent::AgentEvent>>,
    agent_task: &mut Option<tokio::task::JoinHandle<()>>,
) -> actions::LoopSignal {
    let action = match event {
        AppEvent::Key(key) => input::handle_key(app, key),
        AppEvent::Mouse(mouse) => input::handle_mouse(app, mouse),
        AppEvent::Paste(text) => input::handle_paste(app, text),
        AppEvent::Tick => {
            app.tick_count = app.tick_count.wrapping_add(1);
            if app.status_message.as_ref().is_some_and(|s| s.expired()) {
                app.status_message = None;
                app.mark_dirty();
            }
            if let Some(mut rx) = app.model_fetch_rx.take() {
                match rx.try_recv() {
                    Ok((grouped, provider, model)) => {
                        app.cached_model_groups = Some(grouped.clone());
                        if app.model_selector.visible {
                            app.model_selector.favorites = app.favorite_models.clone();
                            app.model_selector.open(grouped, &provider, &model);
                        }
                        app.mark_dirty();
                    }
                    Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                        app.model_fetch_rx = Some(rx);
                    }
                    Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {}
                }
            }
            return actions::LoopSignal::Continue;
        }
        AppEvent::Agent(ev) => {
            app.handle_agent_event(ev);
            return actions::LoopSignal::Continue;
        }
        AppEvent::Resize(_, _) => return actions::LoopSignal::Continue,
    };
    actions::dispatch_action(app, agent, action, agent_rx, agent_task).await
}

pub async fn run_acp(config: crate::config::Config, client: crate::acp::AcpClient) -> Result<()> {
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

    let agent_name = client
        .agent_info()
        .map(|i| i.name.clone())
        .unwrap_or_else(|| "acp".into());
    let model_name = client.current_mode().unwrap_or("acp").to_string();
    let provider_name = agent_name.clone();

    let mut app = app::App::new(
        model_name,
        provider_name,
        agent_name,
        &config.theme.name,
        config.tui.vim_mode,
        config.tui.cursor_shape.clone(),
        config.tui.cursor_blink,
        config.tui.cursor_shape_normal.clone(),
        config.tui.cursor_blink_normal,
    );

    let acp = Arc::new(Mutex::new(client));
    let mut events = EventHandler::new();
    let mut agent_rx: Option<mpsc::UnboundedReceiver<crate::agent::AgentEvent>> = None;
    let mut agent_task: Option<tokio::task::JoinHandle<()>> = None;

    loop {
        terminal.draw(|f| ui::draw(f, &mut app))?;
        apply_cursor_style(&app)?;

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

        match handle_acp_event(&mut app, &acp, event, &mut agent_rx, &mut agent_task).await {
            actions::LoopSignal::Quit => break,
            actions::LoopSignal::OpenEditor => {
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
                if status.is_ok()
                    && let Ok(contents) = std::fs::read_to_string(&tmp)
                {
                    let trimmed = contents.trim_end().to_string();
                    if !trimmed.is_empty() {
                        app.cursor_pos = trimmed.len();
                        app.input = trimmed;
                    }
                }
                let _ = std::fs::remove_file(&tmp);
            }
            _ => {}
        }
    }

    if let Ok(mut c) = acp.try_lock() {
        let _ = c.kill();
    }

    terminal::disable_raw_mode()?;
    execute!(
        std::io::stderr(),
        terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture,
        crossterm::event::DisableBracketedPaste
    )?;
    terminal.show_cursor()?;
    execute!(std::io::stderr(), SetCursorStyle::DefaultUserShape)?;

    Ok(())
}

async fn handle_acp_event(
    app: &mut app::App,
    acp: &Arc<Mutex<crate::acp::AcpClient>>,
    event: AppEvent,
    agent_rx: &mut Option<mpsc::UnboundedReceiver<crate::agent::AgentEvent>>,
    agent_task: &mut Option<tokio::task::JoinHandle<()>>,
) -> actions::LoopSignal {
    let action = match event {
        AppEvent::Key(key) => input::handle_key(app, key),
        AppEvent::Mouse(mouse) => input::handle_mouse(app, mouse),
        AppEvent::Paste(text) => input::handle_paste(app, text),
        AppEvent::Tick => {
            app.tick_count = app.tick_count.wrapping_add(1);
            if app.status_message.as_ref().is_some_and(|s| s.expired()) {
                app.status_message = None;
                app.mark_dirty();
            }
            return actions::LoopSignal::Continue;
        }
        AppEvent::Agent(ev) => {
            app.handle_agent_event(ev);
            return actions::LoopSignal::Continue;
        }
        AppEvent::Resize(_, _) => return actions::LoopSignal::Continue,
    };
    actions::dispatch_acp_action(app, acp, action, agent_rx, agent_task).await
}
