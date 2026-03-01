use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::tui::app::App;
use crate::tui::widgets::{COMMANDS, PaletteEntryKind, ThinkingLevel};

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

        let is_fav = sel.favorites.contains(&entry.model);
        let star = if is_fav { "★ " } else { "  " };
        let mut spans = vec![Span::styled(format!("  {}", prefix), marker_style)];
        spans.push(Span::styled(
            star.to_string(),
            Style::default().fg(app.theme.accent),
        ));
        spans.push(Span::styled(
            crate::tui::ui::display_model(&entry.model),
            name_style,
        ));
        content_lines.push(Line::from(spans));
    }

    if sel.filtered.is_empty() {
        content_lines.push(Line::from(Span::styled(" no matches", app.theme.dim)));
    }

    let search_line = format!(" /{}", sel.query);
    let footer = "↑↓ select  enter confirm  s/* favorite  esc cancel";
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

    let items: Vec<&crate::tui::widgets::PaletteEntry> = palette
        .filtered
        .iter()
        .map(|&i| &palette.entries[i])
        .collect();

    let name_w = items.iter().map(|e| e.name.len()).max().unwrap_or(8) + 2;
    let compact = input_area.width < 50;
    let desc_w = if compact { 16 } else { 24 };

    let content_width = items
        .iter()
        .map(|e| e.name.len() + e.description.len() + e.shortcut.len() + 12)
        .max()
        .unwrap_or(20) as u16;
    let content_height = (items.len() as u16).min(8);

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

    let visible_count = inner.height as usize;
    let scroll = if palette.selected >= visible_count {
        palette.selected - visible_count + 1
    } else {
        0
    };

    let mut cmd_lines: Vec<Line<'static>> = Vec::new();
    for (i, entry) in items.iter().enumerate().skip(scroll).take(visible_count) {
        let is_sel = i == palette.selected;
        let is_skill = entry.kind == PaletteEntryKind::Skill;
        let prefix = if is_skill { "\u{25c7} " } else { "/ " };
        let mut spans = if is_sel {
            vec![
                Span::styled(" \u{25b8} ", Style::default().fg(app.theme.accent)),
                Span::styled(
                    format!("{}{:<width$}", prefix, entry.name, width = name_w),
                    Style::default()
                        .fg(app.theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:<width$}", entry.description, width = desc_w),
                    Style::default().fg(app.theme.accent),
                ),
            ]
        } else {
            vec![
                Span::raw("   "),
                Span::styled(
                    format!("{}{:<width$}", prefix, entry.name, width = name_w),
                    Style::default().fg(Color::Reset),
                ),
                Span::styled(
                    format!("{:<width$}", entry.description, width = desc_w),
                    app.theme.dim,
                ),
            ]
        };
        if !entry.shortcut.is_empty() && !compact {
            spans.push(Span::styled(
                entry.shortcut.clone(),
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

    let mut lines = vec![
        Line::from(""),
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
        Line::from(Span::styled(
            format!("{}a terminal-native ai agent", pad),
            dim,
        )),
        Line::from(""),
    ];

    if compact {
        lines.push(Line::from(Span::styled(
            format!("{}type a message to begin", pad),
            Style::default().fg(muted),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(format!("{}  /help", pad), dim),
            Span::styled("  /model", dim),
            Span::styled("  /sessions", dim),
        ]));
    } else {
        let sep_width = 28usize.min((width as usize).saturating_sub(8));
        let sep = "\u{2500}".repeat(sep_width);
        lines.push(Line::from(Span::styled(
            format!("{}{}", pad, sep),
            border_style,
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("{}type a message to begin", pad),
            Style::default().fg(muted),
        )));
        lines.push(Line::from(""));
        for (key, desc) in [
            ("/help", "commands & keybindings"),
            ("/model", "switch model"),
            ("/sessions", "resume a conversation"),
        ] {
            lines.push(Line::from(vec![
                Span::styled(format!("{}  {:<12}", pad, key), Style::default().fg(muted)),
                Span::styled(desc, dim),
            ]));
        }
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

pub fn draw_context_menu(frame: &mut Frame, app: &mut App) {
    let menu = &app.context_menu;
    if !menu.visible {
        return;
    }

    let labels = crate::tui::widgets::MessageContextMenu::labels();
    let content_width = labels.iter().map(|l| l.len()).max().unwrap_or(10) + 6;
    let content_height = labels.len() as u16 + 2;
    let box_width = (content_width as u16).min(frame.area().width.saturating_sub(2));
    let box_height = content_height;

    let x = menu
        .screen_x
        .min(frame.area().width.saturating_sub(box_width));
    let y = menu
        .screen_y
        .min(frame.area().height.saturating_sub(box_height));
    let popup = Rect::new(x, y, box_width, box_height);
    app.layout.context_menu = Some(popup);

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.muted_fg));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let mut lines: Vec<Line<'static>> = Vec::new();
    for (i, label) in labels.iter().enumerate() {
        let is_sel = i == menu.selected;
        let style = if is_sel {
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Reset)
        };
        let prefix = if is_sel { " \u{25b8} " } else { "   " };
        lines.push(Line::from(Span::styled(
            format!("{}{}", prefix, label),
            style,
        )));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

pub fn draw_question_popup(frame: &mut Frame, app: &mut App) {
    let pq = match app.pending_question.as_ref() {
        Some(q) => q,
        None => return,
    };

    let mut content_lines: Vec<Line<'static>> = Vec::new();
    content_lines.push(Line::from(Span::styled(
        format!(" {}", pq.question),
        Style::default().fg(Color::Reset),
    )));
    content_lines.push(Line::from(""));

    for (i, opt) in pq.options.iter().enumerate() {
        let is_sel = i == pq.selected;
        let (prefix, style) = if is_sel {
            (
                "\u{25b8} ",
                Style::default()
                    .fg(app.theme.accent)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            ("  ", Style::default().fg(Color::Reset))
        };
        content_lines.push(Line::from(Span::styled(
            format!("  {}{}", prefix, opt),
            style,
        )));
    }

    // Custom input row
    let custom_sel = pq.selected >= pq.options.len();
    let custom_style = if custom_sel {
        Style::default()
            .fg(app.theme.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        app.theme.dim
    };
    let custom_text = if pq.custom_input.is_empty() {
        "type your answer\u{2026}".to_string()
    } else {
        format!("{}\u{258f}", pq.custom_input)
    };
    content_lines.push(Line::from(vec![
        Span::styled(
            if custom_sel { "  \u{25b8} " } else { "    " },
            if custom_sel {
                Style::default().fg(app.theme.accent)
            } else {
                Style::default()
            },
        ),
        Span::styled(custom_text, custom_style),
    ]));

    let footer = "\u{2191}\u{2193} select  enter confirm  esc cancel";
    let content_width = content_lines
        .iter()
        .map(|l| l.width())
        .max()
        .unwrap_or(30)
        .max(footer.len() + 2)
        + 4;
    let content_height = content_lines.len() + 3;

    let popup = centered_popup(frame.area(), content_width, content_height);
    app.layout.question_popup = Some(popup);

    frame.render_widget(Clear, popup);

    let block = popup_block("question", app.theme.accent, app.theme.muted_fg);
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

pub fn draw_permission_popup(frame: &mut Frame, app: &mut App) {
    let pp = match app.pending_permission.as_ref() {
        Some(p) => p,
        None => return,
    };

    let mut content_lines: Vec<Line<'static>> = Vec::new();
    content_lines.push(Line::from(Span::styled(
        format!(" Allow {}?", pp.tool_name),
        Style::default()
            .fg(Color::Reset)
            .add_modifier(Modifier::BOLD),
    )));
    content_lines.push(Line::from(Span::styled(
        format!(" {}", &pp.input_summary[..pp.input_summary.len().min(60)]),
        app.theme.dim,
    )));
    content_lines.push(Line::from(""));

    let labels = ["Allow", "Deny"];
    for (i, label) in labels.iter().enumerate() {
        let is_sel = i == pp.selected;
        let (prefix, style) = if is_sel {
            (
                "\u{25b8} ",
                Style::default()
                    .fg(app.theme.accent)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            ("  ", Style::default().fg(Color::Reset))
        };
        content_lines.push(Line::from(Span::styled(
            format!("  {}{}", prefix, label),
            style,
        )));
    }

    let footer = "y allow  n deny  esc cancel";
    let content_width = content_lines
        .iter()
        .map(|l| l.width())
        .max()
        .unwrap_or(30)
        .max(footer.len() + 2)
        + 4;
    let content_height = content_lines.len() + 3;

    let popup = centered_popup(frame.area(), content_width, content_height);
    app.layout.permission_popup = Some(popup);

    frame.render_widget(Clear, popup);

    let block = popup_block("permission", app.theme.accent, app.theme.muted_fg);
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

pub fn draw_rename_popup(frame: &mut Frame, app: &App) {
    let footer = "enter save  esc cancel";
    let display = format!("{}▏", app.rename_input);
    let content_lines: Vec<Line<'static>> = vec![Line::from(vec![
        Span::raw(" "),
        Span::styled(display, Style::default().fg(app.theme.accent)),
    ])];
    let content_width = content_lines
        .iter()
        .map(|l| l.width())
        .max()
        .unwrap_or(20)
        .max(footer.len() + 2)
        .max(30)
        + 4;
    let content_height = content_lines.len() + 3;
    let popup = centered_popup(frame.area(), content_width, content_height);
    frame.render_widget(Clear, popup);
    let block = popup_block("rename session", app.theme.accent, app.theme.muted_fg);
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

pub fn draw_file_picker(frame: &mut Frame, app: &mut App, input_area: Rect) {
    let picker = &app.file_picker;
    if picker.filtered.is_empty() {
        app.layout.file_picker = None;
        return;
    }

    let items: Vec<&crate::tui::widgets::FilePickerEntry> = picker
        .filtered
        .iter()
        .map(|&i| &picker.entries[i])
        .collect();

    let content_width = items.iter().map(|e| e.path.len() + 8).max().unwrap_or(20) as u16;
    let content_height = items.len().min(12) as u16;

    let box_width = (content_width + 2).min(input_area.width.saturating_sub(2));
    let box_height = content_height + 2;

    let popup_y = input_area.y.saturating_sub(box_height);
    let popup = Rect::new(input_area.x + 1, popup_y, box_width, box_height);

    app.layout.file_picker = Some(popup);

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.muted_fg));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let visible_count = inner.height as usize;
    let scroll = if picker.selected >= visible_count {
        picker.selected - visible_count + 1
    } else {
        0
    };

    let mut lines: Vec<Line<'static>> = Vec::new();
    for (i, entry) in items.iter().enumerate().skip(scroll).take(visible_count) {
        let is_sel = i == picker.selected;
        let icon = if entry.is_dir { "\u{25b8} " } else { "  " };
        let display = if entry.is_dir {
            format!("{}/", entry.path)
        } else {
            entry.path.clone()
        };
        if is_sel {
            lines.push(Line::from(vec![
                Span::styled(" \u{25b8} ", Style::default().fg(app.theme.accent)),
                Span::styled(
                    format!("{}{}", icon, display),
                    Style::default()
                        .fg(app.theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::raw("   "),
                Span::styled(
                    format!("{}{}", icon, display),
                    if entry.is_dir {
                        Style::default().fg(app.theme.muted_fg)
                    } else {
                        Style::default().fg(Color::Reset)
                    },
                ),
            ]));
        }
    }

    frame.render_widget(Paragraph::new(lines), inner);
}
