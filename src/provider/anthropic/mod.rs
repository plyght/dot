mod auth;
mod stream;
mod types;

use auth::{AnthropicAuth, AuthResolved, refresh_oauth_token};
use stream::process_sse_stream;
use types::{AnthropicRequest, convert_messages, convert_tools};

use std::{
    future::Future,
    pin::Pin,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use tokio::sync::{mpsc, mpsc::UnboundedReceiver};
use tracing::warn;

use crate::provider::{Message, Provider, StreamEvent, StreamEventType, ToolDefinition};

pub struct AnthropicProvider {
    client: reqwest::Client,
    model: String,
    auth: tokio::sync::Mutex<AnthropicAuth>,
    cached_models: std::sync::Mutex<Option<Vec<String>>>,
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
            cached_models: std::sync::Mutex::new(None),
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
            cached_models: std::sync::Mutex::new(None),
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
            AnthropicAuth::OAuth {
                access_token,
                refresh_token,
                expires_at,
            } => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                // Handle legacy millis-format expires_at from older credentials
                let expires_at_secs = if *expires_at > 1_000_000_000_000 {
                    *expires_at / 1000
                } else {
                    *expires_at
                };

                let token = if now >= expires_at_secs - 60 {
                    let rt = refresh_token.clone();
                    match refresh_oauth_token(&self.client, &rt).await {
                        Ok((new_token, new_expires_at)) => {
                            if let AnthropicAuth::OAuth {
                                access_token,
                                expires_at,
                                ..
                            } = &mut *auth
                            {
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
        let cache = self.cached_models.lock().unwrap();
        cache.clone().unwrap_or_default()
    }

    fn fetch_models(
        &self,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<String>>> + Send + '_>> {
        Box::pin(async move {
            {
                let cache = self.cached_models.lock().unwrap();
                if let Some(ref models) = *cache {
                    return Ok(models.clone());
                }
            }
            let auth = self.resolve_auth().await?;
            let mut all_models: Vec<String> = Vec::new();
            let mut after_id: Option<String> = None;

            loop {
                let mut url = "https://api.anthropic.com/v1/models?limit=1000".to_string();
                if let Some(ref cursor) = after_id {
                    url.push_str(&format!("&after_id={cursor}"));
                }

                let mut req = self
                    .client
                    .get(&url)
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

                let resp = req
                    .send()
                    .await
                    .context("Failed to fetch Anthropic models")?;

                if !resp.status().is_success() {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    return Err(anyhow::anyhow!(
                        "Anthropic models API error {status}: {body}"
                    ));
                }

                let data: serde_json::Value = resp
                    .json()
                    .await
                    .context("Failed to parse Anthropic models response")?;

                if let Some(arr) = data["data"].as_array() {
                    for m in arr {
                        if let Some(id) = m["id"].as_str() {
                            all_models.push(id.to_string());
                        }
                    }
                }

                let has_more = data["has_more"].as_bool().unwrap_or(false);
                if !has_more {
                    break;
                }

                match data["last_id"].as_str() {
                    Some(last) => after_id = Some(last.to_string()),
                    None => break,
                }
            }

            if all_models.is_empty() {
                return Err(anyhow::anyhow!("Anthropic models API returned empty list"));
            }

            all_models.sort();
            let mut cache = self.cached_models.lock().unwrap();
            *cache = Some(all_models.clone());

            Ok(all_models)
        })
    }

    fn stream(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        max_tokens: u32,
        thinking_budget: u32,
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

            let thinking = if thinking_budget >= 1024 {
                Some(serde_json::json!({
                    "type": "enabled",
                    "budget_tokens": thinking_budget,
                }))
            } else {
                None
            };

            let effective_max_tokens = if thinking_budget >= 1024 {
                max_tokens.max(thinking_budget.saturating_add(4096))
            } else {
                max_tokens
            };

            let body = AnthropicRequest {
                model: &self.model,
                messages: convert_messages(&messages),
                max_tokens: effective_max_tokens,
                stream: true,
                system: system.as_deref(),
                tools: convert_tools(&tools),
                temperature: 1.0,
                thinking,
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
            } else if thinking_budget >= 1024 {
                req_builder =
                    req_builder.header("anthropic-beta", "interleaved-thinking-2025-05-14");
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
