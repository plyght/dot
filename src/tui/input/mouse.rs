use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use crate::tui::app::{App, AppMode, ChipKind, InputChip};
use crate::tui::widgets::{PaletteEntryKind, ThinkingLevel};

use super::InputAction;
use super::popups;

fn rect_contains(r: ratatui::layout::Rect, col: u16, row: u16) -> bool {
    col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height
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

pub fn handle_mouse(app: &mut App, mouse: MouseEvent) -> InputAction {
    let col = mouse.column;
    let row = mouse.row;

    match mouse.kind {
        MouseEventKind::ScrollUp => {
            app.selection.clear();
            if app.model_selector.visible
                && let Some(popup) = app.layout.model_selector
                && rect_contains(popup, col, row)
            {
                app.model_selector.up();
                return InputAction::None;
            }
            if app.command_palette.visible
                && let Some(popup) = app.layout.command_palette
                && rect_contains(popup, col, row)
            {
                app.command_palette.up();
                return InputAction::None;
            }
            if app.file_picker.visible
                && let Some(popup) = app.layout.file_picker
                && rect_contains(popup, col, row)
            {
                app.file_picker.up();
                return InputAction::None;
            }
            InputAction::ScrollUp(3)
        }
        MouseEventKind::ScrollDown => {
            app.selection.clear();
            if app.model_selector.visible
                && let Some(popup) = app.layout.model_selector
                && rect_contains(popup, col, row)
            {
                app.model_selector.down();
                return InputAction::None;
            }
            if app.command_palette.visible
                && let Some(popup) = app.layout.command_palette
                && rect_contains(popup, col, row)
            {
                app.command_palette.down();
                return InputAction::None;
            }
            if app.file_picker.visible
                && let Some(popup) = app.layout.file_picker
                && rect_contains(popup, col, row)
            {
                app.file_picker.down();
                return InputAction::None;
            }
            InputAction::ScrollDown(3)
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if app.selection.anchor.is_some() && !app.selection.active {
                app.selection.clear();
            }

            if app.context_menu.visible {
                if let Some(popup) = app.layout.context_menu
                    && rect_contains(popup, col, row)
                {
                    let relative_row = row.saturating_sub(popup.y + 1) as usize;
                    app.context_menu.selected = relative_row.min(1);
                    if let Some((action, msg_idx)) = app.context_menu.confirm() {
                        return match action {
                            0 => InputAction::TruncateToMessage(msg_idx),
                            1 => InputAction::ForkFromMessage(msg_idx),
                            _ => InputAction::None,
                        };
                    }
                }
                app.context_menu.close();
                return InputAction::None;
            }

            if app.model_selector.visible
                && let Some(popup) = app.layout.model_selector
            {
                if !rect_contains(popup, col, row) {
                    app.model_selector.close();
                }
                return InputAction::None;
            }

            if app.agent_selector.visible
                && let Some(popup) = app.layout.agent_selector
            {
                if !rect_contains(popup, col, row) {
                    app.agent_selector.close();
                }
                return InputAction::None;
            }

            if app.help_popup.visible
                && let Some(popup) = app.layout.help_popup
            {
                if !rect_contains(popup, col, row) {
                    app.help_popup.close();
                }
                return InputAction::None;
            }

            if app.thinking_selector.visible
                && let Some(popup) = app.layout.thinking_selector
                && rect_contains(popup, col, row)
            {
                let relative_row = row.saturating_sub(popup.y + 1) as usize;
                if relative_row < ThinkingLevel::all().len() {
                    app.thinking_selector.selected = relative_row;
                    if let Some(level) = app.thinking_selector.confirm() {
                        let budget = level.budget_tokens();
                        app.thinking_budget = budget;
                        return InputAction::SetThinkingLevel(budget);
                    }
                }
            } else if app.thinking_selector.visible
                && let Some(popup) = app.layout.thinking_selector
            {
                if !rect_contains(popup, col, row) {
                    app.thinking_selector.close();
                }
                return InputAction::None;
            }

            if app.session_selector.visible
                && let Some(popup) = app.layout.session_selector
                && !rect_contains(popup, col, row)
            {
                app.session_selector.close();
                return InputAction::None;
            }

            if app.command_palette.visible
                && let Some(popup) = app.layout.command_palette
            {
                if rect_contains(popup, col, row) {
                    let relative_row = row.saturating_sub(popup.y) as usize;
                    if relative_row < app.command_palette.filtered.len() {
                        app.command_palette.selected = relative_row;
                        if let Some(entry) = app.command_palette.confirm() {
                            if entry.kind == PaletteEntryKind::Skill {
                                popups::place_skill_chip(app, &entry.name);
                                return InputAction::LoadSkill { name: entry.name };
                            }
                            app.input.clear();
                            app.cursor_pos = 0;
                            app.chips.clear();
                            return popups::execute_palette_entry(app, entry);
                        }
                    }
                    return InputAction::None;
                } else {
                    app.command_palette.close();
                    return InputAction::None;
                }
            }

            if app.file_picker.visible
                && let Some(popup) = app.layout.file_picker
            {
                if rect_contains(popup, col, row) {
                    let relative_row = row.saturating_sub(popup.y + 1) as usize;
                    if relative_row < app.file_picker.filtered.len() {
                        app.file_picker.selected = relative_row;
                        if let Some(entry) = app.file_picker.confirm() {
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
                    return InputAction::None;
                } else {
                    app.file_picker.close();
                    return InputAction::None;
                }
            }

            if rect_contains(app.layout.input, col, row) {
                if app.vim_mode {
                    app.mode = AppMode::Insert;
                }
                let inner_x = col.saturating_sub(app.layout.input.x + 3);
                let inner_y = row.saturating_sub(app.layout.input.y + 1);
                let target_offset =
                    compute_click_cursor_pos(&app.input, inner_x as usize, inner_y as usize);
                app.cursor_pos = target_offset;
                InputAction::None
            } else if rect_contains(app.layout.messages, col, row) {
                let content_y = app.layout.messages.y + 1;
                if row >= content_y {
                    let content_row = row - content_y;
                    let visual_row = app.scroll_offset + content_row;
                    if let Some(Some((msg_idx, tool_idx))) =
                        app.tool_line_map.get(visual_row as usize).copied()
                    {
                        if app.expanded_tool_calls.contains(&(msg_idx, tool_idx)) {
                            app.expanded_tool_calls.remove(&(msg_idx, tool_idx));
                        } else {
                            app.expanded_tool_calls.insert((msg_idx, tool_idx));
                        }
                        return InputAction::None;
                    }
                    let content_col = col.saturating_sub(app.layout.messages.x);
                    app.selection.start(content_col, visual_row);
                }
                if app.vim_mode && app.mode == AppMode::Insert && app.input.is_empty() {
                    app.mode = AppMode::Normal;
                }
                InputAction::None
            } else {
                InputAction::None
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if app.selection.active {
                let content_y = app.layout.messages.y + 1;
                let content_height = app.layout.messages.height.saturating_sub(1);
                let content_col = col.saturating_sub(app.layout.messages.x);
                let content_row = if row >= content_y {
                    (row - content_y).min(content_height.saturating_sub(1))
                } else {
                    0
                };
                let visual_row = app.scroll_offset + content_row;
                app.selection.update(content_col, visual_row);
            }
            InputAction::None
        }
        MouseEventKind::Up(MouseButton::Left) => {
            if app.selection.active {
                let content_y = app.layout.messages.y + 1;
                let content_height = app.layout.messages.height.saturating_sub(1);
                let content_col = col.saturating_sub(app.layout.messages.x);
                let content_row = if row >= content_y {
                    (row - content_y).min(content_height.saturating_sub(1))
                } else {
                    0
                };
                let visual_row = app.scroll_offset + content_row;
                app.selection.update(content_col, visual_row);
                app.selection.active = false;
                if !app.selection.is_empty_selection() {
                    if let Some(text) = app.extract_selected_text()
                        && !text.trim().is_empty()
                    {
                        crate::tui::app::copy_to_clipboard(&text);
                    }
                } else {
                    app.selection.clear();
                }
            }
            InputAction::None
        }
        MouseEventKind::Down(MouseButton::Right) => {
            if app.context_menu.visible {
                app.context_menu.close();
                return InputAction::None;
            }
            if rect_contains(app.layout.messages, col, row) && !app.is_streaming {
                let content_y = app.layout.messages.y + 1;
                if row >= content_y {
                    let visual_row = (app.scroll_offset + (row - content_y)) as usize;
                    if let Some(&msg_idx) = app.message_line_map.get(visual_row) {
                        app.context_menu.open(msg_idx, col, row);
                    }
                }
            }
            InputAction::None
        }
        _ => InputAction::None,
    }
}
