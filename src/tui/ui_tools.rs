use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::tui::app::App;
use crate::tui::markdown;
use crate::tui::theme::Theme;
use crate::tui::tools::{ToolCallDisplay, ToolCategory, extract_tool_detail};

pub fn render_tool_calls(
    tool_calls: &[ToolCallDisplay],
    theme: &Theme,
    lines: &mut Vec<Line<'static>>,
) {
    for tc in tool_calls {
        lines.push(Line::from(""));

        let (status_icon, status_style) = if tc.is_error {
            ("\u{2717}", theme.error)
        } else {
            ("\u{2713}", theme.tool_success)
        };

        let cat_style = tool_category_style(&tc.category, theme);
        let label = tc.category.label();

        let mut header_spans = vec![
            Span::styled(format!("    {} ", status_icon), status_style),
            Span::styled(format!("{} ", label), cat_style),
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
                ToolCategory::Unknown => {
                    header_spans.push(Span::styled(tc.name.clone(), theme.tool_name));
                }
            }
        } else {
            header_spans.push(Span::styled(tc.name.clone(), theme.tool_name));
        }

        lines.push(Line::from(header_spans));

        if let Some(ref output) = tc.output {
            let show_output = match &tc.category {
                ToolCategory::FileRead => false,
                ToolCategory::FileWrite if !tc.is_error => false,
                ToolCategory::Directory => true,
                ToolCategory::Search => true,
                ToolCategory::Command => true,
                _ => true,
            };

            if show_output && !output.is_empty() {
                let max_lines = if tc.is_error { 6 } else { 4 };
                let max_chars = 300;
                let preview: String = output.chars().take(max_chars).collect();
                let trimmed = if output.len() > max_chars {
                    format!("{}...", preview)
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
                        format!("      {}", ol),
                        output_style,
                    )));
                }
                let total_lines_in_output = trimmed.lines().count();
                if total_lines_in_output > max_lines || output.len() > max_chars {
                    lines.push(Line::from(Span::styled(
                        format!(
                            "      \u{2026} {} more lines",
                            output.lines().count().saturating_sub(max_lines)
                        ),
                        theme.dim,
                    )));
                }
            }
        }
    }
}

pub fn render_streaming_state(app: &App, width: u16, lines: &mut Vec<Line<'static>>) {
    let has_text = !app.current_response.is_empty();
    let has_tool = app.pending_tool_name.is_some();
    let has_completed_tools = !app.current_tool_calls.is_empty();

    if has_completed_tools && !has_text {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  \u{25c6} ", Style::default().fg(app.theme.accent)),
            Span::styled("Assistant", app.theme.assistant_label),
        ]));
        render_tool_calls(&app.current_tool_calls, &app.theme, lines);
    }

    if has_text {
        if !has_completed_tools {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("  \u{25c6} ", Style::default().fg(app.theme.accent)),
                Span::styled("Assistant", app.theme.assistant_label),
            ]));
        }

        if has_completed_tools {
            render_tool_calls(&app.current_tool_calls, &app.theme, lines);
            lines.push(Line::from(""));
        }

        let md_lines =
            markdown::render_markdown(&app.current_response, &app.theme, width.saturating_sub(4));
        for line in md_lines {
            let mut padded = vec![Span::raw("    ")];
            padded.extend(line.spans);
            lines.push(Line::from(padded));
        }
        let cursor_frames = ["\u{2588}", "\u{2589}", "\u{258a}", "\u{258b}"];
        let idx = (app.tick_count / 2 % cursor_frames.len() as u64) as usize;
        lines.push(Line::from(Span::styled(
            format!("    {}", cursor_frames[idx]),
            Style::default().fg(app.theme.accent),
        )));
    } else if has_tool {
        if !has_completed_tools {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("  \u{25c6} ", Style::default().fg(app.theme.accent)),
                Span::styled("Assistant", app.theme.assistant_label),
            ]));
        }

        let tool_name = app.pending_tool_name.as_deref().unwrap_or("");
        let category = ToolCategory::from_name(tool_name);
        let detail = extract_tool_detail(tool_name, &app.pending_tool_input);

        let spinner = ["\u{25dc}", "\u{25dd}", "\u{25de}", "\u{25df}"];
        let idx = (app.tick_count / 2 % spinner.len() as u64) as usize;

        let cat_style = tool_category_style(&category, &app.theme);
        let label = category.label();

        let mut tool_spans = vec![
            Span::styled(format!("    {} ", spinner[idx]), cat_style),
            Span::styled(format!("{} ", label), cat_style),
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

        if let Some(elapsed) = app.streaming_elapsed_secs() {
            tool_spans.push(Span::styled(
                format!(" {}", super::ui::format_elapsed(elapsed)),
                app.theme.thinking,
            ));
        }

        lines.push(Line::from(tool_spans));
    } else if !has_completed_tools {
        lines.push(Line::from(""));
        let braille = [
            "\u{2840}", "\u{2844}", "\u{2846}", "\u{2847}", "\u{2843}", "\u{2841}",
        ];
        let idx = (app.tick_count / 2 % braille.len() as u64) as usize;
        let has_live_thinking = !app.current_thinking.is_empty();
        let mut thinking_spans = vec![
            Span::styled(format!("    {} ", braille[idx]), app.theme.thinking),
            Span::styled("thinking", app.theme.thinking),
        ];
        if let Some(elapsed) = app.streaming_elapsed_secs() {
            thinking_spans.push(Span::styled(
                format!(" {}", super::ui::format_elapsed(elapsed)),
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
                    Span::styled("    \u{2502} ", app.theme.thinking),
                    Span::styled(text_line.to_string(), app.theme.dim),
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
        ToolCategory::Unknown => theme.tool_name,
    }
}
