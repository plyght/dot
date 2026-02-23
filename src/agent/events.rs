use crate::provider::Usage;
#[derive(Debug)]
pub enum AgentEvent {
    TextDelta(String),
    ThinkingDelta(String),
    TextComplete(String),
    ToolCallStart {
        id: String,
        name: String,
    },
    ToolCallInputDelta(String),
    ToolCallExecuting {
        id: String,
        name: String,
        input: String,
    },
    ToolCallResult {
        id: String,
        name: String,
        output: String,
        is_error: bool,
    },
    Done {
        usage: Usage,
    },
    Error(String),
    Compacting,
    Compacted {
        messages_removed: usize,
    },
}

pub(super) struct PendingToolCall {
    pub id: String,
    pub name: String,
    pub input: String,
}
