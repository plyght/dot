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

use anyhow::Result;
use crossterm::{execute, terminal};
use tokio::sync::{Mutex, mpsc};

use crate::agent::{Agent, AgentProfile};
use crate::config::Config;
use crate::db::Db;
use crate::provider::Provider;
use crate::tools::ToolRegistry;

use app::App;
use event::{AppEvent, EventHandler};
use input::InputAction;
use widgets::AgentEntry;

pub async fn run(
    config: Config,
    providers: Vec<Box<dyn Provider>>,
    db: Db,
    tools: ToolRegistry,
    profiles: Vec<AgentProfile>,
) -> Result<()> {
    terminal::enable_raw_mode()?;
    let mut stdout = std::io::stderr();
    execute!(
        stdout,
        terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let result = run_app(&mut terminal, config, providers, db, tools, profiles).await;

    terminal::disable_raw_mode()?;
    execute!(
        std::io::stderr(),
        terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn run_app(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stderr>>,
    config: Config,
    providers: Vec<Box<dyn Provider>>,
    db: Db,
    tools: ToolRegistry,
    profiles: Vec<AgentProfile>,
) -> Result<()> {
    let model_name = providers[0].model().to_string();
    let provider_name = providers[0].name().to_string();
    let agent_name = profiles.first().map(|p| p.name.clone()).unwrap_or_else(|| "dot".to_string());

    let agent = Arc::new(Mutex::new(Agent::new(
        providers, db, &config, tools, profiles,
    )?));
    let mut app = App::new(model_name, provider_name, agent_name, &config.theme.name);
    let mut events = EventHandler::new();
    let mut agent_rx: Option<mpsc::UnboundedReceiver<crate::agent::AgentEvent>> = None;

    loop {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        if let Some(ref mut rx) = agent_rx {
            tokio::select! {
                biased;
                agent_event = rx.recv() => {
                    match agent_event {
                        Some(ev) => {
                            let is_done = matches!(ev, crate::agent::AgentEvent::Done { .. } | crate::agent::AgentEvent::Error(_));
                            app.handle_agent_event(ev);
                            if is_done {
                                agent_rx = None;
                            }
                        }
                        None => {
                            app.is_streaming = false;
                            agent_rx = None;
                        }
                    }
                }
                ui_event = events.next() => {
                    if let Some(ev) = ui_event {
                        match handle_ui_event(&mut app, &agent, ev).await {
                            LoopSignal::Quit => break,
                            LoopSignal::CancelStream => { agent_rx = None; }
                            LoopSignal::Continue => {}
                        }
                    } else {
                        break;
                    }
                }
            }
        } else {
            match events.next().await {
                Some(AppEvent::Key(key)) => {
                    let action = input::handle_key(&mut app, key);
                    match dispatch_action(&mut app, &agent, action, &mut agent_rx).await {
                        LoopSignal::Quit => break,
                        _ => {}
                    }
                }
                Some(AppEvent::Agent(ev)) => {
                    app.handle_agent_event(ev);
                }
                Some(AppEvent::Mouse(mouse)) => {
                    let action = input::handle_mouse(&mut app, mouse);
                    match dispatch_action(&mut app, &agent, action, &mut agent_rx).await {
                        LoopSignal::Quit => break,
                        _ => {}
                    }
                }
                Some(AppEvent::Tick) => {
                    app.tick_count = app.tick_count.wrapping_add(1);
                }
                Some(AppEvent::Resize(_, _)) => {}
                None => break,
            }
        }
    }

    Ok(())
}

enum LoopSignal {
    Continue,
    Quit,
    CancelStream,
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
            app.current_response.clear();
            app.current_tool_calls.clear();
            app.pending_tool_name = None;
            app.error_message = Some("cancelled".to_string());
            return LoopSignal::CancelStream;
        }
        InputAction::SendMessage(msg) => {
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
        InputAction::OpenModelSelector => {
            let agent_lock = agent.lock().await;
            let grouped = agent_lock.fetch_all_models().await;
            let current_provider = agent_lock.current_provider_name().to_string();
            let current_model = agent_lock.current_model().to_string();
            drop(agent_lock);
            app.model_selector.open(grouped, &current_provider, &current_model);
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
        InputAction::SelectModel { provider, model } => {
            let mut agent_lock = agent.lock().await;
            agent_lock.set_active_provider(&provider, &model);
        }
        InputAction::SelectAgent { name } => {
            let mut agent_lock = agent.lock().await;
            agent_lock.switch_agent(&name);
            app.model_name = agent_lock.current_model().to_string();
            app.provider_name = agent_lock.current_provider_name().to_string();
        }
        InputAction::ScrollUp(n) => app.scroll_up(n),
        InputAction::ScrollDown(n) => app.scroll_down(n),
        InputAction::ScrollToTop => app.scroll_to_top(),
        InputAction::ScrollToBottom => app.scroll_to_bottom(),
        InputAction::ClearConversation => app.clear_conversation(),
        InputAction::None => {}
    }
    LoopSignal::Continue
}

async fn handle_ui_event(app: &mut App, agent: &Arc<Mutex<Agent>>, event: AppEvent) -> LoopSignal {
    match event {
        AppEvent::Key(key) => {
            let action = input::handle_key(app, key);
            match action {
                InputAction::Quit => return LoopSignal::Quit,
                InputAction::CancelStream => {
                    app.is_streaming = false;
                    app.streaming_started = None;
                    app.current_response.clear();
                    app.current_tool_calls.clear();
                    app.pending_tool_name = None;
                    app.error_message = Some("cancelled".to_string());
                    return LoopSignal::CancelStream;
                }
                InputAction::OpenModelSelector => {
                    let agent_lock = agent.lock().await;
                    let grouped = agent_lock.fetch_all_models().await;
                    let current_provider = agent_lock.current_provider_name().to_string();
                    let current_model = agent_lock.current_model().to_string();
                    drop(agent_lock);
                    app.model_selector.open(grouped, &current_provider, &current_model);
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
                InputAction::SelectModel { provider, model } => {
                    let mut agent_lock = agent.lock().await;
                    agent_lock.set_active_provider(&provider, &model);
                }
                InputAction::SelectAgent { name } => {
                    let mut agent_lock = agent.lock().await;
                    agent_lock.switch_agent(&name);
                    app.model_name = agent_lock.current_model().to_string();
                    app.provider_name = agent_lock.current_provider_name().to_string();
                }
                InputAction::ScrollUp(n) => app.scroll_up(n),
                InputAction::ScrollDown(n) => app.scroll_down(n),
                InputAction::ScrollToTop => app.scroll_to_top(),
                InputAction::ScrollToBottom => app.scroll_to_bottom(),
                InputAction::ClearConversation => app.clear_conversation(),
                InputAction::SendMessage(_) => {}
                InputAction::None => {}
            }
        }
        AppEvent::Mouse(mouse) => {
            let action = input::handle_mouse(app, mouse);
            match action {
                InputAction::ScrollUp(n) => app.scroll_up(n),
                InputAction::ScrollDown(n) => app.scroll_down(n),
                _ => {}
            }
        }
        AppEvent::Tick => {
            app.tick_count = app.tick_count.wrapping_add(1);
        }
        AppEvent::Agent(ev) => app.handle_agent_event(ev),
        AppEvent::Resize(_, _) => {}
    }
    LoopSignal::Continue
}
