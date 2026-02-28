use std::collections::HashSet;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::style::Color;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
};

use crate::agent::TodoStatus;
use crate::tui::app::{App, AppMode, ChatMessage, InputChip, StatusLevel};
use crate::tui::markdown;
use crate::tui::theme::Theme;
use crate::tui::ui_popups;
use crate::tui::ui_tools;

fn is_compact(w: u16) -> bool {
    w < 60
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let input_height = app.input_height(frame.area().width);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(input_height),
            Constraint::Length(1),
        ])
        .split(frame.area());

    app.layout.header = chunks[0];
    app.layout.messages = chunks[1];
    app.layout.input = chunks[2];
    app.layout.status = chunks[3];

    draw_header(frame, app, chunks[0]);
    draw_messages(frame, app, chunks[1]);
    draw_input(frame, app, chunks[2]);
    draw_status(frame, app, chunks[3]);

    if app.model_selector.visible {
        ui_popups::draw_model_selector(frame, app);
    }

    if app.agent_selector.visible {
        ui_popups::draw_agent_selector(frame, app);
    }

    if app.thinking_selector.visible {
        ui_popups::draw_thinking_selector(frame, app);
    }

    if app.command_palette.visible {
        ui_popups::draw_command_palette(frame, app, chunks[2]);
    }

    if app.file_picker.visible {
        ui_popups::draw_file_picker(frame, app, chunks[2]);
    }

    if app.session_selector.visible {
        ui_popups::draw_session_selector(frame, app);
    }

    if app.help_popup.visible {
        ui_popups::draw_help_popup(frame, app);
    }

    if app.context_menu.visible {
        ui_popups::draw_context_menu(frame, app);
    }

    if app.pending_question.is_some() {
        ui_popups::draw_question_popup(frame, app);
    }

    if app.pending_permission.is_some() {
        ui_popups::draw_permission_popup(frame, app);
    }

    if app.rename_visible {
        ui_popups::draw_rename_popup(frame, app);
    }
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let compact = is_compact(area.width);
    let sep = Span::styled(" \u{00b7} ", app.theme.separator);

    let title_text = app
        .conversation_title
        .as_deref()
        .unwrap_or("new conversation");

    let model_short = display_model(&app.model_name);
    let mut right_spans: Vec<Span<'static>> = Vec::new();

    let model_display: String = if compact {
        let s = model_short;
        if s.chars().count() > 14 {
            let t: String = s.chars().take(13).collect();
            format!("{}\u{2026}", t)
        } else {
            s
        }
    } else {
        model_short
    };

    if !compact || area.width >= 40 {
        right_spans.push(Span::styled(model_display, app.theme.dim));
    }

    if app.thinking_budget > 0 && !compact {
        right_spans.push(sep.clone());
        right_spans.push(Span::styled(
            format!("\u{25c7}{}", app.thinking_level().label()),
            app.theme.thinking,
        ));
    }

    if app.vim_mode {
        right_spans.push(Span::raw(" "));
        let (normal_label, insert_label) = if compact {
            (" N ", " I ")
        } else {
            (" NORMAL ", " INSERT ")
        };
        right_spans.push(match app.mode {
            AppMode::Normal => Span::styled(
                normal_label,
                Style::default()
                    .fg(app.theme.mode_normal_fg)
                    .bg(app.theme.mode_normal_bg),
            ),
            AppMode::Insert => Span::styled(
                insert_label,
                Style::default()
                    .fg(app.theme.mode_insert_fg)
                    .bg(app.theme.mode_insert_bg),
            ),
        });
    }

    if let Some(elapsed) = app.streaming_elapsed_secs() {
        right_spans.push(Span::styled(
            format!(" {}", format_elapsed(elapsed)),
            app.theme.thinking,
        ));
    }

    let right_width: usize = right_spans.iter().map(|s| s.content.chars().count()).sum();

    let show_agent = !compact && !app.agent_name.is_empty() && app.agent_name != "default";

    let agent_width = if show_agent {
        3 + app.agent_name.chars().count()
    } else {
        0
    };

    let max_title = (area.width as usize).saturating_sub(right_width + agent_width + 3);
    let display_title = if title_text.chars().count() > max_title && max_title > 2 {
        let t: String = title_text
            .chars()
            .take(max_title.saturating_sub(1))
            .collect();
        format!("{}\u{2026}", t)
    } else {
        title_text.to_string()
    };

    let mut left_spans = vec![Span::styled(
        format!(" {}", display_title),
        Style::default().fg(app.theme.accent),
    )];

    if show_agent {
        left_spans.push(sep.clone());
        left_spans.push(Span::styled(
            app.agent_name.clone(),
            if app.agent_name == "plan" {
                app.theme.dim
            } else {
                app.theme.status_bar
            },
        ));
    }

    let left_width: usize = left_spans.iter().map(|s| s.content.chars().count()).sum();
    let gap = (area.width as usize).saturating_sub(left_width + right_width + 1);

    let mut spans = left_spans;
    spans.push(Span::raw(" ".repeat(gap)));
    spans.extend(right_spans);

    let header = Line::from(spans);
    frame.render_widget(Paragraph::new(header), area);
}

fn draw_messages(frame: &mut Frame, app: &mut App, area: Rect) {
    let lpad: u16 = if is_compact(area.width) { 0 } else { 1 };
    let inner = Rect {
        x: area.x + lpad,
        y: area.y,
        width: area.width.saturating_sub(lpad + 2),
        height: area.height,
    };

    let mut all_lines: Vec<Line<'static>> = Vec::new();
    let mut line_to_msg: Vec<usize> = Vec::new();
    let mut line_to_tool: Vec<Option<(usize, usize)>> = Vec::new();

    for (msg_idx, msg) in app.messages.iter().enumerate() {
        let before = all_lines.len();
        render_message(
            msg,
            msg_idx,
            &MessageRenderCtx {
                theme: &app.theme,
                thinking_expanded: app.thinking_expanded,
                inner_width: inner.width,
                render_width: area.width,
                expanded_tool_calls: &app.expanded_tool_calls,
            },
            &mut all_lines,
            &mut line_to_tool,
        );
        let after = all_lines.len();
        for _ in before..after {
            line_to_msg.push(msg_idx);
        }
    }

    if !app.todos.is_empty() {
        let pad = if inner.width < 55 { "  " } else { "    " };
        all_lines.push(Line::from(""));
        line_to_msg.push(app.messages.len().saturating_sub(1));
        line_to_tool.push(None);
        let done = app
            .todos
            .iter()
            .filter(|t| t.status == TodoStatus::Completed)
            .count();
        let total = app.todos.len();
        all_lines.push(Line::from(vec![
            Span::styled(format!("{}  ", pad), app.theme.dim),
            Span::styled(format!("{}/{} ", done, total), app.theme.dim),
            Span::styled("tasks", app.theme.dim),
        ]));
        line_to_msg.push(app.messages.len().saturating_sub(1));
        line_to_tool.push(None);
        for todo in &app.todos {
            let (icon, style) = match todo.status {
                TodoStatus::Completed => ("\u{25c6}", app.theme.tool_success),
                TodoStatus::InProgress => ("\u{25c8}", Style::default().fg(app.theme.accent)),
                TodoStatus::Pending => ("\u{25c7}", app.theme.dim),
            };
            all_lines.push(Line::from(vec![
                Span::styled(format!("{}  {} ", pad, icon), style),
                Span::styled(todo.content.clone(), style),
            ]));
            line_to_msg.push(app.messages.len().saturating_sub(1));
            line_to_tool.push(None);
        }
    }

    if app.is_streaming {
        let before_stream = all_lines.len();
        ui_tools::render_streaming_state(app, inner.width, area.width, &mut all_lines);
        let stream_msg_idx = app.messages.len().saturating_sub(1).max(0);
        for _ in before_stream..all_lines.len() {
            line_to_msg.push(stream_msg_idx);
            line_to_tool.push(None);
        }
    }

    if let Some(ref status) = app.status_message
        && !status.expired()
    {
        let (icon, style) = match status.level {
            StatusLevel::Error => ("\u{2718}", app.theme.error),
            StatusLevel::Info => ("\u{25cb}", app.theme.dim),
            StatusLevel::Success => ("\u{2714}", app.theme.tool_success),
        };
        all_lines.push(Line::from(""));
        line_to_tool.push(None);
        all_lines.push(Line::from(vec![
            Span::styled(format!("    {} ", icon), style),
            Span::styled(status.text.clone(), style),
        ]));
        line_to_tool.push(None);
    }

    if all_lines.is_empty() {
        let empty_lines = ui_popups::draw_empty_state(app, inner.width);
        for _ in &empty_lines {
            line_to_tool.push(None);
        }
        all_lines.extend(empty_lines);
    }

    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(app.theme.border);
    let content_area = block.inner(area);
    let wrap_width = content_area.width;

    let paragraph = Paragraph::new(all_lines.clone())
        .block(block.clone())
        .wrap(Wrap { trim: false });
    let total_visual = paragraph.line_count(area.width) as u32;

    let visible = content_area.height as u32;
    app.max_scroll = total_visual.saturating_sub(visible).min(u16::MAX as u32) as u16;
    if app.follow_bottom {
        app.scroll_position = app.max_scroll as f64;
        app.scroll_velocity = 0.0;
        app.scroll_offset = app.max_scroll;
    } else if app.scroll_position > app.max_scroll as f64 {
        app.scroll_position = app.max_scroll as f64;
        app.scroll_velocity = 0.0;
    }

    app.content_width = area.width;
    app.visual_lines = compute_visual_lines(&all_lines, wrap_width);
    app.message_line_map = expand_line_to_msg(&all_lines, &line_to_msg, wrap_width);
    app.tool_line_map = expand_line_to_tool(&all_lines, &line_to_tool, wrap_width);

    let paragraph = Paragraph::new(all_lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset, 0));

    frame.render_widget(paragraph, area);

    let frac = app.scroll_frac();
    if frac > 0.05 && app.scroll_offset < app.max_scroll {
        let content_y = area.y + 1;
        let buf = frame.buffer_mut();
        let alpha = frac as f32;
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut(Position::new(x, content_y)) {
                cell.set_style(Style::default().fg(blend_color(cell.fg, app.theme.bg, alpha)));
            }
        }
    }

    render_selection_highlight(frame, app, area);

    if app.max_scroll > 0 {
        let scrollbar_area = Rect {
            x: area.x + area.width.saturating_sub(1),
            y: area.y + 1,
            width: 1,
            height: area.height.saturating_sub(1),
        };
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .track_symbol(Some("\u{2502}"))
            .thumb_symbol("\u{2503}")
            .begin_symbol(None)
            .end_symbol(None)
            .track_style(app.theme.scrollbar_track)
            .thumb_style(app.theme.scrollbar_thumb);

        let mut state =
            ScrollbarState::new(app.max_scroll as usize).position(app.scroll_offset as usize);
        frame.render_stateful_widget(scrollbar, scrollbar_area, &mut state);
    }
}

struct MessageRenderCtx<'a> {
    theme: &'a Theme,
    thinking_expanded: bool,
    inner_width: u16,
    render_width: u16,
    expanded_tool_calls: &'a HashSet<(usize, usize)>,
}

fn render_message(
    msg: &ChatMessage,
    msg_idx: usize,
    ctx: &MessageRenderCtx<'_>,
    lines: &mut Vec<Line<'static>>,
    line_to_tool: &mut Vec<Option<(usize, usize)>>,
) {
    let compact = ctx.inner_width < 55;
    let body_indent: &str = if compact { "  " } else { "    " };
    let body_indent_cols: u16 = if compact { 2 } else { 4 };

    lines.push(Line::from(""));
    line_to_tool.push(None);

    match msg.role.as_str() {
        "user" => {
            let (marker, cont) = if compact {
                (" \u{203a} ", "   ")
            } else {
                ("  \u{203a} ", "    ")
            };
            let mut content_lines = msg.content.lines();
            if let Some(first) = content_lines.next() {
                line_to_tool.push(None);
                lines.push(Line::from(vec![
                    Span::styled(
                        marker,
                        Style::default()
                            .fg(ctx.theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "you ",
                        Style::default()
                            .fg(ctx.theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(first.to_string(), ctx.theme.user_text),
                ]));
            }
            for text_line in content_lines {
                line_to_tool.push(None);
                lines.push(Line::from(Span::styled(
                    format!("{}{}", cont, text_line),
                    ctx.theme.user_text,
                )));
            }
        }
        "compact" => {
            let pad = if compact { " " } else { "  " };
            for text_line in msg.content.lines() {
                line_to_tool.push(None);
                lines.push(Line::from(vec![
                    Span::styled(pad, ctx.theme.thinking),
                    Span::styled(text_line.to_string(), ctx.theme.dim),
                ]));
            }
        }
        _ => {
            let (diamond, diamond_sp) = if compact {
                (" \u{25c6}", " \u{25c6} ")
            } else {
                ("  \u{25c6}", "  \u{25c6} ")
            };
            let model_label = msg.model.as_deref().map(display_model).unwrap_or_default();
            if model_label.is_empty() {
                line_to_tool.push(None);
                lines.push(Line::from(Span::styled(
                    diamond,
                    Style::default().fg(ctx.theme.accent),
                )));
            } else {
                line_to_tool.push(None);
                lines.push(Line::from(vec![
                    Span::styled(diamond_sp, Style::default().fg(ctx.theme.accent)),
                    Span::styled(model_label, ctx.theme.dim),
                ]));
            }
            if let Some(ref thinking) = msg.thinking {
                render_thinking_block(
                    thinking,
                    ctx.thinking_expanded,
                    ctx.theme,
                    compact,
                    lines,
                    line_to_tool,
                );
            }
            if let Some(ref segments) = msg.segments {
                let mut prev_was_tool = false;
                let mut tool_idx = 0;
                for seg in segments {
                    match seg {
                        crate::tui::tools::StreamSegment::Text(t) => {
                            if prev_was_tool {
                                lines.push(Line::from(""));
                                line_to_tool.push(None);
                            }
                            let md_lines = markdown::render_markdown(
                                t,
                                ctx.theme,
                                ctx.inner_width.saturating_sub(body_indent_cols),
                            );
                            for line in md_lines {
                                let bg = line.spans.first().and_then(|s| s.style.bg);
                                let indent_style =
                                    bg.map(|c| Style::default().bg(c)).unwrap_or_default();
                                let mut padded = vec![Span::styled(body_indent, indent_style)];
                                padded.extend(line.spans);
                                if let Some(bg_color) = bg {
                                    let used: usize =
                                        padded.iter().map(|s| s.content.chars().count()).sum();
                                    let target = ctx.render_width as usize;
                                    if used < target {
                                        padded.push(Span::styled(
                                            " ".repeat(target - used),
                                            Style::default().bg(bg_color),
                                        ));
                                    }
                                }
                                lines.push(Line::from(padded));
                                line_to_tool.push(None);
                            }
                            prev_was_tool = false;
                        }
                        crate::tui::tools::StreamSegment::ToolCall(tc) => {
                            if !prev_was_tool && !lines.is_empty() {
                                lines.push(Line::from(""));
                                line_to_tool.push(None);
                            }
                            ui_tools::render_tool_calls_compact(
                                std::slice::from_ref(tc),
                                ctx.theme,
                                compact,
                                lines,
                                Some(line_to_tool),
                                msg_idx,
                                |_| ctx.expanded_tool_calls.contains(&(msg_idx, tool_idx)),
                            );
                            tool_idx += 1;
                            prev_was_tool = true;
                        }
                    }
                }
            } else {
                if !msg.tool_calls.is_empty() && msg.content.is_empty() {
                    ui_tools::render_tool_calls(
                        &msg.tool_calls,
                        ctx.theme,
                        compact,
                        lines,
                        Some(line_to_tool),
                        msg_idx,
                        |i| ctx.expanded_tool_calls.contains(&(msg_idx, i)),
                    );
                }
                let md_lines = markdown::render_markdown(
                    &msg.content,
                    ctx.theme,
                    ctx.inner_width.saturating_sub(body_indent_cols),
                );
                for line in md_lines {
                    let bg = line.spans.first().and_then(|s| s.style.bg);
                    let indent_style = bg.map(|c| Style::default().bg(c)).unwrap_or_default();
                    let mut padded = vec![Span::styled(body_indent, indent_style)];
                    padded.extend(line.spans);
                    if let Some(bg_color) = bg {
                        let used: usize = padded.iter().map(|s| s.content.chars().count()).sum();
                        let target = ctx.render_width as usize;
                        if used < target {
                            padded.push(Span::styled(
                                " ".repeat(target - used),
                                Style::default().bg(bg_color),
                            ));
                        }
                    }
                    lines.push(Line::from(padded));
                    line_to_tool.push(None);
                }
                if !msg.tool_calls.is_empty() && !msg.content.is_empty() {
                    ui_tools::render_tool_calls_compact(
                        &msg.tool_calls,
                        ctx.theme,
                        compact,
                        lines,
                        Some(line_to_tool),
                        msg_idx,
                        |i| ctx.expanded_tool_calls.contains(&(msg_idx, i)),
                    );
                }
            }
        }
    }
}

fn render_thinking_block(
    thinking: &str,
    expanded: bool,
    theme: &crate::tui::theme::Theme,
    compact: bool,
    lines: &mut Vec<Line<'static>>,
    line_to_tool: &mut Vec<Option<(usize, usize)>>,
) {
    let pad = if compact { "  " } else { "    " };
    let word_count = thinking.split_whitespace().count();
    if expanded {
        line_to_tool.push(None);
        lines.push(Line::from(vec![
            Span::styled(format!("{}\u{25be} ", pad), theme.thinking),
            Span::styled(
                "thinking",
                Style::default()
                    .fg(theme.muted_fg)
                    .add_modifier(Modifier::ITALIC),
            ),
            Span::styled(format!(" \u{00b7} {}w", word_count), theme.dim),
        ]));
        for text_line in thinking.lines() {
            line_to_tool.push(None);
            lines.push(Line::from(vec![
                Span::styled(format!("{}\u{2502} ", pad), theme.thinking),
                Span::styled(
                    text_line.to_string(),
                    Style::default()
                        .fg(theme.muted_fg)
                        .add_modifier(Modifier::ITALIC),
                ),
            ]));
        }
        line_to_tool.push(None);
        lines.push(Line::from(Span::styled(pad.to_string(), theme.thinking)));
    } else {
        line_to_tool.push(None);
        let hint = if word_count > 10 {
            format!(" \u{00b7} {}w", word_count)
        } else {
            String::new()
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{}\u{25b8} ", pad), theme.thinking),
            Span::styled(
                "thinking",
                Style::default()
                    .fg(theme.muted_fg)
                    .add_modifier(Modifier::ITALIC),
            ),
            Span::styled(hint, theme.dim),
            Span::styled("  [t]", theme.dim),
        ]));
    }
}

fn chip_styled_spans(
    text: &str,
    byte_offset: usize,
    chips: &[InputChip],
    accent: Color,
) -> Vec<Span<'static>> {
    let chip_style = Style::default().fg(accent).add_modifier(Modifier::BOLD);
    let end = byte_offset + text.len();
    let mut spans = Vec::new();
    let mut pos = byte_offset;
    let mut sorted: Vec<&InputChip> = chips
        .iter()
        .filter(|c| c.start < end && c.end > byte_offset)
        .collect();
    sorted.sort_by_key(|c| c.start);
    for chip in sorted {
        let cs = chip.start.max(byte_offset);
        let ce = chip.end.min(end);
        if cs > pos {
            let s = pos - byte_offset;
            let e = cs - byte_offset;
            spans.push(Span::raw(text[s..e].to_string()));
        }
        let s = cs - byte_offset;
        let e = ce - byte_offset;
        spans.push(Span::styled(text[s..e].to_string(), chip_style));
        pos = ce;
    }
    if pos < end {
        let s = pos - byte_offset;
        spans.push(Span::raw(text[s..].to_string()));
    }
    if spans.is_empty() {
        spans.push(Span::raw(text.to_string()));
    }
    spans
}

fn draw_input(frame: &mut Frame, app: &App, area: Rect) {
    let can_edit = !app.vim_mode || app.mode == AppMode::Insert;
    let has_input = !app.input.is_empty() || !app.attachments.is_empty();

    let border_style = if can_edit && !app.is_streaming {
        Style::default().fg(app.theme.accent)
    } else if can_edit && app.is_streaming {
        Style::default().fg(app.theme.muted_fg)
    } else {
        app.theme.border
    };
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(border_style);
    let inner = block.inner(area);

    let (prompt, prompt_style) = if app.is_streaming && can_edit {
        ("\u{25c7} ", Style::default().fg(app.theme.accent))
    } else if can_edit {
        (
            "\u{25c6} ",
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        ("\u{25c7} ", Style::default().fg(app.theme.muted_fg))
    };

    let display_lines: Vec<Line<'static>> = if app.is_streaming && !has_input {
        let pulse = ["\u{25c7}", "\u{25c6}"];
        let idx = (app.tick_count / 20 % pulse.len() as u64) as usize;

        let dot_count = ((app.tick_count / 16) % 4) as usize;
        let dots: String = ".".repeat(dot_count);
        let mut spans = vec![
            Span::styled(
                format!("  {} ", pulse[idx]),
                Style::default().fg(app.theme.accent),
            ),
            Span::styled(format!("generating{:<3}", dots), app.theme.dim),
        ];
        if let Some(elapsed) = app.streaming_elapsed_secs() {
            spans.push(Span::styled(
                format!(" \u{00b7} {}", format_elapsed(elapsed)),
                app.theme.dim,
            ));
        }
        if !app.message_queue.is_empty() {
            spans.push(Span::styled(
                format!(" \u{00b7} {} queued", app.message_queue.len()),
                Style::default().fg(app.theme.accent),
            ));
        }
        vec![Line::from(spans)]
    } else if !has_input {
        vec![Line::from(vec![
            Span::styled(prompt, prompt_style),
            Span::styled("message or /help", app.theme.dim),
        ])]
    } else {
        let mut lines = Vec::new();
        if !app.attachments.is_empty() {
            let att_display: Vec<String> = app
                .attachments
                .iter()
                .map(|a| {
                    std::path::Path::new(&a.path)
                        .file_name()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_else(|| a.path.clone())
                })
                .collect();
            lines.push(Line::from(vec![
                Span::styled(prompt, prompt_style),
                Span::styled(
                    format!("\u{1f4ce} {}", att_display.join(", ")),
                    app.theme.dim,
                ),
            ]));
        }
        let display = app.display_input();
        let use_chips = app.paste_blocks.is_empty() && !app.chips.is_empty();
        if display.is_empty() && !app.attachments.is_empty() {
            if lines.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled(prompt, prompt_style),
                    Span::styled("add a message or press enter", app.theme.dim),
                ]));
            }
        } else if use_chips {
            let mut byte_offset: usize = 0;
            for (i, line) in app.input.split('\n').enumerate() {
                let mut spans = if i == 0 && app.attachments.is_empty() {
                    vec![Span::styled(prompt, prompt_style)]
                } else {
                    vec![Span::raw("  ")]
                };
                spans.extend(chip_styled_spans(
                    line,
                    byte_offset,
                    &app.chips,
                    app.theme.accent,
                ));
                if i == 0
                    && app.attachments.is_empty()
                    && app.is_streaming
                    && !app.message_queue.is_empty()
                {
                    spans.push(Span::styled(
                        format!(" ({} queued)", app.message_queue.len()),
                        app.theme.dim,
                    ));
                }
                lines.push(Line::from(spans));
                byte_offset += line.len() + 1;
            }
            if app.input.ends_with('\n') {
                lines.push(Line::from(Span::raw("  ")));
            }
        } else {
            let offset = if app.attachments.is_empty() { 0 } else { 1 };
            for (i, line) in display.lines().enumerate() {
                if i == 0 && offset == 0 {
                    let mut spans = vec![
                        Span::styled(prompt, prompt_style),
                        Span::raw(line.to_string()),
                    ];
                    if app.is_streaming && !app.message_queue.is_empty() {
                        spans.push(Span::styled(
                            format!(" ({} queued)", app.message_queue.len()),
                            app.theme.dim,
                        ));
                    }
                    lines.push(Line::from(spans));
                } else {
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::raw(line.to_string()),
                    ]));
                }
            }
            if display.ends_with('\n') {
                lines.push(Line::from(Span::raw("  ")));
            }
        }
        lines
    };

    let paragraph = Paragraph::new(display_lines).wrap(Wrap { trim: false });
    frame.render_widget(block, area);
    frame.render_widget(paragraph, inner);
    if can_edit && !app.model_selector.visible {
        let blink_on = (app.tick_count / 32).is_multiple_of(2);
        if blink_on {
            let (cx, cy) = cursor_position(&app.input, app.cursor_pos, inner);
            if cy < inner.y + inner.height {
                frame.set_cursor_position((cx, cy));
            }
        }
    }
}

fn cursor_position(input: &str, byte_pos: usize, area: Rect) -> (u16, u16) {
    let before = &input[..byte_pos.min(input.len())];
    let width = area.width as usize;
    if width == 0 {
        return (area.x, area.y);
    }
    let prefix_w: usize = 2;
    let mut visual_row: usize = 0;
    let mut segments = before.split('\n').peekable();
    while let Some(seg) = segments.next() {
        let char_count = seg.chars().count();
        let total = prefix_w + char_count;
        if segments.peek().is_none() {
            let extra_rows = total / width;
            let col = total % width;
            visual_row += extra_rows;
            return (area.x + col as u16, area.y + visual_row as u16);
        }
        let rows = if total == 0 { 1 } else { total.div_ceil(width) };
        visual_row += rows;
    }
    (area.x + prefix_w as u16, area.y + visual_row as u16)
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let compact = is_compact(area.width);

    let token_text = if compact {
        format!(
            " {}i\u{00b7}{}o",
            format_tokens(app.usage.input_tokens),
            format_tokens(app.usage.output_tokens),
        )
    } else {
        format!(
            " {}in \u{00b7} {}out",
            format_tokens(app.usage.input_tokens),
            format_tokens(app.usage.output_tokens),
        )
    };

    let mut left_spans: Vec<Span<'static>> = vec![Span::styled(token_text, app.theme.status_bar)];

    if app.usage.total_cost > 0.0 {
        left_spans.push(Span::styled(
            format!(" \u{00b7} ${:.2}", app.usage.total_cost),
            app.theme.cost,
        ));
    }

    if !app.follow_bottom && app.is_streaming {
        let new_label = if compact {
            " \u{2193}"
        } else {
            " \u{2193} new content"
        };
        left_spans.push(Span::styled(
            new_label,
            Style::default().fg(app.theme.accent),
        ));
    }

    if !app.message_queue.is_empty() {
        let q_label = if compact {
            format!(" {}q", app.message_queue.len())
        } else {
            format!(" \u{00b7} {} queued", app.message_queue.len())
        };
        left_spans.push(Span::styled(q_label, Style::default().fg(app.theme.accent)));
    }

    let left_width: usize = left_spans.iter().map(|s| s.content.chars().count()).sum();

    let context_indicator: Vec<Span<'static>> =
        if app.context_window > 0 && app.last_input_tokens > 0 {
            let pct = (app.last_input_tokens as f64 / app.context_window as f64 * 100.0).min(100.0);
            let bar_width: usize = if compact { 6 } else { 10 };
            let filled = ((pct / 100.0) * bar_width as f64).round() as usize;
            let empty = bar_width.saturating_sub(filled);
            let bar_color = if pct > 80.0 {
                Color::Rgb(243, 139, 168)
            } else if pct > 60.0 {
                Color::Rgb(249, 226, 175)
            } else {
                Color::Rgb(88, 91, 112)
            };
            vec![
                Span::styled("\u{2590}", Style::default().fg(app.theme.code_bg)),
                Span::styled("\u{2588}".repeat(filled), Style::default().fg(bar_color)),
                Span::styled(
                    "\u{2591}".repeat(empty),
                    Style::default().fg(app.theme.code_bg),
                ),
                Span::styled("\u{258c}", Style::default().fg(app.theme.code_bg)),
                Span::styled(format!(" {:.0}% ", pct), app.theme.dim),
            ]
        } else {
            vec![]
        };

    let hint = if app.model_selector.visible
        || app.agent_selector.visible
        || app.thinking_selector.visible
        || app.session_selector.visible
        || app.help_popup.visible
    {
        if compact {
            "\u{2191}\u{2193} \u{23ce} esc "
        } else {
            "\u{2191}\u{2193} enter esc "
        }
    } else if app.is_streaming {
        let esc_active = app
            .esc_hint_until
            .map(|t| std::time::Instant::now() < t)
            .unwrap_or(false);
        if esc_active {
            if compact {
                "esc to cancel "
            } else {
                "press esc again to cancel "
            }
        } else {
            "esc \u{00b7} ^C "
        }
    } else if app.vim_mode && app.mode == AppMode::Normal {
        "i j/k q "
    } else if compact {
        "? "
    } else {
        "? help "
    };

    let ctx_width: usize = context_indicator
        .iter()
        .map(|s| s.content.chars().count())
        .sum();
    let right_width = ctx_width + hint.len();
    let padding = (area.width as usize).saturating_sub(left_width + right_width);

    let mut line_spans = left_spans;
    line_spans.push(Span::raw(" ".repeat(padding)));
    line_spans.extend(context_indicator);
    line_spans.push(Span::styled(hint.to_string(), app.theme.dim));

    frame.render_widget(Paragraph::new(Line::from(line_spans)), area);
}

pub fn format_elapsed(secs: f64) -> String {
    if secs < 1.0 {
        "<1s".to_string()
    } else if secs < 60.0 {
        format!("{:.0}s", secs)
    } else {
        let m = (secs / 60.0).floor() as u32;
        let s = (secs % 60.0).floor() as u32;
        format!("{}m{}s", m, s)
    }
}

pub fn display_model(model: &str) -> String {
    let formatted = format_model_name(model);
    if formatted.chars().count() <= 30 {
        return formatted;
    }
    let truncated: String = formatted.chars().take(29).collect();
    format!("{}\u{2026}", truncated)
}

fn format_model_name(model: &str) -> String {
    let base = if let Some(pos) = model.rfind('-') {
        let suffix = &model[pos + 1..];
        if suffix.len() == 8 && suffix.chars().all(|c| c.is_ascii_digit()) {
            &model[..pos]
        } else {
            model
        }
    } else {
        model
    };

    let parts: Vec<&str> = base.split('-').collect();
    let mut result: Vec<String> = Vec::new();
    let mut i = 0;

    while i < parts.len() {
        let part = parts[i];

        if part.eq_ignore_ascii_case("gpt") {
            result.push("GPT".into());
        } else if part.eq_ignore_ascii_case("claude") {
            result.push("Claude".into());
        } else if part.eq_ignore_ascii_case("latest") {
            // skip
        } else if part.len() >= 2
            && part.as_bytes()[0].eq_ignore_ascii_case(&b'o')
            && part[1..].chars().all(|c| c.is_ascii_digit())
        {
            result.push(part.to_lowercase());
        } else if part.chars().all(|c| c.is_ascii_digit()) {
            let mut version = part.to_string();
            while i + 1 < parts.len()
                && parts[i + 1].len() <= 2
                && parts[i + 1].chars().all(|c| c.is_ascii_digit())
            {
                i += 1;
                version.push('.');
                version.push_str(parts[i]);
            }
            result.push(version);
        } else if part.contains('.') && part.chars().all(|c| c.is_ascii_digit() || c == '.') {
            result.push(part.into());
        } else {
            let mut chars = part.chars();
            let formatted = match chars.next() {
                None => String::new(),
                Some(c) => {
                    format!("{}{}", c.to_uppercase().collect::<String>(), chars.as_str())
                }
            };
            result.push(formatted);
        }
        i += 1;
    }

    if result.is_empty() {
        return model.to_string();
    }
    result.join(" ")
}

fn format_tokens(n: u32) -> String {
    if n >= 1_000_000 {
        format!("{:.1}m", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn compute_visual_lines(lines: &[Line], width: u16) -> Vec<String> {
    let mut visual = Vec::new();
    for line in lines {
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        let chars: Vec<char> = text.chars().collect();
        if chars.is_empty() || width == 0 {
            visual.push(String::new());
        } else {
            for chunk in chars.chunks(width as usize) {
                visual.push(chunk.iter().collect());
            }
        }
    }
    visual
}

fn render_selection_highlight(frame: &mut Frame, app: &App, area: Rect) {
    let range = match app.selection.ordered() {
        Some(r) => r,
        None => return,
    };
    let ((sc, sr), (ec, er)) = range;

    let content_y = area.y + 1;
    let content_height = area.height.saturating_sub(1);
    let scroll = app.scroll_offset;

    let buf = frame.buffer_mut();

    for vis_row in sr..=er {
        if vis_row < scroll {
            continue;
        }
        let screen_row_offset = vis_row - scroll;
        if screen_row_offset >= content_height {
            break;
        }
        let screen_y = content_y + screen_row_offset;

        let row_start = if vis_row == sr { sc } else { 0 };
        let row_end = if vis_row == er { ec } else { area.width };

        for screen_col in row_start..row_end {
            let screen_x = area.x + screen_col;
            if screen_x >= area.x + area.width {
                break;
            }
            if let Some(cell) = buf.cell_mut(Position::new(screen_x, screen_y)) {
                let current = cell.style();
                cell.set_style(current.add_modifier(Modifier::REVERSED));
            }
        }
    }
}

fn color_to_rgb(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Black => (0, 0, 0),
        Color::White => (255, 255, 255),
        Color::DarkGray => (80, 80, 80),
        Color::Gray => (160, 160, 160),
        Color::Red => (204, 36, 29),
        Color::Green => (152, 195, 121),
        Color::Yellow => (229, 192, 123),
        Color::Blue => (97, 175, 239),
        Color::Magenta => (198, 120, 221),
        Color::Cyan => (86, 182, 194),
        _ => (200, 200, 200),
    }
}

fn blend_color(fg: Color, bg: Color, t: f32) -> Color {
    let (fr, fg_g, fb) = color_to_rgb(fg);
    let (br, bg_g, bb) = color_to_rgb(bg);
    let inv = 1.0 - t;
    Color::Rgb(
        (fr as f32 * inv + br as f32 * t) as u8,
        (fg_g as f32 * inv + bg_g as f32 * t) as u8,
        (fb as f32 * inv + bb as f32 * t) as u8,
    )
}

fn expand_line_to_tool(
    lines: &[Line],
    line_to_tool: &[Option<(usize, usize)>],
    width: u16,
) -> Vec<Option<(usize, usize)>> {
    let mut result = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let tool = line_to_tool.get(i).copied().flatten();
        if width == 0 {
            result.push(tool);
        } else {
            let w = line.width();
            let wrapped = if w == 0 {
                1
            } else {
                (w as u32).div_ceil(width as u32).max(1)
            };
            for j in 0..wrapped {
                result.push(if j == 0 { tool } else { None });
            }
        }
    }
    result
}

fn expand_line_to_msg(lines: &[Line], line_to_msg: &[usize], width: u16) -> Vec<usize> {
    let mut result = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let msg_idx = line_to_msg.get(i).copied().unwrap_or(0);
        if width == 0 {
            result.push(msg_idx);
        } else {
            let w = line.width();
            let wrapped = if w == 0 {
                1
            } else {
                (w as u32).div_ceil(width as u32).max(1)
            };
            for _ in 0..wrapped {
                result.push(msg_idx);
            }
        }
    }
    result
}
