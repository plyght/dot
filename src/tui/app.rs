use std::collections::VecDeque;
use std::path::Path;
use std::time::Instant;

use ratatui::layout::Rect;

use crate::agent::{AgentEvent, QuestionResponder, TodoItem};
use crate::tui::theme::Theme;
use crate::tui::tools::{ToolCallDisplay, ToolCategory, extract_tool_detail};
use crate::tui::widgets::{
    AgentSelector, CommandPalette, HelpPopup, MessageContextMenu, ModelSelector, SessionSelector,
    ThinkingLevel, ThinkingSelector,
};

pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub tool_calls: Vec<ToolCallDisplay>,
    pub thinking: Option<String>,
    pub model: Option<String>,
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

#[derive(Debug, Clone)]
pub struct PasteBlock {
    pub start: usize,
    pub end: usize,
    pub line_count: usize,
}

#[derive(Debug, Clone)]
pub struct ImageAttachment {
    pub path: String,
    pub media_type: String,
    pub data: String,
}

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg"];

#[derive(Default)]
pub struct TextSelection {
    pub anchor: Option<(u16, u16)>,
    pub end: Option<(u16, u16)>,
    pub active: bool,
}

impl TextSelection {
    pub fn start(&mut self, col: u16, visual_row: u16) {
        self.anchor = Some((col, visual_row));
        self.end = Some((col, visual_row));
        self.active = true;
    }

    pub fn update(&mut self, col: u16, visual_row: u16) {
        self.end = Some((col, visual_row));
    }

    pub fn clear(&mut self) {
        self.anchor = None;
        self.end = None;
        self.active = false;
    }

    pub fn ordered(&self) -> Option<((u16, u16), (u16, u16))> {
        let a = self.anchor?;
        let e = self.end?;
        if a.1 < e.1 || (a.1 == e.1 && a.0 <= e.0) {
            Some((a, e))
        } else {
            Some((e, a))
        }
    }

    pub fn is_empty_selection(&self) -> bool {
        match (self.anchor, self.end) {
            (Some(a), Some(e)) => a == e,
            _ => true,
        }
    }
}

pub fn media_type_for_path(path: &str) -> Option<String> {
    let ext = Path::new(path).extension()?.to_str()?.to_lowercase();
    match ext.as_str() {
        "png" => Some("image/png".into()),
        "jpg" | "jpeg" => Some("image/jpeg".into()),
        "gif" => Some("image/gif".into()),
        "webp" => Some("image/webp".into()),
        "bmp" => Some("image/bmp".into()),
        "svg" => Some("image/svg+xml".into()),
        _ => None,
    }
}

pub fn is_image_path(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

pub const PASTE_COLLAPSE_THRESHOLD: usize = 5;

#[derive(Debug)]
pub struct PendingQuestion {
    pub question: String,
    pub options: Vec<String>,
    pub selected: usize,
    pub custom_input: String,
    pub responder: Option<QuestionResponder>,
}

#[derive(Debug)]
pub struct PendingPermission {
    pub tool_name: String,
    pub input_summary: String,
    pub selected: usize,
    pub responder: Option<QuestionResponder>,
}

pub struct QueuedMessage {
    pub text: String,
    pub images: Vec<(String, String)>,
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
    pub agent_selector: Option<Rect>,
    pub command_palette: Option<Rect>,
    pub thinking_selector: Option<Rect>,
    pub session_selector: Option<Rect>,
    pub help_popup: Option<Rect>,
    pub context_menu: Option<Rect>,
    pub question_popup: Option<Rect>,
    pub permission_popup: Option<Rect>,
}

pub struct App {
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub cursor_pos: usize,
    pub scroll_offset: u16,
    pub max_scroll: u16,
    pub scroll_position: f64,
    pub scroll_velocity: f64,
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
    pub help_popup: HelpPopup,
    pub streaming_started: Option<Instant>,

    pub thinking_expanded: bool,
    pub thinking_budget: u32,
    pub last_escape_time: Option<Instant>,
    pub follow_bottom: bool,

    pub paste_blocks: Vec<PasteBlock>,
    pub attachments: Vec<ImageAttachment>,
    pub conversation_title: Option<String>,
    pub vim_mode: bool,

    pub selection: TextSelection,
    pub visual_lines: Vec<String>,
    pub content_width: u16,

    pub context_window: u32,
    pub last_input_tokens: u32,

    pub esc_hint_until: Option<Instant>,
    pub todos: Vec<TodoItem>,
    pub message_line_map: Vec<usize>,
    pub context_menu: MessageContextMenu,
    pub pending_question: Option<PendingQuestion>,
    pub pending_permission: Option<PendingPermission>,
    pub message_queue: VecDeque<QueuedMessage>,
    pub history: Vec<String>,
    pub history_index: Option<usize>,
    pub history_draft: String,
    pub skill_entries: Vec<(String, String)>,
    pub custom_command_names: Vec<String>,
    pub rename_input: String,
    pub rename_visible: bool,
    pub favorite_models: Vec<String>,
}
impl App {
    pub fn new(
        model_name: String,
        provider_name: String,
        agent_name: String,
        theme_name: &str,
        vim_mode: bool,
        context_window: u32,
    ) -> Self {
        Self {
            messages: Vec::new(),
            input: String::new(),
            cursor_pos: 0,
            scroll_offset: 0,
            max_scroll: 0,
            scroll_position: 0.0,
            scroll_velocity: 0.0,
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
            help_popup: HelpPopup::new(),
            streaming_started: None,
            thinking_expanded: false,
            thinking_budget: 0,
            last_escape_time: None,
            follow_bottom: true,
            paste_blocks: Vec::new(),
            attachments: Vec::new(),
            conversation_title: None,
            vim_mode,
            selection: TextSelection::default(),
            visual_lines: Vec::new(),
            content_width: 0,
            context_window,
            last_input_tokens: 0,
            esc_hint_until: None,
            todos: Vec::new(),
            message_line_map: Vec::new(),
            context_menu: MessageContextMenu::new(),
            pending_question: None,
            pending_permission: None,
            message_queue: VecDeque::new(),
            history: Vec::new(),
            history_index: None,
            history_draft: String::new(),
            skill_entries: Vec::new(),
            custom_command_names: Vec::new(),
            rename_input: String::new(),
            rename_visible: false,
            favorite_models: Vec::new(),
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
                        model: Some(self.model_name.clone()),
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
                self.last_input_tokens = usage.input_tokens;
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
                    model: None,
                });
            }
            AgentEvent::TitleGenerated(title) => {
                self.conversation_title = Some(title);
            }
            AgentEvent::Compacted { messages_removed } => {
                if let Some(last) = self.messages.last_mut()
                    && last.role == "compact"
                {
                    last.content = format!(
                        "\u{26a1} compacted \u{2014} {} messages summarized",
                        messages_removed
                    );
                }
            }
            AgentEvent::TodoUpdate(items) => {
                self.todos = items;
            }
            AgentEvent::Question {
                question,
                options,
                responder,
                ..
            } => {
                self.pending_question = Some(PendingQuestion {
                    question,
                    options,
                    selected: 0,
                    custom_input: String::new(),
                    responder: Some(responder),
                });
            }
            AgentEvent::PermissionRequest {
                tool_name,
                input_summary,
                responder,
            } => {
                self.pending_permission = Some(PendingPermission {
                    tool_name,
                    input_summary,
                    selected: 0,
                    responder: Some(responder),
                });
            }
        }
    }

    pub fn take_input(&mut self) -> Option<String> {
        let trimmed = self.input.trim().to_string();
        if trimmed.is_empty() && self.attachments.is_empty() {
            return None;
        }
        let display = if self.attachments.is_empty() {
            trimmed.clone()
        } else {
            let att_names: Vec<String> = self
                .attachments
                .iter()
                .map(|a| {
                    Path::new(&a.path)
                        .file_name()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_else(|| a.path.clone())
                })
                .collect();
            if trimmed.is_empty() {
                format!("[{}]", att_names.join(", "))
            } else {
                format!("{} [{}]", trimmed, att_names.join(", "))
            }
        };
        self.messages.push(ChatMessage {
            role: "user".to_string(),
            content: display,
            tool_calls: Vec::new(),
            thinking: None,
            model: None,
        });
        self.input.clear();
        self.cursor_pos = 0;
        self.paste_blocks.clear();
        self.history.push(trimmed.clone());
        self.history_index = None;
        self.history_draft.clear();
        self.is_streaming = true;
        self.streaming_started = Some(Instant::now());
        self.current_response.clear();
        self.current_thinking.clear();
        self.current_tool_calls.clear();
        self.error_message = None;
        self.scroll_to_bottom();
        Some(trimmed)
    }

    pub fn take_attachments(&mut self) -> Vec<ImageAttachment> {
        std::mem::take(&mut self.attachments)
    }

    pub fn queue_input(&mut self) -> bool {
        let trimmed = self.input.trim().to_string();
        if trimmed.is_empty() && self.attachments.is_empty() {
            return false;
        }
        let display = if self.attachments.is_empty() {
            trimmed.clone()
        } else {
            let names: Vec<String> = self
                .attachments
                .iter()
                .map(|a| {
                    Path::new(&a.path)
                        .file_name()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_else(|| a.path.clone())
                })
                .collect();
            if trimmed.is_empty() {
                format!("[{}]", names.join(", "))
            } else {
                format!("{} [{}]", trimmed, names.join(", "))
            }
        };
        self.messages.push(ChatMessage {
            role: "user".to_string(),
            content: display,
            tool_calls: Vec::new(),
            thinking: None,
            model: None,
        });
        let images: Vec<(String, String)> = self
            .attachments
            .drain(..)
            .map(|a| (a.media_type, a.data))
            .collect();
        self.history.push(trimmed.clone());
        self.history_index = None;
        self.history_draft.clear();
        self.message_queue.push_back(QueuedMessage {
            text: trimmed,
            images,
        });
        self.input.clear();
        self.cursor_pos = 0;
        self.paste_blocks.clear();
        self.scroll_to_bottom();
        true
    }

    pub fn input_height(&self, width: u16) -> u16 {
        if self.is_streaming && self.input.is_empty() && self.attachments.is_empty() {
            return 3;
        }
        let w = width as usize;
        if w < 4 {
            return 3;
        }
        let has_input = !self.input.is_empty() || !self.attachments.is_empty();
        if !has_input {
            return 3;
        }
        let mut visual = 0usize;
        if !self.attachments.is_empty() {
            visual += 1;
        }
        let display = self.display_input();
        if display.is_empty() {
            if self.attachments.is_empty() {
                visual += 1;
            }
        } else {
            for line in display.split('\n') {
                let total = 2 + line.chars().count();
                visual += if total == 0 {
                    1
                } else {
                    total.div_ceil(w).max(1)
                };
            }
        }
        (visual as u16 + 1).clamp(3, 12)
    }

    pub fn handle_paste(&mut self, text: String) {
        let line_count = text.lines().count();
        if line_count >= PASTE_COLLAPSE_THRESHOLD {
            let start = self.cursor_pos;
            self.input.insert_str(self.cursor_pos, &text);
            let end = start + text.len();
            self.cursor_pos = end;
            self.paste_blocks.push(PasteBlock {
                start,
                end,
                line_count,
            });
        } else {
            self.input.insert_str(self.cursor_pos, &text);
            self.cursor_pos += text.len();
        }
    }

    pub fn paste_block_at_cursor(&self) -> Option<usize> {
        self.paste_blocks
            .iter()
            .position(|pb| self.cursor_pos > pb.start && self.cursor_pos <= pb.end)
    }

    pub fn delete_paste_block(&mut self, idx: usize) {
        let pb = self.paste_blocks.remove(idx);
        let len = pb.end - pb.start;
        self.input.replace_range(pb.start..pb.end, "");
        self.cursor_pos = pb.start;
        for remaining in &mut self.paste_blocks {
            if remaining.start >= pb.end {
                remaining.start -= len;
                remaining.end -= len;
            }
        }
    }

    pub fn add_image_attachment(&mut self, path: &str) -> Result<(), String> {
        let resolved = if path.starts_with('~') {
            if let Ok(home) = std::env::var("HOME") {
                path.replacen('~', &home, 1)
            } else {
                path.to_string()
            }
        } else {
            path.to_string()
        };

        let fs_path = Path::new(&resolved);
        if !fs_path.exists() {
            return Err(format!("file not found: {}", path));
        }

        let media_type = media_type_for_path(&resolved)
            .ok_or_else(|| format!("unsupported image format: {}", path))?;

        let data = std::fs::read(fs_path).map_err(|e| format!("failed to read {}: {}", path, e))?;
        let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &data);

        if self.attachments.iter().any(|a| a.path == resolved) {
            return Ok(());
        }

        self.attachments.push(ImageAttachment {
            path: resolved,
            media_type,
            data: encoded,
        });
        Ok(())
    }

    pub fn display_input(&self) -> String {
        if self.paste_blocks.is_empty() {
            return self.input.clone();
        }
        let mut result = String::new();
        let mut pos = 0;
        let mut sorted_blocks: Vec<&PasteBlock> = self.paste_blocks.iter().collect();
        sorted_blocks.sort_by_key(|pb| pb.start);
        for pb in sorted_blocks {
            if pb.start > pos {
                result.push_str(&self.input[pos..pb.start]);
            }
            result.push_str(&format!("[pasted {} lines]", pb.line_count));
            pos = pb.end;
        }
        if pos < self.input.len() {
            result.push_str(&self.input[pos..]);
        }
        result
    }

    pub fn scroll_up(&mut self, n: u16) {
        self.follow_bottom = false;
        self.scroll_velocity -= n as f64 * 0.25;
        self.scroll_velocity = self.scroll_velocity.clamp(-40.0, 40.0);
    }

    pub fn scroll_down(&mut self, n: u16) {
        self.scroll_velocity += n as f64 * 0.25;
        self.scroll_velocity = self.scroll_velocity.clamp(-40.0, 40.0);
    }

    pub fn scroll_to_top(&mut self) {
        self.follow_bottom = false;
        self.scroll_position = 0.0;
        self.scroll_velocity = 0.0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.follow_bottom = true;
        self.scroll_position = self.max_scroll as f64;
        self.scroll_velocity = 0.0;
    }

    pub fn scroll_frac(&self) -> f64 {
        self.scroll_position - self.scroll_position.floor()
    }

    pub fn animate_scroll(&mut self) {
        if self.scroll_velocity.abs() < 0.01 && self.scroll_position == self.scroll_position.round()
        {
            return;
        }

        self.scroll_position += self.scroll_velocity;
        self.scroll_velocity *= 0.78;

        if self.scroll_velocity.abs() < 0.08 {
            self.scroll_velocity = 0.0;
            self.scroll_position = self.scroll_position.round();
        }

        if self.scroll_position < 0.0 {
            self.scroll_position = 0.0;
            self.scroll_velocity = 0.0;
        }
        let max = self.max_scroll as f64;
        if self.scroll_position > max {
            self.scroll_position = max;
            self.scroll_velocity = 0.0;
            self.follow_bottom = true;
        }

        self.scroll_offset = self.scroll_position.round() as u16;
    }

    pub fn clear_conversation(&mut self) {
        self.messages.clear();
        self.current_response.clear();
        self.current_thinking.clear();
        self.current_tool_calls.clear();
        self.scroll_offset = 0;
        self.scroll_position = 0.0;
        self.scroll_velocity = 0.0;
        self.max_scroll = 0;
        self.follow_bottom = true;
        self.usage = TokenUsage::default();
        self.last_input_tokens = 0;
        self.error_message = None;
        self.paste_blocks.clear();
        self.attachments.clear();
        self.conversation_title = None;
        self.selection.clear();
        self.visual_lines.clear();
        self.todos.clear();
        self.message_line_map.clear();
        self.esc_hint_until = None;
        self.context_menu.close();
        self.pending_question = None;
        self.pending_permission = None;
        self.message_queue.clear();
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

    pub fn delete_word_before(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }
        let before = &self.input[..self.cursor_pos];
        let trimmed = before.trim_end();
        let new_end = if trimmed.is_empty() {
            0
        } else if let Some(pos) = trimmed.rfind(|c: char| c.is_whitespace()) {
            pos + trimmed[pos..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(1)
        } else {
            0
        };
        self.input.replace_range(new_end..self.cursor_pos, "");
        self.cursor_pos = new_end;
    }

    pub fn delete_to_end(&mut self) {
        self.input.truncate(self.cursor_pos);
    }

    pub fn delete_to_start(&mut self) {
        self.input.replace_range(..self.cursor_pos, "");
        self.cursor_pos = 0;
    }

    pub fn extract_selected_text(&self) -> Option<String> {
        let ((sc, sr), (ec, er)) = self.selection.ordered()?;
        if self.visual_lines.is_empty() || self.content_width == 0 {
            return None;
        }
        let mut text = String::new();
        for row in sr..=er {
            if row as usize >= self.visual_lines.len() {
                break;
            }
            let line = &self.visual_lines[row as usize];
            let chars: Vec<char> = line.chars().collect();
            let start_col = if row == sr {
                (sc as usize).min(chars.len())
            } else {
                0
            };
            let end_col = if row == er {
                (ec as usize).min(chars.len())
            } else {
                chars.len()
            };
            if start_col <= end_col {
                let s = start_col.min(chars.len());
                let e = end_col.min(chars.len());
                text.extend(&chars[s..e]);
            }
            if row < er {
                text.push('\n');
            }
        }
        Some(text)
    }

    pub fn move_cursor_up(&mut self) -> bool {
        let before = &self.input[..self.cursor_pos];
        let line_start = before.rfind('\n').map(|p| p + 1).unwrap_or(0);
        if line_start == 0 {
            return false;
        }
        let col = before[line_start..].chars().count();
        let prev_end = line_start - 1;
        let prev_start = self.input[..prev_end]
            .rfind('\n')
            .map(|p| p + 1)
            .unwrap_or(0);
        let prev_line = &self.input[prev_start..prev_end];
        let target_col = col.min(prev_line.chars().count());
        let offset: usize = prev_line
            .chars()
            .take(target_col)
            .map(|c| c.len_utf8())
            .sum();
        self.cursor_pos = prev_start + offset;
        true
    }

    pub fn move_cursor_down(&mut self) -> bool {
        let after = &self.input[self.cursor_pos..];
        let next_nl = after.find('\n');
        let Some(nl_offset) = next_nl else {
            return false;
        };
        let before = &self.input[..self.cursor_pos];
        let line_start = before.rfind('\n').map(|p| p + 1).unwrap_or(0);
        let col = before[line_start..].chars().count();
        let next_start = self.cursor_pos + nl_offset + 1;
        let next_end = self.input[next_start..]
            .find('\n')
            .map(|p| next_start + p)
            .unwrap_or(self.input.len());
        let next_line = &self.input[next_start..next_end];
        let target_col = col.min(next_line.chars().count());
        let offset: usize = next_line
            .chars()
            .take(target_col)
            .map(|c| c.len_utf8())
            .sum();
        self.cursor_pos = next_start + offset;
        true
    }

    pub fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        match self.history_index {
            None => {
                self.history_draft = self.input.clone();
                self.history_index = Some(self.history.len() - 1);
            }
            Some(0) => return,
            Some(i) => {
                self.history_index = Some(i - 1);
            }
        }
        self.input = self.history[self.history_index.unwrap()].clone();
        self.cursor_pos = self.input.len();
        self.paste_blocks.clear();
    }

    pub fn history_next(&mut self) {
        let Some(idx) = self.history_index else {
            return;
        };
        if idx + 1 >= self.history.len() {
            self.history_index = None;
            self.input = std::mem::take(&mut self.history_draft);
        } else {
            self.history_index = Some(idx + 1);
            self.input = self.history[idx + 1].clone();
        }
        self.cursor_pos = self.input.len();
        self.paste_blocks.clear();
    }
}

pub fn copy_to_clipboard(text: &str) {
    let encoded =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, text.as_bytes());
    let osc = format!("\x1b]52;c;{}\x07", encoded);
    let _ = std::io::Write::write_all(&mut std::io::stderr(), osc.as_bytes());

    #[cfg(target_os = "macos")]
    {
        use std::process::{Command, Stdio};
        if let Ok(mut child) = Command::new("pbcopy").stdin(Stdio::piped()).spawn() {
            if let Some(ref mut stdin) = child.stdin {
                let _ = std::io::Write::write_all(stdin, text.as_bytes());
            }
            let _ = child.wait();
        }
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::{Command, Stdio};
        let result = Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(Stdio::piped())
            .spawn();
        if let Ok(mut child) = result {
            if let Some(ref mut stdin) = child.stdin {
                let _ = std::io::Write::write_all(stdin, text.as_bytes());
            }
            let _ = child.wait();
        }
    }
}
