use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TerminalBackground {
    Dark,
    Light,
}

pub fn detect_terminal_background() -> TerminalBackground {
    if let Ok(val) = std::env::var("TERM_BACKGROUND") {
        match val.to_lowercase().as_str() {
            "light" => return TerminalBackground::Light,
            "dark" => return TerminalBackground::Dark,
            _ => {}
        }
    }

    if let Ok(val) = std::env::var("COLORFGBG")
        && let Some(bg_str) = val.rsplit(';').next()
        && let Ok(bg) = bg_str.trim().parse::<u8>()
    {
        return if bg < 8 || (232..232 + 12).contains(&bg) {
            TerminalBackground::Dark
        } else if (8..=15).contains(&bg) {
            TerminalBackground::Light
        } else {
            TerminalBackground::Dark
        };
    }

    if let Ok(term_program) = std::env::var("TERM_PROGRAM")
        && term_program.to_lowercase().contains("apple_terminal")
    {
        return TerminalBackground::Light;
    }

    TerminalBackground::Dark
}

pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub dim: Style,
    pub accent: Color,
    pub user_label: Style,
    pub assistant_label: Style,
    pub border: Style,
    pub input_prompt: Style,
    pub status_bar: Style,
    pub code_bg: Color,
    pub inline_code: Style,
    pub error: Style,
    pub tool_name: Style,
    pub tool_output: Style,
    pub heading: Style,
    pub bold: Style,
    pub italic: Style,
    pub blockquote: Style,
    pub link: Style,
    pub list_bullet: Style,
    pub scrollbar_track: Style,
    pub scrollbar_thumb: Style,
    pub tool_success: Style,
    pub highlight: Style,
    pub muted_fg: Color,
    pub tool_file_read: Style,
    pub tool_file_write: Style,
    pub tool_directory: Style,
    pub tool_search: Style,
    pub tool_command: Style,
    pub tool_mcp: Style,
    pub tool_skill: Style,
    pub tool_badge_bg: Color,
    pub tool_path: Style,
    pub thinking: Style,
    pub mode_normal_fg: Color,
    pub mode_normal_bg: Color,
    pub mode_insert_fg: Color,
    pub mode_insert_bg: Color,
    pub cost: Style,
    pub user_text: Style,
    pub tool_action: Style,
    pub separator: Style,
    pub tool_exit_ok: Style,
    pub tool_exit_err: Style,
    pub syntect_theme: Option<&'static str>,
}

impl Theme {
    pub fn from_config(name: &str) -> Self {
        match name {
            "light" => Self::light(),
            "terminal" => Self::terminal(),
            "auto" => match detect_terminal_background() {
                TerminalBackground::Light => Self::light(),
                TerminalBackground::Dark => Self::dark(),
            },
            _ => Self::dark(),
        }
    }

    pub fn dark() -> Self {
        let muted = Color::Rgb(88, 91, 112);
        let surface = Color::Rgb(49, 50, 68);
        let accent = Color::Rgb(137, 180, 250);
        let green = Color::Rgb(166, 227, 161);
        let peach = Color::Rgb(250, 179, 135);
        let red = Color::Rgb(243, 139, 168);
        let mauve = Color::Rgb(203, 166, 247);
        let yellow = Color::Rgb(249, 226, 175);
        let teal = Color::Rgb(148, 226, 213);
        let sapphire = Color::Rgb(116, 199, 236);
        let base = Color::Rgb(30, 30, 46);

        Self {
            bg: Color::Reset,
            fg: Color::White,
            dim: Style::default().fg(muted),
            accent,
            muted_fg: muted,
            user_label: Style::default().fg(mauve).add_modifier(Modifier::BOLD),
            assistant_label: Style::default().fg(accent).add_modifier(Modifier::BOLD),
            border: Style::default().fg(surface),
            input_prompt: Style::default().fg(accent),
            status_bar: Style::default().fg(muted),
            code_bg: base,
            inline_code: Style::default().fg(peach),
            error: Style::default().fg(red),
            tool_name: Style::default().fg(yellow).add_modifier(Modifier::BOLD),
            tool_output: Style::default().fg(muted),
            tool_success: Style::default().fg(green),
            heading: Style::default().fg(accent).add_modifier(Modifier::BOLD),
            bold: Style::default().add_modifier(Modifier::BOLD),
            italic: Style::default().add_modifier(Modifier::ITALIC),
            blockquote: Style::default().fg(muted),
            link: Style::default()
                .fg(accent)
                .add_modifier(Modifier::UNDERLINED),
            list_bullet: Style::default().fg(muted),
            scrollbar_track: Style::default().fg(surface),
            scrollbar_thumb: Style::default().fg(muted),
            highlight: Style::default().fg(base).bg(accent),
            tool_file_read: Style::default().fg(sapphire),
            tool_file_write: Style::default().fg(peach),
            tool_directory: Style::default().fg(accent),
            tool_search: Style::default().fg(mauve),
            tool_command: Style::default().fg(green),
            tool_mcp: Style::default().fg(teal),
            tool_skill: Style::default().fg(mauve),
            tool_badge_bg: surface,
            tool_path: Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::UNDERLINED),
            thinking: Style::default().fg(muted),
            mode_normal_fg: base,
            mode_normal_bg: muted,
            mode_insert_fg: base,
            mode_insert_bg: accent,
            cost: Style::default().fg(Color::Rgb(180, 150, 90)),
            user_text: Style::default().fg(Color::Rgb(205, 214, 244)),
            tool_action: Style::default().fg(muted),
            separator: Style::default().fg(Color::Rgb(60, 62, 80)),
            tool_exit_ok: Style::default().fg(green),
            tool_exit_err: Style::default().fg(red),
            syntect_theme: Some("base16-ocean.dark"),
        }
    }

    pub fn light() -> Self {
        let muted = Color::Rgb(140, 143, 161);
        let surface = Color::Rgb(204, 208, 218);
        let accent = Color::Rgb(30, 102, 245);
        let green = Color::Rgb(64, 160, 43);
        let peach = Color::Rgb(254, 100, 11);
        let red = Color::Rgb(210, 15, 57);
        let mauve = Color::Rgb(136, 57, 239);
        let yellow = Color::Rgb(223, 142, 29);
        let teal = Color::Rgb(23, 146, 153);
        let sapphire = Color::Rgb(32, 159, 181);
        let base = Color::Rgb(239, 241, 245);
        let text = Color::Rgb(76, 79, 105);

        Self {
            bg: Color::Reset,
            fg: text,
            dim: Style::default().fg(muted),
            accent,
            muted_fg: muted,
            user_label: Style::default().fg(mauve).add_modifier(Modifier::BOLD),
            assistant_label: Style::default().fg(accent).add_modifier(Modifier::BOLD),
            border: Style::default().fg(surface),
            input_prompt: Style::default().fg(accent),
            status_bar: Style::default().fg(muted),
            code_bg: base,
            inline_code: Style::default().fg(peach),
            error: Style::default().fg(red),
            tool_name: Style::default().fg(yellow).add_modifier(Modifier::BOLD),
            tool_output: Style::default().fg(muted),
            tool_success: Style::default().fg(green),
            heading: Style::default().fg(accent).add_modifier(Modifier::BOLD),
            bold: Style::default().add_modifier(Modifier::BOLD),
            italic: Style::default().add_modifier(Modifier::ITALIC),
            blockquote: Style::default().fg(muted),
            link: Style::default()
                .fg(accent)
                .add_modifier(Modifier::UNDERLINED),
            list_bullet: Style::default().fg(muted),
            scrollbar_track: Style::default().fg(surface),
            scrollbar_thumb: Style::default().fg(muted),
            highlight: Style::default().fg(Color::White).bg(accent),
            tool_file_read: Style::default().fg(sapphire),
            tool_file_write: Style::default().fg(peach),
            tool_directory: Style::default().fg(accent),
            tool_search: Style::default().fg(mauve),
            tool_command: Style::default().fg(green),
            tool_mcp: Style::default().fg(teal),
            tool_skill: Style::default().fg(mauve),
            tool_badge_bg: surface,
            tool_path: Style::default().fg(text).add_modifier(Modifier::UNDERLINED),
            thinking: Style::default().fg(muted),
            mode_normal_fg: Color::White,
            mode_normal_bg: muted,
            mode_insert_fg: Color::White,
            mode_insert_bg: accent,
            cost: Style::default().fg(Color::Rgb(160, 120, 40)),
            user_text: Style::default().fg(text),
            tool_action: Style::default().fg(muted),
            separator: Style::default().fg(surface),
            tool_exit_ok: Style::default().fg(green),
            tool_exit_err: Style::default().fg(red),
            syntect_theme: Some("base16-ocean.light"),
        }
    }

    pub fn terminal() -> Self {
        let red = Color::Indexed(1);
        let green = Color::Indexed(2);
        let yellow = Color::Indexed(3);
        let magenta = Color::Indexed(5);
        let cyan = Color::Indexed(6);
        let bright_black = Color::Indexed(8);

        let fg = Color::Reset;
        let dim = Style::default().fg(bright_black);
        let bold = Style::default().add_modifier(Modifier::BOLD);

        Self {
            bg: Color::Reset,
            fg,
            dim,
            accent: fg,
            muted_fg: bright_black,
            user_label: Style::default().fg(magenta).add_modifier(Modifier::BOLD),
            assistant_label: bold,
            border: dim,
            input_prompt: bold,
            status_bar: dim,
            code_bg: Color::Reset,
            inline_code: Style::default().fg(yellow),
            error: Style::default().fg(red),
            tool_name: Style::default().add_modifier(Modifier::BOLD),
            tool_output: dim,
            tool_success: Style::default().fg(green),
            heading: Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            bold,
            italic: Style::default().add_modifier(Modifier::ITALIC),
            blockquote: dim,
            link: Style::default().fg(cyan).add_modifier(Modifier::UNDERLINED),
            list_bullet: dim,
            scrollbar_track: dim,
            scrollbar_thumb: Style::default().fg(fg),
            highlight: Style::default().add_modifier(Modifier::REVERSED),
            tool_file_read: Style::default().fg(cyan),
            tool_file_write: Style::default().fg(yellow),
            tool_directory: bold,
            tool_search: Style::default().fg(magenta),
            tool_command: Style::default().fg(green),
            tool_mcp: Style::default().fg(cyan),
            tool_skill: Style::default().fg(magenta),
            tool_badge_bg: bright_black,
            tool_path: Style::default().add_modifier(Modifier::UNDERLINED),
            thinking: dim,
            mode_normal_fg: Color::Reset,
            mode_normal_bg: bright_black,
            mode_insert_fg: Color::Indexed(0),
            mode_insert_bg: Color::Reset,
            cost: dim,
            user_text: Style::default(),
            tool_action: dim,
            separator: dim,
            tool_exit_ok: Style::default().fg(green),
            tool_exit_err: Style::default().fg(red),
            syntect_theme: None,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::terminal()
    }
}
