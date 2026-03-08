use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::sync::mpsc;

use crate::agent::{Agent, AgentEvent, AgentProfile, TodoStatus};
use crate::command::CommandRegistry;
use crate::config::Config;
use crate::db::Db;
use crate::extension::HookRegistry;
use crate::memory::MemoryStore;
use crate::provider::Provider;
use crate::tools::ToolRegistry;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    Text,
    Json,
    StreamJson,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Self {
        match s {
            "json" => Self::Json,
            "stream-json" => Self::StreamJson,
            _ => Self::Text,
        }
    }
}

pub struct HeadlessOptions {
    pub prompt: String,
    pub format: OutputFormat,
    pub no_tools: bool,
    pub resume_id: Option<String>,
    pub interactive: bool,
    pub task_id: Option<String>,
}

struct TurnResult {
    text: String,
    tool_calls: Vec<serde_json::Value>,
    session_id: String,
    error: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub async fn run(
    config: Config,
    providers: Vec<Box<dyn Provider>>,
    db: Db,
    memory: Option<Arc<MemoryStore>>,
    tools: ToolRegistry,
    profiles: Vec<AgentProfile>,
    cwd: String,
    skill_names: Vec<(String, String)>,
    hooks: HookRegistry,
    commands: CommandRegistry,
    opts: HeadlessOptions,
) -> Result<()> {
    let _ = skill_names;
    let agents_context = crate::context::AgentsContext::load(&cwd, &config.context);
    let (bg_tx, bg_rx) = mpsc::unbounded_channel();
    let mut agent = Agent::new(
        providers,
        db,
        &config,
        memory,
        tools,
        profiles,
        cwd,
        agents_context,
        hooks,
        commands,
    )?;
    agent.set_background_tx(bg_tx);

    if let Some(ref id) = opts.resume_id {
        let conv = agent
            .get_session(id)
            .with_context(|| format!("resuming session {id}"))?;
        agent.resume_conversation(&conv)?;
    }

    // Emit session info at start for programmatic consumers
    let session_id = agent.conversation_id().to_string();
    if opts.format == OutputFormat::StreamJson {
        let obj = serde_json::json!({
            "type": "session_start",
            "session_id": session_id,
        });
        println!("{obj}");
    }

    // Single-turn: send the prompt and exit
    if !opts.interactive {
        let result = run_turn(&mut agent, &opts.prompt, &opts, bg_rx).await?;
        let success = result.error.is_none();
        emit_turn_end(&result, &opts);

        if let Some(ref task_id) = opts.task_id {
            let db = crate::db::Db::open().ok();
            if let Some(db) = db {
                let status = if success { "completed" } else { "failed" };
                let output = if let Some(ref e) = result.error {
                    e.clone()
                } else {
                    result.text.clone()
                };
                let _ = db.complete_task(task_id, status, Some(&result.session_id), &output);
            }
        }

        if !success {
            std::process::exit(1);
        }
        return Ok(());
    }

    // Multi-turn interactive mode
    // First turn uses the provided prompt (if non-empty)
    let mut bg_rx = bg_rx;
    if !opts.prompt.is_empty() {
        let (result, new_bg_rx) = run_turn_multi(&mut agent, &opts.prompt, &opts, bg_rx).await?;
        bg_rx = new_bg_rx;
        emit_turn_end(&result, &opts);
    }

    // Read subsequent prompts from stdin line by line
    let stdin = tokio::io::stdin();
    let reader = tokio::io::BufReader::new(stdin);
    use tokio::io::AsyncBufReadExt;
    let mut lines = reader.lines();

    loop {
        // Signal readiness for next prompt
        if opts.format == OutputFormat::StreamJson {
            let obj = serde_json::json!({"type": "ready"});
            println!("{obj}");
        } else if opts.format == OutputFormat::Text {
            eprint!("> ");
        }

        let line = match lines.next_line().await {
            Ok(Some(line)) => line,
            Ok(None) => break, // EOF
            Err(e) => {
                eprintln!("[error] reading stdin: {e}");
                break;
            }
        };

        let prompt = line.trim().to_string();
        if prompt.is_empty() {
            continue;
        }
        if prompt == "/quit" || prompt == "/exit" {
            break;
        }

        let (result, new_bg_rx) = run_turn_multi(&mut agent, &prompt, &opts, bg_rx).await?;
        bg_rx = new_bg_rx;
        emit_turn_end(&result, &opts);
    }

    // Emit session end
    let session_id = agent.conversation_id().to_string();
    let title = agent.conversation_title();
    if opts.format == OutputFormat::StreamJson {
        let obj = serde_json::json!({
            "type": "session_end",
            "session_id": session_id,
            "title": title,
        });
        println!("{obj}");
    } else if opts.format == OutputFormat::Text
        && let Some(ref t) = title
    {
        eprintln!("\n[session] {t} ({session_id})");
    }

    agent.cleanup_if_empty();
    Ok(())
}

/// Run a single turn for single-shot mode (consumes bg_rx).
async fn run_turn(
    agent: &mut Agent,
    prompt: &str,
    opts: &HeadlessOptions,
    mut bg_rx: mpsc::UnboundedReceiver<AgentEvent>,
) -> Result<TurnResult> {
    let session_id = agent.conversation_id().to_string();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let future = agent.send_message(prompt, tx);

    let mut text = String::new();
    let mut tool_calls: Vec<serde_json::Value> = Vec::new();
    let mut error: Option<String> = None;

    tokio::pin!(future);

    loop {
        tokio::select! {
            biased;
            result = &mut future => {
                if let Err(e) = result {
                    error = Some(e.to_string());
                }
                // Drain remaining
                while let Ok(ev) = rx.try_recv() {
                    handle_event(&ev, opts, &mut text, &mut tool_calls);
                }
                while let Ok(ev) = bg_rx.try_recv() {
                    handle_event(&ev, opts, &mut text, &mut tool_calls);
                }
                break;
            }
            Some(ev) = rx.recv() => {
                handle_event(&ev, opts, &mut text, &mut tool_calls);
            }
            Some(ev) = bg_rx.recv() => {
                handle_event(&ev, opts, &mut text, &mut tool_calls);
            }
        }
    }

    Ok(TurnResult {
        text,
        tool_calls,
        session_id,
        error,
    })
}

/// Run a single turn for multi-turn mode (returns bg_rx back for reuse).
async fn run_turn_multi(
    agent: &mut Agent,
    prompt: &str,
    opts: &HeadlessOptions,
    mut bg_rx: mpsc::UnboundedReceiver<AgentEvent>,
) -> Result<(TurnResult, mpsc::UnboundedReceiver<AgentEvent>)> {
    let session_id = agent.conversation_id().to_string();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let future = agent.send_message(prompt, tx);

    let mut text = String::new();
    let mut tool_calls: Vec<serde_json::Value> = Vec::new();
    let mut error: Option<String> = None;

    tokio::pin!(future);

    loop {
        tokio::select! {
            biased;
            result = &mut future => {
                if let Err(e) = result {
                    error = Some(e.to_string());
                }
                while let Ok(ev) = rx.try_recv() {
                    handle_event(&ev, opts, &mut text, &mut tool_calls);
                }
                while let Ok(ev) = bg_rx.try_recv() {
                    handle_event(&ev, opts, &mut text, &mut tool_calls);
                }
                break;
            }
            Some(ev) = rx.recv() => {
                handle_event(&ev, opts, &mut text, &mut tool_calls);
            }
            Some(ev) = bg_rx.recv() => {
                handle_event(&ev, opts, &mut text, &mut tool_calls);
            }
        }
    }

    let result = TurnResult {
        text,
        tool_calls,
        session_id,
        error,
    };
    Ok((result, bg_rx))
}

fn emit_turn_end(result: &TurnResult, opts: &HeadlessOptions) {
    let success = result.error.is_none();
    if opts.format == OutputFormat::Json {
        let mut output = serde_json::json!({
            "success": success,
            "session_id": result.session_id,
            "text": result.text,
            "tool_calls": result.tool_calls,
        });
        if let Some(ref e) = result.error {
            output["error"] = serde_json::json!(e);
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&output).unwrap_or_default()
        );
    } else if opts.format == OutputFormat::StreamJson {
        let mut obj = serde_json::json!({
            "type": "turn_complete",
            "success": success,
            "session_id": result.session_id,
            "text": result.text,
        });
        if let Some(ref e) = result.error {
            obj["error"] = serde_json::json!(e);
        }
        println!("{obj}");
    } else if let Some(ref e) = result.error {
        eprintln!("[error] {e}");
    }
}

fn handle_event(
    ev: &AgentEvent,
    opts: &HeadlessOptions,
    final_text: &mut String,
    tool_outputs: &mut Vec<serde_json::Value>,
) {
    match ev {
        AgentEvent::TextDelta(text) => {
            if opts.format == OutputFormat::Text {
                eprint!("{text}");
            } else if opts.format == OutputFormat::StreamJson {
                let obj = serde_json::json!({"type": "text_delta", "text": text});
                println!("{obj}");
            }
        }
        AgentEvent::TextComplete(text) => {
            *final_text = text.clone();
            if opts.format == OutputFormat::Text {
                eprintln!();
                println!("{text}");
            } else if opts.format == OutputFormat::StreamJson {
                let obj = serde_json::json!({"type": "text_complete", "text": text});
                println!("{obj}");
            }
        }
        AgentEvent::ThinkingDelta(text) => {
            if opts.format == OutputFormat::StreamJson {
                let obj = serde_json::json!({"type": "thinking_delta", "text": text});
                println!("{obj}");
            }
        }
        AgentEvent::ToolCallStart { id, name } => {
            if !opts.no_tools {
                if opts.format == OutputFormat::Text {
                    eprintln!("[tool] {name} ({id})");
                } else if opts.format == OutputFormat::StreamJson {
                    let obj = serde_json::json!({"type": "tool_start", "id": id, "name": name});
                    println!("{obj}");
                }
            }
        }
        AgentEvent::ToolCallExecuting { id, name, input } => {
            if !opts.no_tools && opts.format == OutputFormat::StreamJson {
                let obj = serde_json::json!({"type": "tool_executing", "id": id, "name": name, "input": input});
                println!("{obj}");
            }
        }
        AgentEvent::ToolCallResult {
            id,
            name,
            output,
            is_error,
        } => {
            if !opts.no_tools {
                if opts.format == OutputFormat::Text {
                    let prefix = if *is_error { "[error]" } else { "[result]" };
                    eprintln!("{prefix} {name}: {}", truncate(output, 500));
                } else if opts.format == OutputFormat::StreamJson {
                    let obj = serde_json::json!({
                        "type": "tool_result",
                        "id": id,
                        "name": name,
                        "output": output,
                        "is_error": is_error,
                    });
                    println!("{obj}");
                }
            }
            tool_outputs.push(serde_json::json!({
                "id": id,
                "name": name,
                "output": output,
                "is_error": is_error,
            }));
        }
        AgentEvent::Question {
            id,
            question,
            options,
            responder: _,
        } => {
            // In headless mode, questions get auto-answered.
            // The responder is consumed by the agent loop — we emit the event for observability.
            if opts.format == OutputFormat::Text {
                eprintln!("[question] {question}");
                if !options.is_empty() {
                    for (i, opt) in options.iter().enumerate() {
                        eprintln!("  {}: {opt}", i + 1);
                    }
                }
            } else if opts.format == OutputFormat::StreamJson {
                let obj = serde_json::json!({
                    "type": "question",
                    "id": id,
                    "question": question,
                    "options": options,
                });
                println!("{obj}");
            }
        }
        AgentEvent::PermissionRequest {
            tool_name,
            input_summary,
            responder: _,
        } => {
            if opts.format == OutputFormat::Text {
                eprintln!("[permission] {tool_name}: {input_summary}");
            } else if opts.format == OutputFormat::StreamJson {
                let obj = serde_json::json!({
                    "type": "permission_request",
                    "tool_name": tool_name,
                    "input_summary": input_summary,
                });
                println!("{obj}");
            }
        }
        AgentEvent::TodoUpdate(items) => {
            if opts.format == OutputFormat::StreamJson {
                let todos: Vec<serde_json::Value> = items
                    .iter()
                    .map(|t| {
                        serde_json::json!({
                            "content": t.content,
                            "status": match t.status {
                                TodoStatus::Pending => "pending",
                                TodoStatus::InProgress => "in_progress",
                                TodoStatus::Completed => "completed",
                            }
                        })
                    })
                    .collect();
                let obj = serde_json::json!({"type": "todo_update", "todos": todos});
                println!("{obj}");
            } else if opts.format == OutputFormat::Text {
                eprintln!("[todos]");
                for t in items {
                    let icon = match t.status {
                        TodoStatus::Pending => "○",
                        TodoStatus::InProgress => "◑",
                        TodoStatus::Completed => "●",
                    };
                    eprintln!("  {icon} {}", t.content);
                }
            }
        }
        AgentEvent::Done { usage } => {
            if opts.format == OutputFormat::StreamJson {
                let obj = serde_json::json!({
                    "type": "done",
                    "usage": {
                        "input_tokens": usage.input_tokens,
                        "output_tokens": usage.output_tokens,
                        "cache_read_tokens": usage.cache_read_tokens,
                        "cache_write_tokens": usage.cache_write_tokens,
                    }
                });
                println!("{obj}");
            }
        }
        AgentEvent::Error(msg) => {
            if opts.format == OutputFormat::Text {
                eprintln!("[error] {msg}");
            } else if opts.format == OutputFormat::StreamJson {
                let obj = serde_json::json!({"type": "error", "message": msg});
                println!("{obj}");
            }
        }
        AgentEvent::Compacting => {
            if opts.format == OutputFormat::StreamJson {
                let obj = serde_json::json!({"type": "compacting"});
                println!("{obj}");
            } else if opts.format == OutputFormat::Text {
                eprintln!("[compacting conversation...]");
            }
        }
        AgentEvent::Compacted { messages_removed } => {
            if opts.format == OutputFormat::StreamJson {
                let obj =
                    serde_json::json!({"type": "compacted", "messages_removed": messages_removed});
                println!("{obj}");
            }
        }
        AgentEvent::SubagentStart {
            id,
            description,
            background,
        } => {
            if opts.format == OutputFormat::StreamJson {
                let obj = serde_json::json!({"type": "subagent_start", "id": id, "description": description, "background": background});
                println!("{obj}");
            } else if opts.format == OutputFormat::Text {
                eprintln!("[subagent] {description} ({id})");
            }
        }
        AgentEvent::SubagentDelta { id, text } => {
            if opts.format == OutputFormat::StreamJson {
                let obj = serde_json::json!({"type": "subagent_delta", "id": id, "text": text});
                println!("{obj}");
            }
        }
        AgentEvent::SubagentToolStart {
            id,
            tool_name,
            detail,
        } => {
            if opts.format == OutputFormat::StreamJson {
                let obj = serde_json::json!({"type": "subagent_tool_start", "id": id, "tool_name": tool_name, "detail": detail});
                println!("{obj}");
            }
        }
        AgentEvent::SubagentToolComplete { id, tool_name } => {
            if opts.format == OutputFormat::StreamJson {
                let obj = serde_json::json!({"type": "subagent_tool_complete", "id": id, "tool_name": tool_name});
                println!("{obj}");
            }
        }
        AgentEvent::SubagentComplete { id, output } => {
            if opts.format == OutputFormat::StreamJson {
                let obj =
                    serde_json::json!({"type": "subagent_complete", "id": id, "output": output});
                println!("{obj}");
            }
        }
        AgentEvent::SubagentBackgroundDone {
            id,
            description,
            output,
        } => {
            if opts.format == OutputFormat::StreamJson {
                let obj = serde_json::json!({"type": "subagent_background_done", "id": id, "description": description, "output": output});
                println!("{obj}");
            } else if opts.format == OutputFormat::Text {
                eprintln!("[subagent done] {description}");
            }
        }
        AgentEvent::TitleGenerated(title) => {
            if opts.format == OutputFormat::StreamJson {
                let obj = serde_json::json!({"type": "title_generated", "title": title});
                println!("{obj}");
            }
        }
        AgentEvent::MemoryExtracted {
            added,
            updated,
            deleted,
        } => {
            if opts.format == OutputFormat::StreamJson {
                let obj = serde_json::json!({"type": "memory_extracted", "added": added, "updated": updated, "deleted": deleted});
                println!("{obj}");
            }
        }
        AgentEvent::ToolCallInputDelta(_) => {
            // Not useful in headless — tool input is streamed to the model, not the user
        }
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}
