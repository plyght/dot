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

#[derive(Debug, Clone, Copy)]
pub struct SyntaxStyles {
    pub keyword: Style,
    pub string: Style,
    pub comment: Style,
    pub function: Style,
    pub type_name: Style,
    pub number: Style,
    pub constant: Style,
    pub attribute: Style,
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
    pub syntax: Option<SyntaxStyles>,
    pub syntect_theme: Option<&'static str>,
    pub diff_add: Style,
    pub diff_remove: Style,
    pub diff_hunk: Style,
    pub input_bg: Color,
    pub input_fg: Color,
    pub input_dim_fg: Color,
    pub progress_bar_filled: Style,
    pub progress_bar_empty: Style,
    pub streaming_dot: Style,
    pub user_text_bg: Color,
    pub subagent_border: Style,
    pub subagent_header: Style,
    pub subagent_done: Style,
    pub subagent_working: Style,
    pub diff_add_bg: Color,
    pub diff_remove_bg: Color,
    pub assistant_marker: Style,
    pub user_role_label: Style,
    pub assistant_role_label: Style,
    pub message_separator: Style,
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
        let surface = Color::Rgb(42, 44, 60);
        let accent = Color::Rgb(110, 150, 215);
        let green = Color::Rgb(140, 190, 135);
        let peach = Color::Rgb(210, 155, 115);
        let red = Color::Rgb(200, 120, 145);
        let mauve = Color::Rgb(170, 140, 210);
        let yellow = Color::Rgb(210, 190, 150);
        let teal = Color::Rgb(120, 185, 175);
        let sapphire = Color::Rgb(95, 165, 200);
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
            code_bg: surface,
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
            cost: Style::default().fg(Color::Rgb(165, 135, 80)),
            user_text: Style::default().fg(Color::Rgb(205, 214, 244)),
            tool_action: Style::default().fg(muted),
            separator: Style::default().fg(Color::Rgb(52, 54, 72)),
            tool_exit_ok: Style::default().fg(green),
            tool_exit_err: Style::default().fg(red),
            syntax: None,
            syntect_theme: Some("base16-ocean.dark"),
            diff_add: Style::default().fg(green),
            diff_remove: Style::default().fg(red),
            diff_hunk: Style::default().fg(accent),
            input_bg: Color::Rgb(36, 38, 55),
            input_fg: Color::White,
            input_dim_fg: muted,
            progress_bar_filled: Style::default().fg(accent).add_modifier(Modifier::BOLD),
            progress_bar_empty: Style::default().fg(surface),
            streaming_dot: Style::default().fg(accent),
            user_text_bg: Color::Rgb(38, 40, 58),
            subagent_border: Style::default().fg(surface),
            subagent_header: Style::default().fg(accent).add_modifier(Modifier::BOLD),
            subagent_done: Style::default().fg(green),
            subagent_working: Style::default().fg(accent),
            diff_add_bg: Color::Rgb(25, 45, 30),
            diff_remove_bg: Color::Rgb(55, 25, 30),
            assistant_marker: Style::default().fg(accent).add_modifier(Modifier::BOLD),
            user_role_label: Style::default().fg(mauve).add_modifier(Modifier::BOLD),
            assistant_role_label: Style::default().fg(accent).add_modifier(Modifier::BOLD),
            message_separator: Style::default().fg(Color::Rgb(45, 47, 65)),
        }
    }

    pub fn light() -> Self {
        let muted = Color::Rgb(140, 143, 161);
        let surface = Color::Rgb(204, 208, 218);
        let accent = Color::Rgb(35, 90, 210);
        let green = Color::Rgb(55, 135, 40);
        let peach = Color::Rgb(210, 90, 20);
        let red = Color::Rgb(175, 30, 60);
        let mauve = Color::Rgb(110, 55, 190);
        let yellow = Color::Rgb(185, 120, 30);
        let teal = Color::Rgb(25, 125, 130);
        let sapphire = Color::Rgb(30, 130, 155);
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
            code_bg: surface,
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
            cost: Style::default().fg(Color::Rgb(150, 110, 35)),
            user_text: Style::default().fg(text),
            tool_action: Style::default().fg(muted),
            separator: Style::default().fg(surface),
            tool_exit_ok: Style::default().fg(green),
            tool_exit_err: Style::default().fg(red),
            syntax: None,
            syntect_theme: Some("base16-ocean.light"),
            diff_add: Style::default().fg(green),
            diff_remove: Style::default().fg(red),
            diff_hunk: Style::default().fg(accent),
            input_bg: Color::Rgb(210, 214, 225),
            input_fg: text,
            input_dim_fg: muted,
            progress_bar_filled: Style::default().fg(accent).add_modifier(Modifier::BOLD),
            progress_bar_empty: Style::default().fg(surface),
            streaming_dot: Style::default().fg(accent),
            user_text_bg: Color::Rgb(218, 222, 232),
            subagent_border: Style::default().fg(surface),
            subagent_header: Style::default().fg(accent).add_modifier(Modifier::BOLD),
            subagent_done: Style::default().fg(green),
            subagent_working: Style::default().fg(accent),
            diff_add_bg: Color::Rgb(220, 245, 220),
            diff_remove_bg: Color::Rgb(255, 225, 225),
            assistant_marker: Style::default().fg(accent).add_modifier(Modifier::BOLD),
            user_role_label: Style::default().fg(mauve).add_modifier(Modifier::BOLD),
            assistant_role_label: Style::default().fg(accent).add_modifier(Modifier::BOLD),
            message_separator: Style::default().fg(Color::Rgb(195, 200, 212)),
        }
    }

    pub fn terminal() -> Self {
        let dim = Style::default().add_modifier(Modifier::DIM);
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let muted = Color::Indexed(8);

        Self {
            bg: Color::Reset,
            fg: Color::Reset,
            dim,
            accent: Color::Reset,
            muted_fg: muted,
            user_label: bold,
            assistant_label: Style::default(),
            border: dim,
            input_prompt: bold,
            status_bar: dim,
            code_bg: Color::Indexed(0),
            inline_code: Style::default().fg(muted),
            error: Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED),
            tool_name: bold,
            tool_output: dim,
            tool_success: bold,
            heading: Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            bold,
            italic: Style::default().add_modifier(Modifier::ITALIC),
            blockquote: dim,
            link: Style::default().add_modifier(Modifier::UNDERLINED),
            list_bullet: dim,
            scrollbar_track: dim,
            scrollbar_thumb: Style::default(),
            highlight: Style::default().add_modifier(Modifier::REVERSED),
            tool_file_read: Style::default().fg(muted),
            tool_file_write: Style::default().fg(muted),
            tool_directory: bold,
            tool_search: Style::default().fg(muted),
            tool_command: Style::default().fg(muted),
            tool_mcp: Style::default().fg(muted),
            tool_skill: Style::default().fg(muted),
            tool_badge_bg: muted,
            tool_path: Style::default().add_modifier(Modifier::UNDERLINED),
            thinking: dim,
            mode_normal_fg: Color::Reset,
            mode_normal_bg: muted,
            mode_insert_fg: Color::Indexed(0),
            mode_insert_bg: Color::Reset,
            cost: dim,
            user_text: bold,
            tool_action: dim,
            separator: dim,
            tool_exit_ok: bold,
            tool_exit_err: Style::default().add_modifier(Modifier::BOLD),
            syntax: Some(SyntaxStyles {
                keyword: bold,
                string: Style::default().fg(muted),
                comment: dim.add_modifier(Modifier::ITALIC),
                function: bold,
                type_name: Style::default().fg(muted),
                number: Style::default().fg(muted),
                constant: bold,
                attribute: Style::default().fg(muted),
            }),
            syntect_theme: None,
            diff_add: bold,
            diff_remove: dim,
            diff_hunk: Style::default().fg(muted),
            input_bg: muted,
            input_fg: Color::Reset,
            input_dim_fg: muted,
            progress_bar_filled: bold,
            progress_bar_empty: dim,
            streaming_dot: dim,
            user_text_bg: Color::Indexed(0),
            subagent_border: dim,
            subagent_header: bold,
            subagent_done: bold,
            subagent_working: dim,
            diff_add_bg: Color::Reset,
            diff_remove_bg: Color::Reset,
            assistant_marker: bold,
            user_role_label: bold,
            assistant_role_label: Style::default(),
            message_separator: dim,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::terminal()
    }
}
