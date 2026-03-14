use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::tui::app::App;
use crate::tui::markdown;
use crate::tui::theme::Theme;
use crate::tui::tools::{StreamSegment, ToolCallDisplay, ToolCategory, extract_tool_detail};

struct ToolCallsRenderCtx<'a> {
    theme: &'a Theme,
    compact: bool,
    msg_idx: usize,
    width: u16,
    tool_idx_base: usize,
}

pub struct RenderToolCallsParams<'a> {
    pub tool_calls: &'a [ToolCallDisplay],
    pub theme: &'a Theme,
    pub compact: bool,
    pub lines: &'a mut Vec<Line<'static>>,
    pub line_to_tool: Option<&'a mut Vec<Option<(usize, usize)>>>,
    pub msg_idx: usize,
    pub width: u16,
    pub tool_idx_base: usize,
}

pub fn render_tool_calls(params: RenderToolCallsParams<'_>, is_expanded: impl Fn(usize) -> bool) {
    render_tool_calls_inner(
        params.tool_calls,
        &ToolCallsRenderCtx {
            theme: params.theme,
            compact: params.compact,

            msg_idx: params.msg_idx,
            width: params.width,
            tool_idx_base: params.tool_idx_base,
        },
        params.lines,
        is_expanded,
        params.line_to_tool,
    );
}

pub fn render_tool_calls_compact(
    params: RenderToolCallsParams<'_>,
    is_expanded: impl Fn(usize) -> bool,
) {
    render_tool_calls_inner(
        params.tool_calls,
        &ToolCallsRenderCtx {
            theme: params.theme,
            compact: params.compact,

            msg_idx: params.msg_idx,
            width: params.width,
            tool_idx_base: params.tool_idx_base,
        },
        params.lines,
        is_expanded,
        params.line_to_tool,
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
    let pad = if compact { " " } else { "  " };
    let multi = tool_calls.len() > 1;

    for (tool_idx, tc) in tool_calls.iter().enumerate() {
        let expanded = is_expanded(tool_idx);
        let has_content = tc.output.as_ref().is_some_and(|o| !o.is_empty())
            || matches!(
                tc.category,
                ToolCategory::MultiEdit | ToolCategory::Patch | ToolCategory::FileWrite
            );
        let tree = if multi {
            if tool_idx == tool_calls.len() - 1 {
                "\u{2514}\u{2500} "
            } else {
                "\u{251c}\u{2500} "
            }
        } else {
            "  "
        };

        if expanded {
            let cat_style = tool_category_style(&tc.category, ctx.theme);
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
            let label = tc.category.label();

            let mut header_spans = vec![];
            if has_content {
                header_spans.push(Span::styled(
                    format!("{}{}\u{25be} ", pad, tree),
                    ctx.theme.dim,
                ));
            } else {
                header_spans.push(Span::raw(format!("{}{}", pad, tree)));
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
                        let mcp_tool = tc.name.split('_').skip(1).collect::<Vec<_>>().join("_");
                        if !mcp_tool.is_empty() {
                            header_spans.push(Span::styled(mcp_tool, ctx.theme.tool_name));
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
                    ToolCategory::Subagent => {
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
                ltt.push(Some((ctx.msg_idx, ctx.tool_idx_base + tool_idx)));
            }

            render_expanded_output(tc, ctx, lines, &mut line_to_tool);
        } else {
            let label = tc.category.label();
            let detail = collapsed_detail(tc);
            let line_style = if tc.is_error {
                Style::default()
            } else {
                ctx.theme.dim
            };
            let status_icon = if tc.is_error { "\u{2717}" } else { "\u{2713}" };

            lines.push(Line::from(Span::styled(
                format!("{}{}{} {:<6}{}", pad, tree, status_icon, label, detail),
                line_style,
            )));
            if let Some(ltt) = &mut line_to_tool {
                if has_content {
                    ltt.push(Some((ctx.msg_idx, ctx.tool_idx_base + tool_idx)));
                } else {
                    ltt.push(None);
                }
            }
        }
    }
}

fn collapsed_detail(tc: &ToolCallDisplay) -> String {
    if tc.detail.is_empty() {
        return tc.name.clone();
    }
    match &tc.category {
        ToolCategory::Command => format!("$ {}", tc.detail),
        ToolCategory::Mcp { .. } => {
            let mcp = tc.name.split('_').skip(1).collect::<Vec<_>>().join("_");
            if mcp.is_empty() {
                tc.detail.clone()
            } else {
                format!("{} {}", mcp, tc.detail)
            }
        }
        _ => tc.detail.clone(),
    }
}

fn render_expanded_output(
    tc: &ToolCallDisplay,
    ctx: &ToolCallsRenderCtx<'_>,
    lines: &mut Vec<Line<'static>>,
    line_to_tool: &mut Option<&mut Vec<Option<(usize, usize)>>>,
) {
    let compact = ctx.compact;
    let indent: &str = if compact { "  " } else { "    " };
    let indent_len: u16 = if compact { 2 } else { 4 };
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

pub fn render_streaming_state(
    app: &App,
    width: u16,
    lines: &mut Vec<Line<'static>>,
    line_to_tool: &mut Vec<Option<(usize, usize)>>,
    stream_msg_idx: usize,
) {
    let compact = width < 55;
    let pad = if compact { " " } else { "  " };
    let pad_cols: u16 = if compact { 1 } else { 2 };

    let has_segments = !app.streaming_segments.is_empty();
    let has_text = !app.current_response.is_empty();
    let has_tool = app.pending_tool_name.is_some();

    lines.push(Line::from(""));
    line_to_tool.push(None);

    let mut prev_was_tool = false;
    let mut tool_idx_base = 0;
    for seg in &app.streaming_segments {
        match seg {
            StreamSegment::Text(t) => {
                if prev_was_tool {
                    lines.push(Line::from(""));
                    line_to_tool.push(None);
                }
                let md_lines =
                    markdown::render_markdown(t, &app.theme, width.saturating_sub(pad_cols));
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
                    line_to_tool.push(None);
                }
                prev_was_tool = false;
            }
            StreamSegment::ToolCall(tc) => {
                if !prev_was_tool && !lines.is_empty() {
                    lines.push(Line::from(""));
                    line_to_tool.push(None);
                }
                let base = tool_idx_base;
                render_tool_calls_compact(
                    RenderToolCallsParams {
                        tool_calls: std::slice::from_ref(tc),
                        theme: &app.theme,
                        compact,
                        lines,
                        line_to_tool: Some(line_to_tool),
                        msg_idx: stream_msg_idx,
                        width,
                        tool_idx_base: base,
                    },
                    |i| {
                        app.expanded_tool_calls
                            .contains(&(stream_msg_idx, base + i))
                    },
                );
                tool_idx_base += 1;
                prev_was_tool = true;
            }
        }
    }

    if has_text {
        if prev_was_tool {
            lines.push(Line::from(""));
            line_to_tool.push(None);
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
            line_to_tool.push(None);
        }
    } else if has_tool {
        let tool_name = app.pending_tool_name.as_deref().unwrap_or("");
        let category = ToolCategory::from_name(tool_name);
        let detail = extract_tool_detail(tool_name, &app.pending_tool_input);

        let frames = [
            "\u{25cb}", "\u{25d4}", "\u{25d1}", "\u{25d5}", "\u{25cf}", "\u{25d5}", "\u{25d1}",
            "\u{25d4}",
        ];
        let idx = (app.tick_count / 8 % frames.len() as u64) as usize;
        let intent = category.intent();

        let mut tool_spans = vec![
            Span::raw(pad.to_string()),
            Span::raw(format!("{} {} ", frames[idx], intent)),
        ];

        if !detail.is_empty() {
            match &category {
                ToolCategory::Command => {
                    tool_spans.push(Span::raw(format!("$ {}", detail)));
                }
                ToolCategory::Mcp { .. } => {
                    let mcp_tool = tool_name.split('_').skip(1).collect::<Vec<_>>().join("_");
                    if !mcp_tool.is_empty() {
                        tool_spans.push(Span::raw(mcp_tool));
                        tool_spans.push(Span::raw(" "));
                    }
                    tool_spans.push(Span::raw(detail));
                }
                _ => {
                    tool_spans.push(Span::raw(detail));
                }
            }
        } else {
            tool_spans.push(Span::raw(tool_name.to_string()));
        }

        if let Some(ref sub) = app.active_subagent {
            if let Some(ref tool) = sub.current_tool {
                let tool_detail = sub.current_tool_detail.as_deref().unwrap_or("");
                let label = if tool_detail.is_empty() {
                    tool.clone()
                } else {
                    format!("{} {}", tool, tool_detail)
                };
                tool_spans.push(Span::styled(format!(" \u{00b7} {}", label), app.theme.dim));
            }
            let word_count = sub.output.split_whitespace().count();
            let parts: Vec<String> = [
                if sub.tools_completed > 0 {
                    Some(format!("{} tools", sub.tools_completed))
                } else {
                    None
                },
                if word_count > 0 {
                    Some(format!("{}w", word_count))
                } else {
                    None
                },
            ]
            .into_iter()
            .flatten()
            .collect();
            if !parts.is_empty() {
                tool_spans.push(Span::styled(
                    format!(" \u{00b7} {}", parts.join(", ")),
                    app.theme.dim,
                ));
            }
        } else if has_segments {
            let n = app.current_tool_calls.len();
            tool_spans.push(Span::styled(format!(" \u{00b7} {} done", n), app.theme.dim));
        }

        let left_width: usize = tool_spans.iter().map(|s| s.content.chars().count()).sum();
        let mut right_spans: Vec<Span<'static>> = Vec::new();
        if let Some(elapsed) = app.streaming_elapsed_secs() {
            right_spans.push(Span::styled(
                format!(" \u{00b7} {}", super::ui::format_elapsed(elapsed)),
                app.theme.dim,
            ));
        }
        let right_width: usize = right_spans.iter().map(|s| s.content.chars().count()).sum();
        let padding = (width as usize).saturating_sub(left_width + right_width);
        tool_spans.push(Span::raw(" ".repeat(padding)));
        tool_spans.extend(right_spans);

        lines.push(Line::from(tool_spans));
        line_to_tool.push(None);
    } else {
        let blink_on = (app.tick_count / 30).is_multiple_of(2);
        let dot_char = if blink_on { "\u{00b7}" } else { " " };
        let mut dot_spans = vec![
            Span::raw(pad.to_string()),
            Span::styled(dot_char.to_string(), app.theme.streaming_dot),
        ];
        let mut right_spans: Vec<Span<'static>> = Vec::new();
        if let Some(elapsed) = app.streaming_elapsed_secs()
            && elapsed > 3.0
        {
            right_spans.push(Span::styled(format!(" {}s", elapsed as u64), app.theme.dim));
        }
        let left_width: usize = dot_spans.iter().map(|s| s.content.chars().count()).sum();
        let right_width: usize = right_spans.iter().map(|s| s.content.chars().count()).sum();
        let padding = (width as usize).saturating_sub(left_width + right_width);
        dot_spans.push(Span::raw(" ".repeat(padding)));
        dot_spans.extend(right_spans);
        let has_live_thinking = !app.current_thinking.is_empty();
        lines.push(Line::from(dot_spans));
        line_to_tool.push(None);
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
                line_to_tool.push(None);
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
        ToolCategory::Subagent => theme.tool_skill,
        ToolCategory::Unknown => theme.tool_name,
    }
}
