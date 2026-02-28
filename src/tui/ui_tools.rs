use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::tui::app::App;
use crate::tui::markdown;
use crate::tui::theme::Theme;
use crate::tui::tools::{StreamSegment, ToolCallDisplay, ToolCategory, extract_tool_detail};

struct ToolCallsRenderCtx<'a> {
    theme: &'a Theme,
    compact: bool,
    show_verbose_output: bool,
    msg_idx: usize,
    width: u16,
}

pub fn render_tool_calls(
    tool_calls: &[ToolCallDisplay],
    theme: &Theme,
    compact: bool,
    lines: &mut Vec<Line<'static>>,
    line_to_tool: Option<&mut Vec<Option<(usize, usize)>>>,
    msg_idx: usize,
    width: u16,
    is_expanded: impl Fn(usize) -> bool,
) {
    render_tool_calls_inner(
        tool_calls,
        &ToolCallsRenderCtx {
            theme,
            compact,
            show_verbose_output: true,
            msg_idx,
            width,
        },
        lines,
        is_expanded,
        line_to_tool,
    );
}

pub fn render_tool_calls_compact(
    tool_calls: &[ToolCallDisplay],
    theme: &Theme,
    compact: bool,
    lines: &mut Vec<Line<'static>>,
    line_to_tool: Option<&mut Vec<Option<(usize, usize)>>>,
    msg_idx: usize,
    width: u16,
    is_expanded: impl Fn(usize) -> bool,
) {
    render_tool_calls_inner(
        tool_calls,
        &ToolCallsRenderCtx {
            theme,
            compact,
            show_verbose_output: false,
            msg_idx,
            width,
        },
        lines,
        is_expanded,
        line_to_tool,
    );
}

fn render_tool_calls_inner(
    tool_calls: &[ToolCallDisplay],
    ctx: &ToolCallsRenderCtx<'_>,
    lines: &mut Vec<Line<'static>>,
    is_expanded: impl Fn(usize) -> bool,
    mut line_to_tool: Option<&mut Vec<Option<(usize, usize)>>>,
) {
    let compact = ctx.compact;
    let out_pad = if compact { "      " } else { "          " };

    for (tool_idx, tc) in tool_calls.iter().enumerate() {
        let cat_style = tool_category_style(&tc.category, ctx.theme);
        let label = tc.category.label();
        let (status_icon, status_style) = if tc.is_error {
            ("\u{2717}", ctx.theme.tool_exit_err)
        } else {
            ("\u{2713}", ctx.theme.tool_exit_ok)
        };
        let label_style = if tc.is_error {
            ctx.theme.error
        } else {
            cat_style
        };

        let has_content = tc.output.as_ref().map_or(false, |o| !o.is_empty())
            || matches!(
                tc.category,
                ToolCategory::MultiEdit | ToolCategory::Patch | ToolCategory::FileWrite
            );
        let expanded = is_expanded(tool_idx);

        let mut header_spans = vec![];
        if has_content {
            let arrow = if expanded { "\u{25be}" } else { "\u{25b8}" };
            let pad = if compact { " " } else { "   " };
            header_spans.push(Span::styled(format!("{}{}", pad, arrow), ctx.theme.dim));
        } else {
            let pad = if compact { "  " } else { "    " };
            header_spans.push(Span::raw(pad.to_string()));
        }
        header_spans.push(Span::styled(format!("{} ", status_icon), status_style));
        header_spans.push(Span::styled(format!("{:<6}", label), label_style));

        if !tc.detail.is_empty() {
            match &tc.category {
                ToolCategory::FileRead
                | ToolCategory::FileWrite
                | ToolCategory::MultiEdit
                | ToolCategory::Directory => {
                    header_spans.push(Span::styled(tc.detail.clone(), ctx.theme.tool_path));
                }
                ToolCategory::Command => {
                    header_spans.push(Span::styled(
                        format!("$ {}", tc.detail),
                        Style::default().fg(ctx.theme.muted_fg),
                    ));
                }
                ToolCategory::Search => {
                    header_spans.push(Span::styled(tc.detail.clone(), ctx.theme.dim));
                }
                ToolCategory::Mcp { .. } => {
                    let mcp_tool_name = tc.name.split('_').skip(1).collect::<Vec<_>>().join("_");
                    if !mcp_tool_name.is_empty() {
                        header_spans.push(Span::styled(mcp_tool_name, ctx.theme.tool_name));
                        if !tc.detail.is_empty() {
                            header_spans.push(Span::raw(" "));
                            header_spans.push(Span::styled(tc.detail.clone(), ctx.theme.dim));
                        }
                    }
                }
                ToolCategory::Skill => {
                    header_spans.push(Span::styled(tc.detail.clone(), ctx.theme.tool_skill));
                }
                ToolCategory::Glob | ToolCategory::Grep => {
                    header_spans.push(Span::styled(tc.detail.clone(), ctx.theme.dim));
                }
                ToolCategory::WebFetch => {
                    header_spans.push(Span::styled(tc.detail.clone(), ctx.theme.tool_path));
                }
                ToolCategory::Patch => {
                    header_spans.push(Span::styled(tc.detail.clone(), ctx.theme.dim));
                }
                ToolCategory::Snapshot => {
                    header_spans.push(Span::styled(tc.detail.clone(), ctx.theme.dim));
                }
                ToolCategory::Batch => {
                    header_spans.push(Span::styled(tc.detail.clone(), ctx.theme.dim));
                }
                ToolCategory::Question => {
                    header_spans.push(Span::styled(tc.detail.clone(), ctx.theme.dim));
                }
                ToolCategory::Unknown => {
                    header_spans.push(Span::styled(tc.name.clone(), ctx.theme.tool_name));
                }
            }
        } else {
            header_spans.push(Span::styled(tc.name.clone(), ctx.theme.tool_name));
        }

        lines.push(Line::from(header_spans));
        if let Some(ltt) = &mut line_to_tool {
            ltt.push(Some((ctx.msg_idx, tool_idx)));
        }

        let should_show = if tc.is_error {
            true
        } else if !ctx.show_verbose_output {
            matches!(
                tc.category,
                ToolCategory::FileWrite | ToolCategory::Patch | ToolCategory::MultiEdit
            )
        } else {
            !matches!(tc.category, ToolCategory::FileRead)
        };

        if expanded {
            render_expanded_output(tc, ctx, lines, &mut line_to_tool);
        } else if let Some(ref output) = tc.output
            && should_show
            && !output.is_empty()
        {
            let (max_lines, max_chars) = if tc.is_error { (6, 400) } else { (4, 400) };
            let preview: String = output.chars().take(max_chars).collect();
            let trimmed = if output.len() <= max_chars {
                preview.clone()
            } else {
                format!("{}\u{2026}", preview)
            };

            let output_style = if tc.is_error {
                ctx.theme.error
            } else {
                ctx.theme.tool_output
            };

            for ol in trimmed.lines().take(max_lines) {
                lines.push(Line::from(Span::styled(
                    format!("{}{}", out_pad, ol),
                    output_style,
                )));
                if let Some(ltt) = &mut line_to_tool {
                    ltt.push(None);
                }
            }
            let total_lines_in_output = trimmed.lines().count();
            if total_lines_in_output > max_lines || output.len() > max_chars {
                lines.push(Line::from(Span::styled(
                    format!(
                        "{}\u{2026} {} more lines",
                        out_pad,
                        output.lines().count().saturating_sub(max_lines)
                    ),
                    ctx.theme.dim,
                )));
                if let Some(ltt) = &mut line_to_tool {
                    ltt.push(None);
                }
            }
        }
    }
}

fn render_expanded_output(
    tc: &ToolCallDisplay,
    ctx: &ToolCallsRenderCtx<'_>,
    lines: &mut Vec<Line<'static>>,
    line_to_tool: &mut Option<&mut Vec<Option<(usize, usize)>>>,
) {
    let compact = ctx.compact;
    let indent: &str = if compact { "    " } else { "        " };
    let indent_len: u16 = if compact { 4 } else { 8 };
    let code_width = ctx.width.saturating_sub(indent_len);

    if code_width < 10 {
        if let Some(ref output) = tc.output {
            render_plain_output(output, indent, ctx, lines, line_to_tool);
        }
        return;
    }

    let (content, lang) = expanded_content(tc);

    if content.is_empty() {
        return;
    }

    let code_lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let mut block = Vec::new();
    markdown::render_code_block(&lang, &code_lines, ctx.theme, code_width, &mut block);

    for line in block {
        let mut padded = vec![Span::raw(indent.to_string())];
        padded.extend(line.spans);
        lines.push(Line::from(padded));
        if let Some(ltt) = line_to_tool {
            ltt.push(None);
        }
    }
}

fn expanded_content(tc: &ToolCallDisplay) -> (String, String) {
    match &tc.category {
        ToolCategory::FileRead => {
            let content = tc.output.clone().unwrap_or_default();
            let lang = lang_from_path(&tc.detail);
            (content, lang)
        }
        ToolCategory::FileWrite => {
            if let Some(written) = extract_write_content(&tc.input) {
                (written, lang_from_path(&tc.detail))
            } else {
                (tc.output.clone().unwrap_or_default(), String::new())
            }
        }
        ToolCategory::MultiEdit => {
            if let Some(diff) = generate_edit_diff(&tc.input) {
                (diff, "diff".to_string())
            } else {
                (tc.output.clone().unwrap_or_default(), String::new())
            }
        }
        ToolCategory::Patch => {
            if let Some(diff) = generate_patch_diff(&tc.input) {
                (diff, "diff".to_string())
            } else {
                (tc.output.clone().unwrap_or_default(), String::new())
            }
        }
        ToolCategory::Command => (tc.output.clone().unwrap_or_default(), String::new()),
        _ => (tc.output.clone().unwrap_or_default(), String::new()),
    }
}

fn render_plain_output(
    output: &str,
    indent: &str,
    ctx: &ToolCallsRenderCtx<'_>,
    lines: &mut Vec<Line<'static>>,
    line_to_tool: &mut Option<&mut Vec<Option<(usize, usize)>>>,
) {
    let style = if output.is_empty() {
        return;
    } else {
        ctx.theme.tool_output
    };

    for ol in output.lines() {
        lines.push(Line::from(Span::styled(format!("{}{}", indent, ol), style)));
        if let Some(ltt) = line_to_tool {
            ltt.push(None);
        }
    }
}

fn lang_from_path(path: &str) -> String {
    path.rsplit('.')
        .next()
        .filter(|ext| ext.len() <= 10 && ext.chars().all(|c| c.is_alphanumeric()))
        .unwrap_or("")
        .to_string()
}

fn extract_write_content(input: &str) -> Option<String> {
    let val: serde_json::Value = serde_json::from_str(input).ok()?;
    val.get("content")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn generate_edit_diff(input: &str) -> Option<String> {
    let val: serde_json::Value = serde_json::from_str(input).ok()?;
    let edits = val.get("edits")?.as_array()?;
    if edits.is_empty() {
        return None;
    }
    let mut diff = String::new();
    for (i, edit) in edits.iter().enumerate() {
        let old = edit.get("old_text").and_then(|v| v.as_str()).unwrap_or("");
        let new = edit.get("new_text").and_then(|v| v.as_str()).unwrap_or("");
        if edits.len() > 1 {
            diff.push_str(&format!("@@ edit {} @@\n", i + 1));
        }
        for line in old.lines() {
            diff.push('-');
            diff.push_str(line);
            diff.push('\n');
        }
        for line in new.lines() {
            diff.push('+');
            diff.push_str(line);
            diff.push('\n');
        }
    }
    if diff.is_empty() { None } else { Some(diff) }
}

fn generate_patch_diff(input: &str) -> Option<String> {
    let val: serde_json::Value = serde_json::from_str(input).ok()?;
    let patches = val.get("patches")?.as_array()?;
    if patches.is_empty() {
        return None;
    }
    let mut diff = String::new();
    for patch in patches {
        let path = patch.get("path").and_then(|v| v.as_str()).unwrap_or("file");
        let old = patch.get("old").and_then(|v| v.as_str()).unwrap_or("");
        let new = patch.get("new").and_then(|v| v.as_str()).unwrap_or("");
        diff.push_str(&format!("@@ {} @@\n", path));
        for line in old.lines() {
            diff.push('-');
            diff.push_str(line);
            diff.push('\n');
        }
        for line in new.lines() {
            diff.push('+');
            diff.push_str(line);
            diff.push('\n');
        }
    }
    if diff.is_empty() { None } else { Some(diff) }
}

pub fn render_streaming_state(app: &App, width: u16, lines: &mut Vec<Line<'static>>) {
    let compact = width < 55;
    let pad = if compact { "  " } else { "    " };
    let pad_cols: u16 = if compact { 2 } else { 4 };
    let diamond_sp = if compact { " \u{25c6} " } else { "  \u{25c6} " };

    let has_segments = !app.streaming_segments.is_empty();
    let has_text = !app.current_response.is_empty();
    let has_tool = app.pending_tool_name.is_some();

    let model_label = super::ui::display_model(&app.model_name);
    let model_header = vec![
        Span::styled(diamond_sp, Style::default().fg(app.theme.accent)),
        Span::styled(model_label, app.theme.dim),
    ];

    if has_segments || has_text || has_tool {
        lines.push(Line::from(""));
        lines.push(Line::from(model_header.clone()));

        let mut prev_was_tool = false;
        for seg in &app.streaming_segments {
            match seg {
                StreamSegment::Text(t) => {
                    if prev_was_tool {
                        lines.push(Line::from(""));
                    }
                    let md_lines =
                        markdown::render_markdown(t, &app.theme, width.saturating_sub(pad_cols));
                    for line in md_lines {
                        let bg = line.spans.first().and_then(|s| s.style.bg);
                        let mut padded = vec![Span::raw(pad.to_string())];
                        padded.extend(line.spans);
                        if let Some(c) = bg {
                            let used: usize =
                                padded.iter().map(|s| s.content.chars().count()).sum();
                            let target = width as usize;
                            if used < target {
                                padded.push(Span::styled(
                                    " ".repeat(target - used),
                                    Style::default().bg(c),
                                ));
                            }
                        }
                        lines.push(Line::from(padded));
                    }
                    prev_was_tool = false;
                }
                StreamSegment::ToolCall(tc) => {
                    if !prev_was_tool && !lines.is_empty() {
                        lines.push(Line::from(""));
                    }
                    render_tool_calls_compact(
                        std::slice::from_ref(tc),
                        &app.theme,
                        compact,
                        lines,
                        None,
                        0,
                        width,
                        |_| false,
                    );
                    prev_was_tool = true;
                }
            }
        }

        if has_text {
            if prev_was_tool {
                lines.push(Line::from(""));
            }
            let md_lines = markdown::render_markdown(
                &app.current_response,
                &app.theme,
                width.saturating_sub(pad_cols),
            );
            for line in md_lines {
                let bg = line.spans.first().and_then(|s| s.style.bg);
                let mut padded = vec![Span::raw(pad.to_string())];
                padded.extend(line.spans);
                if let Some(c) = bg {
                    let used: usize = padded.iter().map(|s| s.content.chars().count()).sum();
                    let target = width as usize;
                    if used < target {
                        padded.push(Span::styled(
                            " ".repeat(target - used),
                            Style::default().bg(c),
                        ));
                    }
                }
                lines.push(Line::from(padded));
            }
        } else if has_tool {
            let tool_name = app.pending_tool_name.as_deref().unwrap_or("");
            let category = ToolCategory::from_name(tool_name);
            let detail = extract_tool_detail(tool_name, &app.pending_tool_input);

            let cat_style = tool_category_style(&category, &app.theme);
            let frames = [
                "\u{25cb}", "\u{25d4}", "\u{25d1}", "\u{25d5}", "\u{25cf}", "\u{25d5}", "\u{25d1}",
                "\u{25d4}",
            ];
            let idx = (app.tick_count / 8 % frames.len() as u64) as usize;
            let intent = category.intent();
            let mut tool_spans = vec![
                Span::styled(format!("{}{} ", pad, frames[idx]), cat_style),
                Span::styled(format!("{} ", intent), cat_style),
            ];

            if !detail.is_empty() {
                match &category {
                    ToolCategory::FileRead
                    | ToolCategory::FileWrite
                    | ToolCategory::MultiEdit
                    | ToolCategory::Directory => {
                        tool_spans.push(Span::styled(detail, app.theme.tool_path));
                    }
                    ToolCategory::Command => {
                        tool_spans.push(Span::styled(format!("$ {}", detail), app.theme.dim));
                    }
                    ToolCategory::Mcp { .. } => {
                        let mcp_tool = tool_name.split('_').skip(1).collect::<Vec<_>>().join("_");
                        tool_spans.push(Span::styled(mcp_tool, app.theme.tool_name));
                        if !detail.is_empty() {
                            tool_spans.push(Span::raw(" "));
                            tool_spans.push(Span::styled(detail, app.theme.dim));
                        }
                    }
                    _ => {
                        tool_spans.push(Span::styled(detail, app.theme.dim));
                    }
                }
            } else {
                tool_spans.push(Span::styled(tool_name.to_string(), app.theme.tool_name));
            }

            if has_segments {
                let n = app.current_tool_calls.len();
                tool_spans.push(Span::styled(format!(" \u{00b7} {} done", n), app.theme.dim));
            }

            if let Some(elapsed) = app.streaming_elapsed_secs() {
                tool_spans.push(Span::styled(
                    format!(" \u{00b7} {}", super::ui::format_elapsed(elapsed)),
                    app.theme.dim,
                ));
            }

            lines.push(Line::from(tool_spans));
        }
    } else {
        lines.push(Line::from(""));
        lines.push(Line::from(model_header.clone()));
        let frames = [
            "\u{25cb}", "\u{25d4}", "\u{25d1}", "\u{25d5}", "\u{25cf}", "\u{25d5}", "\u{25d1}",
            "\u{25d4}",
        ];
        let idx = (app.tick_count / 8 % frames.len() as u64) as usize;
        let has_live_thinking = !app.current_thinking.is_empty();
        let mut thinking_spans = vec![
            Span::styled(format!("{}{} ", pad, frames[idx]), app.theme.thinking),
            Span::styled(
                "thinking",
                ratatui::style::Style::default()
                    .fg(app.theme.muted_fg)
                    .add_modifier(ratatui::style::Modifier::ITALIC),
            ),
        ];
        if let Some(elapsed) = app.streaming_elapsed_secs() {
            thinking_spans.push(Span::styled(
                format!(" \u{00b7} {}", super::ui::format_elapsed(elapsed)),
                app.theme.dim,
            ));
        }
        if has_live_thinking {
            thinking_spans.push(Span::styled("  [t]", app.theme.dim));
        }

        lines.push(Line::from(thinking_spans));
        if has_live_thinking && app.thinking_expanded {
            let thinking = app.current_thinking.clone();
            for text_line in thinking.lines() {
                lines.push(Line::from(vec![
                    Span::styled(format!("{}\u{2502} ", pad), app.theme.thinking),
                    Span::styled(
                        text_line.to_string(),
                        ratatui::style::Style::default()
                            .fg(app.theme.muted_fg)
                            .add_modifier(ratatui::style::Modifier::ITALIC),
                    ),
                ]));
            }
        }
    }
}

pub fn tool_category_style(category: &ToolCategory, theme: &Theme) -> Style {
    match category {
        ToolCategory::FileRead => theme.tool_file_read,
        ToolCategory::FileWrite => theme.tool_file_write,
        ToolCategory::MultiEdit => theme.tool_file_write,
        ToolCategory::Directory => theme.tool_directory,
        ToolCategory::Search => theme.tool_search,
        ToolCategory::Command => theme.tool_command,
        ToolCategory::Mcp { .. } => theme.tool_mcp,
        ToolCategory::Skill => theme.tool_skill,
        ToolCategory::Glob | ToolCategory::Grep => theme.tool_search,
        ToolCategory::WebFetch => theme.tool_mcp,
        ToolCategory::Patch => theme.tool_file_write,
        ToolCategory::Batch => theme.tool_command,
        ToolCategory::Snapshot => theme.tool_directory,
        ToolCategory::Question => theme.tool_skill,
        ToolCategory::Unknown => theme.tool_name,
    }
}
