use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, bail};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};

use super::types::{JsonRpcMessage, JsonRpcNotification, JsonRpcRequest};

pub struct AcpTransport {
    child: Child,
    reader: BufReader<tokio::process::ChildStdout>,
    writer: ChildStdin,
    counter: AtomicU64,
    pub(super) buffered_notifications: VecDeque<JsonRpcNotification>,
    pub(super) buffered_requests: VecDeque<JsonRpcRequest>,
}

impl AcpTransport {
    pub fn spawn(command: &str, args: &[String], env: &[(String, String)]) -> Result<Self> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        for (k, v) in env {
            cmd.env(k, v);
        }
        let mut child = cmd
            .spawn()
            .with_context(|| format!("spawning agent: {command}"))?;
        let stdout = child.stdout.take().context("child stdout missing")?;
        let stderr = child.stderr.take().context("child stderr missing")?;
        let writer = child.stdin.take().context("child stdin missing")?;
        let reader = BufReader::new(stdout);

        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::debug!(target: "acp::stderr", "{line}");
            }
        });

        Ok(Self {
            child,
            reader,
            writer,
            counter: AtomicU64::new(1),
            buffered_notifications: VecDeque::new(),
            buffered_requests: VecDeque::new(),
        })
    }

    async fn write_line(&mut self, line: &str) -> Result<()> {
        self.writer
            .write_all(line.as_bytes())
            .await
            .context("writing to agent stdin")?;
        self.writer
            .write_all(b"\n")
            .await
            .context("writing newline to agent stdin")?;
        self.writer.flush().await.context("flushing agent stdin")
    }

    pub fn next_id(&self) -> u64 {
        self.counter.fetch_add(1, Ordering::SeqCst)
    }

    pub async fn write_request(
        &mut self,
        id: u64,
        method: &str,
        params: serde_json::Value,
    ) -> Result<()> {
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let line = serde_json::to_string(&req).context("serializing request")?;
        self.write_line(&line).await
    }

    pub async fn send_request(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let id = self.counter.fetch_add(1, Ordering::SeqCst);
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let line = serde_json::to_string(&req).context("serializing request")?;
        self.write_line(&line).await?;

        loop {
            let msg = self.read_message().await?;
            match msg {
                JsonRpcMessage::Response(resp) => {
                    if resp.id == id {
                        if let Some(err) = resp.error {
                            bail!("JSON-RPC error {}: {}", err.code, err.message);
                        }
                        return resp.result.context("response missing result");
                    }
                    tracing::warn!("unexpected response id {}, expected {id}", resp.id);
                }
                JsonRpcMessage::Notification(n) => {
                    self.buffered_notifications.push_back(n);
                }
                JsonRpcMessage::Request(r) => {
                    self.buffered_requests.push_back(r);
                }
            }
        }
    }

    pub async fn send_notification(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<()> {
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        let line = serde_json::to_string(&msg).context("serializing notification")?;
        self.write_line(&line).await
    }

    pub async fn read_message(&mut self) -> Result<JsonRpcMessage> {
        let mut buf = String::new();
        let n = self
            .reader
            .read_line(&mut buf)
            .await
            .context("reading from agent stdout")?;
        if n == 0 {
            bail!("agent stdout closed");
        }
        serde_json::from_str(buf.trim()).context("parsing JSON-RPC message")
    }

    pub async fn send_response(&mut self, id: u64, result: serde_json::Value) -> Result<()> {
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result,
        });
        let line = serde_json::to_string(&msg).context("serializing response")?;
        self.write_line(&line).await
    }

    pub async fn send_error_response(&mut self, id: u64, code: i32, message: &str) -> Result<()> {
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": code, "message": message },
        });
        let line = serde_json::to_string(&msg).context("serializing error response")?;
        self.write_line(&line).await
    }

    pub fn drain_notifications(&mut self) -> Vec<JsonRpcNotification> {
        self.buffered_notifications.drain(..).collect()
    }

    pub fn drain_requests(&mut self) -> Vec<JsonRpcRequest> {
        self.buffered_requests.drain(..).collect()
    }

    pub fn kill(&mut self) -> Result<()> {
        self.child.start_kill().context("killing agent process")
    }
}
