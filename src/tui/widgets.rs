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
