pub mod app;
pub mod event;
pub mod input;
pub mod markdown;
pub mod theme;
pub mod ui;

use std::sync::Arc;

use anyhow::Result;
use crossterm::{execute, terminal};
use tokio::sync::{mpsc, Mutex};

use crate::agent::Agent;
use crate::config::Config;
use crate::db::Db;
use crate::provider::Provider;

use app::App;
use event::{AppEvent, EventHandler};
use input::InputAction;

pub async fn run(config: Config, provider: Box<dyn Provider>, db: Db) -> Result<()> {
    terminal::enable_raw_mode()?;
    let mut stdout = std::io::stderr();
    execute!(
        stdout,
        terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let result = run_app(&mut terminal, config, provider, db).await;

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
    provider: Box<dyn Provider>,
    db: Db,
) -> Result<()> {
    let model_name = provider.model().to_string();
    let provider_name = provider.name().to_string();

    let agent = Arc::new(Mutex::new(Agent::new(provider, db, &config)?));
    let mut app = App::new(model_name, provider_name);
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
                        if handle_ui_event(&mut app, &agent, ev).await {
                            break;
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
                    match action {
                        InputAction::Quit => break,
                        InputAction::SendMessage(msg) => {
                            let (tx, rx) = mpsc::unbounded_channel();
                            agent_rx = Some(rx);

                            let agent_clone = Arc::clone(&agent);
                            tokio::spawn(async move {
                                let mut agent = agent_clone.lock().await;
                                if let Err(e) = agent.send_message(&msg, tx).await {
                                    tracing::error!("Agent send_message error: {}", e);
                                }
                            });
                        }
                        InputAction::OpenModelSelector => {
                            let agent_lock = agent.lock().await;
                            let models = match agent_lock.fetch_models().await {
                                Ok(m) => m,
                                Err(_) => agent_lock.available_models(),
                            };
                            let current = agent_lock.current_model().to_string();
                            drop(agent_lock);
                            app.model_selector.open(models, &current);
                        }
                        InputAction::SelectModel(model) => {
                            let mut agent_lock = agent.lock().await;
                            agent_lock.set_model(model);
                        }
                        InputAction::ScrollUp(n) => app.scroll_up(n),
                        InputAction::ScrollDown(n) => app.scroll_down(n),
                        InputAction::ScrollToTop => app.scroll_to_top(),
                        InputAction::ScrollToBottom => app.scroll_to_bottom(),
                        InputAction::ClearConversation => app.clear_conversation(),
                        InputAction::None => {}
                    }
                }
                Some(AppEvent::Agent(ev)) => {
                    app.handle_agent_event(ev);
                }
                Some(AppEvent::Tick) => {}
                Some(AppEvent::Resize(_, _)) => {}
                None => break,
            }
        }
    }

    Ok(())
}

async fn handle_ui_event(app: &mut App, agent: &Arc<Mutex<Agent>>, event: AppEvent) -> bool {
    match event {
        AppEvent::Key(key) => {
            let action = input::handle_key(app, key);
            match action {
                InputAction::Quit => return true,
                InputAction::OpenModelSelector => {
                    let agent_lock = agent.lock().await;
                    let models = match agent_lock.fetch_models().await {
                        Ok(m) => m,
                        Err(_) => agent_lock.available_models(),
                    };
                    let current = agent_lock.current_model().to_string();
                    drop(agent_lock);
                    app.model_selector.open(models, &current);
                }
                InputAction::SelectModel(model) => {
                    let mut agent_lock = agent.lock().await;
                    agent_lock.set_model(model);
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
        AppEvent::Tick => {}
        AppEvent::Agent(ev) => app.handle_agent_event(ev),
        AppEvent::Resize(_, _) => {}
    }
    false
}
