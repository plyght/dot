use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::tui::app::App;
use crate::tui::widgets::COMMANDS;
use crate::tui::widgets::ThinkingLevel;

fn popup_block(title: &str, theme_accent: Color, theme_muted: Color) -> Block<'static> {
    Block::default()
        .title(Span::styled(
            format!(" {} ", title),
            Style::default()
                .fg(theme_accent)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme_muted))
}

fn centered_popup(area: Rect, content_width: usize, content_height: usize) -> Rect {
    let width = (content_width as u16).min(area.width.saturating_sub(4));
    let height = (content_height as u16).min(area.height.saturating_sub(2));
    let x = area.width.saturating_sub(width) / 2;
    let y = area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width, height)
}

pub fn draw_model_selector(frame: &mut Frame, app: &mut App) {
    let sel = &app.model_selector;
    if sel.filtered.is_empty() && sel.query.is_empty() {
        return;
    }

    let mut content_lines: Vec<Line<'static>> = Vec::new();
    let mut last_provider: Option<&str> = None;

    for (item_idx, &entry_idx) in sel.filtered.iter().enumerate() {
        let entry = &sel.entries[entry_idx];

        if last_provider != Some(&entry.provider) {
            if last_provider.is_some() {
                content_lines.push(Line::from(""));
            }
            content_lines.push(Line::from(Span::styled(
                format!("  {}", entry.provider),
                Style::default()
                    .fg(app.theme.muted_fg)
                    .add_modifier(Modifier::BOLD),
            )));
            last_provider = Some(&entry.provider);
        }

        let is_current = entry.provider == sel.current_provider && entry.model == sel.current_model;
        let is_sel = item_idx == sel.selected;

        let (prefix, marker_style) = if is_sel {
            ("\u{25b8} ", Style::default().fg(app.theme.accent))
        } else {
            ("  ", Style::default().fg(app.theme.muted_fg))
        };

        let name_style = if is_sel {
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD)
        } else if is_current {
            Style::default().fg(app.theme.accent)
        } else {
            Style::default().fg(Color::Reset)
        };

        let mut spans = vec![Span::styled(format!("  {}", prefix), marker_style)];
        spans.push(Span::styled(entry.model.clone(), name_style));

        content_lines.push(Line::from(spans));
    }

    if sel.filtered.is_empty() {
        content_lines.push(Line::from(Span::styled(" no matches", app.theme.dim)));
    }

    let search_line = format!(" /{}", sel.query);
    let footer = "\u{2191}\u{2193} select  enter confirm  esc cancel";
    let content_width = content_lines
        .iter()
        .map(|l| l.width())
        .max()
        .unwrap_or(20)
        .max(footer.len() + 2)
        .max(search_line.len())
        + 4;
    let content_height = content_lines.len() + 4;

    let popup = centered_popup(frame.area(), content_width, content_height);
    app.layout.model_selector = Some(popup);

    frame.render_widget(Clear, popup);

    let block = popup_block("model", app.theme.accent, app.theme.muted_fg);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let mut all_lines: Vec<Line<'static>> = Vec::new();

    let search_display = if sel.query.is_empty() {
        Line::from(Span::styled(" type to filter\u{2026}", app.theme.dim))
    } else {
        Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::raw(sel.query.clone()),
            Span::styled("\u{258f}", Style::default().fg(app.theme.accent)),
        ])
    };
    all_lines.push(search_display);
    all_lines.push(Line::from(""));
    all_lines.extend(content_lines);
    all_lines.push(Line::from(""));
    all_lines.push(Line::from(Span::styled(
        format!(" {}", footer),
        app.theme.dim,
    )));

    frame.render_widget(Paragraph::new(all_lines), inner);
}

pub fn draw_agent_selector(frame: &mut Frame, app: &mut App) {
    let sel = &app.agent_selector;
    if sel.entries.is_empty() {
        return;
    }

    let mut content_lines: Vec<Line<'static>> = Vec::new();
    for (i, entry) in sel.entries.iter().enumerate() {
        let is_current = entry.name == sel.current;
        let is_sel = i == sel.selected;

        let (prefix, marker_style) = if is_sel {
            ("\u{25b8} ", Style::default().fg(app.theme.accent))
        } else {
            ("  ", Style::default())
        };

        let name_style = if is_sel {
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD)
        } else if is_current {
            Style::default().fg(app.theme.accent)
        } else {
            Style::default().fg(Color::Reset)
        };

        let desc_style = if is_sel {
            Style::default().fg(app.theme.accent)
        } else {
            app.theme.dim
        };

        let mut spans = vec![Span::styled(format!("  {}", prefix), marker_style)];
        spans.push(Span::styled(entry.name.clone(), name_style));
        spans.push(Span::styled(format!("  {}", entry.description), desc_style));

        content_lines.push(Line::from(spans));
    }

    let footer = "\u{2191}\u{2193} select  enter confirm  esc cancel";
    let content_width = content_lines
        .iter()
        .map(|l| l.width())
        .max()
        .unwrap_or(20)
        .max(footer.len() + 2)
        + 4;
    let content_height = content_lines.len() + 3;

    let popup = centered_popup(frame.area(), content_width, content_height);
    app.layout.agent_selector = Some(popup);

    frame.render_widget(Clear, popup);

    let block = popup_block("agent", app.theme.accent, app.theme.muted_fg);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let mut all_lines: Vec<Line<'static>> = Vec::new();
    all_lines.extend(content_lines);
    all_lines.push(Line::from(""));
    all_lines.push(Line::from(Span::styled(
        format!(" {}", footer),
        app.theme.dim,
    )));

    frame.render_widget(Paragraph::new(all_lines), inner);
}

pub fn draw_command_palette(frame: &mut Frame, app: &mut App, input_area: Rect) {
    let palette = &app.command_palette;
    if palette.filtered.is_empty() {
        app.layout.command_palette = None;
        return;
    }

    let items: Vec<(&str, &str, &str)> = palette
        .filtered
        .iter()
        .map(|&i| {
            (
                COMMANDS[i].name,
                COMMANDS[i].description,
                COMMANDS[i].shortcut,
            )
        })
        .collect();

    let content_width = items
        .iter()
        .map(|(n, d, s)| n.len() + d.len() + s.len() + 12)
        .max()
        .unwrap_or(20) as u16;
    let content_height = items.len() as u16;

    let box_width = (content_width + 2).min(input_area.width.saturating_sub(2));
    let box_height = content_height + 2;

    let popup_y = input_area.y.saturating_sub(box_height);
    let popup = Rect::new(input_area.x + 1, popup_y, box_width, box_height);

    app.layout.command_palette = Some(popup);

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.muted_fg));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let compact = popup.width < 50;
    let (name_w, desc_w) = if compact { (8, 16) } else { (10, 24) };

    let mut cmd_lines: Vec<Line<'static>> = Vec::new();
    for (i, (name, desc, shortcut)) in items.iter().enumerate() {
        let is_sel = i == palette.selected;
        let mut spans = if is_sel {
            vec![
                Span::styled(" \u{25b8} ", Style::default().fg(app.theme.accent)),
                Span::styled(
                    format!("/{:<width$}", name, width = name_w),
                    Style::default()
                        .fg(app.theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:<width$}", desc, width = desc_w),
                    Style::default().fg(app.theme.accent),
                ),
            ]
        } else {
            vec![
                Span::raw("   "),
                Span::styled(
                    format!("/{:<width$}", name, width = name_w),
                    Style::default().fg(Color::Reset),
                ),
                Span::styled(format!("{:<width$}", desc, width = desc_w), app.theme.dim),
            ]
        };
        if !shortcut.is_empty() && !compact {
            spans.push(Span::styled(
                shortcut.to_string(),
                Style::default().fg(app.theme.muted_fg),
            ));
        }
        cmd_lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(cmd_lines), inner);
}

pub fn draw_empty_state(app: &App, width: u16) -> Vec<Line<'static>> {
    let accent = app.theme.accent;
    let dim = app.theme.dim;
    let border_style = app.theme.border;
    let muted = app.theme.muted_fg;
    let compact = width < 55;
    let pad = if compact { "   " } else { "       " };
    let sep_width = if compact {
        (width as usize).saturating_sub(6).min(24)
    } else {
        32
    };
    let sep_line = Line::from(Span::styled(
        format!("{}{}", pad, "\u{2500}".repeat(sep_width)),
        border_style,
    ));

    let mut lines = vec![
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            format!("{}\u{25c6}", pad),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("{}dot", pad),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        sep_line,
        Line::from(""),
    ];

    if compact {
        lines.push(Line::from(Span::styled(
            format!("{}type a message to start", pad),
            dim,
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("{}/help  /model  /sessions", pad),
            Style::default().fg(muted),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            format!("{}type a message to get started", pad),
            dim,
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(format!("{}/help", pad), Style::default().fg(muted)),
            Span::styled("  \u{00b7}  ", dim),
            Span::styled("/model", Style::default().fg(muted)),
            Span::styled("  \u{00b7}  ", dim),
            Span::styled("/sessions", Style::default().fg(muted)),
        ]));
    }

    lines.push(Line::from(""));
    lines
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
        let is_sel = i == sel.selected;

        let (prefix, marker_style) = if is_sel {
            ("\u{25b8} ", Style::default().fg(app.theme.accent))
        } else {
            ("  ", Style::default())
        };

        let name_style = if is_sel {
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD)
        } else if is_current {
            Style::default().fg(app.theme.accent)
        } else {
            Style::default().fg(Color::Reset)
        };

        let desc_style = if is_sel {
            Style::default().fg(app.theme.accent)
        } else {
            app.theme.dim
        };

        let mut spans = vec![Span::styled(format!("  {}", prefix), marker_style)];
        spans.push(Span::styled(level.label().to_string(), name_style));
        spans.push(Span::styled(
            format!("  {}", level.description()),
            desc_style,
        ));

        content_lines.push(Line::from(spans));
    }

    let footer = "\u{2191}\u{2193} select  enter confirm  esc cancel";
    let content_width = content_lines
        .iter()
        .map(|l| l.width())
        .max()
        .unwrap_or(20)
        .max(footer.len() + 2)
        + 4;
    let content_height = content_lines.len() + 3;

    let popup = centered_popup(frame.area(), content_width, content_height);
    app.layout.thinking_selector = Some(popup);

    frame.render_widget(Clear, popup);

    let block = popup_block("thinking", app.theme.accent, app.theme.muted_fg);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let mut all_lines: Vec<Line<'static>> = Vec::new();
    all_lines.extend(content_lines);
    all_lines.push(Line::from(""));
    all_lines.push(Line::from(Span::styled(
        format!(" {}", footer),
        app.theme.dim,
    )));

    frame.render_widget(Paragraph::new(all_lines), inner);
}

pub fn draw_help_popup(frame: &mut Frame, app: &mut App) {
    let mut content_lines: Vec<Line<'static>> = Vec::new();

    let heading = Style::default()
        .fg(app.theme.accent)
        .add_modifier(Modifier::BOLD);
    let key_style = Style::default().fg(Color::Reset);
    let desc_style = app.theme.dim;

    content_lines.push(Line::from(Span::styled(" commands", heading)));
    content_lines.push(Line::from(""));
    for c in COMMANDS {
        let mut spans = vec![
            Span::styled(format!("   /{:<12}", c.name), key_style),
            Span::styled(c.description.to_string(), desc_style),
        ];
        if !c.shortcut.is_empty() {
            let pad = 24usize.saturating_sub(c.description.len());
            spans.push(Span::styled(" ".repeat(pad), desc_style));
            spans.push(Span::styled(
                c.shortcut.to_string(),
                Style::default().fg(app.theme.muted_fg),
            ));
        }
        content_lines.push(Line::from(spans));
    }

    content_lines.push(Line::from(""));
    if app.vim_mode {
        content_lines.push(Line::from(Span::styled(" navigation", heading)));
        content_lines.push(Line::from(""));
        for (key, desc) in [
            ("j/k", "scroll up/down"),
            ("g/G", "top/bottom"),
            ("^D/^U", "half-page scroll"),
            ("i/Esc", "insert/normal mode"),
            ("t", "toggle thinking"),
            ("q", "quit"),
        ] {
            content_lines.push(Line::from(vec![
                Span::styled(format!("   {:<14}", key), key_style),
                Span::styled(desc, desc_style),
            ]));
        }
    } else {
        content_lines.push(Line::from(Span::styled(" navigation", heading)));
        content_lines.push(Line::from(""));
        for (key, desc) in [
            ("Up/Down", "scroll messages"),
            ("PgUp/PgDn", "page scroll"),
            ("^D", "half-page down"),
        ] {
            content_lines.push(Line::from(vec![
                Span::styled(format!("   {:<14}", key), key_style),
                Span::styled(desc, desc_style),
            ]));
        }
    }

    content_lines.push(Line::from(""));
    content_lines.push(Line::from(Span::styled(" editing", heading)));
    content_lines.push(Line::from(""));
    for (key, desc) in [
        ("^A/^E", "start/end of line"),
        ("^W", "delete word"),
        ("^K/^U", "delete to end/start"),
        ("^C", "clear input or quit"),
    ] {
        content_lines.push(Line::from(vec![
            Span::styled(format!("   {:<14}", key), key_style),
            Span::styled(desc, desc_style),
        ]));
    }

    let footer = "esc close";
    let content_width = content_lines
        .iter()
        .map(|l| l.width())
        .max()
        .unwrap_or(30)
        .max(footer.len() + 2)
        + 4;
    let content_height = content_lines.len() + 3;

    let popup = centered_popup(frame.area(), content_width, content_height);
    app.layout.help_popup = Some(popup);

    frame.render_widget(Clear, popup);

    let block = popup_block("help", app.theme.accent, app.theme.muted_fg);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let mut all_lines: Vec<Line<'static>> = Vec::new();
    all_lines.extend(content_lines);
    all_lines.push(Line::from(""));
    all_lines.push(Line::from(Span::styled(
        format!(" {}", footer),
        app.theme.dim,
    )));

    frame.render_widget(Paragraph::new(all_lines), inner);
}

pub fn draw_session_selector(frame: &mut Frame, app: &mut App) {
    let sel = &app.session_selector;
    if !sel.visible {
        return;
    }

    let mut content_lines: Vec<Line<'static>> = Vec::new();

    for (item_idx, &entry_idx) in sel.filtered.iter().enumerate() {
        let entry = &sel.entries[entry_idx];
        let is_sel = item_idx == sel.selected;

        let (prefix, title_style) = if is_sel {
            (
                "\u{25b8} ",
                Style::default()
                    .fg(app.theme.accent)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            ("  ", Style::default().fg(Color::Reset))
        };

        let sub_style = if is_sel {
            Style::default().fg(app.theme.accent)
        } else {
            app.theme.dim
        };

        content_lines.push(Line::from(vec![
            Span::styled(
                format!("  {}", prefix),
                if is_sel {
                    Style::default().fg(app.theme.accent)
                } else {
                    Style::default()
                },
            ),
            Span::styled(entry.title.clone(), title_style),
            Span::styled(format!("  {}", entry.subtitle), sub_style),
        ]));
    }

    if sel.filtered.is_empty() && !sel.query.is_empty() {
        content_lines.push(Line::from(Span::styled(" no matches", app.theme.dim)));
    } else if sel.entries.is_empty() {
        content_lines.push(Line::from(Span::styled(
            " no sessions in this directory",
            app.theme.dim,
        )));
    }

    let footer = "\u{2191}\u{2193} select  enter resume  esc cancel";
    let content_width = content_lines
        .iter()
        .map(|l| l.width())
        .max()
        .unwrap_or(30)
        .max(footer.len() + 2)
        + 4;
    let content_height = content_lines.len() + 4;

    let popup = centered_popup(frame.area(), content_width, content_height);
    app.layout.session_selector = Some(popup);

    frame.render_widget(Clear, popup);

    let block = popup_block("sessions", app.theme.accent, app.theme.muted_fg);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let search_display = if sel.query.is_empty() {
        Line::from(Span::styled(" type to filter\u{2026}", app.theme.dim))
    } else {
        Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::raw(sel.query.clone()),
            Span::styled("\u{258f}", Style::default().fg(app.theme.accent)),
        ])
    };

    let mut all_lines: Vec<Line<'static>> = Vec::new();
    all_lines.push(search_display);
    all_lines.push(Line::from(""));
    all_lines.extend(content_lines);
    all_lines.push(Line::from(""));
    all_lines.push(Line::from(Span::styled(
        format!(" {}", footer),
        app.theme.dim,
    )));

    frame.render_widget(Paragraph::new(all_lines), inner);
}
