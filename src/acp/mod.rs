mod transport;
pub mod types;

use transport::AcpTransport;
pub use types::*;

use anyhow::{Context, Result};

pub enum AcpMessage {
    Notification(SessionNotification),
    IncomingRequest {
        id: u64,
        method: String,
        params: serde_json::Value,
    },
    PromptComplete(PromptResponse),
    Response {
        id: u64,
        result: std::result::Result<serde_json::Value, JsonRpcError>,
    },
}

pub struct AcpClient {
    transport: AcpTransport,
    session_id: Option<SessionId>,
    agent_info: Option<Implementation>,
    agent_capabilities: Option<AgentCapabilities>,
    modes: Option<SessionModeState>,
    config_options: Option<Vec<SessionConfigOption>>,
}

impl AcpClient {
    pub fn start(command: &str, args: &[String], env: &[(String, String)]) -> Result<Self> {
        Ok(Self {
            transport: AcpTransport::spawn(command, args, env)?,
            session_id: None,
            agent_info: None,
            agent_capabilities: None,
            modes: None,
            config_options: None,
        })
    }

    pub async fn initialize(&mut self) -> Result<InitializeResponse> {
        let params = serde_json::to_value(InitializeRequest {
            protocol_version: 1,
            client_capabilities: ClientCapabilities {
                fs: FsCapabilities {
                    read_text_file: true,
                    write_text_file: true,
                },
                terminal: true,
            },
            client_info: Some(Implementation {
                name: "dot".into(),
                title: Some("dot".into()),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
        })
        .context("serializing initialize request")?;

        let raw = self.transport.send_request("initialize", params).await?;
        let resp: InitializeResponse =
            serde_json::from_value(raw).context("parsing initialize response")?;
        self.agent_info = resp.agent_info.clone();
        self.agent_capabilities = Some(resp.agent_capabilities.clone());
        if let Some(ref info) = resp.agent_info {
            tracing::info!(agent = %info.name, version = ?info.version, "ACP initialized");
        }
        Ok(resp)
    }

    pub async fn authenticate(&mut self, method_id: &str) -> Result<AuthenticateResponse> {
        let params = serde_json::to_value(AuthenticateRequest {
            method_id: method_id.into(),
        })
        .context("serializing authenticate request")?;
        let raw = self.transport.send_request("authenticate", params).await?;
        serde_json::from_value(raw).context("parsing authenticate response")
    }

    pub async fn new_session(
        &mut self,
        cwd: &str,
        mcp_servers: Vec<McpServer>,
    ) -> Result<NewSessionResponse> {
        let params = serde_json::to_value(NewSessionRequest {
            cwd: cwd.into(),
            mcp_servers,
        })
        .context("serializing session/new request")?;
        let raw = self.transport.send_request("session/new", params).await?;
        let resp: NewSessionResponse =
            serde_json::from_value(raw).context("parsing session/new response")?;
        self.session_id = Some(resp.session_id.clone());
        self.modes = resp.modes.clone();
        self.config_options = resp.config_options.clone();
        tracing::info!(session_id = %resp.session_id, "ACP session created");
        Ok(resp)
    }

    pub async fn load_session(
        &mut self,
        session_id: &str,
        cwd: &str,
        mcp_servers: Vec<McpServer>,
    ) -> Result<LoadSessionResponse> {
        let params = serde_json::to_value(LoadSessionRequest {
            session_id: session_id.into(),
            cwd: cwd.into(),
            mcp_servers,
        })
        .context("serializing session/load request")?;
        let raw = self.transport.send_request("session/load", params).await?;
        let resp: LoadSessionResponse =
            serde_json::from_value(raw).context("parsing session/load response")?;
        self.session_id = Some(session_id.into());
        self.modes = resp.modes.clone();
        self.config_options = resp.config_options.clone();
        Ok(resp)
    }

    pub async fn send_prompt(&mut self, text: &str) -> Result<()> {
        let sid = self
            .session_id
            .as_deref()
            .context("no active session")?
            .to_string();
        let params = serde_json::to_value(PromptRequest {
            session_id: sid,
            prompt: vec![ContentBlock::Text { text: text.into() }],
        })
        .context("serializing session/prompt request")?;
        let id = self.transport.next_id();
        self.transport
            .write_request(id, "session/prompt", params)
            .await
    }

    pub async fn send_prompt_with_content(&mut self, content: Vec<ContentBlock>) -> Result<()> {
        let sid = self
            .session_id
            .as_deref()
            .context("no active session")?
            .to_string();
        let params = serde_json::to_value(PromptRequest {
            session_id: sid,
            prompt: content,
        })
        .context("serializing session/prompt request")?;
        let id = self.transport.next_id();
        self.transport
            .write_request(id, "session/prompt", params)
            .await
    }

    pub async fn read_next(&mut self) -> Result<AcpMessage> {
        if let Some(n) = self.transport.buffered_notifications.pop_front()
            && let Ok(sn) = serde_json::from_value::<SessionNotification>(n.params.clone())
        {
            return Ok(AcpMessage::Notification(sn));
        }
        if let Some(r) = self.transport.buffered_requests.pop_front() {
            return Ok(AcpMessage::IncomingRequest {
                id: r.id,
                method: r.method,
                params: r.params,
            });
        }
        loop {
            let msg = self.transport.read_message().await?;
            match msg {
                JsonRpcMessage::Notification(n) => {
                    if let Ok(sn) = serde_json::from_value::<SessionNotification>(n.params.clone())
                    {
                        return Ok(AcpMessage::Notification(sn));
                    }
                }
                JsonRpcMessage::Request(r) => {
                    return Ok(AcpMessage::IncomingRequest {
                        id: r.id,
                        method: r.method,
                        params: r.params,
                    });
                }
                JsonRpcMessage::Response(resp) => {
                    if let Some(err) = resp.error {
                        return Ok(AcpMessage::Response {
                            id: resp.id,
                            result: Err(err),
                        });
                    }
                    let result = resp.result.unwrap_or(serde_json::Value::Null);
                    if let Ok(pr) = serde_json::from_value::<PromptResponse>(result.clone()) {
                        return Ok(AcpMessage::PromptComplete(pr));
                    }
                    return Ok(AcpMessage::Response {
                        id: resp.id,
                        result: Ok(result),
                    });
                }
            }
        }
    }

    pub async fn cancel(&mut self) -> Result<()> {
        let sid = self
            .session_id
            .as_deref()
            .context("no active session")?
            .to_string();
        let params = serde_json::to_value(CancelNotification { session_id: sid })
            .context("serializing cancel")?;
        self.transport
            .send_notification("session/cancel", params)
            .await
    }

    pub async fn set_mode(&mut self, mode_id: &str) -> Result<SetSessionModeResponse> {
        let sid = self
            .session_id
            .as_deref()
            .context("no active session")?
            .to_string();
        let params = serde_json::to_value(SetSessionModeRequest {
            session_id: sid,
            mode_id: mode_id.into(),
        })
        .context("serializing set_mode request")?;
        let raw = self
            .transport
            .send_request("session/set_mode", params)
            .await?;
        serde_json::from_value(raw).context("parsing set_mode response")
    }

    pub fn drain_notifications(&mut self) -> Vec<SessionNotification> {
        self.transport
            .drain_notifications()
            .into_iter()
            .filter_map(|n| serde_json::from_value::<SessionNotification>(n.params).ok())
            .collect()
    }

    pub fn drain_incoming_requests(&mut self) -> Vec<JsonRpcRequest> {
        self.transport.drain_requests()
    }

    pub async fn respond(&mut self, id: u64, result: serde_json::Value) -> Result<()> {
        self.transport.send_response(id, result).await
    }

    pub async fn respond_error(&mut self, id: u64, code: i32, message: &str) -> Result<()> {
        self.transport.send_error_response(id, code, message).await
    }

    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    pub fn agent_info(&self) -> Option<&Implementation> {
        self.agent_info.as_ref()
    }

    pub fn current_mode(&self) -> Option<&str> {
        self.modes.as_ref().map(|m| m.current_mode_id.as_str())
    }

    pub fn available_modes(&self) -> &[SessionMode] {
        self.modes
            .as_ref()
            .map(|m| m.available_modes.as_slice())
            .unwrap_or(&[])
    }

    pub fn set_current_mode(&mut self, mode_id: &str) {
        if let Some(ref mut modes) = self.modes {
            modes.current_mode_id = mode_id.to_string();
        }
    }

    pub fn config_options(&self) -> &[SessionConfigOption] {
        self.config_options.as_deref().unwrap_or(&[])
    }

    pub fn set_config_options(&mut self, options: Vec<SessionConfigOption>) {
        self.config_options = Some(options);
    }

    pub fn kill(&mut self) -> Result<()> {
        self.transport.kill()
    }
}
