#[derive(Debug, Clone, PartialEq)]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Clone)]
pub struct TodoItem {
    pub content: String,
    pub status: TodoStatus,
}

pub struct QuestionResponder(pub tokio::sync::oneshot::Sender<String>);

impl std::fmt::Debug for QuestionResponder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("QuestionResponder")
    }
}

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
    TitleGenerated(String),
    TodoUpdate(Vec<TodoItem>),
    Question {
        id: String,
        question: String,
        options: Vec<String>,
        responder: QuestionResponder,
    },
    PermissionRequest {
        tool_name: String,
        input_summary: String,
        responder: QuestionResponder,
    },
    SubagentStart {
        id: String,
        description: String,
        background: bool,
    },
    SubagentDelta {
        id: String,
        text: String,
    },
    SubagentToolStart {
        id: String,
        tool_name: String,
        detail: String,
    },
    SubagentToolComplete {
        id: String,
        tool_name: String,
    },
    SubagentComplete {
        id: String,
        output: String,
    },
    SubagentBackgroundDone {
        id: String,
        description: String,
        output: String,
    },
    MemoryExtracted {
        added: usize,
        updated: usize,
        deleted: usize,
    },
    AsideDelta(String),
    AsideDone,
    AsideError(String),
}

pub(super) struct PendingToolCall {
    pub id: String,
    pub name: String,
    pub input: String,
}
