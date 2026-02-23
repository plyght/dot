use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
};

use crate::tui::app::{App, AppMode};
use crate::tui::markdown;
use crate::tui::ui_popups;
use crate::tui::ui_tools;

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

    if app.command_palette.visible {
        ui_popups::draw_command_palette(frame, app, chunks[2]);
    }
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let mode_indicator = match app.mode {
        AppMode::Normal => Span::styled(
            " NORMAL ",
            Style::default()
                .fg(app.theme.mode_normal_fg)
                .bg(app.theme.mode_normal_bg),
        ),
        AppMode::Insert => Span::styled(
            " INSERT ",
            Style::default()
                .fg(app.theme.mode_insert_fg)
                .bg(app.theme.mode_insert_bg),
        ),
    };

    let sep = Span::styled(" \u{2502} ", app.theme.border);

    let model_short = shorten_model(&app.model_name);
    let model_display = format!("{}/{}", app.provider_name, model_short);

    let mut spans = vec![
        Span::styled(
            " dot ",
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        sep.clone(),
        Span::styled(model_display, app.theme.status_bar),
    ];

    if app.agent_name != "default" && !app.agent_name.is_empty() {
        spans.push(sep.clone());
        spans.push(Span::styled(
            format!("@{}", app.agent_name),
            Style::default().fg(app.theme.accent),
        ));
    }

    spans.push(Span::raw("  "));
    spans.push(mode_indicator);

    if let Some(elapsed) = app.streaming_elapsed_secs() {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(format_elapsed(elapsed), app.theme.thinking));
    }

    let header = Line::from(spans);
    frame.render_widget(Paragraph::new(header), area);
}

fn draw_messages(frame: &mut Frame, app: &mut App, area: Rect) {
    let inner = Rect {
        x: area.x + 1,
        y: area.y,
        width: area.width.saturating_sub(3),
        height: area.height,
    };

    let mut all_lines: Vec<Line<'static>> = Vec::new();

    for msg in &app.messages {
        all_lines.push(Line::from(""));

        if msg.role == "user" {
            all_lines.push(Line::from(vec![
                Span::styled(
                    "  \u{25cf} ",
                    Style::default().fg(app.theme.muted_fg),
                ),
                Span::styled("You", app.theme.user_label),
            ]));
            for text_line in msg.content.lines() {
                all_lines.push(Line::from(Span::raw(format!("    {}", text_line))));
            }
        } else {
            all_lines.push(Line::from(vec![
                Span::styled(
                    "  \u{25c6} ",
                    Style::default().fg(app.theme.accent),
                ),
                Span::styled("Assistant", app.theme.assistant_label),
            ]));

            let md_lines =
                markdown::render_markdown(&msg.content, &app.theme, inner.width);
            for line in md_lines {
                let mut padded = vec![Span::raw("    ")];
                padded.extend(line.spans);
                all_lines.push(Line::from(padded));
            }

            ui_tools::render_tool_calls(&msg.tool_calls, &app.theme, &mut all_lines);
        }
    }

    if app.is_streaming {
        ui_tools::render_streaming_state(app, inner.width, &mut all_lines);
    }

    if let Some(ref err) = app.error_message {
        all_lines.push(Line::from(""));
        all_lines.push(Line::from(Span::styled(
            format!("    {}", err),
            app.theme.dim,
        )));
    }

    if all_lines.is_empty() {
        all_lines.extend(ui_popups::draw_empty_state(app));
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

    if app.max_scroll > 0 {
        let scrollbar_area = Rect {
            x: area.x + area.width.saturating_sub(1),
            y: area.y + 1,
            width: 1,
            height: area.height.saturating_sub(1),
        };
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .track_symbol(Some("\u{2502}"))
            .thumb_symbol("\u{2588}")
            .begin_symbol(None)
            .end_symbol(None)
            .track_style(app.theme.scrollbar_track)
            .thumb_style(app.theme.scrollbar_thumb);

        let mut state =
            ScrollbarState::new(app.max_scroll as usize).position(app.scroll_offset as usize);
        frame.render_stateful_widget(scrollbar, scrollbar_area, &mut state);
    }
}

fn draw_input(frame: &mut Frame, app: &App, area: Rect) {
    let border_style = if app.mode == AppMode::Insert && !app.is_streaming {
        Style::default().fg(app.theme.accent)
    } else {
        app.theme.border
    };

    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(border_style);

    let inner = block.inner(area);

    let display_lines: Vec<Line<'static>> = if app.is_streaming {
        let spinner = ["\u{25dc}", "\u{25dd}", "\u{25de}", "\u{25df}"];
        let idx = (app.tick_count / 2 % spinner.len() as u64) as usize;

        let mut spans = vec![
            Span::styled(format!("  {} ", spinner[idx]), app.theme.dim),
            Span::styled("generating response", app.theme.dim),
        ];
        if let Some(elapsed) = app.streaming_elapsed_secs() {
            spans.push(Span::styled(
                format!(" {}", format_elapsed(elapsed)),
                app.theme.dim,
            ));
        }
        vec![Line::from(spans)]
    } else if app.input.is_empty() {
        vec![Line::from(vec![
            Span::styled("\u{276f} ", app.theme.input_prompt),
            Span::styled("message or /model, /help, /agent", app.theme.dim),
        ])]
    } else {
        let mut lines = Vec::new();
        for (i, line) in app.input.lines().enumerate() {
            if i == 0 {
                lines.push(Line::from(vec![
                    Span::styled("\u{276f} ", app.theme.input_prompt),
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

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let left = format!(
        " \u{25b8} {}in \u{00b7} {}out",
        format_tokens(app.usage.input_tokens),
        format_tokens(app.usage.output_tokens),
    );

    let scroll_indicator = if app.max_scroll > 0 {
        let pct = if app.max_scroll == 0 {
            100
        } else {
            (app.scroll_offset as u32 * 100 / app.max_scroll as u32).min(100)
        };
        format!(" {}% ", pct)
    } else {
        String::new()
    };

    let right = if app.model_selector.visible || app.agent_selector.visible {
        "\u{2191}\u{2193} select \u{00b7} enter confirm \u{00b7} esc cancel "
    } else if app.mode == AppMode::Insert {
        "/model \u{00b7} /agent \u{00b7} esc normal \u{00b7} ctrl+c quit "
    } else {
        "i insert \u{00b7} j/k scroll \u{00b7} tab agents \u{00b7} q quit "
    };

    let padding = area.width.saturating_sub(
        left.len() as u16 + scroll_indicator.len() as u16 + right.len() as u16,
    );

    let line = Line::from(vec![
        Span::styled(left, app.theme.status_bar),
        Span::styled(scroll_indicator, app.theme.dim),
        Span::raw(" ".repeat(padding as usize)),
        Span::styled(right.to_string(), app.theme.status_bar),
    ]);

    frame.render_widget(Paragraph::new(line), area);
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

fn shorten_model(model: &str) -> String {
    if model.len() <= 30 {
        return model.to_string();
    }
    if let Some(idx) = model.rfind('-') {
        let suffix = &model[idx..];
        if suffix.len() > 8 {
            return format!("{}{}", &model[..25], "\u{2026}");
        }
    }
    format!("{}\u{2026}", &model[..29])
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
