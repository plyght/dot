use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::tui::theme::Theme;

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

pub fn render_markdown(text: &str, theme: &Theme, width: u16) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut in_code_block = false;
    let mut code_lang = String::new();
    let mut code_lines: Vec<String> = Vec::new();

    for raw_line in text.lines() {
        if raw_line.starts_with("```") {
            if in_code_block {
                render_code_block(&code_lang, &code_lines, theme, &mut lines);
                code_lines.clear();
                code_lang.clear();
                in_code_block = false;
            } else {
                in_code_block = true;
                code_lang = raw_line.trim_start_matches('`').trim().to_string();
            }
            continue;
        }

        if in_code_block {
            code_lines.push(raw_line.to_string());
            continue;
        }

        if raw_line.is_empty() {
            lines.push(Line::from(""));
            continue;
        }

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
        render_code_block(&code_lang, &code_lines, theme, &mut lines);
    }

    lines
}

fn render_code_block(
    lang: &str,
    code_lines: &[String],
    theme: &Theme,
    output: &mut Vec<Line<'static>>,
) {
    let label = if lang.is_empty() {
        String::new()
    } else {
        format!(" {} ", lang)
    };
    if !label.is_empty() {
        output.push(Line::from(Span::styled(
            label,
            Style::default().fg(Color::DarkGray),
        )));
    }

    let code_style = Style::default().fg(Color::White).bg(theme.code_bg);

    for line in code_lines {
        output.push(Line::from(Span::styled(format!("  {}", line), code_style)));
    }

    if code_lines.is_empty() {
        output.push(Line::from(Span::styled("  ", code_style)));
    }
}

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
                        if ch == '*' {
                            if chars.peek().map(|(_, c)| *c == '*').unwrap_or(false) {
                                chars.next();
                                closed = true;
                                break;
                            }
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
