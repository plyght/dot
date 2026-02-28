use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::app::{App, AppMode};

use super::InputAction;
use super::popups;

pub(super) fn handle_normal(app: &mut App, key: KeyEvent) -> InputAction {
    match key.code {
        KeyCode::Char('q') => InputAction::Quit,
        KeyCode::Char('i') => {
            app.mode = AppMode::Insert;
            InputAction::None
        }
        KeyCode::Enter => {
            if !app.input.is_empty() {
                app.mode = AppMode::Insert;
                handle_send(app)
            } else {
                app.mode = AppMode::Insert;
                InputAction::None
            }
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
        KeyCode::Tab => InputAction::ToggleAgent,
        KeyCode::Char('t') => InputAction::ToggleThinking,
        KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            InputAction::OpenRenamePopup
        }
        _ => InputAction::None,
    }
}

pub(super) fn handle_insert(app: &mut App, key: KeyEvent) -> InputAction {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return InputAction::OpenRenamePopup;
            }
            KeyCode::Char('t') => return InputAction::CycleThinkingLevel,
            KeyCode::Char('a') => {
                app.move_cursor_home();
                return InputAction::None;
            }
            KeyCode::Char('e') => {
                return InputAction::OpenExternalEditor;
            }
            KeyCode::Char('w') => {
                app.delete_word_before();
                return InputAction::None;
            }
            KeyCode::Char('k') => {
                app.delete_to_end();
                return InputAction::None;
            }
            KeyCode::Char('u') => {
                app.delete_to_start();
                return InputAction::None;
            }
            KeyCode::Char('j') => {
                if !app.input.is_empty() {
                    app.insert_char('\n');
                }
                return InputAction::None;
            }
            _ => {}
        }
    }

    if app.is_streaming {
        return match key.code {
            KeyCode::Esc => {
                app.mode = AppMode::Normal;
                InputAction::None
            }
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                if !app.input.is_empty() {
                    app.insert_char('\n');
                }
                InputAction::None
            }
            KeyCode::Enter => handle_send(app),
            KeyCode::Char(c) => handle_char_input(app, c),
            KeyCode::Backspace => handle_backspace(app),
            KeyCode::Up => {
                if !app.move_cursor_up() {
                    app.history_prev();
                }
                InputAction::None
            }
            KeyCode::Down => {
                if !app.move_cursor_down() {
                    app.history_next();
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
        };
    }

    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
            InputAction::None
        }
        KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
            if !app.input.is_empty() {
                app.insert_char('\n');
            }
            InputAction::None
        }
        KeyCode::Enter => handle_send(app),
        KeyCode::Char(c) => handle_char_input(app, c),
        KeyCode::Backspace => handle_backspace(app),
        KeyCode::Up => {
            if !app.move_cursor_up() {
                app.history_prev();
            }
            InputAction::None
        }
        KeyCode::Down => {
            if !app.move_cursor_down() {
                app.history_next();
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

pub(super) fn handle_simple(app: &mut App, key: KeyEvent) -> InputAction {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('t') => return InputAction::CycleThinkingLevel,
            KeyCode::Char('a') => {
                app.move_cursor_home();
                return InputAction::None;
            }
            KeyCode::Char('e') => {
                return InputAction::OpenExternalEditor;
            }
            KeyCode::Char('w') => {
                app.delete_word_before();
                return InputAction::None;
            }
            KeyCode::Char('k') => {
                app.delete_to_end();
                return InputAction::None;
            }
            KeyCode::Char('u') => {
                app.delete_to_start();
                return InputAction::None;
            }
            KeyCode::Char('d') => return InputAction::ScrollDown(10),
            KeyCode::Char('j') => {
                if !app.input.is_empty() {
                    app.insert_char('\n');
                }
                return InputAction::None;
            }
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return InputAction::OpenRenamePopup;
            }
            _ => {}
        }
    }

    if app.is_streaming {
        return match key.code {
            KeyCode::Up => {
                if !app.move_cursor_up() {
                    app.history_prev();
                }
                InputAction::None
            }
            KeyCode::Down => {
                if !app.move_cursor_down() {
                    app.history_next();
                }
                InputAction::None
            }
            KeyCode::PageUp => InputAction::ScrollUp(20),
            KeyCode::PageDown => InputAction::ScrollDown(20),
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                if !app.input.is_empty() {
                    app.insert_char('\n');
                }
                InputAction::None
            }
            KeyCode::Enter => handle_send(app),
            KeyCode::Char(c) => handle_char_input(app, c),
            KeyCode::Backspace => handle_backspace(app),
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
        };
    }

    match key.code {
        KeyCode::Esc => InputAction::None,
        KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
            if !app.input.is_empty() {
                app.insert_char('\n');
            }
            InputAction::None
        }
        KeyCode::Enter => handle_send(app),
        KeyCode::Up => {
            if !app.move_cursor_up() {
                app.history_prev();
            }
            InputAction::None
        }
        KeyCode::Down => {
            if !app.move_cursor_down() {
                app.history_next();
            }
            InputAction::None
        }
        KeyCode::PageUp => InputAction::ScrollUp(20),
        KeyCode::PageDown => InputAction::ScrollDown(20),
        KeyCode::Tab => InputAction::ToggleAgent,
        KeyCode::Char(c) => handle_char_input(app, c),
        KeyCode::Backspace => handle_backspace(app),
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

fn handle_send(app: &mut App) -> InputAction {
    parse_at_references(app);
    if app.is_streaming {
        app.queue_input();
        return InputAction::None;
    }
    if let Some(msg) = app.take_input() {
        if let Some(rest) = msg.strip_prefix('/') {
            let mut parts = rest.splitn(2, char::is_whitespace);
            let cmd = parts.next().unwrap_or_default().to_string();
            let args = parts.next().unwrap_or_default().to_string();
            let builtin = matches!(
                cmd.as_str(),
                "model" | "agent" | "thinking" | "sessions" | "new" | "clear" | "help" | "export"
            );
            if cmd == "rename" {
                return if args.is_empty() {
                    InputAction::OpenRenamePopup
                } else {
                    InputAction::RenameSession(args)
                };
            }
            if cmd == "export" {
                return InputAction::ExportSession(if args.is_empty() { None } else { Some(args) });
            }
            if builtin {
                return popups::execute_command(app, &cmd);
            }
            if app.custom_command_names.contains(&cmd) {
                return InputAction::RunCustomCommand { name: cmd, args };
            }
        }
        InputAction::SendMessage(msg)
    } else {
        InputAction::None
    }
}

fn handle_char_input(app: &mut App, c: char) -> InputAction {
    app.insert_char(c);
    if app.input == "/" {
        if app.command_palette.entries.is_empty() {
            app.command_palette.set_skills(&app.skill_entries);
        }
        app.command_palette.open(&app.input);
    } else if app.input.starts_with('/') && app.command_palette.visible {
        app.command_palette.update_filter(&app.input);
        if app.command_palette.filtered.is_empty() {
            app.command_palette.close();
        }
    }
    if c == '@' && !app.file_picker.visible {
        let pos = app.cursor_pos - 1;
        let at_boundary = pos == 0
            || app
                .input
                .as_bytes()
                .get(pos.wrapping_sub(1))
                .is_none_or(|b| b.is_ascii_whitespace());
        if at_boundary {
            app.file_picker.open(pos);
        }
    }
    InputAction::None
}

fn handle_backspace(app: &mut App) -> InputAction {
    if let Some(chip_idx) = app.chip_at_cursor() {
        app.delete_chip(chip_idx);
    } else if let Some(pb_idx) = app.paste_block_at_cursor() {
        app.delete_paste_block(pb_idx);
    } else {
        app.delete_char_before();
    }
    if app.input.starts_with('/') && !app.input.is_empty() {
        if !app.command_palette.visible {
            if app.command_palette.entries.is_empty() {
                app.command_palette.set_skills(&app.skill_entries);
            }
            app.command_palette.open(&app.input);
        } else {
            app.command_palette.update_filter(&app.input);
        }
    } else if app.command_palette.visible {
        app.command_palette.close();
    }
    InputAction::None
}

fn parse_at_references(app: &mut App) {
    let words: Vec<String> = app.input.split_whitespace().map(String::from).collect();
    for word in &words {
        if let Some(path) = word.strip_prefix('@')
            && !path.is_empty()
            && crate::tui::app::is_image_path(path)
        {
            match app.add_image_attachment(path) {
                Ok(()) => {}
                Err(e) => {
                    app.status_message = Some(crate::tui::app::StatusMessage::error(e));
                }
            }
        }
    }
}
