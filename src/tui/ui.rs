use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::tui::app::{App, AppMode, COMMANDS};
use crate::tui::markdown;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(5),
            Constraint::Length(1),
        ])
        .split(frame.area());

    draw_header(frame, app, chunks[0]);
    draw_messages(frame, app, chunks[1]);
    draw_input(frame, app, chunks[2]);
    draw_status(frame, app, chunks[3]);

    if app.model_selector.visible {
        draw_model_selector(frame, app);
    }

    if app.command_palette.visible {
        draw_command_palette(frame, app, chunks[2]);
    }


fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let mode_indicator = match app.mode {
        AppMode::Normal => Span::styled(
            " NORMAL ",
            Style::default().fg(Color::Black).bg(Color::DarkGray),
        ),
        AppMode::Insert => Span::styled(
            " INSERT ",
            Style::default().fg(Color::Black).bg(Color::Cyan),
        ),
    };

    let header = Line::from(vec![
        Span::styled(
            " dot",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" · ", app.theme.border),
        Span::styled(app.model_name.clone(), app.theme.status_bar),
        Span::raw("  "),
        mode_indicator,
    ]);

    frame.render_widget(Paragraph::new(header), area);
}

fn draw_messages(frame: &mut Frame, app: &mut App, area: Rect) {
    let inner = Rect {
        x: area.x + 1,
        y: area.y,
        width: area.width.saturating_sub(2),
        height: area.height,
    };

    let mut all_lines: Vec<Line<'static>> = Vec::new();

    for msg in &app.messages {
        all_lines.push(Line::from(""));

        if msg.role == "user" {
            all_lines.push(Line::from(Span::styled("You", app.theme.user_label)));
            for text_line in msg.content.lines() {
                all_lines.push(Line::from(Span::raw(text_line.to_string())));
            }
        } else {
            all_lines.push(Line::from(Span::styled(
                "Assistant",
                app.theme.assistant_label,
            )));

            let md_lines = markdown::render_markdown(&msg.content, &app.theme, inner.width);
            all_lines.extend(md_lines);

            for tc in &msg.tool_calls {
                all_lines.push(Line::from(""));
                let status = if tc.is_error { "✗" } else { "✓" };
                all_lines.push(Line::from(vec![
                    Span::styled(
                        format!("  {} ", status),
                        if tc.is_error {
                            app.theme.error
                        } else {
                            Style::default().fg(Color::Green)
                        },
                    ),
                    Span::styled(tc.name.clone(), app.theme.tool_name),
                ]));
                if let Some(ref output) = tc.output {
                    let preview: String = output.chars().take(200).collect();
                    let trimmed = if output.len() > 200 {
                        format!("{}...", preview)
                    } else {
                        preview
                    };
                    for ol in trimmed.lines().take(4) {
                        all_lines.push(Line::from(Span::styled(
                            format!("    {}", ol),
                            app.theme.tool_output,
                        )));
                    }
                }
            }
        }
    }

    if app.is_streaming && !app.current_response.is_empty() {
        all_lines.push(Line::from(""));
        all_lines.push(Line::from(Span::styled(
            "Assistant",
            app.theme.assistant_label,
        )));
        let md_lines = markdown::render_markdown(&app.current_response, &app.theme, inner.width);
        all_lines.extend(md_lines);
        all_lines.push(Line::from(Span::styled(
            "▊",
            Style::default().fg(Color::Cyan),
        )));
    } else if app.is_streaming {
        all_lines.push(Line::from(""));
        if let Some(ref tool_name) = app.pending_tool_name {
            all_lines.push(Line::from(vec![
                Span::styled("  ◌ ", Style::default().fg(Color::Yellow)),
                Span::styled(tool_name.clone(), app.theme.tool_name),
                Span::styled(" ...", app.theme.dim),
            ]));
        } else {
            all_lines.push(Line::from(Span::styled("  thinking...", app.theme.dim)));
        }
    }

    if let Some(ref err) = app.error_message {
        all_lines.push(Line::from(""));
        all_lines.push(Line::from(Span::styled(
            format!("  {}", err),
            app.theme.dim,
        )));
    }

    if all_lines.is_empty() {
        all_lines.push(Line::from(""));
        all_lines.push(Line::from(Span::styled(
            "  no messages yet. start typing.",
            app.theme.dim,
        )));
    }

    let total_lines = all_lines.len() as u16;
    let visible = inner.height;
    app.max_scroll = total_lines.saturating_sub(visible);
    if app.scroll_offset > app.max_scroll {
        app.scroll_offset = app.max_scroll;
    }

    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(app.theme.border);

    let paragraph = Paragraph::new(all_lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset, 0));

    frame.render_widget(paragraph, area);
}

fn draw_input(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(app.theme.border);

    let inner = block.inner(area);

    let display_lines: Vec<Line<'static>> = if app.is_streaming {
        vec![Line::from(Span::styled(
            "  waiting for response...",
            app.theme.dim,
        ))]
    } else if app.input.is_empty() {
        vec![Line::from(vec![
            Span::styled("> ", app.theme.input_prompt),
            Span::styled("message or /model, /help", app.theme.dim),
        ])]
    } else {
        let mut lines = Vec::new();
        for (i, line) in app.input.lines().enumerate() {
            if i == 0 {
                lines.push(Line::from(vec![
                    Span::styled("> ", app.theme.input_prompt),
                    Span::raw(line.to_string()),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::raw(line.to_string()),
                ]));
            }
        }
        if app.input.ends_with('\n') {
            lines.push(Line::from(Span::raw("  ")));
        }
        lines
    };

    let paragraph = Paragraph::new(display_lines).wrap(Wrap { trim: false });

    frame.render_widget(block, area);
    frame.render_widget(paragraph, inner);

    if app.mode == AppMode::Insert && !app.is_streaming && !app.model_selector.visible {
        let (cx, cy) = cursor_position(&app.input, app.cursor_pos, inner);
        if cy < inner.y + inner.height {
            frame.set_cursor_position((cx, cy));
        }
    }
}

fn cursor_position(input: &str, byte_pos: usize, area: Rect) -> (u16, u16) {
    let before = &input[..byte_pos.min(input.len())];
    let mut row: u16 = 0;
    let mut col: u16 = 2;

    for ch in before.chars() {
        if ch == '\n' {
            row += 1;
            col = 2;
        } else {
            col += 1;
        }
    }

    (area.x + col, area.y + row)
}

fn draw_model_selector(frame: &mut Frame, app: &App) {
    let models = &app.model_selector.models;
    let selected = app.model_selector.selected;

    let width = models.iter().map(|m| m.len()).max().unwrap_or(20) as u16 + 6;
    let height = models.len() as u16 + 4;

    let area = frame.area();
    let x = area.width.saturating_sub(width) / 2;
    let y = area.height.saturating_sub(height) / 2;

    let popup = Rect::new(x, y, width.min(area.width), height.min(area.height));

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" model ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let mut lines: Vec<Line<'static>> = Vec::new();

    for (i, model) in models.iter().enumerate() {
        let is_current = model == &app.model_name;
        let marker = if is_current { "* " } else { "  " };

        if i == selected {
            lines.push(Line::from(Span::styled(
                format!(" {}{} ", marker, model),
                Style::default().fg(Color::Black).bg(Color::Cyan),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                format!(" {}{} ", marker, model),
                Style::default().fg(Color::Reset),
            )));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " j/k ↑↓ select · enter confirm · esc cancel",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let left = format!(
        " tokens: {}in · {}out",
        format_tokens(app.usage.input_tokens),
        format_tokens(app.usage.output_tokens),
    );

    let right = if app.model_selector.visible {
        "j/k select · enter confirm · esc cancel "
    } else if app.mode == AppMode::Insert {
        "/model · esc normal · ctrl+c quit "
    } else {
        "i insert · j/k scroll · q quit "
    };

    let padding = area
        .width
        .saturating_sub(left.len() as u16 + right.len() as u16);

    let line = Line::from(vec![
        Span::styled(left, app.theme.status_bar),
        Span::raw(" ".repeat(padding as usize)),
        Span::styled(right.to_string(), app.theme.status_bar),
    ]);

    frame.render_widget(Paragraph::new(line), area);
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
