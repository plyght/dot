mod modes;
mod mouse;
mod popups;

use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::app::{App, AppMode};

pub use mouse::handle_mouse;

pub enum InputAction {
    AnswerQuestion(String),
    AnswerPermission(String),
    None,
    SendMessage(String),
    Quit,
    CancelStream,
    ScrollUp(u16),
    ScrollDown(u16),
    ScrollToTop,
    ScrollToBottom,
    ClearConversation,
    NewConversation,
    OpenModelSelector,
    OpenAgentSelector,
    ToggleAgent,
    OpenThinkingSelector,
    OpenSessionSelector,
    SelectModel { provider: String, model: String },
    SelectAgent { name: String },
    ResumeSession { id: String },
    SetThinkingLevel(u32),
    ToggleThinking,
    CycleThinkingLevel,
    TruncateToMessage(usize),
    ForkFromMessage(usize),
    LoadSkill { name: String },
    RunCustomCommand { name: String, args: String },
    OpenRenamePopup,
    RenameSession(String),
    ExportSession(Option<String>),
    OpenExternalEditor,
}

pub fn handle_paste(app: &mut App, text: String) -> InputAction {
    if app.vim_mode && app.mode != AppMode::Insert {
        return InputAction::None;
    }

    let trimmed = text.trim_end_matches('\n').to_string();
    if trimmed.is_empty() {
        return InputAction::None;
    }

    if crate::tui::app::is_image_path(trimmed.trim()) {
        let path = trimmed.trim().trim_matches('"').trim_matches('\'');
        match app.add_image_attachment(path) {
            Ok(()) => {}
            Err(e) => app.status_message = Some(crate::tui::app::StatusMessage::error(e)),
        }
        return InputAction::None;
    }

    app.handle_paste(trimmed);
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

    if app.rename_visible {
        return popups::handle_rename_popup(app, key);
    }

    if app.pending_question.is_some() {
        return popups::handle_question_popup(app, key);
    }

    if app.pending_permission.is_some() {
        return popups::handle_permission_popup(app, key);
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
