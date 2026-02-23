use crossterm::event::{Event as CEvent, EventStream, KeyEventKind, MouseEventKind};
use futures::StreamExt;
use tokio::sync::mpsc;

use crate::agent::AgentEvent;

pub enum AppEvent {
    Key(crossterm::event::KeyEvent),
    Mouse(crossterm::event::MouseEvent),
    Paste(String),
    Tick,
    Agent(AgentEvent),
    Resize(u16, u16),
}

impl Default for EventHandler {
    fn default() -> Self {
        Self::new()
    }
}

pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<AppEvent>,
    tx: mpsc::UnboundedSender<AppEvent>,
    _task: tokio::task::JoinHandle<()>,
}

impl EventHandler {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let event_tx = tx.clone();

        let task = tokio::spawn(async move {
            let mut reader = EventStream::new();
            let mut tick = tokio::time::interval(std::time::Duration::from_millis(16));

            loop {
                tokio::select! {
                    maybe_event = reader.next() => {
                        match maybe_event {
                            Some(Ok(CEvent::Key(key))) => {
                                if key.kind == KeyEventKind::Press
                                    && event_tx.send(AppEvent::Key(key)).is_err()
                                {
                                    return;
                                }
                            }
                            Some(Ok(CEvent::Mouse(mouse))) => {
                                let forward = matches!(
                                    mouse.kind,
                                    MouseEventKind::Down(_)
                                        | MouseEventKind::Up(_)
                                        | MouseEventKind::Drag(_)
                                        | MouseEventKind::ScrollUp
                                        | MouseEventKind::ScrollDown
                                );
                                if forward
                                    && event_tx.send(AppEvent::Mouse(mouse)).is_err()
                                {
                                    return;
                                }
                            }
                            Some(Ok(CEvent::Paste(text))) => {
                                if event_tx.send(AppEvent::Paste(text)).is_err() {
                                    return;
                                }
                            }
                            Some(Ok(CEvent::Resize(w, h))) => {
                                if event_tx.send(AppEvent::Resize(w, h)).is_err() {
                                    return;
                                }
                            }
                            Some(Ok(_)) => {}
                            Some(Err(_)) => return,
                            None => return,
                        }
                    }
                    _ = tick.tick() => {
                        if event_tx.send(AppEvent::Tick).is_err() {
                            return;
                        }
                    }
                }
            }
        });

        Self {
            rx,
            tx,
            _task: task,
        }
    }

    pub async fn next(&mut self) -> Option<AppEvent> {
        self.rx.recv().await
    }

    pub fn tx(&self) -> mpsc::UnboundedSender<AppEvent> {
        self.tx.clone()
    }
}
