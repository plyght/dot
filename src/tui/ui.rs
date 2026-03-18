use std::collections::HashSet;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::style::Color;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};

use crate::agent::TodoStatus;
use crate::tui::app::{
    App, AppMode, BackgroundSubagentInfo, ChatMessage, ChipKind, InputChip, StatusLevel,
};
use crate::tui::markdown;
use crate::tui::theme::Theme;
use crate::tui::ui_popups;
use crate::tui::ui_tools;

fn is_compact(w: u16) -> bool {
    w < 60
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    if app.welcome_screen.visible || app.login_popup.from_welcome && app.login_popup.visible {
        frame.render_widget(ratatui::widgets::Clear, frame.area());
        if app.welcome_screen.visible {
            ui_popups::draw_welcome_screen(frame, app);
        }
        if app.login_popup.visible {
            ui_popups::draw_login_popup(frame, app);
        }
        return;
    }

    let input_height = app.input_height(frame.area().width);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(input_height),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(frame.area());

    app.layout.header = chunks[0];
    app.layout.messages = chunks[1];
    app.layout.input = Rect {
        x: chunks[2].x,
        y: chunks[2].y,
        width: chunks[2].width,
        height: chunks[2].height + chunks[3].height + chunks[4].height,
    };
    app.layout.status = chunks[5];

    draw_status_header(frame, app, chunks[0]);
    draw_messages(frame, app, chunks[1]);
    draw_input_separator(frame, app, chunks[2]);
    draw_input(frame, app, chunks[3]);
    draw_input_separator(frame, app, chunks[4]);
    draw_token_bar(frame, app, chunks[5]);

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
        ui_popups::draw_command_palette(frame, app, app.layout.input);
    }

    if app.file_picker.visible {
        ui_popups::draw_file_picker(frame, app, app.layout.input);
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

    if app.login_popup.visible {
        ui_popups::draw_login_popup(frame, app);
    }

    if app.aside_popup.visible {
        ui_popups::draw_aside_popup(frame, app);
    }
}

fn draw_status_header(frame: &mut Frame, app: &App, area: Rect) {
    let compact = is_compact(area.width);

    let title_text = app
        .conversation_title
        .as_deref()
        .unwrap_or("new conversation");

    let model_short = display_model(&app.model_name);
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

    let right_text = if !compact || area.width >= 40 {
        format!("{} ", model_display)
    } else {
        String::new()
    };

    let right_width = right_text.chars().count();
    let max_title = (area.width as usize).saturating_sub(right_width + 2);
    let display_title = if title_text.chars().count() > max_title && max_title > 2 {
        let t: String = title_text
            .chars()
            .take(max_title.saturating_sub(1))
            .collect();
        format!("{}\u{2026}", t)
    } else {
        title_text.to_string()
    };

    let left_text = format!(" {}", display_title);
    let left_width = left_text.chars().count();
    let gap = (area.width as usize).saturating_sub(left_width + right_width);

    let spans = vec![
        Span::styled(left_text, app.theme.dim),
        Span::raw(" ".repeat(gap)),
        Span::styled(right_text, app.theme.dim),
    ];

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_messages(frame: &mut Frame, app: &mut App, area: Rect) {
    let (panel_area, msg_area) = if !app.background_subagents.is_empty() {
        let max_tool_lines = app
            .background_subagents
            .iter()
            .map(|b| b.tool_history.len())
            .max()
            .unwrap_or(0);
        let panel_h = ((5 + max_tool_lines) as u16).min(area.height / 2).max(4);
        let pa = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: panel_h,
        };
        let ma = Rect {
            x: area.x,
            y: area.y + panel_h,
            width: area.width,
            height: area.height.saturating_sub(panel_h),
        };
        (Some(pa), ma)
    } else {
        (None, area)
    };

    if let Some(pa) = panel_area {
        draw_subagent_panel(frame, app, pa);
    }

    let area = msg_area;

    let [content_area, scrollbar_area] = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .areas(area);

    app.layout.messages = content_area;

    let lpad: u16 = if is_compact(content_area.width) { 0 } else { 1 };
    let inner = Rect {
        x: content_area.x + lpad,
        y: content_area.y,
        width: content_area.width.saturating_sub(lpad + 2),
        height: content_area.height,
    };

    let block = Block::default();
    let paragraph_area = content_area;
    let wrap_width = block.inner(paragraph_area).width;

    let content_width = content_area.width;
    let need_rebuild =
        app.render_dirty || app.render_cache.as_ref().map(|c| c.width) != Some(content_width);

    if need_rebuild {
        let msg_count = app.messages.len();
        let can_reuse_msg_cache = app.message_cache.as_ref().is_some_and(|mc| {
            mc.width == content_width
                && mc.message_count == msg_count
                && mc.expanded_snapshot == app.expanded_tool_calls
                && mc.thinking_expanded == app.thinking_expanded
        });

        let (mut all_lines, mut line_to_msg, mut line_to_tool) = if can_reuse_msg_cache {
            let mc = app.message_cache.as_ref().unwrap();
            (
                mc.lines.clone(),
                mc.line_to_msg.clone(),
                mc.line_to_tool.clone(),
            )
        } else {
            let mut lines: Vec<Line<'static>> = Vec::new();
            let mut ltm: Vec<usize> = Vec::new();
            let mut ltt: Vec<Option<(usize, usize)>> = Vec::new();

            for (msg_idx, msg) in app.messages.iter().enumerate() {
                let before = lines.len();
                render_message(
                    msg,
                    msg_idx,
                    &MessageRenderCtx {
                        theme: &app.theme,
                        thinking_expanded: app.thinking_expanded,
                        inner_width: wrap_width,
                        expanded_tool_calls: &app.expanded_tool_calls,
                    },
                    &mut lines,
                    &mut ltt,
                );
                let after = lines.len();
                for _ in before..after {
                    ltm.push(msg_idx);
                }
            }

            let (lines, ltm, ltt) = pre_wrap_lines(lines, ltm, ltt, wrap_width);

            app.message_cache = Some(crate::tui::app::MessageCache {
                lines: lines.clone(),
                line_to_msg: ltm.clone(),
                line_to_tool: ltt.clone(),
                message_count: msg_count,
                width: content_width,
                expanded_snapshot: app.expanded_tool_calls.clone(),
                thinking_expanded: app.thinking_expanded,
            });

            (lines, ltm, ltt)
        };

        if !app.todos.is_empty() {
            let pad = if inner.width < 55 { "  " } else { "    " };
            let done = app
                .todos
                .iter()
                .filter(|t| t.status == TodoStatus::Completed)
                .count();
            let total = app.todos.len();

            let label = format!("{}/{} tasks ", done, total);
            let prefix = format!("{}\u{25c6} ", pad);
            let bar_budget = (inner.width as usize)
                .saturating_sub(prefix.chars().count() + label.chars().count());
            let bar_width = bar_budget.clamp(4, 16);
            let filled = if total > 0 {
                (done * bar_width) / total
            } else {
                0
            };
            let empty = bar_width - filled;
            let filled_bar: String = "\u{2501}".repeat(filled);
            let empty_bar: String = "\u{2591}".repeat(empty);

            all_lines.push(Line::from(""));
            line_to_msg.push(app.messages.len().saturating_sub(1));
            line_to_tool.push(None);
            all_lines.push(Line::from(vec![
                Span::styled(prefix, app.theme.dim),
                Span::styled(label, app.theme.dim),
                Span::styled(filled_bar, app.theme.progress_bar_filled),
                Span::styled(empty_bar, app.theme.progress_bar_empty),
            ]));
            line_to_msg.push(app.messages.len().saturating_sub(1));
            line_to_tool.push(None);
        }

        let tail_start = all_lines.len();

        if app.is_streaming {
            let stream_msg_idx = app.messages.len();
            ui_tools::render_streaming_state(
                app,
                wrap_width,
                &mut all_lines,
                &mut line_to_tool,
                stream_msg_idx,
            );
            let added = all_lines.len() - tail_start;
            for _ in 0..added {
                line_to_msg.push(stream_msg_idx);
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
            line_to_msg.push(app.messages.len().saturating_sub(1));
            all_lines.push(Line::from(vec![
                Span::styled(format!("    {} ", icon), style),
                Span::styled(status.text.clone(), style),
            ]));
            line_to_tool.push(None);
            line_to_msg.push(app.messages.len().saturating_sub(1));
        }

        if all_lines.is_empty() {
            let empty_lines = ui_popups::draw_empty_state(app, inner.width, inner.height);
            for _ in &empty_lines {
                line_to_tool.push(None);
                line_to_msg.push(0);
            }
            all_lines.extend(empty_lines);
        }

        if tail_start < all_lines.len() {
            let tail_lines = all_lines.split_off(tail_start);
            let tail_msg = line_to_msg.split_off(tail_start);
            let tail_tool = line_to_tool.split_off(tail_start);
            let (wrapped_tail, wrapped_msg, wrapped_tool) =
                pre_wrap_lines(tail_lines, tail_msg, tail_tool, wrap_width);
            all_lines.extend(wrapped_tail);
            line_to_msg.extend(wrapped_msg);
            line_to_tool.extend(wrapped_tool);
        }

        let total_visual = all_lines.len() as u32;

        app.content_width = content_width;
        app.message_line_map.clone_from(&line_to_msg);
        app.tool_line_map.clone_from(&line_to_tool);

        app.render_cache = Some(crate::tui::app::RenderCache {
            lines: all_lines,
            line_to_msg,
            line_to_tool,
            total_visual,
            width: content_width,
            wrap_heights: Vec::new(),
        });
        app.render_dirty = false;
    }

    let cache = app.render_cache.as_ref().unwrap();
    let total_visual = cache.total_visual;

    let visible = content_area.height as u32;
    app.max_scroll = total_visual.saturating_sub(visible);
    if app.follow_bottom || app.scroll_offset > app.max_scroll {
        app.scroll_offset = app.max_scroll;
    }

    let target = app.scroll_offset;
    let margin = visible.min(50);
    let skip_lines = target.saturating_sub(margin) as usize;
    let end_lines = ((target + visible + margin) as usize).min(cache.lines.len());

    let render_lines = &cache.lines[skip_lines..end_lines];
    let render_scroll = (target - skip_lines as u32).min(u16::MAX as u32) as u16;

    let paragraph = Paragraph::new(render_lines.to_vec())
        .block(block)
        .scroll((render_scroll, 0));

    frame.render_widget(paragraph, paragraph_area);

    let code_bg = app.theme.code_bg;
    let content_y = content_area.y;
    let content_h = content_area.height as usize;
    let body_cols = if is_compact(content_area.width) {
        2u16
    } else {
        4u16
    };
    let bg_left = content_area.x + body_cols;
    let bg_right = content_area.x + inner.width;
    let buf = frame.buffer_mut();
    let mut is_code: Vec<bool> = (0..content_h)
        .map(|dy| {
            buf.cell_mut(Position::new(bg_left, content_y + dy as u16))
                .map(|c| c.bg == code_bg)
                .unwrap_or(false)
        })
        .collect();
    for i in 1..content_h.saturating_sub(1) {
        if !is_code[i]
            && is_code[i.saturating_sub(1)]
            && is_code.get(i + 1).copied().unwrap_or(false)
        {
            is_code[i] = true;
        }
    }
    for (dy, &fill) in is_code.iter().enumerate() {
        if fill {
            let y = content_y + dy as u16;
            for x in bg_left..bg_right {
                if let Some(cell) = buf.cell_mut(Position::new(x, y)) {
                    cell.bg = code_bg;
                }
            }
        }
    }

    render_selection_highlight(frame, app, paragraph_area);

    if app.max_scroll > 0 {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .track_symbol(Some("\u{2502}"))
            .thumb_symbol("\u{2503}")
            .begin_symbol(None)
            .end_symbol(None)
            .track_style(app.theme.scrollbar_track)
            .thumb_style(app.theme.scrollbar_thumb);

        let (sb_total, sb_pos) = if app.max_scroll <= u16::MAX as u32 {
            (app.max_scroll as usize, app.scroll_offset as usize)
        } else {
            let scale = app.max_scroll as f64 / u16::MAX as f64;
            (
                (u16::MAX as usize),
                (app.scroll_offset as f64 / scale) as usize,
            )
        };
        let mut state = ScrollbarState::new(sb_total).position(sb_pos);
        frame.render_stateful_widget(scrollbar, scrollbar_area, &mut state);
    }
}

struct MessageRenderCtx<'a> {
    theme: &'a Theme,
    thinking_expanded: bool,
    inner_width: u16,
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
    let body_indent: &str = if compact { " " } else { "  " };
    let body_indent_cols: u16 = if compact { 1 } else { 2 };

    lines.push(Line::from(""));
    line_to_tool.push(None);

    match msg.role.as_str() {
        "user" => {
            if msg_idx > 0 {
                let sep_width = ctx.inner_width.saturating_sub(4) as usize;
                if sep_width > 0 {
                    lines.push(Line::from(vec![
                        Span::raw(body_indent),
                        Span::styled("\u{2500}".repeat(sep_width), ctx.theme.message_separator),
                    ]));
                    line_to_tool.push(None);
                }
            }

            let w = ctx.inner_width as usize;
            let right_pad = 2usize;
            let left_indent = body_indent_cols as usize;
            let content_width = w.saturating_sub(left_indent + right_pad);
            let user_style = ctx.theme.user_text.add_modifier(Modifier::BOLD);
            let chip_style = ctx.theme.dim;
            let chips: Vec<InputChip> = msg
                .chips
                .clone()
                .unwrap_or_else(|| parse_mention_chips(&msg.content));
            let line_count = msg.content.lines().count();
            let mut byte_offset = 0;
            for (i, text_line) in msg.content.lines().enumerate() {
                let line_spans =
                    chip_styled_spans(text_line, byte_offset, &chips, chip_style, Some(user_style));
                byte_offset += text_line.len();
                if i < line_count - 1 {
                    byte_offset += 1;
                }
                let content_line = Line::from(line_spans.clone());
                let wrapped = char_wrap(vec![content_line], content_width as u16);
                for row in wrapped {
                    line_to_tool.push(None);
                    let row_chars: usize =
                        row.spans.iter().map(|s| s.content.chars().count()).sum();
                    let left = w.saturating_sub(left_indent + row_chars + right_pad);
                    let mut line_vec = vec![Span::raw(body_indent), Span::raw(" ".repeat(left))];
                    line_vec.extend(row.spans);
                    line_vec.push(Span::raw(" ".repeat(right_pad)));
                    lines.push(Line::from(line_vec));
                }
            }
            if line_count > 1 {
                lines.push(Line::from(""));
                line_to_tool.push(None);
            }
            line_to_tool.push(None);
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
            if let Some(ref thinking) = msg.thinking {
                render_thinking_block(
                    thinking,
                    ctx.thinking_expanded,
                    ctx.theme,
                    compact,
                    lines,
                    line_to_tool,
                    msg_idx,
                    ctx.inner_width,
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
                                let mut padded = vec![Span::raw(body_indent)];
                                padded.extend(line.spans);
                                if let Some(bg_color) = bg {
                                    let used: usize =
                                        padded.iter().map(|s| s.content.chars().count()).sum();
                                    let target = ctx.inner_width as usize;
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
                                ui_tools::RenderToolCallsParams {
                                    tool_calls: std::slice::from_ref(tc),
                                    theme: ctx.theme,
                                    compact,
                                    lines,
                                    line_to_tool: Some(line_to_tool),
                                    msg_idx,
                                    width: ctx.inner_width,
                                    tool_idx_base: tool_idx,
                                },
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
                        ui_tools::RenderToolCallsParams {
                            tool_calls: &msg.tool_calls,
                            theme: ctx.theme,
                            compact,
                            lines,
                            line_to_tool: Some(line_to_tool),
                            msg_idx,
                            width: ctx.inner_width,
                            tool_idx_base: 0,
                        },
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
                    let mut padded = vec![Span::raw(body_indent)];
                    padded.extend(line.spans);
                    if let Some(bg_color) = bg {
                        let used: usize = padded.iter().map(|s| s.content.chars().count()).sum();
                        let target = ctx.inner_width as usize;
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
                        ui_tools::RenderToolCallsParams {
                            tool_calls: &msg.tool_calls,
                            theme: ctx.theme,
                            compact,
                            lines,
                            line_to_tool: Some(line_to_tool),
                            msg_idx,
                            width: ctx.inner_width,
                            tool_idx_base: 0,
                        },
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
    msg_idx: usize,
    width: u16,
) {
    let pad = if compact { "  " } else { "    " };
    let prefix = format!("{}\u{2502} ", pad);
    let prefix_chars = prefix.chars().count();
    let word_count = thinking.split_whitespace().count();
    let secs = (word_count / 8).max(1);
    if expanded {
        line_to_tool.push(Some((msg_idx, usize::MAX)));
        lines.push(Line::from(vec![
            Span::styled(format!("{}\u{25be} ", pad), theme.dim),
            Span::styled(
                "thinking",
                Style::default()
                    .fg(theme.muted_fg)
                    .add_modifier(Modifier::ITALIC),
            ),
        ]));
        let content_width = (width as usize).saturating_sub(prefix_chars);
        let thinking_style = Style::default()
            .fg(theme.muted_fg)
            .add_modifier(Modifier::ITALIC);
        for text_line in thinking.lines() {
            let chars: Vec<char> = text_line.chars().collect();
            if content_width == 0 || chars.len() <= content_width {
                line_to_tool.push(None);
                lines.push(Line::from(vec![
                    Span::styled(prefix.clone(), theme.dim),
                    Span::styled(text_line.to_string(), thinking_style),
                ]));
            } else {
                for chunk in chars.chunks(content_width) {
                    line_to_tool.push(None);
                    lines.push(Line::from(vec![
                        Span::styled(prefix.clone(), theme.dim),
                        Span::styled(chunk.iter().collect::<String>(), thinking_style),
                    ]));
                }
            }
        }
        line_to_tool.push(None);
        lines.push(Line::from(Span::styled(pad.to_string(), theme.dim)));
    } else {
        line_to_tool.push(Some((msg_idx, usize::MAX)));
        lines.push(Line::from(vec![
            Span::styled(format!("{}\u{25b8} ", pad), theme.dim),
            Span::styled(
                format!("thought for {}s", secs),
                Style::default()
                    .fg(theme.muted_fg)
                    .add_modifier(Modifier::ITALIC),
            ),
        ]));
    }
}

fn parse_mention_chips(content: &str) -> Vec<InputChip> {
    let mut chips = Vec::new();
    let mut i = 0;
    let bytes = content.as_bytes();
    while i < bytes.len() {
        let at_boundary = i == 0
            || bytes
                .get(i.saturating_sub(1))
                .is_some_and(|b| b.is_ascii_whitespace());
        if at_boundary {
            if bytes.get(i) == Some(&b'@') {
                let start = i;
                i += 1;
                while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                if i > start + 1 {
                    chips.push(InputChip {
                        start,
                        end: i,
                        kind: ChipKind::File,
                    });
                }
                continue;
            }
            if bytes.get(i) == Some(&b'/') {
                let start = i;
                i += 1;
                while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                if i > start + 1 {
                    chips.push(InputChip {
                        start,
                        end: i,
                        kind: ChipKind::Skill,
                    });
                }
                continue;
            }
        }
        i += 1;
    }
    chips
}

fn chip_styled_spans(
    text: &str,
    byte_offset: usize,
    chips: &[InputChip],
    chip_style: Style,
    base_style: Option<Style>,
) -> Vec<Span<'static>> {
    let mk_base = |s: &str| match base_style {
        Some(style) => Span::styled(s.to_string(), style),
        None => Span::raw(s.to_string()),
    };
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
            spans.push(mk_base(&text[s..e]));
        }
        let s = cs - byte_offset;
        let e = ce - byte_offset;
        spans.push(Span::styled(text[s..e].to_string(), chip_style));
        pos = ce;
    }
    if pos < end {
        let s = pos - byte_offset;
        spans.push(mk_base(&text[s..]));
    }
    if spans.is_empty() {
        spans.push(mk_base(text));
    }
    spans
}

fn draw_input_separator(frame: &mut Frame, app: &App, area: Rect) {
    let line = "\u{2500}".repeat(area.width as usize);
    let paragraph = Paragraph::new(line).style(app.theme.dim);
    frame.render_widget(paragraph, area);
}

fn draw_input(frame: &mut Frame, app: &App, area: Rect) {
    let inner = area;
    let can_edit = !app.vim_mode || app.mode == AppMode::Insert;
    let has_input = !app.input.is_empty() || !app.attachments.is_empty();

    let (prompt, prompt_style) = if app.is_streaming && !has_input {
        ("\u{203a} ", Style::default().fg(app.theme.input_dim_fg))
    } else if can_edit {
        (
            "\u{203a} ",
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        ("\u{203a} ", Style::default().fg(app.theme.muted_fg))
    };

    let text_style = if app.is_streaming && !has_input {
        Style::default().fg(app.theme.input_dim_fg)
    } else {
        Style::default().fg(app.theme.input_fg)
    };

    let display_lines: Vec<Line<'static>> = if app.is_streaming && !has_input {
        let dim = Style::default().fg(app.theme.input_dim_fg);
        let word = "generating";
        let wave_pos = (app.tick_count / 4) as usize;
        let mut left_spans: Vec<Span<'static>> = Vec::new();
        for (i, ch) in word.chars().enumerate() {
            let dist = if wave_pos % (word.len() + 4) > i {
                (wave_pos % (word.len() + 4)) - i
            } else {
                i - (wave_pos % (word.len() + 4))
            };
            let style = if dist == 0 {
                Style::default().fg(app.theme.accent)
            } else if dist <= 2 {
                if let (Color::Rgb(ar, ag, ab), Color::Rgb(dr, dg, db)) =
                    (app.theme.accent, app.theme.input_dim_fg)
                {
                    let t = if dist == 1 { 0.5 } else { 0.8 };
                    let lerp = |a: u8, b: u8, t: f32| -> u8 {
                        (a as f32 + (b as f32 - a as f32) * t) as u8
                    };
                    Style::default().fg(Color::Rgb(
                        lerp(ar, dr, t),
                        lerp(ag, dg, t),
                        lerp(ab, db, t),
                    ))
                } else if dist == 1 {
                    Style::default().fg(app.theme.accent)
                } else {
                    dim
                }
            } else {
                dim
            };
            left_spans.push(Span::styled(String::from(ch), style));
        }
        let mut right_spans: Vec<Span<'static>> = Vec::new();
        if let Some(elapsed) = app.streaming_elapsed_secs() {
            right_spans.push(Span::styled(format!(" {}", format_elapsed(elapsed)), dim));
        }
        if !app.message_queue.is_empty() {
            right_spans.push(Span::styled(
                format!(" {} queued", app.message_queue.len()),
                dim,
            ));
        }
        let left_width: usize = left_spans.iter().map(|s| s.content.chars().count()).sum();
        let right_width: usize = right_spans.iter().map(|s| s.content.chars().count()).sum();
        let padding = (inner.width as usize).saturating_sub(left_width + right_width);
        left_spans.push(Span::raw(" ".repeat(padding)));
        left_spans.extend(right_spans);
        vec![Line::from(left_spans)]
    } else if !has_input {
        vec![Line::from(vec![Span::styled(prompt, prompt_style)])]
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
                    Style::default()
                        .fg(app.theme.accent)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    None,
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

    let wrapped = char_wrap(display_lines, inner.width);
    let paragraph = Paragraph::new(wrapped).style(text_style);
    frame.render_widget(paragraph, inner);
    if can_edit && !app.model_selector.visible && (has_input || !app.is_streaming) {
        let display = app.display_input();
        let dcursor = app.display_cursor_pos();
        let (cx, cy) = cursor_position(&display, dcursor, inner);
        if cy < inner.y + inner.height {
            frame.set_cursor_position((cx, cy));
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

fn draw_token_bar(frame: &mut Frame, app: &App, area: Rect) {
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
        let new_label = " \u{2193}";
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

    if !app.background_subagents.is_empty() {
        let total = app.background_subagents.len();
        let done = app.background_subagents.iter().filter(|b| b.done).count();
        let bg_label = if compact {
            format!(" {}bg", total - done)
        } else if done == total {
            format!(" \u{00b7} {}/{} bg done", done, total)
        } else {
            format!(" \u{00b7} {} bg agents", total - done)
        };
        left_spans.push(Span::styled(bg_label, app.theme.subagent_working));
    }

    let left_width: usize = left_spans.iter().map(|s| s.content.chars().count()).sum();

    let mut right_spans: Vec<Span<'static>> = Vec::new();

    if app.context_window > 0 && app.last_input_tokens > 0 {
        let pct = (app.last_input_tokens as f64 / app.context_window as f64 * 100.0).min(100.0);
        let color = if pct > 80.0 {
            Color::Rgb(243, 139, 168)
        } else if pct > 60.0 {
            Color::Rgb(249, 226, 175)
        } else {
            app.theme.dim.fg.unwrap_or(Color::Gray)
        };
        right_spans.push(Span::styled(
            format!("{:.0}% ", pct),
            Style::default().fg(color),
        ));
    }

    if app.vim_mode {
        let mode_char = match app.mode {
            AppMode::Normal => "N",
            AppMode::Insert => "I",
        };
        right_spans.push(Span::styled(format!("{} ", mode_char), app.theme.dim));
    }

    let right_width: usize = right_spans.iter().map(|s| s.content.chars().count()).sum();
    let padding = (area.width as usize).saturating_sub(left_width + right_width);

    let mut line_spans = left_spans;
    line_spans.push(Span::raw(" ".repeat(padding)));
    line_spans.extend(right_spans);

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

type PreWrapResult = (Vec<Line<'static>>, Vec<usize>, Vec<Option<(usize, usize)>>);

fn pre_wrap_lines(
    lines: Vec<Line<'static>>,
    line_to_msg: Vec<usize>,
    line_to_tool: Vec<Option<(usize, usize)>>,
    width: u16,
) -> PreWrapResult {
    use unicode_width::UnicodeWidthChar;
    if width == 0 {
        return (lines, line_to_msg, line_to_tool);
    }
    let w = width as usize;
    let mut out_lines = Vec::with_capacity(lines.len());
    let mut out_msg = Vec::with_capacity(lines.len());
    let mut out_tool = Vec::with_capacity(lines.len());

    for (i, line) in lines.into_iter().enumerate() {
        let msg = line_to_msg.get(i).copied().unwrap_or(0);
        let tool = line_to_tool.get(i).copied().flatten();
        if line.width() <= w {
            out_lines.push(line);
            out_msg.push(msg);
            out_tool.push(tool);
            continue;
        }
        let mut row: Vec<Span<'static>> = Vec::new();
        let mut row_len = 0usize;
        for span in line.spans {
            let style = span.style;
            let text = span.content.to_string();
            let mut seg_start = 0;
            for (byte_pos, ch) in text.char_indices() {
                let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
                if row_len + cw > w && row_len > 0 {
                    if seg_start < byte_pos {
                        row.push(Span::styled(text[seg_start..byte_pos].to_string(), style));
                    }
                    out_lines.push(Line::from(std::mem::take(&mut row)));
                    out_msg.push(msg);
                    out_tool.push(tool);
                    row_len = 0;
                    seg_start = byte_pos;
                }
                row_len += cw;
            }
            if seg_start < text.len() {
                row.push(Span::styled(text[seg_start..].to_string(), style));
            }
        }
        if !row.is_empty() {
            out_lines.push(Line::from(row));
            out_msg.push(msg);
            out_tool.push(tool);
        }
    }
    (out_lines, out_msg, out_tool)
}

fn char_wrap(lines: Vec<Line<'static>>, width: u16) -> Vec<Line<'static>> {
    if width == 0 {
        return lines;
    }
    let w = width as usize;
    let mut result = Vec::new();
    for line in lines {
        let line_w: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
        if line_w <= w {
            result.push(line);
            continue;
        }
        let mut row: Vec<Span<'static>> = Vec::new();
        let mut row_len = 0usize;
        for span in line.spans {
            let style = span.style;
            let text = span.content.to_string();
            let mut seg_start = 0;
            for (byte_pos, _ch) in text.char_indices() {
                if row_len >= w {
                    if seg_start < byte_pos {
                        row.push(Span::styled(text[seg_start..byte_pos].to_string(), style));
                    }
                    result.push(Line::from(std::mem::take(&mut row)));
                    row_len = 0;
                    seg_start = byte_pos;
                }
                row_len += 1;
            }
            if seg_start < text.len() {
                row.push(Span::styled(text[seg_start..].to_string(), style));
            }
        }
        if !row.is_empty() {
            result.push(Line::from(row));
        }
    }
    result
}

fn draw_subagent_panel(frame: &mut Frame, app: &App, area: Rect) {
    if area.height < 3 {
        return;
    }

    let total = app.background_subagents.len();
    let done_count = app.background_subagents.iter().filter(|b| b.done).count();
    let spinner_frames = ["◌", "◔", "◑", "◕", "●", "◕", "◑", "◔"];
    let spin = spinner_frames[(app.tick_count / 8 % 8) as usize];

    let header_text = if done_count == total {
        format!(" {} subagents — {}/{} completed", spin, done_count, total)
    } else {
        format!(
            " {} running {} subagents ({}/{} completed)",
            spin,
            total - done_count,
            done_count,
            total
        )
    };
    let header_line = Line::from(Span::styled(header_text, app.theme.subagent_header));
    frame.render_widget(
        Paragraph::new(header_line),
        Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        },
    );

    if area.height < 4 {
        return;
    }

    let cols_area = Rect {
        x: area.x,
        y: area.y + 1,
        width: area.width,
        height: area.height - 1,
    };
    let visible_count = (total).min(5);
    if visible_count == 0 {
        return;
    }

    let col_width = cols_area.width / visible_count as u16;
    let constraints: Vec<Constraint> = (0..visible_count)
        .map(|_| Constraint::Length(col_width))
        .collect();
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(cols_area);

    for (i, bg) in app
        .background_subagents
        .iter()
        .take(visible_count)
        .enumerate()
    {
        draw_subagent_column(frame, app, bg, cols[i]);
    }
}

fn draw_subagent_column(frame: &mut Frame, app: &App, bg: &BackgroundSubagentInfo, area: Rect) {
    let spinner_frames = ["◌", "◔", "◑", "◕", "●", "◕", "◑", "◔"];
    let spin = spinner_frames[(app.tick_count / 8 % 8) as usize];

    let status_char = if bg.done { "●" } else { spin };
    let status_style = if bg.done {
        app.theme.subagent_done
    } else {
        app.theme.subagent_working
    };

    let max_title = area.width.saturating_sub(4) as usize;
    let desc: String = if bg.description.chars().count() > max_title && max_title > 1 {
        let t: String = bg
            .description
            .chars()
            .take(max_title.saturating_sub(1))
            .collect();
        format!("{}\u{2026}", t)
    } else {
        bg.description.clone()
    };

    let title_line = Line::from(vec![
        Span::styled(format!("{} ", status_char), status_style),
        Span::styled(
            desc,
            if bg.done {
                app.theme.dim
            } else {
                app.theme.subagent_header
            },
        ),
    ]);

    let block = Block::bordered()
        .border_style(app.theme.subagent_border)
        .title(title_line);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 {
        return;
    }

    let mut lines: Vec<Line<'static>> = Vec::new();

    let history_show = (inner.height as usize).saturating_sub(2);
    let history = &bg.tool_history;
    let start = history.len().saturating_sub(history_show);
    if start > 0 {
        lines.push(Line::from(Span::styled(
            format!("  ... +{} earlier tools", start),
            app.theme.dim,
        )));
    }
    for entry in &history[start..] {
        let (icon, style) = if entry.is_error {
            ("✗", app.theme.error)
        } else if entry.done {
            ("✓", app.theme.subagent_done)
        } else {
            (
                spinner_frames[(app.tick_count / 8 % 8) as usize],
                app.theme.subagent_working,
            )
        };
        let cat = crate::tui::tools::ToolCategory::from_name(&entry.name);
        let label = if entry.detail.is_empty() {
            cat.label()
        } else {
            format!("{} {}", cat.label(), entry.detail)
        };
        let max = inner.width.saturating_sub(3) as usize;
        let display: String = if label.chars().count() > max && max > 1 {
            let t: String = label.chars().take(max.saturating_sub(1)).collect();
            format!("{}\u{2026}", t)
        } else {
            label
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{} ", icon), style),
            Span::styled(display, if entry.done { app.theme.dim } else { style }),
        ]));
    }

    if let Some(ref ct) = bg.current_tool {
        let cat = crate::tui::tools::ToolCategory::from_name(ct);
        let detail = bg.current_tool_detail.as_deref().unwrap_or("");
        let label = if detail.is_empty() {
            format!("{} {}", cat.intent(), cat.label())
        } else {
            format!("{} {}", cat.intent(), detail)
        };
        let ct_max = inner.width.saturating_sub(3) as usize;
        let display: String = if label.chars().count() > ct_max && ct_max > 1 {
            let t: String = label.chars().take(ct_max.saturating_sub(1)).collect();
            format!("{}\u{2026}", t)
        } else {
            label
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", spinner_frames[(app.tick_count / 8 % 8) as usize]),
                app.theme.subagent_working,
            ),
            Span::styled(display, app.theme.subagent_working),
        ]));
    }

    let elapsed = bg.started.elapsed().as_secs_f64();
    let elapsed_str = format_elapsed(elapsed);
    let footer = if bg.done {
        Line::from(Span::styled(
            format!("Done {}  {}t", elapsed_str, bg.tools_completed),
            app.theme.subagent_done,
        ))
    } else {
        Line::from(Span::styled(
            format!("Working… {}  {}t", elapsed_str, bg.tools_completed),
            app.theme.dim,
        ))
    };

    let content_height = inner.height as usize;
    let body_lines = content_height.saturating_sub(1);
    let render_lines: Vec<Line<'static>> = lines.into_iter().take(body_lines).collect();

    let mut all: Vec<Line<'static>> = render_lines;
    while all.len() < body_lines {
        all.push(Line::from(""));
    }
    all.push(footer);

    frame.render_widget(Paragraph::new(all), inner);
}

fn render_selection_highlight(frame: &mut Frame, app: &App, area: Rect) {
    let range = match app.selection.ordered() {
        Some(r) => r,
        None => return,
    };
    let ((sc, sr), (ec, er)) = range;

    let content_y = area.y;
    let content_height = area.height as u32;
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
        let screen_y = content_y + screen_row_offset as u16;

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
