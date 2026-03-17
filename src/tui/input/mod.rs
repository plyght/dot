mod modes;
mod mouse;
mod popups;

use std::path::Path;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::app::{App, AppMode};

pub use mouse::handle_mouse;

fn path_exists(path: &str) -> bool {
    let resolved = if path.starts_with('~') {
        std::env::var("HOME")
            .map(|h| path.replacen('~', &h, 1))
            .unwrap_or_else(|_| path.to_string())
    } else {
        path.to_string()
    };
    Path::new(&resolved).exists()
}

pub enum InputAction {
    AnswerQuestion(String),
    AnswerPermission(String),
    None,
    SendMessage(String),
    Quit,
    CancelStream,
    ScrollUp(u32),
    ScrollDown(u32),
    ScrollToTop,
    ScrollToBottom,
    ClearConversation,
    NewConversation,
    OpenModelSelector,
    OpenAgentSelector,
    ToggleAgent,
    OpenThinkingSelector,
    OpenSessionSelector,
    SelectModel {
        provider: String,
        model: String,
    },
    SelectAgent {
        name: String,
    },
    ResumeSession {
        id: String,
    },
    SetThinkingLevel(u32),
    ToggleThinking,
    CycleThinkingLevel,
    TruncateToMessage(usize),
    ForkFromMessage(usize),
    RevertToMessage(usize),
    CopyMessage(usize),
    LoadSkill {
        name: String,
    },
    RunCustomCommand {
        name: String,
        args: String,
    },
    OpenRenamePopup,
    RenameSession(String),
    ExportSession(Option<String>),
    OpenExternalEditor,
    OpenLoginPopup,
    LoginSubmitApiKey {
        provider: String,
        key: String,
    },
    LoginOAuth {
        provider: String,
        create_key: bool,
        code: String,
        verifier: String,
    },
    AskAside {
        question: String,
    },
}

enum PasteItem {
    Path(String),
    Plain(String),
}

pub fn handle_paste(app: &mut App, text: String) -> InputAction {
    if app.login_popup.visible {
        let trimmed = text.trim().to_string();
        if !trimmed.is_empty() {
            match app.login_popup.step {
                crate::tui::widgets::LoginStep::OAuthWaiting => {
                    app.login_popup.code_input.push_str(&trimmed);
                }
                crate::tui::widgets::LoginStep::EnterApiKey => {
                    app.login_popup.key_input.push_str(&trimmed);
                }
                _ => {}
            }
        }
        return InputAction::None;
    }

    if app.vim_mode && app.mode != AppMode::Insert {
        return InputAction::None;
    }

    let trimmed = text.trim_end_matches('\n');
    if trimmed.is_empty() {
        return InputAction::None;
    }

    let lines: Vec<&str> = trimmed
        .split('\n')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let mut items: Vec<PasteItem> = Vec::new();
    for line in &lines {
        if let Some(path) = crate::tui::app::normalize_paste_path(line)
            && path_exists(&path)
        {
            items.push(PasteItem::Path(path));
            continue;
        }
        items.push(PasteItem::Plain((*line).to_string()));
    }

    let mut plain_buf: Vec<String> = Vec::new();
    for item in items {
        match item {
            PasteItem::Path(path) => {
                if !plain_buf.is_empty() {
                    app.handle_paste(plain_buf.join("\n"));
                    plain_buf.clear();
                }
                if crate::tui::app::is_image_path(&path) {
                    if let Err(e) = app.add_image_attachment(&path) {
                        app.status_message = Some(crate::tui::app::StatusMessage::error(e));
                    }
                } else {
                    app.insert_file_reference(&path);
                }
            }
            PasteItem::Plain(s) => plain_buf.push(s),
        }
    }
    if !plain_buf.is_empty() {
        app.handle_paste(plain_buf.join("\n"));
    }

    InputAction::None
}

pub fn handle_key(app: &mut App, key: KeyEvent) -> InputAction {
    if app.selection.anchor.is_some() {
        app.selection.clear();
    }

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
        if app.file_picker.visible {
            app.file_picker.close();
            return InputAction::None;
        }
        if app.thinking_selector.visible {
            app.thinking_selector.close();
            return InputAction::None;
        }
        if app.session_selector.visible {
            app.session_selector.close();
            return InputAction::None;
        }
        if app.help_popup.visible {
            app.help_popup.close();
            return InputAction::None;
        }
        if app.is_streaming {
            return InputAction::CancelStream;
        }
        if !app.input.is_empty() || !app.attachments.is_empty() {
            app.input.clear();
            app.cursor_pos = 0;
            app.paste_blocks.clear();
            app.attachments.clear();
            return InputAction::None;
        }
        return InputAction::Quit;
    }

    if key.code == KeyCode::Esc && app.is_streaming {
        let now = Instant::now();
        if let Some(hint_until) = app.esc_hint_until
            && now < hint_until
        {
            app.esc_hint_until = None;
            app.last_escape_time = None;
            return InputAction::CancelStream;
        }
        app.esc_hint_until = Some(now + Duration::from_secs(3));
        app.last_escape_time = Some(now);
        return InputAction::None;
    }

    if app.model_selector.visible {
        return popups::handle_model_selector(app, key);
    }

    if app.agent_selector.visible {
        return popups::handle_agent_selector(app, key);
    }

    if app.thinking_selector.visible {
        return popups::handle_thinking_selector(app, key);
    }

    if app.session_selector.visible {
        return popups::handle_session_selector(app, key);
    }

    if app.help_popup.visible {
        if matches!(key.code, KeyCode::Esc | KeyCode::Enter) {
            app.help_popup.close();
        }
        return InputAction::None;
    }

    if app.aside_popup.visible {
        return popups::handle_aside_popup(app, key);
    }

    if app.rename_visible {
        return popups::handle_rename_popup(app, key);
    }

    if app.pending_question.is_some() {
        return popups::handle_question_popup(app, key);
    }

    if app.pending_permission.is_some() {
        return popups::handle_permission_popup(app, key);
    }

    if app.welcome_screen.visible {
        return popups::handle_welcome_screen(app, key);
    }

    if app.login_popup.visible {
        return popups::handle_login_popup(app, key);
    }

    if app.context_menu.visible {
        return popups::handle_context_menu(app, key);
    }

    if app.command_palette.visible {
        return popups::handle_command_palette(app, key);
    }

    if app.file_picker.visible {
        return popups::handle_file_picker(app, key);
    }

    if key.modifiers.contains(KeyModifiers::CONTROL)
        && key.code == KeyCode::Char('e')
        && (!app.vim_mode || app.mode == AppMode::Insert)
    {
        return InputAction::OpenExternalEditor;
    }

    if app.vim_mode {
        match app.mode {
            AppMode::Normal => modes::handle_normal(app, key),
            AppMode::Insert => modes::handle_insert(app, key),
        }
    } else {
        modes::handle_simple(app, key)
    }
}
