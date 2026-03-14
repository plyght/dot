use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::tui::app::App;
use crate::tui::widgets::{
    COMMANDS, LoginPopup, LoginStep, PaletteEntryKind, ThinkingLevel, WelcomeScreen,
};

fn popup_block(title: &str, _accent: Color, _muted: Color) -> Block<'static> {
    let line = Line::from(vec![
        Span::styled("\u{2500} ", Style::default().fg(Color::Indexed(8))),
        Span::raw(title.to_owned()),
        Span::raw(" "),
    ]);
    Block::default()
        .title(line)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Indexed(8)))
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
                app.theme.dim,
            )));
            last_provider = Some(&entry.provider);
        }

        let _is_current =
            entry.provider == sel.current_provider && entry.model == sel.current_model;
        let is_sel = item_idx == sel.selected;

        let prefix = if is_sel { "\u{203a} " } else { "  " };
        let marker_style = if is_sel {
            Style::default()
        } else {
            app.theme.dim
        };

        let name_style = if is_sel {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let is_fav = sel.favorites.contains(&entry.model);
        let star = if is_fav { "\u{2605} " } else { "  " };
        let mut spans = vec![Span::styled(format!("  {}", prefix), marker_style)];
        spans.push(Span::styled(star.to_string(), app.theme.dim));
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
    let footer = "\u{2191}\u{2193} select  enter confirm  s/* favorite  esc cancel";
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
            Span::styled("\u{258f}", Style::default()),
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
        let _is_current = entry.name == sel.current;
        let is_sel = i == sel.selected;

        let prefix = if is_sel { "\u{203a} " } else { "  " };
        let marker_style = Style::default();

        let name_style = if is_sel {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let desc_style = app.theme.dim;

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
        .border_style(Style::default().fg(Color::Indexed(8)));
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
                Span::styled(" \u{203a} ", Style::default()),
                Span::styled(
                    format!("{}{:<width$}", prefix, entry.name, width = name_w),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:<width$}", entry.description, width = desc_w),
                    app.theme.dim,
                ),
            ]
        } else {
            vec![
                Span::raw("   "),
                Span::styled(
                    format!("{}{:<width$}", prefix, entry.name, width = name_w),
                    Style::default(),
                ),
                Span::styled(
                    format!("{:<width$}", entry.description, width = desc_w),
                    app.theme.dim,
                ),
            ]
        };
        if !entry.shortcut.is_empty() && !compact {
            spans.push(Span::styled(entry.shortcut.clone(), app.theme.dim));
        }
        cmd_lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(cmd_lines), inner);
}

pub fn draw_empty_state(app: &App, width: u16) -> Vec<Line<'static>> {
    let dim = app.theme.dim;
    let compact = width < 55;

    if compact {
        return vec![
            Line::from(""),
            Line::from(""),
            Line::from(vec![
                Span::raw("  \u{25c6} "),
                Span::styled("dot", Style::default().add_modifier(Modifier::BOLD)),
            ]),
            Line::from(Span::styled("  type a message to begin", dim)),
            Line::from(""),
        ];
    }

    let art = [
        "          @@@@@          ",
        "          @@@@@          ",
        "          @@@@@          ",
        "                         ",
        "@@@@@@@@@@@@@@@@@@@@@@@@@",
        "@@@@@@@@@@@@@@@@@@@@@@@@@",
        "@@@@@@@@@@@@@@@@@@@@@@@@@",
    ];
    let subtitle = "a terminal-native ai agent";
    let sep = "\u{2500}".repeat(7);
    let hints = "/help \u{00b7} /model \u{00b7} /sessions";

    let mut lines = vec![Line::from(""), Line::from(""), Line::from("")];

    for a in &art {
        lines.push(Line::from(Span::styled(*a, dim)).alignment(Alignment::Center));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(subtitle, dim)).alignment(Alignment::Center));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(sep, app.theme.border)).alignment(Alignment::Center));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(hints, dim)).alignment(Alignment::Center));
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
        let _is_current = level == sel.current;
        let is_sel = i == sel.selected;

        let prefix = if is_sel { "\u{203a} " } else { "  " };
        let marker_style = Style::default();

        let name_style = if is_sel {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let desc_style = app.theme.dim;

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

    let heading = Style::default().add_modifier(Modifier::BOLD);
    let key_style = Style::default();
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
            spans.push(Span::styled(c.shortcut.to_string(), app.theme.dim));
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
            ("\u{203a} ", Style::default().add_modifier(Modifier::BOLD))
        } else {
            ("  ", Style::default())
        };

        let sub_style = app.theme.dim;

        content_lines.push(Line::from(vec![
            Span::styled(format!("  {}", prefix), Style::default()),
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
            Span::styled("\u{258f}", Style::default()),
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
        .border_style(Style::default().fg(Color::Indexed(8)));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let mut lines: Vec<Line<'static>> = Vec::new();
    for (i, label) in labels.iter().enumerate() {
        let is_sel = i == menu.selected;
        let style = if is_sel {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let prefix = if is_sel { " \u{203a} " } else { "   " };
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
        Style::default(),
    )));
    content_lines.push(Line::from(""));

    for (i, opt) in pq.options.iter().enumerate() {
        let is_sel = i == pq.selected;
        let (prefix, style) = if is_sel {
            ("\u{203a} ", Style::default().add_modifier(Modifier::BOLD))
        } else {
            ("  ", Style::default())
        };
        content_lines.push(Line::from(Span::styled(
            format!("  {}{}", prefix, opt),
            style,
        )));
    }

    let custom_sel = pq.selected >= pq.options.len();
    let custom_style = if custom_sel {
        Style::default().add_modifier(Modifier::BOLD)
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
            if custom_sel { "  \u{203a} " } else { "    " },
            Style::default(),
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
        Style::default().add_modifier(Modifier::BOLD),
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
            ("\u{203a} ", Style::default().add_modifier(Modifier::BOLD))
        } else {
            ("  ", Style::default())
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
    let display = format!("{}\u{258f}", app.rename_input);
    let content_lines: Vec<Line<'static>> = vec![Line::from(vec![
        Span::raw(" "),
        Span::styled(display, Style::default()),
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
        .border_style(Style::default().fg(Color::Indexed(8)));
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
        let icon = if entry.is_dir { "\u{203a} " } else { "  " };
        let display = if entry.is_dir {
            format!("{}/", entry.path)
        } else {
            entry.path.clone()
        };
        if is_sel {
            lines.push(Line::from(vec![
                Span::styled(" \u{203a} ", Style::default()),
                Span::styled(
                    format!("{}{}", icon, display),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::raw("   "),
                Span::styled(
                    format!("{}{}", icon, display),
                    if entry.is_dir {
                        app.theme.dim
                    } else {
                        Style::default()
                    },
                ),
            ]));
        }
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

pub fn draw_login_popup(frame: &mut Frame, app: &mut App) {
    let lp = &app.login_popup;
    let accent = app.theme.accent;
    let muted = app.theme.muted_fg;

    let (title, hint) = match lp.step {
        LoginStep::SelectProvider => (
            "login",
            "esc cancel  \u{2191}\u{2193} navigate  enter select",
        ),
        LoginStep::SelectMethod => (
            "anthropic",
            "esc back  \u{2191}\u{2193} navigate  enter select",
        ),
        LoginStep::EnterApiKey => ("enter api key", "esc back  enter submit"),
        LoginStep::OAuthWaiting => ("authorize", "esc cancel  enter submit code"),
        LoginStep::OAuthExchanging => ("exchanging...", "esc cancel"),
    };

    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(""));

    if lp.step == LoginStep::OAuthWaiting {
        lines.push(Line::from(Span::styled(
            "  browser opened for authorization",
            Style::default().fg(accent),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  paste the URL or code after authorizing:",
            Style::default(),
        )));
        lines.push(Line::from(""));
        let display: String = if lp.code_input.is_empty() {
            "paste code here...".to_string()
        } else {
            let len = lp.code_input.len();
            if len > 40 {
                format!("{}...", &lp.code_input[..37])
            } else {
                lp.code_input.clone()
            }
        };
        lines.push(Line::from(vec![
            Span::styled("  \u{203a} ", Style::default().fg(accent)),
            Span::styled(
                display,
                if lp.code_input.is_empty() {
                    Style::default().fg(muted)
                } else {
                    Style::default()
                },
            ),
        ]));
    } else if lp.step == LoginStep::OAuthExchanging {
        lines.push(Line::from(Span::styled(
            "  exchanging code for credentials...",
            Style::default().fg(accent),
        )));
    } else if lp.step == LoginStep::EnterApiKey {
        let provider = lp.provider.as_deref().unwrap_or("provider");
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(format!("{} API key", provider), Style::default().fg(accent)),
        ]));
        lines.push(Line::from(""));
        let masked: String = "\u{2022}".repeat(lp.key_input.len());
        lines.push(Line::from(vec![
            Span::styled("  \u{203a} ", Style::default().fg(accent)),
            Span::styled(
                if masked.is_empty() {
                    "paste or type your key...".to_string()
                } else {
                    masked
                },
                if lp.key_input.is_empty() {
                    Style::default().fg(muted)
                } else {
                    Style::default()
                },
            ),
        ]));
    } else {
        let items: Vec<(&str, bool)> = match lp.step {
            LoginStep::SelectProvider => LoginPopup::providers()
                .iter()
                .enumerate()
                .map(|(i, p)| (*p, i == lp.selected))
                .collect(),
            LoginStep::SelectMethod => LoginPopup::anthropic_methods()
                .iter()
                .enumerate()
                .map(|(i, m)| (*m, i == lp.selected))
                .collect(),
            _ => Vec::new(),
        };
        for (label, selected) in &items {
            if *selected {
                lines.push(Line::from(vec![
                    Span::styled("  \u{203a} ", Style::default().fg(accent)),
                    Span::styled(
                        label.to_string(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::styled("    ", Style::default()),
                    Span::styled(label.to_string(), Style::default()),
                ]));
            }
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("  {}", hint),
        Style::default().fg(muted),
    )));

    let content_height = lines.len() + 2;
    let content_width = 48;
    let area = centered_popup(frame.area(), content_width, content_height);

    let block = popup_block(title, accent, muted);
    frame.render_widget(Clear, area);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(lines), inner);
    app.layout.login_popup = Some(area);
}

pub fn draw_welcome_screen(frame: &mut Frame, app: &mut App) {
    let accent = app.theme.accent;
    let muted = app.theme.muted_fg;
    let dim = Style::default().fg(Color::Indexed(8));

    let inner_w: usize = 52;
    let sep: String = "\u{2500}".repeat(inner_w);

    let mut lines: Vec<Line<'static>> = vec![
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "dot",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center),
        Line::from(Span::styled("minimal ai agent", dim)).alignment(Alignment::Center),
        Line::from(""),
        Line::from(Span::styled(sep, dim)),
        Line::from(""),
        Line::from(Span::styled("get started", Style::default())).alignment(Alignment::Center),
        Line::from(""),
    ];

    let choices = WelcomeScreen::choices();
    for (i, (label, desc)) in choices.iter().enumerate() {
        let selected = i == app.welcome_screen.selected;
        let (prefix, label_style) = if selected {
            (
                Span::styled("  \u{203a} ", Style::default().fg(accent)),
                Style::default().add_modifier(Modifier::BOLD),
            )
        } else {
            (Span::styled("    ", Style::default()), Style::default())
        };
        lines.push(Line::from(vec![
            prefix,
            Span::styled(label.to_string(), label_style),
        ]));
        lines.push(Line::from(vec![
            Span::styled("      ", Style::default()),
            Span::styled(desc.to_string(), Style::default().fg(muted)),
        ]));
        if i < choices.len() - 1 {
            lines.push(Line::from(""));
        }
    }

    lines.push(Line::from(""));
    lines.push(
        Line::from(Span::styled(
            "\u{2191}\u{2193} navigate   enter select   esc dismiss",
            Style::default().fg(muted),
        ))
        .alignment(Alignment::Center),
    );
    lines.push(Line::from(""));

    let full = frame.area();
    frame.render_widget(Clear, full);

    let content_height = lines.len() + 2;
    let content_width = 54;
    let area = centered_popup(full, content_width, content_height);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(lines), inner);
    app.layout.welcome_screen = Some(area);
}
