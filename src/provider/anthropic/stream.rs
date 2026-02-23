use std::collections::HashMap;

use anyhow::Context;
use futures::StreamExt;
use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::provider::{StreamEvent, StreamEventType, Usage};

use super::types::stop_reason_from_str;

struct ThinkingAccum {
    thinking: String,
    signature: String,
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

pub(super) async fn process_sse_stream(
    response: reqwest::Response,
    tx: mpsc::UnboundedSender<StreamEvent>,
) -> anyhow::Result<()> {
    let mut byte_stream = response.bytes_stream();
    let mut buffer = String::new();

    let mut block_types: HashMap<usize, String> = HashMap::new();
    let mut thinking_accums: HashMap<usize, ThinkingAccum> = HashMap::new();
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
                            } else if block_type == "thinking" {
                                thinking_accums.insert(
                                    index,
                                    ThinkingAccum {
                                        thinking: String::new(),
                                        signature: String::new(),
                                    },
                                );
                            }

                            block_types.insert(index, block_type);
                        }

                        "content_block_delta" => {
                            let index = json["index"].as_u64().unwrap_or(0) as usize;
                            let delta = &json["delta"];
                            let delta_type = delta["type"].as_str().unwrap_or("");

                            match delta_type {
                                "text_delta" => {
                                    let text = delta["text"].as_str().unwrap_or("").to_string();
                                    if !text.is_empty() {
                                        send!(StreamEventType::TextDelta(text));
                                    }
                                }
                                "input_json_delta" => {
                                    let partial =
                                        delta["partial_json"].as_str().unwrap_or("").to_string();
                                    if !partial.is_empty() {
                                        send!(StreamEventType::ToolUseInputDelta(partial));
                                    }
                                }
                                "thinking_delta" => {
                                    let text = delta["thinking"].as_str().unwrap_or("").to_string();
                                    if !text.is_empty() {
                                        if let Some(accum) = thinking_accums.get_mut(&index) {
                                            accum.thinking.push_str(&text);
                                        }
                                        send!(StreamEventType::ThinkingDelta(text));
                                    }
                                }
                                "signature_delta" => {
                                    let sig = delta["signature"].as_str().unwrap_or("").to_string();
                                    if let Some(accum) = thinking_accums.get_mut(&index) {
                                        accum.signature = sig;
                                    }
                                }
                                other => {
                                    debug!("Unknown delta type: {other}");
                                }
                            }
                        }

                        "content_block_stop" => {
                            let index = json["index"].as_u64().unwrap_or(0) as usize;
                            let btype = block_types.get(&index).map(|s| s.as_str()).unwrap_or("");
                            if btype == "tool_use" {
                                send!(StreamEventType::ToolUseEnd);
                            } else if btype == "thinking" {
                                if let Some(accum) = thinking_accums.remove(&index) {
                                    send!(StreamEventType::ThinkingComplete {
                                        thinking: accum.thinking,
                                        signature: accum.signature,
                                    });
                                }
                            }
                            block_types.remove(&index);
                        }

                        "message_delta" => {
                            let stop_reason_str =
                                json["delta"]["stop_reason"].as_str().unwrap_or("end_turn");
                            let stop_reason = stop_reason_from_str(stop_reason_str);
                            let output_tokens =
                                json["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32;
                            let cache_creation = json["usage"]["cache_creation_input_tokens"]
                                .as_u64()
                                .unwrap_or(0)
                                as u32;
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
