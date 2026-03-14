use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::{App, ChipKind, InputChip, StatusMessage};
use crate::tui::widgets::{LoginStep, PaletteEntry, PaletteEntryKind};

use super::InputAction;

pub(super) fn handle_model_selector(app: &mut App, key: KeyEvent) -> InputAction {
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
        KeyCode::Char('*') | KeyCode::Char('s') => {
            app.model_selector.toggle_favorite();
            app.favorite_models = app.model_selector.favorites.clone();
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

pub(super) fn handle_agent_selector(app: &mut App, key: KeyEvent) -> InputAction {
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

pub(super) fn handle_thinking_selector(app: &mut App, key: KeyEvent) -> InputAction {
    match key.code {
        KeyCode::Esc => {
            app.thinking_selector.close();
            InputAction::None
        }
        KeyCode::Up => {
            app.thinking_selector.up();
            InputAction::None
        }
        KeyCode::Down | KeyCode::Tab => {
            app.thinking_selector.down();
            InputAction::None
        }
        KeyCode::Enter => {
            if let Some(level) = app.thinking_selector.confirm() {
                let budget = level.budget_tokens();
                app.thinking_budget = budget;
                InputAction::SetThinkingLevel(budget)
            } else {
                InputAction::None
            }
        }
        _ => InputAction::None,
    }
}

pub(super) fn handle_session_selector(app: &mut App, key: KeyEvent) -> InputAction {
    match key.code {
        KeyCode::Esc => {
            app.session_selector.close();
            InputAction::None
        }
        KeyCode::Up => {
            app.session_selector.up();
            InputAction::None
        }
        KeyCode::Down | KeyCode::Tab => {
            app.session_selector.down();
            InputAction::None
        }
        KeyCode::Enter => {
            if let Some(id) = app.session_selector.confirm() {
                InputAction::ResumeSession { id }
            } else {
                InputAction::None
            }
        }
        KeyCode::Backspace => {
            app.session_selector.query.pop();
            app.session_selector.apply_filter();
            InputAction::None
        }
        KeyCode::Char(c) => {
            app.session_selector.query.push(c);
            app.session_selector.apply_filter();
            InputAction::None
        }
        _ => InputAction::None,
    }
}

pub(super) fn handle_command_palette(app: &mut App, key: KeyEvent) -> InputAction {
    match key.code {
        KeyCode::Esc => {
            app.command_palette.close();
            InputAction::None
        }
        KeyCode::Up => {
            app.command_palette.up();
            InputAction::None
        }
        KeyCode::Tab => {
            if let Some(&idx) = app
                .command_palette
                .filtered
                .get(app.command_palette.selected)
            {
                let entry = app.command_palette.entries[idx].clone();
                app.command_palette.close();
                if entry.kind == PaletteEntryKind::Command {
                    app.input.clear();
                    app.cursor_pos = 0;
                    app.chips.clear();
                    return execute_palette_entry(app, entry);
                }
                place_skill_chip(app, &entry.name);
                return InputAction::None;
            }
            InputAction::None
        }
        KeyCode::Down => {
            app.command_palette.down();
            InputAction::None
        }
        KeyCode::Enter => {
            if let Some(entry) = app.command_palette.confirm() {
                if entry.kind == PaletteEntryKind::Skill {
                    place_skill_chip(app, &entry.name);
                    return InputAction::LoadSkill { name: entry.name };
                }
                app.input.clear();
                app.cursor_pos = 0;
                app.chips.clear();
                execute_palette_entry(app, entry)
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

pub(super) fn handle_file_picker(app: &mut App, key: KeyEvent) -> InputAction {
    match key.code {
        KeyCode::Esc => {
            app.file_picker.close();
            InputAction::None
        }
        KeyCode::Up => {
            app.file_picker.up();
            InputAction::None
        }
        KeyCode::Down => {
            app.file_picker.down();
            InputAction::None
        }
        KeyCode::Enter | KeyCode::Tab => {
            if let Some(entry) = app.file_picker.confirm() {
                if entry.is_dir {
                    let new_query = format!("{}/", entry.path);
                    let start = app.file_picker.at_pos;
                    let end = app.cursor_pos;
                    app.input
                        .replace_range(start..end, &format!("@{}", new_query));
                    app.cursor_pos = start + 1 + new_query.len();
                    app.file_picker.open(start);
                    app.file_picker.update_query(&new_query);
                } else {
                    let path = entry.path;
                    let start = app.file_picker.at_pos;
                    let end = app.cursor_pos;
                    let text = format!("@{} ", path);
                    let old_len = end - start;
                    app.input.replace_range(start..end, &text);
                    app.adjust_chips(start, old_len, text.len());
                    let chip_end = start + 1 + path.len();
                    app.chips.push(InputChip {
                        start,
                        end: chip_end,
                        kind: ChipKind::File,
                    });
                    app.cursor_pos = start + text.len();
                }
            }
            InputAction::None
        }
        KeyCode::Backspace => {
            app.delete_char_before();
            let at_pos = app.file_picker.at_pos;
            if app.cursor_pos <= at_pos {
                app.file_picker.close();
            } else {
                let query = app.input[at_pos + 1..app.cursor_pos].to_string();
                app.file_picker.update_query(&query);
                if app.file_picker.filtered.is_empty() {
                    app.file_picker.close();
                }
            }
            InputAction::None
        }
        KeyCode::Char(c) => {
            app.insert_char(c);
            let at_pos = app.file_picker.at_pos;
            let query = app.input[at_pos + 1..app.cursor_pos].to_string();
            app.file_picker.update_query(&query);
            if app.file_picker.filtered.is_empty() {
                app.file_picker.close();
            }
            InputAction::None
        }
        _ => InputAction::None,
    }
}

pub(super) fn execute_command(app: &mut App, cmd_name: &str) -> InputAction {
    match cmd_name {
        "model" => InputAction::OpenModelSelector,
        "agent" => InputAction::OpenAgentSelector,
        "thinking" => InputAction::OpenThinkingSelector,
        "sessions" => InputAction::OpenSessionSelector,
        "new" => InputAction::NewConversation,
        "clear" => {
            app.clear_conversation();
            InputAction::None
        }
        "help" => {
            app.help_popup.open();
            InputAction::None
        }
        "login" => InputAction::OpenLoginPopup,
        other => {
            if app.custom_command_names.contains(&other.to_string()) {
                InputAction::RunCustomCommand {
                    name: other.to_string(),
                    args: String::new(),
                }
            } else {
                InputAction::None
            }
        }
    }
}

pub(super) fn execute_palette_entry(app: &mut App, entry: PaletteEntry) -> InputAction {
    match entry.kind {
        PaletteEntryKind::Command => execute_command(app, &entry.name),
        PaletteEntryKind::Skill => InputAction::LoadSkill { name: entry.name },
    }
}

pub(super) fn place_skill_chip(app: &mut App, name: &str) {
    let text = format!("/{} ", name);
    app.input.clear();
    app.cursor_pos = 0;
    app.chips.clear();
    app.paste_blocks.clear();
    app.input.push_str(&text);
    app.cursor_pos = text.len();
    let chip_end = 1 + name.len();
    app.chips.push(InputChip {
        start: 0,
        end: chip_end,
        kind: ChipKind::Skill,
    });
}

pub(super) fn handle_context_menu(app: &mut App, key: KeyEvent) -> InputAction {
    match key.code {
        KeyCode::Esc => {
            app.context_menu.close();
            InputAction::None
        }
        KeyCode::Up => {
            app.context_menu.up();
            InputAction::None
        }
        KeyCode::Down | KeyCode::Tab => {
            app.context_menu.down();
            InputAction::None
        }
        KeyCode::Enter => {
            if let Some((action, msg_idx)) = app.context_menu.confirm() {
                match action {
                    0 => InputAction::RevertToMessage(msg_idx),
                    1 => InputAction::ForkFromMessage(msg_idx),
                    2 => InputAction::CopyMessage(msg_idx),
                    _ => InputAction::None,
                }
            } else {
                InputAction::None
            }
        }
        _ => InputAction::None,
    }
}

pub(super) fn handle_rename_popup(app: &mut App, key: KeyEvent) -> InputAction {
    match key.code {
        KeyCode::Esc => {
            app.rename_visible = false;
            app.rename_input.clear();
            InputAction::None
        }
        KeyCode::Enter => {
            let title = app.rename_input.trim().to_string();
            app.rename_visible = false;
            app.rename_input.clear();
            if title.is_empty() {
                InputAction::None
            } else {
                InputAction::RenameSession(title)
            }
        }
        KeyCode::Backspace => {
            app.rename_input.pop();
            InputAction::None
        }
        KeyCode::Char(c) => {
            app.rename_input.push(c);
            InputAction::None
        }
        _ => InputAction::None,
    }
}

pub(super) fn handle_question_popup(app: &mut App, key: KeyEvent) -> InputAction {
    let pq = app.pending_question.as_mut().unwrap();
    match key.code {
        KeyCode::Esc => {
            if let Some(responder) = pq.responder.take() {
                let _ = responder.0.send("[cancelled]".to_string());
            }
            app.pending_question = None;
            InputAction::None
        }
        KeyCode::Up => {
            if pq.selected > 0 {
                pq.selected -= 1;
            }
            InputAction::None
        }
        KeyCode::Down | KeyCode::Tab => {
            let max = if pq.options.is_empty() {
                0
            } else {
                pq.options.len()
            };
            if pq.selected < max {
                pq.selected += 1;
            }
            InputAction::None
        }
        KeyCode::Enter => {
            let answer = if pq.options.is_empty() || pq.selected >= pq.options.len() {
                if pq.custom_input.is_empty() {
                    "ok".to_string()
                } else {
                    pq.custom_input.clone()
                }
            } else {
                pq.options[pq.selected].clone()
            };
            if let Some(responder) = pq.responder.take() {
                let _ = responder.0.send(answer.clone());
            }
            app.pending_question = None;
            InputAction::AnswerQuestion(answer)
        }
        KeyCode::Char(c) => {
            pq.custom_input.push(c);
            pq.selected = pq.options.len();
            InputAction::None
        }
        KeyCode::Backspace => {
            pq.custom_input.pop();
            InputAction::None
        }
        _ => InputAction::None,
    }
}

pub(super) fn handle_permission_popup(app: &mut App, key: KeyEvent) -> InputAction {
    let pp = app.pending_permission.as_mut().unwrap();
    match key.code {
        KeyCode::Esc => {
            if let Some(responder) = pp.responder.take() {
                let _ = responder.0.send("deny".to_string());
            }
            app.pending_permission = None;
            InputAction::None
        }
        KeyCode::Up => {
            if pp.selected > 0 {
                pp.selected -= 1;
            }
            InputAction::None
        }
        KeyCode::Down | KeyCode::Tab => {
            if pp.selected < 1 {
                pp.selected += 1;
            }
            InputAction::None
        }
        KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
            let answer = if pp.selected == 0 { "allow" } else { "deny" };
            if let Some(responder) = pp.responder.take() {
                let _ = responder.0.send(answer.to_string());
            }
            app.pending_permission = None;
            InputAction::AnswerPermission(answer.to_string())
        }
        KeyCode::Char('n') | KeyCode::Char('N') => {
            if let Some(responder) = pp.responder.take() {
                let _ = responder.0.send("deny".to_string());
            }
            app.pending_permission = None;
            InputAction::AnswerPermission("deny".to_string())
        }
        _ => InputAction::None,
    }
}

pub(super) fn handle_login_popup(app: &mut App, key: KeyEvent) -> InputAction {
    let lp = &mut app.login_popup;
    match lp.step {
        LoginStep::SelectProvider => match key.code {
            KeyCode::Esc => {
                let back_to_welcome = lp.from_welcome;
                lp.close();
                if back_to_welcome {
                    app.welcome_screen.open();
                }
                InputAction::None
            }
            KeyCode::Up => {
                lp.up();
                InputAction::None
            }
            KeyCode::Down | KeyCode::Tab => {
                lp.down();
                InputAction::None
            }
            KeyCode::Enter => {
                let provider = lp.selected;
                match provider {
                    0 => {
                        lp.provider = Some("anthropic".to_string());
                        lp.step = LoginStep::SelectMethod;
                        lp.selected = 0;
                    }
                    1 => {
                        lp.provider = Some("openai".to_string());
                        lp.step = LoginStep::EnterApiKey;
                        lp.selected = 0;
                        lp.key_input.clear();
                    }
                    2 => {
                        lp.close();
                        app.status_message = Some(StatusMessage::info(
                            "run `dot login` from terminal for GitHub Copilot",
                        ));
                    }
                    _ => {}
                }
                InputAction::None
            }
            _ => InputAction::None,
        },
        LoginStep::SelectMethod => match key.code {
            KeyCode::Esc => {
                lp.step = LoginStep::SelectProvider;
                lp.selected = 0;
                InputAction::None
            }
            KeyCode::Up => {
                lp.up();
                InputAction::None
            }
            KeyCode::Down | KeyCode::Tab => {
                lp.down();
                InputAction::None
            }
            KeyCode::Enter => {
                let method = lp.selected;
                match method {
                    0 | 1 => {
                        let create_key = method == 1;
                        match crate::auth::oauth::generate_oauth_url(create_key) {
                            Ok((url, verifier)) => {
                                let _ = open::that(&url);
                                lp.oauth_url = Some(url);
                                lp.oauth_verifier = Some(verifier);
                                lp.oauth_create_key = create_key;
                                lp.code_input.clear();
                                lp.step = LoginStep::OAuthWaiting;
                            }
                            Err(e) => {
                                lp.close();
                                app.status_message =
                                    Some(StatusMessage::error(format!("oauth url: {e}")));
                            }
                        }
                        InputAction::None
                    }
                    2 => {
                        lp.step = LoginStep::EnterApiKey;
                        lp.key_input.clear();
                        InputAction::None
                    }
                    _ => InputAction::None,
                }
            }
            _ => InputAction::None,
        },
        LoginStep::EnterApiKey => match key.code {
            KeyCode::Esc => {
                lp.step = LoginStep::SelectProvider;
                lp.selected = 0;
                lp.key_input.clear();
                InputAction::None
            }
            KeyCode::Enter => {
                let key = lp.key_input.trim().to_string();
                if key.is_empty() {
                    return InputAction::None;
                }
                let provider = lp.provider.clone().unwrap_or_default();
                lp.close();
                InputAction::LoginSubmitApiKey { provider, key }
            }
            KeyCode::Backspace => {
                lp.key_input.pop();
                InputAction::None
            }
            KeyCode::Char(c) => {
                lp.key_input.push(c);
                InputAction::None
            }
            _ => InputAction::None,
        },
        LoginStep::OAuthWaiting => match key.code {
            KeyCode::Esc => {
                lp.step = LoginStep::SelectProvider;
                lp.selected = 0;
                lp.code_input.clear();
                lp.oauth_url = None;
                lp.oauth_verifier = None;
                InputAction::None
            }
            KeyCode::Enter => {
                let code = lp.code_input.trim().to_string();
                if code.is_empty() {
                    return InputAction::None;
                }
                let verifier = lp.oauth_verifier.clone().unwrap_or_default();
                let create_key = lp.oauth_create_key;
                lp.step = LoginStep::OAuthExchanging;
                InputAction::LoginOAuth {
                    provider: "anthropic".to_string(),
                    create_key,
                    code,
                    verifier,
                }
            }
            KeyCode::Backspace => {
                lp.code_input.pop();
                InputAction::None
            }
            KeyCode::Char(c) => {
                lp.code_input.push(c);
                InputAction::None
            }
            _ => InputAction::None,
        },
        LoginStep::OAuthExchanging => match key.code {
            KeyCode::Esc => {
                lp.close();
                InputAction::None
            }
            _ => InputAction::None,
        },
    }
}

pub(super) fn handle_welcome_screen(app: &mut App, key: KeyEvent) -> InputAction {
    match key.code {
        KeyCode::Up => {
            app.welcome_screen.up();
            InputAction::None
        }
        KeyCode::Down | KeyCode::Tab => {
            app.welcome_screen.down();
            InputAction::None
        }
        KeyCode::Enter => {
            if let Some(choice) = app.welcome_screen.confirm() {
                match choice {
                    crate::tui::widgets::WelcomeChoice::Login => {
                        app.login_popup.open();
                        app.login_popup.from_welcome = true;
                        InputAction::None
                    }
                    crate::tui::widgets::WelcomeChoice::UseEnvKeys => {
                        app.status_message = Some(crate::tui::app::StatusMessage::success(
                            "using environment keys",
                        ));
                        InputAction::None
                    }
                    crate::tui::widgets::WelcomeChoice::SetEnvVars => {
                        app.status_message = Some(crate::tui::app::StatusMessage::info(
                            "set ANTHROPIC_API_KEY or OPENAI_API_KEY in your shell",
                        ));
                        InputAction::None
                    }
                }
            } else {
                InputAction::None
            }
        }
        KeyCode::Esc => {
            app.welcome_screen.close();
            InputAction::None
        }
        _ => InputAction::None,
    }
}
