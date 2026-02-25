use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::tui::app::App;
use crate::tui::markdown;
use crate::tui::theme::Theme;
use crate::tui::tools::{ToolCallDisplay, ToolCategory, extract_tool_detail};

pub fn render_tool_calls(
    tool_calls: &[ToolCallDisplay],
    theme: &Theme,
    compact: bool,
    lines: &mut Vec<Line<'static>>,
) {
    render_tool_calls_inner(tool_calls, theme, compact, lines, true);
}

pub fn render_tool_calls_compact(
    tool_calls: &[ToolCallDisplay],
    theme: &Theme,
    compact: bool,
    lines: &mut Vec<Line<'static>>,
) {
    render_tool_calls_inner(tool_calls, theme, compact, lines, false);
}

fn render_tool_calls_inner(
    tool_calls: &[ToolCallDisplay],
    theme: &Theme,
    compact: bool,
    lines: &mut Vec<Line<'static>>,
    show_verbose_output: bool,
) {
    let hdr_pad = if compact { "  " } else { "    " };
    let out_pad = if compact { "      " } else { "          " };

    for tc in tool_calls {
        let cat_style = tool_category_style(&tc.category, theme);
        let label = tc.category.label();
        let (status_icon, status_style) = if tc.is_error {
            ("\u{2717}", theme.tool_exit_err)
        } else {
            ("\u{2713}", theme.tool_exit_ok)
        };
        let label_style = if tc.is_error { theme.error } else { cat_style };
        let mut header_spans = vec![
            Span::styled(format!("{}{} ", hdr_pad, status_icon), status_style),
            Span::styled(format!("{:<6}", label), label_style),
        ];

        if !tc.detail.is_empty() {
            match &tc.category {
                ToolCategory::FileRead | ToolCategory::FileWrite | ToolCategory::Directory => {
                    header_spans.push(Span::styled(tc.detail.clone(), theme.tool_path));
                }
                ToolCategory::Command => {
                    header_spans.push(Span::styled(
                        format!("$ {}", tc.detail),
                        Style::default().fg(theme.muted_fg),
                    ));
                }
                ToolCategory::Search => {
                    header_spans.push(Span::styled(tc.detail.clone(), theme.dim));
                }
                ToolCategory::Mcp { .. } => {
                    let mcp_tool_name = tc.name.split('_').skip(1).collect::<Vec<_>>().join("_");
                    if !mcp_tool_name.is_empty() {
                        header_spans.push(Span::styled(mcp_tool_name, theme.tool_name));
                        if !tc.detail.is_empty() {
                            header_spans.push(Span::raw(" "));
                            header_spans.push(Span::styled(tc.detail.clone(), theme.dim));
                        }
                    }
                }
                ToolCategory::Skill => {
                    header_spans.push(Span::styled(tc.detail.clone(), theme.tool_skill));
                }
                ToolCategory::Glob | ToolCategory::Grep => {
                    header_spans.push(Span::styled(tc.detail.clone(), theme.dim));
                }
                ToolCategory::WebFetch => {
                    header_spans.push(Span::styled(tc.detail.clone(), theme.tool_path));
                }
                ToolCategory::Patch => {
                    header_spans.push(Span::styled(tc.detail.clone(), theme.dim));
                }
                ToolCategory::Snapshot => {
                    header_spans.push(Span::styled(tc.detail.clone(), theme.dim));
                }
                ToolCategory::Question => {
                    header_spans.push(Span::styled(tc.detail.clone(), theme.dim));
                }
                ToolCategory::Unknown => {
                    header_spans.push(Span::styled(tc.name.clone(), theme.tool_name));
                }
            }
        } else {
            header_spans.push(Span::styled(tc.name.clone(), theme.tool_name));
        }

        lines.push(Line::from(header_spans));

        let should_show = if tc.is_error {
            true
        } else if !show_verbose_output {
            matches!(tc.category, ToolCategory::FileWrite | ToolCategory::Patch)
        } else {
            !matches!(tc.category, ToolCategory::FileRead)
        };

        if let Some(ref output) = tc.output
            && should_show
            && !output.is_empty()
        {
            let max_lines = if tc.is_error { 6 } else { 4 };
            let max_chars = 400;
            let preview: String = output.chars().take(max_chars).collect();
            let trimmed = if output.len() > max_chars {
                format!("{}\u{2026}", preview)
            } else {
                preview
            };

            let output_style = if tc.is_error {
                theme.error
            } else {
                theme.tool_output
            };

            for ol in trimmed.lines().take(max_lines) {
                lines.push(Line::from(Span::styled(
                    format!("{}{}", out_pad, ol),
                    output_style,
                )));
            }
            let total_lines_in_output = trimmed.lines().count();
            if total_lines_in_output > max_lines || output.len() > max_chars {
                lines.push(Line::from(Span::styled(
                    format!(
                        "{}\u{2026} {} more lines",
                        out_pad,
                        output.lines().count().saturating_sub(max_lines)
                    ),
                    theme.dim,
                )));
            }
        }
    }
}

pub fn render_streaming_state(app: &App, width: u16, lines: &mut Vec<Line<'static>>) {
    let compact = width < 55;
    let pad = if compact { "  " } else { "    " };
    let pad_cols: u16 = if compact { 2 } else { 4 };
    let diamond_sp = if compact { " \u{25c6} " } else { "  \u{25c6} " };

    let has_text = !app.current_response.is_empty();
    let has_tool = app.pending_tool_name.is_some();
    let has_completed_tools = !app.current_tool_calls.is_empty();

    let model_label = super::ui::shorten_model(&app.model_name);
    let model_header = vec![
        Span::styled(diamond_sp, Style::default().fg(app.theme.accent)),
        Span::styled(model_label, app.theme.dim),
    ];

    if has_completed_tools && !has_text {
        lines.push(Line::from(""));
        lines.push(Line::from(model_header.clone()));
        render_tool_calls(&app.current_tool_calls, &app.theme, compact, lines);
    }

    if has_text {
        if !has_completed_tools {
            lines.push(Line::from(""));
            lines.push(Line::from(model_header.clone()));
        }

        if has_completed_tools {
            render_tool_calls(&app.current_tool_calls, &app.theme, compact, lines);
            lines.push(Line::from(""));
        }

        let md_lines = markdown::render_markdown(
            &app.current_response,
            &app.theme,
            width.saturating_sub(pad_cols),
        );
        for line in md_lines {
            let mut padded = vec![Span::raw(pad)];
            padded.extend(line.spans);
            lines.push(Line::from(padded));
        }
        let blink = (app.tick_count / 32).is_multiple_of(2);
        if blink {
            lines.push(Line::from(Span::styled(
                format!("{}\u{258d}", pad),
                Style::default().fg(app.theme.accent),
            )));
        } else {
            lines.push(Line::from(Span::raw(pad)));
        }
    } else if has_tool {
        if !has_completed_tools {
            lines.push(Line::from(""));
            lines.push(Line::from(model_header.clone()));
        }

        let tool_name = app.pending_tool_name.as_deref().unwrap_or("");
        let category = ToolCategory::from_name(tool_name);
        let detail = extract_tool_detail(tool_name, &app.pending_tool_input);

        let cat_style = tool_category_style(&category, &app.theme);
        let frames = ["\u{25cb}", "\u{25d4}", "\u{25d1}", "\u{25d5}", "\u{25cf}", "\u{25d5}", "\u{25d1}", "\u{25d4}"];
        let idx = (app.tick_count / 8 % frames.len() as u64) as usize;
        let intent = category.intent();
        let mut tool_spans = vec![
            Span::styled(format!("{}{} ", pad, frames[idx]), cat_style),
            Span::styled(format!("{} ", intent), cat_style),
        ];

        if !detail.is_empty() {
            match &category {
                ToolCategory::FileRead | ToolCategory::FileWrite | ToolCategory::Directory => {
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

        if has_completed_tools {
            let n = app.current_tool_calls.len();
            tool_spans.push(Span::styled(
                format!(" \u{00b7} {} done", n),
                app.theme.dim,
            ));
        }

        if let Some(elapsed) = app.streaming_elapsed_secs() {
            tool_spans.push(Span::styled(
                format!(" \u{00b7} {}", super::ui::format_elapsed(elapsed)),
                app.theme.dim,
            ));
        }

        lines.push(Line::from(tool_spans));
    } else if !has_completed_tools {
        lines.push(Line::from(""));
        lines.push(Line::from(model_header.clone()));
        let frames = ["\u{25cb}", "\u{25d4}", "\u{25d1}", "\u{25d5}", "\u{25cf}", "\u{25d5}", "\u{25d1}", "\u{25d4}"];
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
        ToolCategory::Directory => theme.tool_directory,
        ToolCategory::Search => theme.tool_search,
        ToolCategory::Command => theme.tool_command,
        ToolCategory::Mcp { .. } => theme.tool_mcp,
        ToolCategory::Skill => theme.tool_skill,
        ToolCategory::Glob | ToolCategory::Grep => theme.tool_search,
        ToolCategory::WebFetch => theme.tool_mcp,
        ToolCategory::Patch => theme.tool_file_write,
        ToolCategory::Snapshot => theme.tool_directory,
        ToolCategory::Question => theme.tool_skill,
        ToolCategory::Unknown => theme.tool_name,
    }
}
