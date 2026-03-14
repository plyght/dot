#[derive(Clone)]
pub struct ModelEntry {
    pub provider: String,
    pub model: String,
}

pub struct ModelSelector {
    pub visible: bool,
    pub entries: Vec<ModelEntry>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub query: String,
    pub current_provider: String,
    pub current_model: String,
    pub favorites: Vec<String>,
}

impl Default for ModelSelector {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelSelector {
    pub fn new() -> Self {
        Self {
            visible: false,
            entries: Vec::new(),
            filtered: Vec::new(),
            selected: 0,
            query: String::new(),
            current_provider: String::new(),
            current_model: String::new(),
            favorites: Vec::new(),
        }
    }

    pub fn open(
        &mut self,
        grouped: Vec<(String, Vec<String>)>,
        current_provider: &str,
        current_model: &str,
    ) {
        self.entries.clear();
        for (provider, models) in grouped {
            for model in models {
                self.entries.push(ModelEntry {
                    provider: provider.clone(),
                    model,
                });
            }
        }
        self.current_provider = current_provider.to_string();
        self.current_model = current_model.to_string();
        self.query.clear();
        self.visible = true;
        self.apply_filter();
        if let Some(pos) = self.filtered.iter().position(|&i| {
            self.entries[i].provider == current_provider && self.entries[i].model == current_model
        }) {
            self.selected = pos;
        }
    }

    pub fn toggle_favorite(&mut self) -> Option<String> {
        let idx = *self.filtered.get(self.selected)?;
        let model = self.entries[idx].model.clone();
        if let Some(pos) = self.favorites.iter().position(|f| f == &model) {
            self.favorites.remove(pos);
        } else {
            self.favorites.push(model.clone());
        }
        Some(model)
    }

    pub fn apply_filter(&mut self) {
        let q = self.query.to_lowercase();
        self.filtered = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                if q.is_empty() {
                    return true;
                }
                e.model.to_lowercase().contains(&q) || e.provider.to_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect();
        self.filtered.sort_by(|&a, &b| {
            let a_fav = self.favorites.contains(&self.entries[a].model);
            let b_fav = self.favorites.contains(&self.entries[b].model);
            b_fav.cmp(&a_fav)
        });
        if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len().saturating_sub(1);
        }
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.query.clear();
    }

    pub fn up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn down(&mut self) {
        if self.selected + 1 < self.filtered.len() {
            self.selected += 1;
        }
    }

    pub fn confirm(&mut self) -> Option<ModelEntry> {
        if self.visible && !self.filtered.is_empty() {
            self.visible = false;
            let entry = self.entries[self.filtered[self.selected]].clone();
            self.query.clear();
            Some(entry)
        } else {
            None
        }
    }
}

#[derive(Clone)]
pub struct AgentEntry {
    pub name: String,
    pub description: String,
}

pub struct AgentSelector {
    pub visible: bool,
    pub entries: Vec<AgentEntry>,
    pub selected: usize,
    pub current: String,
}

impl Default for AgentSelector {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentSelector {
    pub fn new() -> Self {
        Self {
            visible: false,
            entries: Vec::new(),
            selected: 0,
            current: String::new(),
        }
    }

    pub fn open(&mut self, agents: Vec<AgentEntry>, current: &str) {
        self.entries = agents;
        self.current = current.to_string();
        self.visible = true;
        self.selected = self
            .entries
            .iter()
            .position(|e| e.name == current)
            .unwrap_or(0);
    }

    pub fn close(&mut self) {
        self.visible = false;
    }

    pub fn up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn down(&mut self) {
        if self.selected + 1 < self.entries.len() {
            self.selected += 1;
        }
    }

    pub fn confirm(&mut self) -> Option<AgentEntry> {
        if self.visible && !self.entries.is_empty() {
            self.visible = false;
            Some(self.entries[self.selected].clone())
        } else {
            None
        }
    }
}

use chrono::{DateTime, Utc};

pub struct SlashCommand {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub description: &'static str,
    pub shortcut: &'static str,
}

pub const COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        name: "model",
        aliases: &["m"],
        description: "switch model",
        shortcut: "",
    },
    SlashCommand {
        name: "agent",
        aliases: &["a"],
        description: "switch agent profile",
        shortcut: "Tab",
    },
    SlashCommand {
        name: "clear",
        aliases: &["cl"],
        description: "clear conversation",
        shortcut: "",
    },
    SlashCommand {
        name: "help",
        aliases: &["h"],
        description: "show commands",
        shortcut: "",
    },
    SlashCommand {
        name: "thinking",
        aliases: &["t", "think"],
        description: "set thinking level",
        shortcut: "^T",
    },
    SlashCommand {
        name: "sessions",
        aliases: &["s", "sess"],
        description: "resume a previous session",
        shortcut: "",
    },
    SlashCommand {
        name: "new",
        aliases: &["n"],
        description: "start new conversation",
        shortcut: "",
    },
    SlashCommand {
        name: "rename",
        aliases: &["r"],
        description: "rename this session",
        shortcut: "^R",
    },
    SlashCommand {
        name: "export",
        aliases: &["e"],
        description: "export session to markdown",
        shortcut: "",
    },
    SlashCommand {
        name: "login",
        aliases: &["l"],
        description: "manage provider credentials",
        shortcut: "",
    },
];
#[derive(Debug, Clone, PartialEq)]
pub enum PaletteEntryKind {
    Command,
    Skill,
}

#[derive(Debug, Clone)]
pub struct PaletteEntry {
    pub name: String,
    pub description: String,
    pub shortcut: String,
    pub kind: PaletteEntryKind,
}

pub struct CommandPalette {
    pub visible: bool,
    pub selected: usize,
    pub filtered: Vec<usize>,
    pub entries: Vec<PaletteEntry>,
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandPalette {
    pub fn new() -> Self {
        Self {
            visible: false,
            selected: 0,
            filtered: Vec::new(),
            entries: Vec::new(),
        }
    }

    pub fn set_skills(&mut self, skills: &[(String, String)]) {
        self.entries.clear();
        for cmd in COMMANDS {
            self.entries.push(PaletteEntry {
                name: cmd.name.to_string(),
                description: cmd.description.to_string(),
                shortcut: cmd.shortcut.to_string(),
                kind: PaletteEntryKind::Command,
            });
        }
        for (name, desc) in skills {
            self.entries.push(PaletteEntry {
                name: name.clone(),
                description: desc.clone(),
                shortcut: String::new(),
                kind: PaletteEntryKind::Skill,
            });
        }
    }

    pub fn add_custom_commands(&mut self, commands: &[(&str, &str)]) {
        for (name, desc) in commands {
            self.entries.push(PaletteEntry {
                name: name.to_string(),
                description: desc.to_string(),
                shortcut: String::new(),
                kind: PaletteEntryKind::Command,
            });
        }
    }

    pub fn update_filter(&mut self, input: &str) {
        if self.entries.is_empty() {
            for cmd in COMMANDS {
                self.entries.push(PaletteEntry {
                    name: cmd.name.to_string(),
                    description: cmd.description.to_string(),
                    shortcut: cmd.shortcut.to_string(),
                    kind: PaletteEntryKind::Command,
                });
            }
        }
        let query = input.strip_prefix('/').unwrap_or(input).to_lowercase();
        self.filtered = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                if query.is_empty() {
                    return true;
                }
                e.name.to_lowercase().starts_with(&query)
                    || e.description.to_lowercase().contains(&query)
            })
            .map(|(i, _)| i)
            .collect();
        if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len().saturating_sub(1);
        }
    }

    pub fn open(&mut self, input: &str) {
        self.visible = true;
        self.selected = 0;
        self.update_filter(input);
    }

    pub fn close(&mut self) {
        self.visible = false;
    }

    pub fn up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn down(&mut self) {
        if self.selected + 1 < self.filtered.len() {
            self.selected += 1;
        }
    }

    pub fn confirm(&mut self) -> Option<PaletteEntry> {
        if self.visible && !self.filtered.is_empty() {
            self.visible = false;
            Some(self.entries[self.filtered[self.selected]].clone())
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThinkingLevel {
    Off,
    Low,
    Medium,
    High,
}

impl ThinkingLevel {
    pub fn budget_tokens(self) -> u32 {
        match self {
            ThinkingLevel::Off => 0,
            ThinkingLevel::Low => 1024,
            ThinkingLevel::Medium => 8192,
            ThinkingLevel::High => 32768,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ThinkingLevel::Off => "off",
            ThinkingLevel::Low => "low",
            ThinkingLevel::Medium => "medium",
            ThinkingLevel::High => "high",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            ThinkingLevel::Off => "no extended thinking",
            ThinkingLevel::Low => "1k token budget",
            ThinkingLevel::Medium => "8k token budget",
            ThinkingLevel::High => "32k token budget",
        }
    }

    pub fn all() -> &'static [ThinkingLevel] {
        &[
            ThinkingLevel::Off,
            ThinkingLevel::Low,
            ThinkingLevel::Medium,
            ThinkingLevel::High,
        ]
    }

    pub fn from_budget(budget: u32) -> Self {
        match budget {
            0 => ThinkingLevel::Off,
            1..=4095 => ThinkingLevel::Low,
            4096..=16383 => ThinkingLevel::Medium,
            _ => ThinkingLevel::High,
        }
    }

    pub fn next(self) -> Self {
        let all = Self::all();
        let idx = all.iter().position(|l| *l == self).unwrap_or(0);
        all[(idx + 1) % all.len()]
    }
}

pub struct ThinkingSelector {
    pub visible: bool,
    pub selected: usize,
    pub current: ThinkingLevel,
}

impl Default for ThinkingSelector {
    fn default() -> Self {
        Self::new()
    }
}

impl ThinkingSelector {
    pub fn new() -> Self {
        Self {
            visible: false,
            selected: 0,
            current: ThinkingLevel::Off,
        }
    }

    pub fn open(&mut self, current: ThinkingLevel) {
        self.current = current;
        self.selected = ThinkingLevel::all()
            .iter()
            .position(|l| *l == current)
            .unwrap_or(0);
        self.visible = true;
    }

    pub fn close(&mut self) {
        self.visible = false;
    }

    pub fn up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn down(&mut self) {
        if self.selected + 1 < ThinkingLevel::all().len() {
            self.selected += 1;
        }
    }

    pub fn confirm(&mut self) -> Option<ThinkingLevel> {
        if self.visible {
            self.visible = false;
            Some(ThinkingLevel::all()[self.selected])
        } else {
            None
        }
    }
}

#[derive(Clone)]
pub struct SessionEntry {
    pub id: String,
    pub title: String,
    pub subtitle: String,
}

pub struct SessionSelector {
    pub visible: bool,
    pub entries: Vec<SessionEntry>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub query: String,
}

impl Default for SessionSelector {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionSelector {
    pub fn new() -> Self {
        Self {
            visible: false,
            entries: Vec::new(),
            filtered: Vec::new(),
            selected: 0,
            query: String::new(),
        }
    }

    pub fn open(&mut self, entries: Vec<SessionEntry>) {
        self.entries = entries;
        self.query.clear();
        self.visible = true;
        self.selected = 0;
        self.apply_filter();
    }

    pub fn apply_filter(&mut self) {
        let q = self.query.to_lowercase();
        self.filtered = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                if q.is_empty() {
                    return true;
                }
                e.title.to_lowercase().contains(&q) || e.subtitle.to_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect();
        if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len().saturating_sub(1);
        }
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.query.clear();
    }

    pub fn up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn down(&mut self) {
        if self.selected + 1 < self.filtered.len() {
            self.selected += 1;
        }
    }

    pub fn confirm(&mut self) -> Option<String> {
        if self.visible && !self.filtered.is_empty() {
            self.visible = false;
            let id = self.entries[self.filtered[self.selected]].id.clone();
            self.query.clear();
            Some(id)
        } else {
            None
        }
    }
}

pub struct HelpPopup {
    pub visible: bool,
}

impl Default for HelpPopup {
    fn default() -> Self {
        Self::new()
    }
}

impl HelpPopup {
    pub fn new() -> Self {
        Self { visible: false }
    }

    pub fn open(&mut self) {
        self.visible = true;
    }

    pub fn close(&mut self) {
        self.visible = false;
    }
}

pub fn time_ago(iso: &str) -> String {
    if let Ok(dt) = iso.parse::<DateTime<Utc>>() {
        let secs = Utc::now().signed_duration_since(dt).num_seconds();
        if secs < 60 {
            return "just now".to_string();
        }
        if secs < 3600 {
            return format!("{}m ago", secs / 60);
        }
        if secs < 86400 {
            return format!("{}h ago", secs / 3600);
        }
        if secs < 604800 {
            return format!("{}d ago", secs / 86400);
        }
        return format!("{}w ago", secs / 604800);
    }
    iso.to_string()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoginStep {
    SelectProvider,
    SelectMethod,
    EnterApiKey,
    OAuthWaiting,
    OAuthExchanging,
}

pub struct LoginPopup {
    pub visible: bool,
    pub step: LoginStep,
    pub selected: usize,
    pub provider: Option<String>,
    pub key_input: String,
    pub status: Option<String>,
    pub oauth_url: Option<String>,
    pub oauth_verifier: Option<String>,
    pub oauth_create_key: bool,
    pub code_input: String,
    pub from_welcome: bool,
}

impl Default for LoginPopup {
    fn default() -> Self {
        Self::new()
    }
}

impl LoginPopup {
    pub fn new() -> Self {
        Self {
            visible: false,
            step: LoginStep::SelectProvider,
            selected: 0,
            provider: None,
            key_input: String::new(),
            status: None,
            oauth_url: None,
            oauth_verifier: None,
            oauth_create_key: false,
            code_input: String::new(),
            from_welcome: false,
        }
    }

    pub fn open(&mut self) {
        self.visible = true;
        self.step = LoginStep::SelectProvider;
        self.selected = 0;
        self.provider = None;
        self.key_input.clear();
        self.code_input.clear();
        self.status = None;
        self.oauth_url = None;
        self.oauth_verifier = None;
        self.oauth_create_key = false;
        self.from_welcome = false;
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.key_input.clear();
        self.code_input.clear();
        self.status = None;
        self.oauth_url = None;
        self.oauth_verifier = None;
    }

    pub fn providers() -> &'static [&'static str] {
        &["Anthropic", "OpenAI", "GitHub Copilot"]
    }

    pub fn anthropic_methods() -> &'static [&'static str] {
        &[
            "Claude Pro/Max (OAuth)",
            "Create API Key (OAuth)",
            "Enter API Key",
        ]
    }

    pub fn up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn down(&mut self) {
        let max = match self.step {
            LoginStep::SelectProvider => Self::providers().len(),
            LoginStep::SelectMethod => Self::anthropic_methods().len(),
            LoginStep::EnterApiKey | LoginStep::OAuthWaiting | LoginStep::OAuthExchanging => 0,
        };
        if max > 0 && self.selected + 1 < max {
            self.selected += 1;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WelcomeChoice {
    Login,
    UseEnvKeys,
    SetEnvVars,
}

pub struct WelcomeScreen {
    pub visible: bool,
    pub selected: usize,
}

impl Default for WelcomeScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl WelcomeScreen {
    pub fn new() -> Self {
        Self {
            visible: false,
            selected: 0,
        }
    }

    pub fn open(&mut self) {
        self.visible = true;
        self.selected = 0;
    }

    pub fn close(&mut self) {
        self.visible = false;
    }

    pub fn choices() -> &'static [(&'static str, &'static str)] {
        &[
            ("Login", "OAuth or API key"),
            ("Use env keys", "ANTHROPIC_API_KEY / OPENAI_API_KEY"),
            ("Set env variables", "configure keys in your shell"),
        ]
    }

    pub fn up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn down(&mut self) {
        if self.selected + 1 < Self::choices().len() {
            self.selected += 1;
        }
    }

    pub fn confirm(&mut self) -> Option<WelcomeChoice> {
        if !self.visible {
            return None;
        }
        self.visible = false;
        match self.selected {
            0 => Some(WelcomeChoice::Login),
            1 => Some(WelcomeChoice::UseEnvKeys),
            2 => Some(WelcomeChoice::SetEnvVars),
            _ => None,
        }
    }
}

pub struct MessageContextMenu {
    pub visible: bool,
    pub message_index: usize,
    pub selected: usize,
    pub screen_x: u16,
    pub screen_y: u16,
}

impl Default for MessageContextMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageContextMenu {
    pub fn new() -> Self {
        Self {
            visible: false,
            message_index: 0,
            selected: 0,
            screen_x: 0,
            screen_y: 0,
        }
    }

    pub fn open(&mut self, message_index: usize, x: u16, y: u16) {
        self.visible = true;
        self.message_index = message_index;
        self.selected = 0;
        self.screen_x = x;
        self.screen_y = y;
    }

    pub fn close(&mut self) {
        self.visible = false;
    }

    pub fn up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn down(&mut self) {
        if self.selected < Self::labels().len() - 1 {
            self.selected += 1;
        }
    }

    pub fn confirm(&mut self) -> Option<(usize, usize)> {
        if self.visible {
            self.visible = false;
            Some((self.selected, self.message_index))
        } else {
            None
        }
    }

    pub fn labels() -> &'static [&'static str] {
        &["revert to message", "fork from here", "copy"]
    }
}

#[derive(Clone)]
pub struct FilePickerEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

pub struct FilePicker {
    pub visible: bool,
    pub entries: Vec<FilePickerEntry>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub query: String,
    pub at_pos: usize,
    base_dir: String,
}

impl Default for FilePicker {
    fn default() -> Self {
        Self::new()
    }
}

impl FilePicker {
    pub fn new() -> Self {
        Self {
            visible: false,
            entries: Vec::new(),
            filtered: Vec::new(),
            selected: 0,
            query: String::new(),
            at_pos: 0,
            base_dir: String::new(),
        }
    }

    pub fn open(&mut self, at_pos: usize) {
        self.visible = true;
        self.at_pos = at_pos;
        self.query.clear();
        self.selected = 0;
        self.base_dir.clear();
        self.populate();
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.query.clear();
        self.entries.clear();
        self.filtered.clear();
    }

    pub fn populate(&mut self) {
        let (dir, _) = self.dir_and_filter();
        self.base_dir = dir.clone();
        self.entries.clear();

        let read_path = if dir.is_empty() {
            ".".to_string()
        } else {
            dir.clone()
        };
        let Ok(rd) = std::fs::read_dir(&read_path) else {
            return;
        };

        let mut dirs = Vec::new();
        let mut files = Vec::new();

        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let rel = if dir.is_empty() {
                name.clone()
            } else {
                format!("{}{}", dir, name)
            };
            let e = FilePickerEntry {
                name,
                path: rel,
                is_dir,
            };
            if is_dir {
                dirs.push(e);
            } else {
                files.push(e);
            }
        }

        dirs.sort_by(|a, b| a.name.cmp(&b.name));
        files.sort_by(|a, b| a.name.cmp(&b.name));
        self.entries.extend(dirs);
        self.entries.extend(files);
        self.apply_filter();
    }

    fn dir_and_filter(&self) -> (String, String) {
        if let Some(pos) = self.query.rfind('/') {
            (
                self.query[..=pos].to_string(),
                self.query[pos + 1..].to_string(),
            )
        } else {
            (String::new(), self.query.clone())
        }
    }

    pub fn apply_filter(&mut self) {
        let (_, filter) = self.dir_and_filter();
        let q = filter.to_lowercase();
        self.filtered = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                if q.is_empty() {
                    return true;
                }
                e.name.to_lowercase().starts_with(&q) || e.name.to_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect();
        if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len().saturating_sub(1);
        }
    }

    pub fn update_query(&mut self, query: &str) {
        let (old_dir, _) = self.dir_and_filter();
        self.query = query.to_string();
        let (new_dir, _) = self.dir_and_filter();
        if new_dir != old_dir {
            self.populate();
        } else {
            self.apply_filter();
        }
    }

    pub fn up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn down(&mut self) {
        if self.selected + 1 < self.filtered.len() {
            self.selected += 1;
        }
    }

    pub fn confirm(&mut self) -> Option<FilePickerEntry> {
        if self.visible && !self.filtered.is_empty() {
            self.visible = false;
            let entry = self.entries[self.filtered[self.selected]].clone();
            self.query.clear();
            Some(entry)
        } else {
            None
        }
    }
}
