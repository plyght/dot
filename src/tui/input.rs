use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::tui::app::{App, AppMode};
use crate::tui::widgets::COMMANDS;

pub enum InputAction {
    None,
    SendMessage(String),
    Quit,
    CancelStream,
    ScrollUp(u16),
    ScrollDown(u16),
    ScrollToTop,
    ScrollToBottom,
    ClearConversation,
    OpenModelSelector,
    OpenAgentSelector,
    SelectModel { provider: String, model: String },
    SelectAgent { name: String },
}

pub fn handle_key(app: &mut App, key: KeyEvent) -> InputAction {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        if app.model_selector.visible {
            app.model_selector.close();
            return InputAction::None;
        }
        if app.agent_selector.visible {
            app.agent_selector.close();
            return InputAction::None;
        }
        if app.command_palette.visible {
            app.command_palette.close();
            return InputAction::None;
        }
        if app.is_streaming {
            return InputAction::CancelStream;
        }
        return InputAction::Quit;
    }

    if app.model_selector.visible {
        return handle_model_selector(app, key);
    }

    if app.agent_selector.visible {
        return handle_agent_selector(app, key);
    }

    if app.command_palette.visible {
        return handle_command_palette(app, key);
    }

    match app.mode {
        AppMode::Normal => handle_normal(app, key),
        AppMode::Insert => handle_insert(app, key),
    }
}

fn handle_model_selector(app: &mut App, key: KeyEvent) -> InputAction {
    match key.code {
        KeyCode::Esc => {
            app.model_selector.close();
            InputAction::None
        }
        KeyCode::Up => {
            app.model_selector.up();
            InputAction::None
        }
        KeyCode::Down | KeyCode::Tab => {
            app.model_selector.down();
            InputAction::None
        }
        KeyCode::Enter => {
            if let Some(entry) = app.model_selector.confirm() {
                app.model_name = entry.model.clone();
                app.provider_name = entry.provider.clone();
                InputAction::SelectModel {
                    provider: entry.provider,
                    model: entry.model,
                }
            } else {
                InputAction::None
            }
        }
        KeyCode::Backspace => {
            app.model_selector.query.pop();
            app.model_selector.apply_filter();
            InputAction::None
        }
        KeyCode::Char(c) => {
            app.model_selector.query.push(c);
            app.model_selector.apply_filter();
            InputAction::None
        }
        _ => InputAction::None,
    }
}

fn handle_agent_selector(app: &mut App, key: KeyEvent) -> InputAction {
    match key.code {
        KeyCode::Esc => {
            app.agent_selector.close();
            InputAction::None
        }
        KeyCode::Up => {
            app.agent_selector.up();
            InputAction::None
        }
        KeyCode::Down | KeyCode::Tab => {
            app.agent_selector.down();
            InputAction::None
        }
        KeyCode::Enter => {
            if let Some(entry) = app.agent_selector.confirm() {
                app.agent_name = entry.name.clone();
                InputAction::SelectAgent { name: entry.name }
            } else {
                InputAction::None
            }
        }
        _ => InputAction::None,
    }
}

fn handle_command_palette(app: &mut App, key: KeyEvent) -> InputAction {
    match key.code {
        KeyCode::Esc => {
            app.command_palette.close();
            InputAction::None
        }
        KeyCode::Up => {
            app.command_palette.up();
            InputAction::None
        }
        KeyCode::Down | KeyCode::Tab => {
            app.command_palette.down();
            InputAction::None
        }
        KeyCode::Enter => {
            if let Some(cmd_name) = app.command_palette.confirm() {
                app.input.clear();
                app.cursor_pos = 0;
                execute_command(app, cmd_name)
            } else {
                InputAction::None
            }
        }
        KeyCode::Backspace => {
            app.delete_char_before();
            if app.input.is_empty() || !app.input.starts_with('/') {
                app.command_palette.close();
            } else {
                app.command_palette.update_filter(&app.input);
            }
            InputAction::None
        }
        KeyCode::Char(c) => {
            app.insert_char(c);
            app.command_palette.update_filter(&app.input);
            if app.command_palette.filtered.is_empty() {
                app.command_palette.close();
            }
            InputAction::None
        }
        _ => InputAction::None,
    }
}

fn execute_command(app: &mut App, cmd_name: &str) -> InputAction {
    match cmd_name {
        "model" => InputAction::OpenModelSelector,
        "agent" => InputAction::OpenAgentSelector,
        "clear" => {
            app.clear_conversation();
            InputAction::None
        }
        "help" => {
            let help: Vec<String> = COMMANDS
                .iter()
                .map(|c| format!("/{} — {}", c.name, c.description))
                .collect();
            app.error_message = Some(help.join(" · "));
            InputAction::None
        }
        _ => InputAction::None,
    }
}

fn handle_normal(app: &mut App, key: KeyEvent) -> InputAction {
    match key.code {
        KeyCode::Char('q') => InputAction::Quit,
        KeyCode::Char('i') | KeyCode::Enter => {
            app.mode = AppMode::Insert;
            InputAction::None
        }
        KeyCode::Char('j') | KeyCode::Down => InputAction::ScrollDown(1),
        KeyCode::Char('k') | KeyCode::Up => InputAction::ScrollUp(1),
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            InputAction::ScrollDown(10)
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            InputAction::ScrollUp(10)
        }
        KeyCode::Char('g') => InputAction::ScrollToTop,
        KeyCode::Char('G') => InputAction::ScrollToBottom,
        KeyCode::PageUp => InputAction::ScrollUp(20),
        KeyCode::PageDown => InputAction::ScrollDown(20),
        KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            InputAction::ClearConversation
        }
        KeyCode::Tab => InputAction::OpenAgentSelector,
        _ => InputAction::None,
    }
}

fn handle_insert(app: &mut App, key: KeyEvent) -> InputAction {
    if app.is_streaming {
        match key.code {
            KeyCode::Esc => {
                app.mode = AppMode::Normal;
            }
            _ => {}
        }
        return InputAction::None;
    }

    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
            InputAction::None
        }
        KeyCode::Enter => {
            if let Some(msg) = app.take_input() {
                InputAction::SendMessage(msg)
            } else {
                InputAction::None
            }
        }
        KeyCode::Char(c) => {
            app.insert_char(c);
            if app.input == "/" {
                app.command_palette.open(&app.input);
            } else if app.input.starts_with('/') && app.command_palette.visible {
                app.command_palette.update_filter(&app.input);
                if app.command_palette.filtered.is_empty() {
                    app.command_palette.close();
                }
            }
            InputAction::None
        }
        KeyCode::Backspace => {
            app.delete_char_before();
            if app.input.starts_with('/') && !app.input.is_empty() {
                if !app.command_palette.visible {
                    app.command_palette.open(&app.input);
                } else {
                    app.command_palette.update_filter(&app.input);
                }
            } else if app.command_palette.visible {
                app.command_palette.close();
            }
            InputAction::None
        }
        KeyCode::Left => {
            app.move_cursor_left();
            InputAction::None
        }
        KeyCode::Right => {
            app.move_cursor_right();
            InputAction::None
        }
        KeyCode::Home => {
            app.move_cursor_home();
            InputAction::None
        }
        KeyCode::End => {
            app.move_cursor_end();
            InputAction::None
        }
        _ => InputAction::None,
    }
}

fn rect_contains(r: ratatui::layout::Rect, col: u16, row: u16) -> bool {
    col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height
}

pub fn handle_mouse(app: &mut App, mouse: MouseEvent) -> InputAction {
    let col = mouse.column;
    let row = mouse.row;

    match mouse.kind {
        MouseEventKind::ScrollUp => {
            if app.model_selector.visible {
                if let Some(popup) = app.layout.model_selector {
                    if rect_contains(popup, col, row) {
                        app.model_selector.up();
                        return InputAction::None;
                    }
                }
            }
            InputAction::ScrollUp(3)
        }
        MouseEventKind::ScrollDown => {
            if app.model_selector.visible {
                if let Some(popup) = app.layout.model_selector {
                    if rect_contains(popup, col, row) {
                        app.model_selector.down();
                        return InputAction::None;
                    }
                }
            }
            InputAction::ScrollDown(3)
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if app.model_selector.visible {
                if let Some(popup) = app.layout.model_selector {
                    if !rect_contains(popup, col, row) {
                        app.model_selector.close();
                    }
                    return InputAction::None;
                }
            }

            if app.agent_selector.visible {
                app.agent_selector.close();
                return InputAction::None;
            }

            if app.command_palette.visible {
                if let Some(popup) = app.layout.command_palette {
                    if rect_contains(popup, col, row) {
                        let relative_row = row.saturating_sub(popup.y) as usize;
                        if relative_row < app.command_palette.filtered.len() {
                            app.command_palette.selected = relative_row;
                            if let Some(cmd_name) = app.command_palette.confirm() {
                                app.input.clear();
                                app.cursor_pos = 0;
                                return execute_command(app, cmd_name);
                            }
                        }
                        return InputAction::None;
                    } else {
                        app.command_palette.close();
                        return InputAction::None;
                    }
                }
            }

            if rect_contains(app.layout.input, col, row) {
                app.mode = AppMode::Insert;
                let inner_x = col.saturating_sub(app.layout.input.x + 3);
                let inner_y = row.saturating_sub(app.layout.input.y + 1);
                let target_offset =
                    compute_click_cursor_pos(&app.input, inner_x as usize, inner_y as usize);
                app.cursor_pos = target_offset;
                InputAction::None
            } else if rect_contains(app.layout.messages, col, row) {
                if app.mode == AppMode::Insert && app.input.is_empty() {
                    app.mode = AppMode::Normal;
                }
                InputAction::None
            } else {
                InputAction::None
            }
        }
        _ => InputAction::None,
    }
}

fn compute_click_cursor_pos(input: &str, target_col: usize, target_row: usize) -> usize {
    let mut row: usize = 0;
    let mut col: usize = 0;
    let mut byte_pos: usize = 0;

    for ch in input.chars() {
        if row == target_row && col >= target_col {
            return byte_pos;
        }
        if ch == '\n' {
            if row == target_row {
                return byte_pos;
            }
            row += 1;
            col = 0;
        } else {
            col += 1;
        }
        byte_pos += ch.len_utf8();
    }

    byte_pos
}
