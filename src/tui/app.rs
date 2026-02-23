use crate::agent::AgentEvent;
use crate::tui::theme::Theme;

pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub tool_calls: Vec<ToolCallDisplay>,
}

pub struct ToolCallDisplay {
    pub name: String,
    pub input: String,
    pub output: Option<String>,
    pub is_error: bool,
}

pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_cost: f64,
}

impl Default for TokenUsage {
    fn default() -> Self {
        Self {
            input_tokens: 0,
            output_tokens: 0,
            total_cost: 0.0,
        }
    }
}

#[derive(PartialEq, Clone, Copy)]
pub enum AppMode {
    Normal,
    Insert,
}


pub struct ModelSelector {
    pub visible: bool,
    pub models: Vec<String>,
    pub selected: usize,
}

impl ModelSelector {
    pub fn new() -> Self {
        Self {
            visible: false,
            models: Vec::new(),
            selected: 0,
        }
    }

    pub fn open(&mut self, models: Vec<String>, current: &str) {
        self.selected = models.iter().position(|m| m == current).unwrap_or(0);
        self.models = models;
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
        if self.selected + 1 < self.models.len() {
            self.selected += 1;
        }
    }

    pub fn confirm(&mut self) -> Option<String> {
        if self.visible {
            self.visible = false;
            self.models.get(self.selected).cloned()
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
                cmd.name.starts_with(&query)
                    || cmd.aliases.iter().any(|a| a.starts_with(&query))
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

pub struct App {
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub cursor_pos: usize,
    pub scroll_offset: u16,
    pub max_scroll: u16,
    pub is_streaming: bool,
    pub current_response: String,
    pub should_quit: bool,
    pub mode: AppMode,
    pub usage: TokenUsage,
    pub model_name: String,
    pub provider_name: String,
    pub theme: Theme,

    pub pending_tool_name: Option<String>,
    pub pending_tool_input: String,
    pub current_tool_calls: Vec<ToolCallDisplay>,
    pub error_message: Option<String>,
    pub model_selector: ModelSelector,
    pub command_palette: CommandPalette,
}

impl App {
    pub fn new(model_name: String, provider_name: String) -> Self {
        Self {
            messages: Vec::new(),
            input: String::new(),
            cursor_pos: 0,
            scroll_offset: 0,
            max_scroll: 0,
            is_streaming: false,
            current_response: String::new(),
            should_quit: false,
            mode: AppMode::Insert,
            usage: TokenUsage::default(),
            model_name,
            provider_name,
            theme: Theme::default(),
            pending_tool_name: None,
            pending_tool_input: String::new(),
            current_tool_calls: Vec::new(),
            error_message: None,
            model_selector: ModelSelector::new(),
            command_palette: CommandPalette::new(),
        }
    }

    pub fn handle_agent_event(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::TextDelta(text) => {
                self.current_response.push_str(&text);
            }
            AgentEvent::TextComplete(text) => {
                if !text.is_empty() || !self.current_response.is_empty() {
                    let content = if self.current_response.is_empty() {
                        text
                    } else {
                        self.current_response.clone()
                    };
                    self.messages.push(ChatMessage {
                        role: "assistant".to_string(),
                        content,
                        tool_calls: std::mem::take(&mut self.current_tool_calls),
                    });
                }
                self.current_response.clear();
            }
            AgentEvent::ToolCallStart { name, .. } => {
                self.pending_tool_name = Some(name);
                self.pending_tool_input.clear();
            }
            AgentEvent::ToolCallInputDelta(delta) => {
                self.pending_tool_input.push_str(&delta);
            }
            AgentEvent::ToolCallExecuting { name, input, .. } => {
                self.pending_tool_name = Some(name.clone());
                self.pending_tool_input = input;
            }
            AgentEvent::ToolCallResult {
                name,
                output,
                is_error,
                ..
            } => {
                let input = std::mem::take(&mut self.pending_tool_input);
                self.current_tool_calls.push(ToolCallDisplay {
                    name: name.clone(),
                    input,
                    output: Some(output),
                    is_error,
                });
                self.pending_tool_name = None;
            }
            AgentEvent::Done { usage } => {
                self.is_streaming = false;
                self.usage.input_tokens += usage.input_tokens;
                self.usage.output_tokens += usage.output_tokens;
                self.scroll_to_bottom();
            }
            AgentEvent::Error(msg) => {
                self.is_streaming = false;
                self.error_message = Some(msg);
            }
        }
    }

    pub fn take_input(&mut self) -> Option<String> {
        let trimmed = self.input.trim().to_string();
        if trimmed.is_empty() {
            return None;
        }
        self.messages.push(ChatMessage {
            role: "user".to_string(),
            content: trimmed.clone(),
            tool_calls: Vec::new(),
        });
        self.input.clear();
        self.cursor_pos = 0;
        self.is_streaming = true;
        self.current_response.clear();
        self.current_tool_calls.clear();
        self.error_message = None;
        self.scroll_to_bottom();
        Some(trimmed)
    }

    pub fn scroll_up(&mut self, n: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
    }

    pub fn scroll_down(&mut self, n: u16) {
        self.scroll_offset = (self.scroll_offset + n).min(self.max_scroll);
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.max_scroll;
    }

    pub fn clear_conversation(&mut self) {
        self.messages.clear();
        self.current_response.clear();
        self.current_tool_calls.clear();
        self.scroll_offset = 0;
        self.max_scroll = 0;
        self.usage = TokenUsage::default();
        self.error_message = None;
    }

    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    pub fn delete_char_before(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self.input[..self.cursor_pos]
                .chars()
                .last()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.cursor_pos -= prev;
            self.input.remove(self.cursor_pos);
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self.input[..self.cursor_pos]
                .chars()
                .last()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.cursor_pos -= prev;
        }
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor_pos < self.input.len() {
            let next = self.input[self.cursor_pos..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.cursor_pos += next;
        }
    }

    pub fn move_cursor_home(&mut self) {
        self.cursor_pos = 0;
    }

    pub fn move_cursor_end(&mut self) {
        self.cursor_pos = self.input.len();
    }
}
