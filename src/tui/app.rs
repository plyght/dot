use std::time::Instant;

use ratatui::layout::Rect;

use crate::agent::AgentEvent;
use crate::tui::theme::Theme;
use crate::tui::tools::{ToolCallDisplay, ToolCategory, extract_tool_detail};
use crate::tui::widgets::{
    AgentSelector, CommandPalette, ModelSelector, SessionSelector, ThinkingLevel, ThinkingSelector,
};

pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub tool_calls: Vec<ToolCallDisplay>,
    pub thinking: Option<String>,
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

#[derive(Default)]
pub struct LayoutRects {
    pub header: Rect,
    pub messages: Rect,
    pub input: Rect,
    pub status: Rect,
    pub model_selector: Option<Rect>,
    pub command_palette: Option<Rect>,
    pub thinking_selector: Option<Rect>,
    pub session_selector: Option<Rect>,
}

pub struct App {
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub cursor_pos: usize,
    pub scroll_offset: u16,
    pub max_scroll: u16,
    pub is_streaming: bool,
    pub current_response: String,
    pub current_thinking: String,
    pub should_quit: bool,
    pub mode: AppMode,
    pub usage: TokenUsage,
    pub model_name: String,
    pub provider_name: String,
    pub agent_name: String,
    pub theme: Theme,
    pub tick_count: u64,
    pub layout: LayoutRects,

    pub pending_tool_name: Option<String>,
    pub pending_tool_input: String,
    pub current_tool_calls: Vec<ToolCallDisplay>,
    pub error_message: Option<String>,
    pub model_selector: ModelSelector,
    pub agent_selector: AgentSelector,
    pub command_palette: CommandPalette,
    pub thinking_selector: ThinkingSelector,
    pub session_selector: SessionSelector,
    pub streaming_started: Option<Instant>,

    pub thinking_expanded: bool,
    pub thinking_budget: u32,
    pub last_escape_time: Option<Instant>,
    pub follow_bottom: bool,
}

impl App {
    pub fn new(
        model_name: String,
        provider_name: String,
        agent_name: String,
        theme_name: &str,
    ) -> Self {
        Self {
            messages: Vec::new(),
            input: String::new(),
            cursor_pos: 0,
            scroll_offset: 0,
            max_scroll: 0,
            is_streaming: false,
            current_response: String::new(),
            current_thinking: String::new(),
            should_quit: false,
            mode: AppMode::Insert,
            usage: TokenUsage::default(),
            model_name,
            provider_name,
            agent_name,
            theme: Theme::from_config(theme_name),
            tick_count: 0,
            layout: LayoutRects::default(),
            pending_tool_name: None,
            pending_tool_input: String::new(),
            current_tool_calls: Vec::new(),
            error_message: None,
            model_selector: ModelSelector::new(),
            agent_selector: AgentSelector::new(),
            command_palette: CommandPalette::new(),
            thinking_selector: ThinkingSelector::new(),
            session_selector: SessionSelector::new(),
            streaming_started: None,
            thinking_expanded: false,
            thinking_budget: 0,
            last_escape_time: None,
            follow_bottom: true,
        }
    }

    pub fn streaming_elapsed_secs(&self) -> Option<f64> {
        self.streaming_started
            .map(|start| start.elapsed().as_secs_f64())
    }

    pub fn thinking_level(&self) -> ThinkingLevel {
        ThinkingLevel::from_budget(self.thinking_budget)
    }

    pub fn handle_agent_event(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::TextDelta(text) => {
                self.current_response.push_str(&text);
            }
            AgentEvent::ThinkingDelta(text) => {
                self.current_thinking.push_str(&text);
            }
            AgentEvent::TextComplete(text) => {
                if !text.is_empty() || !self.current_response.is_empty() {
                    let content = if self.current_response.is_empty() {
                        text
                    } else {
                        self.current_response.clone()
                    };
                    let thinking = if self.current_thinking.is_empty() {
                        None
                    } else {
                        Some(self.current_thinking.clone())
                    };
                    self.messages.push(ChatMessage {
                        role: "assistant".to_string(),
                        content,
                        tool_calls: std::mem::take(&mut self.current_tool_calls),
                        thinking,
                    });
                }
                self.current_response.clear();
                self.current_thinking.clear();
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
                let category = ToolCategory::from_name(&name);
                let detail = extract_tool_detail(&name, &input);
                self.current_tool_calls.push(ToolCallDisplay {
                    name: name.clone(),
                    input,
                    output: Some(output),
                    is_error,
                    category,
                    detail,
                });
                self.pending_tool_name = None;
            }
            AgentEvent::Done { usage } => {
                self.is_streaming = false;
                self.streaming_started = None;
                self.usage.input_tokens += usage.input_tokens;
                self.usage.output_tokens += usage.output_tokens;
            }
            AgentEvent::Error(msg) => {
                self.is_streaming = false;
                self.streaming_started = None;
                self.error_message = Some(msg);
            }
            AgentEvent::Compacting => {
                self.messages.push(ChatMessage {
                    role: "compact".to_string(),
                    content: "\u{26a1} compacting context\u{2026}".to_string(),
                    tool_calls: Vec::new(),
                    thinking: None,
                });
            }
            AgentEvent::Compacted { messages_removed } => {
                if let Some(last) = self.messages.last_mut() {
                    if last.role == "compact" {
                        last.content = format!(
                            "\u{26a1} compacted \u{2014} {} messages summarized",
                            messages_removed
                        );
                    }
                }
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
            thinking: None,
        });
        self.input.clear();
        self.cursor_pos = 0;
        self.is_streaming = true;
        self.streaming_started = Some(Instant::now());
        self.current_response.clear();
        self.current_thinking.clear();
        self.current_tool_calls.clear();
        self.error_message = None;
        self.scroll_to_bottom();
        Some(trimmed)
    }

    pub fn scroll_up(&mut self, n: u16) {
        self.follow_bottom = false;
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
    }

    pub fn scroll_down(&mut self, n: u16) {
        self.scroll_offset = (self.scroll_offset + n).min(self.max_scroll);
        if self.scroll_offset >= self.max_scroll {
            self.follow_bottom = true;
        }
    }

    pub fn scroll_to_top(&mut self) {
        self.follow_bottom = false;
        self.scroll_offset = 0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.follow_bottom = true;
        self.scroll_offset = self.max_scroll;
    }

    pub fn clear_conversation(&mut self) {
        self.messages.clear();
        self.current_response.clear();
        self.current_thinking.clear();
        self.current_tool_calls.clear();
        self.scroll_offset = 0;
        self.max_scroll = 0;
        self.follow_bottom = true;
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
