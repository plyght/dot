use std::{collections::HashMap, future::Future, pin::Pin, time::{SystemTime, UNIX_EPOCH}};

use anyhow::Context;
use futures::StreamExt;
use serde::Serialize;
use tokio::sync::{mpsc, mpsc::UnboundedReceiver};
use tracing::{debug, warn};

use crate::provider::{
    ContentBlock, Message, Provider, Role, StopReason, StreamEvent, StreamEventType,
    ToolDefinition, Usage,
};

pub enum AnthropicAuth {
    ApiKey(String),
    OAuth {
        access_token: String,
        refresh_token: String,
        expires_at: i64,
    },
}

pub struct AnthropicProvider {
    client: reqwest::Client,
    model: String,
    auth: tokio::sync::Mutex<AnthropicAuth>,
    cached_models: tokio::sync::Mutex<Option<Vec<String>>>,
}

impl AnthropicProvider {
    pub fn new_with_api_key(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("dot/0.1.0")
                .build()
                .expect("Failed to build reqwest client"),
            model: model.into(),
            auth: tokio::sync::Mutex::new(AnthropicAuth::ApiKey(api_key.into())),
            cached_models: tokio::sync::Mutex::new(None),
        }
    }

    pub fn new_with_oauth(
        access_token: impl Into<String>,
        refresh_token: impl Into<String>,
        expires_at: i64,
        model: impl Into<String>,
    ) -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("claude-code/2.1.49 (external, cli)")
                .build()
                .expect("Failed to build reqwest client"),
            model: model.into(),
            auth: tokio::sync::Mutex::new(AnthropicAuth::OAuth {
                access_token: access_token.into(),
                refresh_token: refresh_token.into(),
                expires_at,
            }),
            cached_models: tokio::sync::Mutex::new(None),
        }
    }

    async fn resolve_auth(&self) -> anyhow::Result<AuthResolved> {
        let mut auth = self.auth.lock().await;
        match &*auth {
            AnthropicAuth::ApiKey(key) => Ok(AuthResolved {
                header_name: "x-api-key".to_string(),
                header_value: key.clone(),
                is_oauth: false,
            }),
            AnthropicAuth::OAuth { access_token, refresh_token, expires_at } => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;

                let token = if now >= *expires_at - 60 {
                    let rt = refresh_token.clone();
                    match refresh_oauth_token(&self.client, &rt).await {
                        Ok((new_token, new_expires_at)) => {
                            if let AnthropicAuth::OAuth { access_token, expires_at, .. } = &mut *auth {
                                *access_token = new_token.clone();
                                *expires_at = new_expires_at;
                            }
                            new_token
                        }
                        Err(e) => {
                            warn!("OAuth token refresh failed: {e}");
                            access_token.clone()
                        }
                    }
                } else {
                    access_token.clone()
                };

                Ok(AuthResolved {
                    header_name: "Authorization".to_string(),
                    header_value: format!("Bearer {token}"),
                    is_oauth: true,
                })
            }
        }
    }
}

struct AuthResolved {
    header_name: String,
    header_value: String,
    is_oauth: bool,
}

async fn refresh_oauth_token(
    client: &reqwest::Client,
    refresh_token: &str,
) -> anyhow::Result<(String, i64)> {
    let resp = client
        .post("https://console.anthropic.com/v1/oauth/token")
        .json(&serde_json::json!({
            "grant_type": "refresh_token",
            "refresh_token": refresh_token,
            "client_id": "9d1c250a-e61b-44d9-88ed-5944d1962f5e",
        }))
        .send()
        .await
        .context("Failed to send OAuth refresh request")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("OAuth refresh failed {status}: {body}"));
    }

    let data: serde_json::Value = resp.json().await.context("Failed to parse OAuth refresh response")?;
    let access_token = data["access_token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No access_token in refresh response"))?
        .to_string();
    let expires_in = data["expires_in"].as_i64().unwrap_or(3600);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let expires_at = now + expires_in;

    Ok((access_token, expires_at))
}

#[derive(Serialize)]
struct AnthropicRequest<'a> {
    model: &'a str,
    messages: Vec<serde_json::Value>,
    max_tokens: u32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<serde_json::Value>,
    temperature: f64,
}

fn convert_content_block(block: &ContentBlock) -> serde_json::Value {
    match block {
        ContentBlock::Text(text) => serde_json::json!({
            "type": "text",
            "text": text,
        }),
        ContentBlock::ToolUse { id, name, input } => serde_json::json!({
            "type": "tool_use",
            "id": id,
            "name": name,
            "input": input,
        }),
        ContentBlock::ToolResult { tool_use_id, content, is_error } => serde_json::json!({
            "type": "tool_result",
            "tool_use_id": tool_use_id,
            "content": content,
            "is_error": is_error,
        }),
    }
}

fn convert_messages(messages: &[Message]) -> Vec<serde_json::Value> {
    messages
        .iter()
        .filter(|m| m.role != Role::System)
        .map(|m| {
            let role = match m.role {
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::System => "user",
            };
            let content: Vec<serde_json::Value> =
                m.content.iter().map(convert_content_block).collect();
            serde_json::json!({ "role": role, "content": content })
        })
        .collect()
}

fn convert_tools(tools: &[ToolDefinition]) -> Vec<serde_json::Value> {
    tools
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.input_schema,
            })
        })
        .collect()
}

fn parse_sse_block(block: &str) -> Option<(String, String)> {
    let mut event_type = String::new();
    let mut data_lines: Vec<&str> = Vec::new();

    for line in block.lines() {
        if let Some(rest) = line.strip_prefix("event:") {
            event_type = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(rest.trim());
        }
    }

    let data = data_lines.join("\n");
    if data.is_empty() {
        return None;
    }

    Some((event_type, data))
}

fn stop_reason_from_str(s: &str) -> StopReason {
    match s {
        "end_turn" => StopReason::EndTurn,
        "max_tokens" => StopReason::MaxTokens,
        "tool_use" => StopReason::ToolUse,
        "stop_sequence" => StopReason::StopSequence,
        _ => StopReason::EndTurn,
    }
}

async fn process_sse_stream(
    response: reqwest::Response,
    tx: mpsc::UnboundedSender<StreamEvent>,
) -> anyhow::Result<()> {
    let mut byte_stream = response.bytes_stream();
    let mut buffer = String::new();

    let mut block_types: HashMap<usize, String> = HashMap::new();
    let mut accumulated_input_tokens: u32 = 0;

    macro_rules! send {
        ($event:expr) => {
            let _ = tx.send(StreamEvent { event_type: $event });
        };
    }

    while let Some(chunk_result) = byte_stream.next().await {
        let chunk = chunk_result.context("Error reading Anthropic stream chunk")?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        loop {
            match buffer.find("\n\n") {
                None => break,
                Some(pos) => {
                    let block = buffer[..pos].to_string();
                    buffer = buffer[pos + 2..].to_string();

                    if block.trim().is_empty() {
                        continue;
                    }

                    let (event_type, data) = match parse_sse_block(&block) {
                        Some(pair) => pair,
                        None => continue,
                    };

                    let json: serde_json::Value = match serde_json::from_str(&data) {
                        Ok(v) => v,
                        Err(e) => {
                            debug!("Failed to parse SSE data JSON: {e} | data: {data}");
                            continue;
                        }
                    };

                    let json_type = json["type"].as_str().unwrap_or(&event_type);

                    match json_type {
                        "message_start" => {
                            let usage = &json["message"]["usage"];
                            accumulated_input_tokens =
                                usage["input_tokens"].as_u64().unwrap_or(0) as u32;
                            send!(StreamEventType::MessageStart);
                        }

                        "content_block_start" => {
                            let index = json["index"].as_u64().unwrap_or(0) as usize;
                            let block_type = json["content_block"]["type"]
                                .as_str()
                                .unwrap_or("text")
                                .to_string();

                            if block_type == "tool_use" {
                                let id = json["content_block"]["id"]
                                    .as_str()
                                    .unwrap_or("")
                                    .to_string();
                                let name = json["content_block"]["name"]
                                    .as_str()
                                    .unwrap_or("")
                                    .to_string();
                                debug!("Tool use start: id={id} name={name}");
                                send!(StreamEventType::ToolUseStart { id, name });
                            }

                            block_types.insert(index, block_type);
                        }

                        "content_block_delta" => {
                            let delta = &json["delta"];
                            let delta_type = delta["type"].as_str().unwrap_or("");

                            match delta_type {
                                "text_delta" => {
                                    let text =
                                        delta["text"].as_str().unwrap_or("").to_string();
                                    if !text.is_empty() {
                                        send!(StreamEventType::TextDelta(text));
                                    }
                                }
                                "input_json_delta" => {
                                    let partial = delta["partial_json"]
                                        .as_str()
                                        .unwrap_or("")
                                        .to_string();
                                    if !partial.is_empty() {
                                        send!(StreamEventType::ToolUseInputDelta(partial));
                                    }
                                }
                                other => {
                                    debug!("Unknown delta type: {other}");
                                }
                            }
                        }

                        "content_block_stop" => {
                            let index = json["index"].as_u64().unwrap_or(0) as usize;
                            if block_types
                                .get(&index)
                                .map(|t| t == "tool_use")
                                .unwrap_or(false)
                            {
                                send!(StreamEventType::ToolUseEnd);
                            }
                            block_types.remove(&index);
                        }

                        "message_delta" => {
                            let stop_reason_str = json["delta"]["stop_reason"]
                                .as_str()
                                .unwrap_or("end_turn");
                            let stop_reason = stop_reason_from_str(stop_reason_str);
                            let output_tokens =
                                json["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32;
                            let cache_creation = json["usage"]
                                ["cache_creation_input_tokens"]
                                .as_u64()
                                .unwrap_or(0) as u32;
                            let cache_read = json["usage"]["cache_read_input_tokens"]
                                .as_u64()
                                .unwrap_or(0) as u32;

                            send!(StreamEventType::MessageEnd {
                                stop_reason,
                                usage: Usage {
                                    input_tokens: accumulated_input_tokens,
                                    output_tokens,
                                    cache_read_tokens: cache_read,
                                    cache_write_tokens: cache_creation,
                                },
                            });
                        }

                        "message_stop" => {}

                        "ping" => {
                            debug!("Received SSE ping");
                        }

                        "error" => {
                            let msg = json["error"]["message"]
                                .as_str()
                                .unwrap_or("Unknown Anthropic error")
                                .to_string();
                            warn!("Anthropic SSE error: {msg}");
                            send!(StreamEventType::Error(msg));
                        }

                        other => {
                            debug!("Unknown SSE event type: {other}");
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

impl Provider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn model(&self) -> &str {
        &self.model
    }


    fn set_model(&mut self, model: String) {
        self.model = model;
    }

    fn available_models(&self) -> Vec<String> {
        let cache = self.cached_models.blocking_lock();
        cache.clone().unwrap_or_else(|| vec![
            "claude-sonnet-4-20250514".to_string(),
            "claude-opus-4-20250514".to_string(),
            "claude-haiku-4-20250414".to_string(),
        ])
    }

    fn fetch_models(
        &self,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<String>>> + Send + '_>> {
        Box::pin(async move {
            {
                let cache = self.cached_models.lock().await;
                if let Some(ref models) = *cache {
                    return Ok(models.clone());
                }
            }

            let auth = self.resolve_auth().await?;

            let mut req = self
                .client
                .get("https://api.anthropic.com/v1/models")
                .header(&auth.header_name, &auth.header_value)
                .header("anthropic-version", "2023-06-01");

            if auth.is_oauth {
                req = req
                    .header(
                        "anthropic-beta",
                        "oauth-2025-04-20,interleaved-thinking-2025-05-14",
                    )
                    .header("user-agent", "claude-code/2.1.49 (external, cli)");
            }

            let resp = req.send().await.context("Failed to fetch Anthropic models")?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                warn!("Anthropic models API error {status}: {body}");
                return Ok(self.available_models());
            }

            let data: serde_json::Value = resp
                .json()
                .await
                .context("Failed to parse Anthropic models response")?;

            let mut models: Vec<String> = data["data"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| m["id"].as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            if models.is_empty() {
                return Ok(self.available_models());
            }

            models.sort();

            let mut cache = self.cached_models.lock().await;
            *cache = Some(models.clone());

            Ok(models)
        })
    }

    fn stream(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        max_tokens: u32,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<UnboundedReceiver<StreamEvent>>> + Send + '_>>
    {
        let messages = messages.to_vec();
        let system = system.map(String::from);
        let tools = tools.to_vec();

        Box::pin(async move {
            let auth = self.resolve_auth().await?;

            let url = if auth.is_oauth {
                "https://api.anthropic.com/v1/messages?beta=true".to_string()
            } else {
                "https://api.anthropic.com/v1/messages".to_string()
            };

            let body = AnthropicRequest {
                model: &self.model,
                messages: convert_messages(&messages),
                max_tokens,
                stream: true,
                system: system.as_deref(),
                tools: convert_tools(&tools),
                temperature: 1.0,
            };

            let mut req_builder = self
                .client
                .post(&url)
                .header(&auth.header_name, &auth.header_value)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json");

            if auth.is_oauth {
                req_builder = req_builder
                    .header(
                        "anthropic-beta",
                        "oauth-2025-04-20,interleaved-thinking-2025-05-14",
                    )
                    .header("user-agent", "claude-code/2.1.49 (external, cli)");
            }

            let response = req_builder
                .json(&body)
                .send()
                .await
                .context("Failed to connect to Anthropic API")?;

            if !response.status().is_success() {
                let status = response.status();
                let body_text = response.text().await.unwrap_or_default();
                return Err(anyhow::anyhow!("Anthropic API error {status}: {body_text}"));
            }

            let (tx, rx) = mpsc::unbounded_channel::<StreamEvent>();
            let tx_clone = tx.clone();

            tokio::spawn(async move {
                if let Err(e) = process_sse_stream(response, tx_clone.clone()).await {
                    let _ = tx_clone.send(StreamEvent {
                        event_type: StreamEventType::Error(e.to_string()),
                    });
                }
            });

            Ok(rx)
        })
    }
}
