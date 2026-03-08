pub(crate) mod schema;

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection};
use std::path::PathBuf;
use uuid::Uuid;

fn db_path() -> Result<PathBuf> {
    let dot_dir = crate::config::Config::data_dir();
    std::fs::create_dir_all(&dot_dir).context("Could not create dot data directory")?;
    Ok(dot_dir.join("dot.db"))
}

#[derive(Debug, Clone)]
pub struct ConversationSummary {
    pub id: String,
    pub title: Option<String>,
    pub model: String,
    pub provider: String,
    pub cwd: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct Conversation {
    pub id: String,
    pub title: Option<String>,
    pub model: String,
    pub provider: String,
    pub cwd: String,
    pub created_at: String,
    pub updated_at: String,
    pub messages: Vec<DbMessage>,
    pub last_input_tokens: u32,
}

#[derive(Debug, Clone)]
pub struct DbMessage {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub token_count: u32,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct DbToolCall {
    pub id: String,
    pub message_id: String,
    pub name: String,
    pub input: String,
    pub output: Option<String>,
    pub is_error: bool,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct TaskRecord {
    pub id: String,
    pub prompt: String,
    pub status: String,
    pub session_id: Option<String>,
    pub pid: Option<i64>,
    pub output: Option<String>,
    pub cwd: String,
    pub created_at: String,
    pub completed_at: Option<String>,
}

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open() -> Result<Self> {
        let path = db_path()?;
        tracing::debug!("Opening database at {:?}", path);
        let conn = Connection::open(&path)
            .with_context(|| format!("Failed to open database at {:?}", path))?;
        let db = Db { conn };
        db.init()?;
        Ok(db)
    }

    pub fn init(&self) -> Result<()> {
        self.conn
            .execute_batch(&format!(
                "{}\n;\n{}\n;\n{}\n;\n{}",
                schema::CREATE_CONVERSATIONS,
                schema::CREATE_MESSAGES,
                schema::CREATE_TOOL_CALLS,
                schema::CREATE_TASKS,
            ))
            .context("Failed to initialize database schema")?;

        let _ = self.conn.execute(
            "ALTER TABLE conversations ADD COLUMN cwd TEXT NOT NULL DEFAULT ''",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE conversations ADD COLUMN last_input_tokens INTEGER NOT NULL DEFAULT 0",
            [],
        );

        tracing::debug!("Database schema initialized");
        Ok(())
    }

    pub fn create_conversation(&self, model: &str, provider: &str, cwd: &str) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "INSERT INTO conversations (id, title, model, provider, cwd, created_at, updated_at) \
                 VALUES (?1, NULL, ?2, ?3, ?4, ?5, ?6)",
                params![id, model, provider, cwd, now, now],
            )
            .context("Failed to create conversation")?;
        tracing::debug!("Created conversation {}", id);
        Ok(id)
    }

    pub fn list_conversations(&self, limit: usize) -> Result<Vec<ConversationSummary>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, title, model, provider, cwd, created_at, updated_at \
                 FROM conversations ORDER BY updated_at DESC LIMIT ?1",
            )
            .context("Failed to prepare list_conversations query")?;

        let rows = stmt
            .query_map(params![limit as i64], |row| {
                Ok(ConversationSummary {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    model: row.get(2)?,
                    provider: row.get(3)?,
                    cwd: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })
            .context("Failed to list conversations")?;

        let mut conversations = Vec::new();
        for row in rows {
            conversations.push(row.context("Failed to read conversation row")?);
        }
        Ok(conversations)
    }

    pub fn list_conversations_for_cwd(
        &self,
        cwd: &str,
        limit: usize,
    ) -> Result<Vec<ConversationSummary>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, title, model, provider, cwd, created_at, updated_at \
                 FROM conversations WHERE cwd = ?1 ORDER BY updated_at DESC LIMIT ?2",
            )
            .context("Failed to prepare list_conversations_for_cwd query")?;

        let rows = stmt
            .query_map(params![cwd, limit as i64], |row| {
                Ok(ConversationSummary {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    model: row.get(2)?,
                    provider: row.get(3)?,
                    cwd: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })
            .context("Failed to list conversations for cwd")?;

        let mut conversations = Vec::new();
        for row in rows {
            conversations.push(row.context("Failed to read conversation row")?);
        }
        Ok(conversations)
    }

    pub fn get_conversation(&self, id: &str) -> Result<Conversation> {
        let (summary, last_input_tokens) = self
            .conn
            .query_row(
                "SELECT id, title, model, provider, cwd, created_at, updated_at, last_input_tokens \
                 FROM conversations WHERE id = ?1",
                params![id],
                |row| {
                    Ok((
                        ConversationSummary {
                            id: row.get(0)?,
                            title: row.get(1)?,
                            model: row.get(2)?,
                            provider: row.get(3)?,
                            cwd: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                            created_at: row.get(5)?,
                            updated_at: row.get(6)?,
                        },
                        row.get::<_, i64>(7).unwrap_or(0) as u32,
                    ))
                },
            )
            .context("Failed to get conversation")?;

        let messages = self.get_messages(id)?;
        Ok(Conversation {
            id: summary.id,
            title: summary.title,
            model: summary.model,
            provider: summary.provider,
            cwd: summary.cwd,
            created_at: summary.created_at,
            updated_at: summary.updated_at,
            messages,
            last_input_tokens,
        })
    }

    pub fn update_conversation_title(&self, id: &str, title: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
                params![title, now, id],
            )
            .context("Failed to update conversation title")?;
        Ok(())
    }

    pub fn delete_conversation(&self, id: &str) -> Result<()> {
        self.conn
            .execute(
                "DELETE FROM tool_calls WHERE message_id IN \
                 (SELECT id FROM messages WHERE conversation_id = ?1)",
                params![id],
            )
            .context("Failed to delete tool calls for conversation")?;

        self.conn
            .execute(
                "DELETE FROM messages WHERE conversation_id = ?1",
                params![id],
            )
            .context("Failed to delete messages for conversation")?;

        self.conn
            .execute("DELETE FROM conversations WHERE id = ?1", params![id])
            .context("Failed to delete conversation")?;

        tracing::debug!("Deleted conversation {}", id);
        Ok(())
    }

    pub fn truncate_messages(&self, conversation_id: &str, keep: usize) -> Result<()> {
        let ids: Vec<String> = {
            let mut stmt = self
                .conn
                .prepare(
                    "SELECT id FROM messages WHERE conversation_id = ?1 ORDER BY created_at ASC",
                )
                .context("Failed to prepare truncate query")?;
            let rows = stmt
                .query_map(params![conversation_id], |row| row.get::<_, String>(0))
                .context("Failed to query messages for truncation")?;
            let mut all = Vec::new();
            for row in rows {
                all.push(row.context("Failed to read message id")?);
            }
            all
        };
        let to_delete = &ids[keep.min(ids.len())..];
        for id in to_delete {
            self.conn
                .execute("DELETE FROM tool_calls WHERE message_id = ?1", params![id])
                .context("Failed to delete tool calls for truncated message")?;
            self.conn
                .execute("DELETE FROM messages WHERE id = ?1", params![id])
                .context("Failed to delete truncated message")?;
        }
        Ok(())
    }

    pub fn add_message(&self, conversation_id: &str, role: &str, content: &str) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "INSERT INTO messages \
                 (id, conversation_id, role, content, token_count, created_at) \
                 VALUES (?1, ?2, ?3, ?4, 0, ?5)",
                params![id, conversation_id, role, content, now],
            )
            .context("Failed to add message")?;

        self.conn
            .execute(
                "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
                params![now, conversation_id],
            )
            .context("Failed to update conversation timestamp")?;

        tracing::debug!("Added message {} to conversation {}", id, conversation_id);
        Ok(id)
    }

    pub fn get_messages(&self, conversation_id: &str) -> Result<Vec<DbMessage>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, conversation_id, role, content, token_count, created_at \
                 FROM messages WHERE conversation_id = ?1 ORDER BY created_at ASC",
            )
            .context("Failed to prepare get_messages query")?;

        let rows = stmt
            .query_map(params![conversation_id], |row| {
                Ok(DbMessage {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    token_count: row.get::<_, i64>(4)? as u32,
                    created_at: row.get(5)?,
                })
            })
            .context("Failed to get messages")?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(row.context("Failed to read message row")?);
        }
        Ok(messages)
    }

    pub fn update_last_input_tokens(&self, conversation_id: &str, tokens: u32) -> Result<()> {
        self.conn
            .execute(
                "UPDATE conversations SET last_input_tokens = ?1 WHERE id = ?2",
                params![tokens as i64, conversation_id],
            )
            .context("Failed to update last_input_tokens")?;
        Ok(())
    }

    pub fn update_message_tokens(&self, id: &str, tokens: u32) -> Result<()> {
        self.conn
            .execute(
                "UPDATE messages SET token_count = ?1 WHERE id = ?2",
                params![tokens as i64, id],
            )
            .context("Failed to update message tokens")?;
        Ok(())
    }

    pub fn add_tool_call(
        &self,
        message_id: &str,
        tool_id: &str,
        name: &str,
        input: &str,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "INSERT INTO tool_calls \
                 (id, message_id, name, input, output, is_error, created_at) \
                 VALUES (?1, ?2, ?3, ?4, NULL, 0, ?5)",
                params![tool_id, message_id, name, input, now],
            )
            .context("Failed to add tool call")?;
        tracing::debug!("Added tool call {} for message {}", tool_id, message_id);
        Ok(())
    }

    pub fn update_tool_result(&self, tool_id: &str, output: &str, is_error: bool) -> Result<()> {
        self.conn
            .execute(
                "UPDATE tool_calls SET output = ?1, is_error = ?2 WHERE id = ?3",
                params![output, is_error as i64, tool_id],
            )
            .context("Failed to update tool result")?;
        Ok(())
    }

    pub fn get_tool_calls(&self, message_id: &str) -> Result<Vec<DbToolCall>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, message_id, name, input, output, is_error, created_at \
                 FROM tool_calls WHERE message_id = ?1 ORDER BY created_at ASC",
            )
            .context("Failed to prepare get_tool_calls query")?;

        let rows = stmt
            .query_map(params![message_id], |row| {
                Ok(DbToolCall {
                    id: row.get(0)?,
                    message_id: row.get(1)?,
                    name: row.get(2)?,
                    input: row.get(3)?,
                    output: row.get(4)?,
                    is_error: row.get::<_, i64>(5)? != 0,
                    created_at: row.get(6)?,
                })
            })
            .context("Failed to get tool calls")?;

        let mut calls = Vec::new();
        for row in rows {
            calls.push(row.context("Failed to read tool call row")?);
        }
        Ok(calls)
    }

    pub fn get_user_message_history(&self, limit: usize) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT content FROM messages WHERE role = 'user' \
                 ORDER BY created_at DESC LIMIT ?1",
            )
            .context("Failed to prepare user history query")?;

        let rows = stmt
            .query_map(params![limit as i64], |row| row.get::<_, String>(0))
            .context("Failed to query user history")?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(row.context("Failed to read history row")?);
        }
        messages.reverse();
        Ok(messages)
    }

    pub fn create_task(&self, id: &str, prompt: &str, pid: u32, cwd: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "INSERT INTO tasks (id, prompt, status, pid, cwd, created_at) \
                 VALUES (?1, ?2, 'running', ?3, ?4, ?5)",
                params![id, prompt, pid as i64, cwd, now],
            )
            .context("creating task")?;
        Ok(())
    }

    pub fn complete_task(
        &self,
        id: &str,
        status: &str,
        session_id: Option<&str>,
        output: &str,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "UPDATE tasks SET status = ?1, session_id = ?2, output = ?3, completed_at = ?4 WHERE id = ?5",
                params![status, session_id, output, now, id],
            )
            .context("completing task")?;
        Ok(())
    }

    pub fn list_tasks(&self, limit: usize) -> Result<Vec<TaskRecord>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, prompt, status, session_id, pid, output, cwd, created_at, completed_at \
                 FROM tasks ORDER BY created_at DESC LIMIT ?1",
            )
            .context("preparing list_tasks")?;
        let rows = stmt
            .query_map(params![limit as i64], |row| {
                Ok(TaskRecord {
                    id: row.get(0)?,
                    prompt: row.get(1)?,
                    status: row.get(2)?,
                    session_id: row.get(3)?,
                    pid: row.get(4)?,
                    output: row.get(5)?,
                    cwd: row.get::<_, Option<String>>(6)?.unwrap_or_default(),
                    created_at: row.get(7)?,
                    completed_at: row.get(8)?,
                })
            })
            .context("listing tasks")?;
        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row.context("reading task row")?);
        }
        Ok(tasks)
    }

    pub fn get_task(&self, id: &str) -> Result<TaskRecord> {
        self.conn
            .query_row(
                "SELECT id, prompt, status, session_id, pid, output, cwd, created_at, completed_at \
                 FROM tasks WHERE id = ?1",
                params![id],
                |row| {
                    Ok(TaskRecord {
                        id: row.get(0)?,
                        prompt: row.get(1)?,
                        status: row.get(2)?,
                        session_id: row.get(3)?,
                        pid: row.get(4)?,
                        output: row.get(5)?,
                        cwd: row.get::<_, Option<String>>(6)?.unwrap_or_default(),
                        created_at: row.get(7)?,
                        completed_at: row.get(8)?,
                    })
                },
            )
            .context("getting task")
    }
}
