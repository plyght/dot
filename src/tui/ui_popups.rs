use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::tui::app::App;
use crate::tui::widgets::COMMANDS;
use crate::tui::widgets::ThinkingLevel;

pub fn draw_model_selector(frame: &mut Frame, app: &mut App) {
    let sel = &app.model_selector;
    if sel.filtered.is_empty() && sel.query.is_empty() {
        return;
    }

    let mut content_lines: Vec<Line<'static>> = Vec::new();
    let mut last_provider: Option<&str> = None;
    let mut item_idx = 0usize;

    for &entry_idx in &sel.filtered {
        let entry = &sel.entries[entry_idx];

        if last_provider != Some(&entry.provider) {
            if last_provider.is_some() {
                content_lines.push(Line::from(""));
            }
            content_lines.push(Line::from(Span::styled(
                format!(" {}", entry.provider.to_uppercase()),
                Style::default()
                    .fg(app.theme.muted_fg)
                    .add_modifier(Modifier::BOLD),
            )));
            last_provider = Some(&entry.provider);
        }

        let is_current = entry.provider == sel.current_provider && entry.model == sel.current_model;
        let marker = if is_current { "\u{25cf} " } else { "  " };

        let style = if item_idx == sel.selected {
            app.theme.highlight
        } else {
            Style::default().fg(Color::Reset)
        };

        content_lines.push(Line::from(Span::styled(
            format!("  {}{} ", marker, entry.model),
            style,
        )));
        item_idx += 1;
    }

    if sel.filtered.is_empty() {
        content_lines.push(Line::from(Span::styled(" no matches", app.theme.dim)));
    }

    let search_line = format!(" /{}", sel.query);
    let footer = " \u{2191}\u{2193}/scroll select \u{00b7} enter confirm \u{00b7} esc cancel";
    let content_width = content_lines
        .iter()
        .map(|l| l.width())
        .max()
        .unwrap_or(20)
        .max(footer.len())
        .max(search_line.len())
        + 4;
    let content_height = content_lines.len() + 4;

    let area = frame.area();
    let width = (content_width as u16).min(area.width.saturating_sub(4));
    let height = (content_height as u16).min(area.height.saturating_sub(2));
    let x = area.width.saturating_sub(width) / 2;
    let y = area.height.saturating_sub(height) / 2;
    let popup = Rect::new(x, y, width, height);

    app.layout.model_selector = Some(popup);

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(Span::styled(
            " model ",
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.muted_fg));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let mut all_lines: Vec<Line<'static>> = Vec::new();

    let search_display = if sel.query.is_empty() {
        Line::from(Span::styled(" type to search...", app.theme.dim))
    } else {
        Line::from(vec![
            Span::styled(" /", Style::default().fg(app.theme.accent)),
            Span::raw(sel.query.clone()),
            Span::styled("\u{258f}", Style::default().fg(app.theme.accent)),
        ])
    };
    all_lines.push(search_display);
    all_lines.push(Line::from(""));

    all_lines.extend(content_lines);

    all_lines.push(Line::from(""));
    all_lines.push(Line::from(Span::styled(footer.to_string(), app.theme.dim)));

    let paragraph = Paragraph::new(all_lines);
    frame.render_widget(paragraph, inner);
}

pub fn draw_agent_selector(frame: &mut Frame, app: &mut App) {
    let sel = &app.agent_selector;
    if sel.entries.is_empty() {
        return;
    }

    let mut content_lines: Vec<Line<'static>> = Vec::new();
    for (i, entry) in sel.entries.iter().enumerate() {
        let is_current = entry.name == sel.current;
        let marker = if is_current { "\u{25cf} " } else { "  " };

        let style = if i == sel.selected {
            app.theme.highlight
        } else {
            Style::default().fg(Color::Reset)
        };

        content_lines.push(Line::from(vec![
            Span::styled(format!("  {}{}", marker, entry.name), style),
            Span::styled(
                format!("  {}", entry.description),
                if i == sel.selected {
                    style
                } else {
                    app.theme.dim
                },
            ),
        ]));
    }

    let footer = " \u{2191}\u{2193} select \u{00b7} enter confirm \u{00b7} esc cancel";
    let content_width = content_lines
        .iter()
        .map(|l| l.width())
        .max()
        .unwrap_or(20)
        .max(footer.len())
        + 4;
    let content_height = content_lines.len() + 3;

    let area = frame.area();
    let width = (content_width as u16).min(area.width.saturating_sub(4));
    let height = (content_height as u16).min(area.height.saturating_sub(2));
    let x = area.width.saturating_sub(width) / 2;
    let y = area.height.saturating_sub(height) / 2;
    let popup = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(Span::styled(
            " agent ",
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.muted_fg));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let mut all_lines: Vec<Line<'static>> = Vec::new();
    all_lines.extend(content_lines);
    all_lines.push(Line::from(""));
    all_lines.push(Line::from(Span::styled(footer.to_string(), app.theme.dim)));

    let paragraph = Paragraph::new(all_lines);
    frame.render_widget(paragraph, inner);
}

pub fn draw_command_palette(frame: &mut Frame, app: &mut App, input_area: Rect) {
    let palette = &app.command_palette;
    if palette.filtered.is_empty() {
        app.layout.command_palette = None;
        return;
    }

    let items: Vec<(&str, &str)> = palette
        .filtered
        .iter()
        .map(|&i| (COMMANDS[i].name, COMMANDS[i].description))
        .collect();

    let width = items
        .iter()
        .map(|(n, d)| n.len() + d.len() + 8)
        .max()
        .unwrap_or(20) as u16;
    let height = items.len() as u16;

    let popup_y = input_area.y.saturating_sub(height + 1);
    let popup = Rect::new(
        input_area.x + 1,
        popup_y,
        width.min(input_area.width.saturating_sub(2)),
        height,
    );

    app.layout.command_palette = Some(popup);

    frame.render_widget(Clear, popup);

    let mut lines: Vec<Line<'static>> = Vec::new();
    for (i, (name, desc)) in items.iter().enumerate() {
        let style = if i == palette.selected {
            app.theme.highlight
        } else {
            Style::default().fg(Color::Reset)
        };
        lines.push(Line::from(Span::styled(
            format!("  /{:<10} {}", name, desc),
            style,
        )));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, popup);
}

pub fn draw_empty_state(app: &App) -> Vec<Line<'static>> {
    let accent = app.theme.accent;
    let dim = app.theme.dim;
    vec![
        Line::from(""),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "       \u{25c6}",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "    welcome to dot",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled("    start a conversation below", dim)),
        Line::from(Span::styled(
            "    /help for commands \u{00b7} /model to switch \u{00b7} /agent for profiles",
            dim,
        )),
        Line::from(""),
    ]
}

pub fn draw_thinking_selector(frame: &mut Frame, app: &mut App) {
    let sel = &app.thinking_selector;
    if !sel.visible {
        return;
    }

    let levels = ThinkingLevel::all();
    let mut content_lines: Vec<Line<'static>> = Vec::new();

    for (i, &level) in levels.iter().enumerate() {
        let is_current = level == sel.current;
        let marker = if is_current { "\u{25cf} " } else { "  " };

        let style = if i == sel.selected {
            app.theme.highlight
        } else {
            Style::default().fg(Color::Reset)
        };

        content_lines.push(Line::from(vec![
            Span::styled(format!("  {}{}", marker, level.label()), style),
            Span::styled(
                format!("  {}", level.description()),
                if i == sel.selected {
                    style
                } else {
                    app.theme.dim
                },
            ),
        ]));
    }

    let footer = " \u{2191}\u{2193} select \u{00b7} enter confirm \u{00b7} esc cancel";
    let content_width = content_lines
        .iter()
        .map(|l| l.width())
        .max()
        .unwrap_or(20)
        .max(footer.len())
        + 4;
    let content_height = content_lines.len() + 3;

    let area = frame.area();
    let width = (content_width as u16).min(area.width.saturating_sub(4));
    let height = (content_height as u16).min(area.height.saturating_sub(2));
    let x = area.width.saturating_sub(width) / 2;
    let y = area.height.saturating_sub(height) / 2;
    let popup = Rect::new(x, y, width, height);

    app.layout.thinking_selector = Some(popup);

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(Span::styled(
            " thinking ",
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.muted_fg));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let mut all_lines: Vec<Line<'static>> = Vec::new();
    all_lines.extend(content_lines);
    all_lines.push(Line::from(""));
    all_lines.push(Line::from(Span::styled(footer.to_string(), app.theme.dim)));

    let paragraph = Paragraph::new(all_lines);
    frame.render_widget(paragraph, inner);
}

pub fn draw_session_selector(frame: &mut Frame, app: &mut App) {
    let sel = &app.session_selector;
    if !sel.visible {
        return;
    }

    let mut content_lines: Vec<Line<'static>> = Vec::new();
    let mut item_idx = 0usize;

    for &entry_idx in &sel.filtered {
        let entry = &sel.entries[entry_idx];
        let style = if item_idx == sel.selected {
            app.theme.highlight
        } else {
            Style::default().fg(Color::Reset)
        };
        let dim = if item_idx == sel.selected {
            app.theme.highlight
        } else {
            app.theme.dim
        };
        content_lines.push(Line::from(vec![
            Span::styled(format!("  {}", entry.title), style),
            Span::styled(format!("  {}", entry.subtitle), dim),
        ]));
        item_idx += 1;
    }

    if sel.filtered.is_empty() && !sel.query.is_empty() {
        content_lines.push(Line::from(Span::styled(" no matches", app.theme.dim)));
    } else if sel.entries.is_empty() {
        content_lines.push(Line::from(Span::styled(
            " no sessions in this directory",
            app.theme.dim,
        )));
    }

    let footer = " \u{2191}\u{2193} select \u{00b7} enter resume \u{00b7} esc cancel";
    let content_width = content_lines
        .iter()
        .map(|l| l.width())
        .max()
        .unwrap_or(30)
        .max(footer.len())
        + 4;
    let content_height = content_lines.len() + 4;

    let area = frame.area();
    let width = (content_width as u16).min(area.width.saturating_sub(4));
    let height = (content_height as u16).min(area.height.saturating_sub(2));
    let x = area.width.saturating_sub(width) / 2;
    let y = area.height.saturating_sub(height) / 2;
    let popup = Rect::new(x, y, width, height);

    app.layout.session_selector = Some(popup);

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(Span::styled(
            " sessions ",
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.muted_fg));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let search_display = if sel.query.is_empty() {
        Line::from(Span::styled(" type to search...", app.theme.dim))
    } else {
        Line::from(vec![
            Span::styled(" /", Style::default().fg(app.theme.accent)),
            Span::raw(sel.query.clone()),
            Span::styled("\u{258f}", Style::default().fg(app.theme.accent)),
        ])
    };

    let mut all_lines: Vec<Line<'static>> = Vec::new();
    all_lines.push(search_display);
    all_lines.push(Line::from(""));
    all_lines.extend(content_lines);
    all_lines.push(Line::from(""));
    all_lines.push(Line::from(Span::styled(footer.to_string(), app.theme.dim)));

    let paragraph = Paragraph::new(all_lines);
    frame.render_widget(paragraph, inner);
}
