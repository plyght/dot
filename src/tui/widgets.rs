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
}

pub const COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        name: "model",
        aliases: &["m"],
        description: "switch model",
    },
    SlashCommand {
        name: "agent",
        aliases: &["a"],
        description: "switch agent profile",
    },
    SlashCommand {
        name: "clear",
        aliases: &["cl"],
        description: "clear conversation",
    },
    SlashCommand {
        name: "help",
        aliases: &["h"],
        description: "show commands",
    },
    SlashCommand {
        name: "thinking",
        aliases: &["t", "think"],
        description: "set thinking level",
    },
    SlashCommand {
        name: "sessions",
        aliases: &["s", "sess"],
        description: "resume a previous session",
    },
    SlashCommand {
        name: "new",
        aliases: &["n"],
        description: "start new conversation",
    },
];

pub struct CommandPalette {
    pub visible: bool,
    pub selected: usize,
    pub filtered: Vec<usize>,
}

impl CommandPalette {
    pub fn new() -> Self {
        Self {
            visible: false,
            selected: 0,
            filtered: Vec::new(),
        }
    }

    pub fn update_filter(&mut self, input: &str) {
        let query = input.strip_prefix('/').unwrap_or(input).to_lowercase();
        self.filtered = COMMANDS
            .iter()
            .enumerate()
            .filter(|(_, cmd)| {
                if query.is_empty() {
                    return true;
                }
                cmd.name.starts_with(&query) || cmd.aliases.iter().any(|a| a.starts_with(&query))
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

    pub fn confirm(&mut self) -> Option<&'static str> {
        if self.visible && !self.filtered.is_empty() {
            self.visible = false;
            Some(COMMANDS[self.filtered[self.selected]].name)
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
}

pub struct ThinkingSelector {
    pub visible: bool,
    pub selected: usize,
    pub current: ThinkingLevel,
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
