use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::app::{App, AppMode, COMMANDS};

pub enum InputAction {
    None,
    SendMessage(String),
    Quit,
    ScrollUp(u16),
    ScrollDown(u16),
    ScrollToTop,
    ScrollToBottom,
    ClearConversation,
    OpenModelSelector,
    SelectModel(String),
}

pub fn handle_key(app: &mut App, key: KeyEvent) -> InputAction {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        if app.model_selector.visible {
            app.model_selector.close();
            return InputAction::None;
        }
        if app.command_palette.visible {
            app.command_palette.close();
            return InputAction::None;
        }
        return InputAction::Quit;
    }

    if app.model_selector.visible {
        return handle_model_selector(app, key);
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
        KeyCode::Up | KeyCode::Char('k') => {
            app.model_selector.up();
            InputAction::None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.model_selector.down();
            InputAction::None
        }
        KeyCode::Enter => {
            if let Some(model) = app.model_selector.confirm() {
                app.model_name = model.clone();
                InputAction::SelectModel(model)
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
