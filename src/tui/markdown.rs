use std::sync::LazyLock;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use syntect::highlighting::ThemeSet;
use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxSet};

use crate::tui::theme::{SyntaxStyles, Theme};

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

#[derive(Clone, Copy)]
enum ScopeKind {
    Keyword,
    Str,
    Comment,
    Function,
    Type,
    Number,
    Constant,
    Attribute,
}

static SCOPE_MATCHERS: LazyLock<Vec<(Scope, ScopeKind)>> = LazyLock::new(|| {
    use ScopeKind::*;
    [
        ("entity.other.attribute-name", Attribute),
        ("entity.name.function", Function),
        ("entity.name.type", Type),
        ("entity.name.class", Type),
        ("entity.name.tag", Keyword),
        ("constant.character", Str),
        ("constant.language", Constant),
        ("constant.numeric", Number),
        ("support.function", Function),
        ("support.type", Type),
        ("variable.language", Keyword),
        ("meta.attribute", Attribute),
        ("keyword", Keyword),
        ("storage", Keyword),
        ("comment", Comment),
        ("string", Str),
    ]
    .into_iter()
    .filter_map(|(s, kind)| Some((Scope::new(s).ok()?, kind)))
    .collect()
});

fn resolve_scope(stack: &ScopeStack, styles: &SyntaxStyles) -> Style {
    for scope in stack.as_slice().iter().rev() {
        for (prefix, kind) in SCOPE_MATCHERS.iter() {
            if prefix.is_prefix_of(*scope) {
                return match kind {
                    ScopeKind::Keyword => styles.keyword,
                    ScopeKind::Str => styles.string,
                    ScopeKind::Comment => styles.comment,
                    ScopeKind::Function => styles.function,
                    ScopeKind::Type => styles.type_name,
                    ScopeKind::Number => styles.number,
                    ScopeKind::Constant => styles.constant,
                    ScopeKind::Attribute => styles.attribute,
                };
            }
        }
    }
    Style::default()
}

fn word_wrap(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }
    let mut result: Vec<String> = Vec::new();
    for raw in text.lines() {
        if raw.is_empty() {
            result.push(String::new());
            continue;
        }
        let mut current = String::new();
        let mut current_len: usize = 0;
        for word in raw.split_whitespace() {
            let word_len = word.chars().count();
            if current.is_empty() {
                current.push_str(word);
                current_len = word_len;
            } else if current_len + 1 + word_len <= max_width {
                current.push(' ');
                current.push_str(word);
                current_len += 1 + word_len;
            } else {
                result.push(std::mem::take(&mut current));
                current.push_str(word);
                current_len = word_len;
            }
        }
        if !current.is_empty() {
            result.push(current);
        }
    }
    if result.is_empty() {
        result.push(String::new());
    }
    result
}

fn truncate_code_line(line: &str, max_chars: usize) -> String {
    if line.chars().count() <= max_chars {
        return line.to_string();
    }
    let truncated: String = line.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{}…", truncated)
}

pub fn render_markdown(text: &str, theme: &Theme, width: u16) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut in_code_block = false;
    let mut code_lang = String::new();
    let mut code_lines: Vec<String> = Vec::new();
    let mut just_closed_code = false;

    for raw_line in text.lines() {
        if raw_line.starts_with("```") {
            if in_code_block {
                render_code_block(&code_lang, &code_lines, theme, width, &mut lines);
                code_lines.clear();
                code_lang.clear();
                in_code_block = false;
                just_closed_code = true;
            } else {
                in_code_block = true;
                code_lang = raw_line.trim_start_matches('`').trim().to_string();
                if let Some(last) = lines.last() {
                    if last.spans.iter().all(|s| s.content.trim().is_empty()) {
                        lines.pop();
                    }
                }
            }
            continue;
        }

        if in_code_block {
            code_lines.push(raw_line.to_string());
            continue;
        }

        if raw_line.is_empty() {
            if just_closed_code {
                continue;
            }
            lines.push(Line::from(""));
            continue;
        }
        just_closed_code = false;

        if let Some(heading) = raw_line.strip_prefix("### ") {
            lines.push(Line::from(Span::styled(
                heading.to_string(),
                theme
                    .heading
                    .patch(Style::default().add_modifier(Modifier::BOLD)),
            )));
        } else if let Some(heading) = raw_line.strip_prefix("## ") {
            lines.push(Line::from(Span::styled(heading.to_string(), theme.heading)));
        } else if let Some(heading) = raw_line.strip_prefix("# ") {
            lines.push(Line::from(Span::styled(
                heading.to_string(),
                theme
                    .heading
                    .patch(Style::default().add_modifier(Modifier::BOLD)),
            )));
        } else if let Some(quote) = raw_line.strip_prefix("> ") {
            lines.push(Line::from(vec![
                Span::styled("  │ ", theme.blockquote),
                Span::styled(quote.to_string(), theme.blockquote),
            ]));
        } else if raw_line.starts_with("- ") || raw_line.starts_with("* ") {
            let content = &raw_line[2..];
            let prefix_len = 4usize;
            let wrap_w = (width as usize).saturating_sub(prefix_len);
            let sub_lines = word_wrap(content, wrap_w);
            for (i, sub) in sub_lines.into_iter().enumerate() {
                if i == 0 {
                    let spans = parse_inline(&sub, theme);
                    let mut full = vec![Span::styled("  \u{00b7} ", theme.list_bullet)];
                    full.extend(spans);
                    lines.push(Line::from(full));
                } else {
                    let spans = parse_inline(&sub, theme);
                    let mut full = vec![Span::raw("    ")];
                    full.extend(spans);
                    lines.push(Line::from(full));
                }
            }
        } else if raw_line
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
            && raw_line.contains(". ")
        {
            if let Some(pos) = raw_line.find(". ") {
                let num = &raw_line[..pos + 2];
                let content = &raw_line[pos + 2..];
                let prefix_len = num.chars().count() + 3;
                let wrap_w = (width as usize).saturating_sub(prefix_len);
                let sub_lines = word_wrap(content, wrap_w);
                let indent = " ".repeat(prefix_len);
                for (i, sub) in sub_lines.into_iter().enumerate() {
                    if i == 0 {
                        let spans = parse_inline(&sub, theme);
                        let mut full = vec![Span::styled(format!("  {} ", num), theme.list_bullet)];
                        full.extend(spans);
                        lines.push(Line::from(full));
                    } else {
                        let spans = parse_inline(&sub, theme);
                        let mut full = vec![Span::raw(indent.clone())];
                        full.extend(spans);
                        lines.push(Line::from(full));
                    }
                }
            }
        } else if raw_line.trim() == "---" || raw_line.trim() == "***" {
            lines.push(Line::from(Span::styled(
                "\u{2500}".repeat(width.saturating_sub(4) as usize),
                theme.border,
            )));
        } else {
            let sub_lines = word_wrap(raw_line, width as usize);
            for sub in sub_lines {
                let spans = parse_inline(&sub, theme);
                lines.push(Line::from(spans));
            }
        }
    }

    if in_code_block {
        render_code_block(&code_lang, &code_lines, theme, width, &mut lines);
    }

    let mut deduped: Vec<Line<'static>> = Vec::with_capacity(lines.len());
    let mut prev_empty = false;
    for line in lines {
        let is_empty = line.spans.iter().all(|s| s.content.is_empty());
        if is_empty && prev_empty {
            continue;
        }
        prev_empty = is_empty;
        deduped.push(line);
    }
    deduped
}

pub fn render_code_block(
    lang: &str,
    code_lines: &[String],
    theme: &Theme,
    width: u16,
    output: &mut Vec<Line<'static>>,
) {
    let w = width as usize;
    let bg = theme.code_bg;
    let pad = " ";
    let pad_len = 1;

    output.push(Line::from(""));

    let fill = |content_len: usize| -> String { " ".repeat(w.saturating_sub(content_len)) };

    // Top line: language badge or blank, all on code_bg
    if !lang.is_empty() {
        let badge = format!("{}{}", pad, lang);
        let badge_len = badge.chars().count();
        output.push(Line::from(vec![
            Span::styled(badge, Style::default().fg(theme.muted_fg).bg(bg)),
            Span::styled(fill(badge_len), Style::default().bg(bg)),
        ]));
    } else {
        output.push(Line::from(Span::styled(
            " ".repeat(w),
            Style::default().bg(bg),
        )));
    }

    let is_diff = lang == "diff" || lang == "patch";
    if is_diff {
        for raw_line in code_lines {
            let line = &truncate_code_line(raw_line, w.saturating_sub(pad_len));
            let diff_style = if line.starts_with('+') {
                theme.diff_add.bg(bg)
            } else if line.starts_with('-') {
                theme.diff_remove.bg(bg)
            } else if line.starts_with('@') {
                theme.diff_hunk.bg(bg)
            } else {
                Style::default().fg(theme.fg).bg(bg)
            };
            let content = format!("{}{}", pad, line);
            let content_len = content.chars().count();
            output.push(Line::from(vec![
                Span::styled(content, diff_style),
                Span::styled(fill(content_len), Style::default().bg(bg)),
            ]));
        }
        if code_lines.is_empty() {
            output.push(Line::from(Span::styled(
                " ".repeat(w),
                Style::default().bg(bg),
            )));
        }
    } else if let Some(syntect_theme_name) = theme.syntect_theme
        && !lang.is_empty()
        && let Some(syntax) = SYNTAX_SET.find_syntax_by_token(lang)
        && let Some(st_theme) = THEME_SET.themes.get(syntect_theme_name)
    {
        let mut highlighter = syntect::easy::HighlightLines::new(syntax, st_theme);
        for raw_line in code_lines {
            let line: &str = &truncate_code_line(raw_line, w.saturating_sub(pad_len));
            let highlighted = highlighter.highlight_line(line, &SYNTAX_SET);
            match highlighted {
                Ok(ranges) => {
                    let mut spans = vec![Span::styled(pad, Style::default().bg(bg))];
                    let mut content_len = pad_len;
                    for (style, text) in ranges {
                        let fg = style.foreground;
                        let clean = text.trim_end_matches('\n');
                        if clean.is_empty() {
                            continue;
                        }
                        content_len += clean.chars().count();
                        spans.push(Span::styled(
                            clean.to_string(),
                            Style::default().fg(Color::Rgb(fg.r, fg.g, fg.b)).bg(bg),
                        ));
                    }
                    spans.push(Span::styled(fill(content_len), Style::default().bg(bg)));
                    output.push(Line::from(spans));
                }
                Err(_) => {
                    let content = format!("{}{}", pad, line);
                    let content_len = content.chars().count();
                    output.push(Line::from(vec![
                        Span::styled(content, Style::default().fg(theme.fg).bg(bg)),
                        Span::styled(fill(content_len), Style::default().bg(bg)),
                    ]));
                }
            }
        }
        if code_lines.is_empty() {
            output.push(Line::from(Span::styled(
                " ".repeat(w),
                Style::default().bg(bg),
            )));
        }
    } else if let Some(styles) = &theme.syntax
        && !lang.is_empty()
        && let Some(syntax) = SYNTAX_SET.find_syntax_by_token(lang)
    {
        let mut state = ParseState::new(syntax);
        let mut stack = ScopeStack::new();
        for raw_line in code_lines {
            let line = &truncate_code_line(raw_line, w.saturating_sub(pad_len));
            match state.parse_line(line, &SYNTAX_SET) {
                Ok(ops) => {
                    let mut spans = vec![Span::styled(pad, Style::default().bg(bg))];
                    let mut content_len = pad_len;
                    let mut prev = 0;
                    for (pos, op) in &ops {
                        let pos = (*pos).min(line.len());
                        if pos > prev {
                            let text = &line[prev..pos];
                            content_len += text.chars().count();
                            spans.push(Span::styled(
                                text.to_string(),
                                resolve_scope(&stack, styles).bg(bg),
                            ));
                        }
                        let _ = stack.apply(op);
                        prev = pos;
                    }
                    if prev < line.len() {
                        let text = &line[prev..];
                        content_len += text.chars().count();
                        spans.push(Span::styled(
                            text.to_string(),
                            resolve_scope(&stack, styles).bg(bg),
                        ));
                    }
                    spans.push(Span::styled(fill(content_len), Style::default().bg(bg)));
                    output.push(Line::from(spans));
                }
                Err(_) => {
                    let content = format!("{}{}", pad, line);
                    let content_len = content.chars().count();
                    output.push(Line::from(vec![
                        Span::styled(content, Style::default().fg(theme.fg).bg(bg)),
                        Span::styled(fill(content_len), Style::default().bg(bg)),
                    ]));
                }
            }
        }
        if code_lines.is_empty() {
            output.push(Line::from(Span::styled(
                " ".repeat(w),
                Style::default().bg(bg),
            )));
        }
    } else {
        let code_style = Style::default().fg(theme.fg).bg(bg);
        for raw_line in code_lines {
            let line = &truncate_code_line(raw_line, w.saturating_sub(pad_len));
            let content = format!("{}{}", pad, line);
            let content_len = content.chars().count();
            output.push(Line::from(vec![
                Span::styled(content, code_style),
                Span::styled(fill(content_len), Style::default().bg(bg)),
            ]));
        }
        if code_lines.is_empty() {
            output.push(Line::from(Span::styled(
                " ".repeat(w),
                Style::default().bg(bg),
            )));
        }
    }

    output.push(Line::from(""));
}

#[allow(clippy::while_let_on_iterator)]
fn parse_inline(text: &str, theme: &Theme) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut chars = text.char_indices().peekable();
    let mut current = String::new();

    while let Some((_i, c)) = chars.next() {
        match c {
            '`' => {
                if !current.is_empty() {
                    spans.push(Span::raw(std::mem::take(&mut current)));
                }
                let mut code = String::new();
                let mut closed = false;
                while let Some((_, ch)) = chars.next() {
                    if ch == '`' {
                        closed = true;
                        break;
                    }
                    code.push(ch);
                }
                if closed {
                    spans.push(Span::styled(code, theme.inline_code));
                } else {
                    spans.push(Span::raw(format!("`{}", code)));
                }
            }
            '*' => {
                let next_is_star = chars.peek().map(|(_, ch)| *ch == '*').unwrap_or(false);
                if next_is_star {
                    chars.next();
                    if !current.is_empty() {
                        spans.push(Span::raw(std::mem::take(&mut current)));
                    }
                    let mut bold_text = String::new();
                    let mut closed = false;
                    while let Some((_, ch)) = chars.next() {
                        if ch == '*' && chars.peek().map(|(_, c)| *c == '*').unwrap_or(false) {
                            chars.next();
                            closed = true;
                            break;
                        }
                        bold_text.push(ch);
                    }
                    if closed {
                        spans.push(Span::styled(bold_text, theme.bold));
                    } else {
                        spans.push(Span::raw(format!("**{}", bold_text)));
                    }
                } else {
                    if !current.is_empty() {
                        spans.push(Span::raw(std::mem::take(&mut current)));
                    }
                    let mut italic_text = String::new();
                    let mut closed = false;
                    while let Some((_, ch)) = chars.next() {
                        if ch == '*' {
                            closed = true;
                            break;
                        }
                        italic_text.push(ch);
                    }
                    if closed {
                        spans.push(Span::styled(italic_text, theme.italic));
                    } else {
                        spans.push(Span::raw(format!("*{}", italic_text)));
                    }
                }
            }
            '[' => {
                if !current.is_empty() {
                    spans.push(Span::raw(std::mem::take(&mut current)));
                }
                let mut link_text = String::new();
                let mut found_bracket = false;
                while let Some((_, ch)) = chars.next() {
                    if ch == ']' {
                        found_bracket = true;
                        break;
                    }
                    link_text.push(ch);
                }
                if found_bracket && chars.peek().map(|(_, c)| *c == '(').unwrap_or(false) {
                    chars.next();
                    let mut _url = String::new();
                    while let Some((_, ch)) = chars.next() {
                        if ch == ')' {
                            break;
                        }
                        _url.push(ch);
                    }
                    spans.push(Span::styled(link_text, theme.link));
                } else {
                    spans.push(Span::raw(format!("[{}", link_text)));
                    if found_bracket {
                        spans.push(Span::raw("]"));
                    }
                }
            }
            _ => {
                current.push(c);
            }
        }
    }

    if !current.is_empty() {
        spans.push(Span::raw(current));
    }

    if spans.is_empty() {
        spans.push(Span::raw(""));
    }

    spans
}
