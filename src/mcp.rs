use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use crate::tools::Tool;

const PROTOCOL_VERSION: &str = "2024-11-05";

#[derive(Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    id: u64,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

#[derive(Serialize)]
struct JsonRpcNotification {
    jsonrpc: &'static str,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

#[derive(Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<u64>,
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

#[derive(Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[allow(dead_code)]
    data: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpToolDef {
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

#[derive(Debug, Deserialize)]
struct ToolsListResult {
    tools: Vec<McpToolDef>,
}

#[derive(Debug, Deserialize)]
struct ToolCallContent {
    #[allow(dead_code)]
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ToolCallResult {
    content: Vec<ToolCallContent>,
    #[serde(rename = "isError", default)]
    is_error: bool,
}

struct ClientInner {
    stdin: BufWriter<std::process::ChildStdin>,
    stdout: BufReader<std::process::ChildStdout>,
    next_id: u64,
}

pub struct McpClient {
    server_name: String,
    inner: Mutex<ClientInner>,
    _child: Mutex<Child>,
}

impl McpClient {
    pub fn start(
        server_name: &str,
        command: &[String],
        env: &HashMap<String, String>,
    ) -> Result<Self> {
        if command.is_empty() {
            bail!("MCP server '{}' has empty command", server_name);
        }

        let mut cmd = Command::new(&command[0]);
        if command.len() > 1 {
            cmd.args(&command[1..]);
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        for (k, v) in env {
            cmd.env(k, v);
        }

        let mut child = cmd
            .spawn()
            .with_context(|| format!("Failed to start MCP server '{}'", server_name))?;

        let stdin = child.stdin.take().context("Failed to get stdin")?;
        let stdout = child.stdout.take().context("Failed to get stdout")?;

        Ok(McpClient {
            server_name: server_name.to_string(),
            inner: Mutex::new(ClientInner {
                stdin: BufWriter::new(stdin),
                stdout: BufReader::new(stdout),
                next_id: 1,
            }),
            _child: Mutex::new(child),
        })
    }

    fn send_request(&self, method: &str, params: Option<Value>) -> Result<Value> {
        let mut inner = self.inner.lock().map_err(|e| anyhow::anyhow!("{}", e))?;

        let id = inner.next_id;
        inner.next_id += 1;

        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method: method.to_string(),
            params,
        };

        let msg = serde_json::to_string(&request)?;
        writeln!(inner.stdin, "{}", msg)?;
        inner.stdin.flush()?;

        loop {
            let mut line = String::new();
            let bytes_read = inner.stdout.read_line(&mut line)?;
            if bytes_read == 0 {
                bail!(
                    "MCP server '{}' closed connection unexpectedly",
                    self.server_name
                );
            }
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let response: JsonRpcResponse = match serde_json::from_str(line) {
                Ok(r) => r,
                Err(_) => continue,
            };

            if response.id == Some(id) {
                if let Some(error) = response.error {
                    bail!(
                        "MCP error from '{}': {} (code {})",
                        self.server_name,
                        error.message,
                        error.code
                    );
                }
                return response
                    .result
                    .ok_or_else(|| anyhow::anyhow!("Empty result from '{}'", self.server_name));
            }
        }
    }

    fn send_notification(&self, method: &str, params: Option<Value>) -> Result<()> {
        let mut inner = self.inner.lock().map_err(|e| anyhow::anyhow!("{}", e))?;

        let notification = JsonRpcNotification {
            jsonrpc: "2.0",
            method: method.to_string(),
            params,
        };

        let msg = serde_json::to_string(&notification)?;
        writeln!(inner.stdin, "{}", msg)?;
        inner.stdin.flush()?;
        Ok(())
    }

    pub fn initialize(&self) -> Result<()> {
        let params = serde_json::json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {
                "name": "dot",
                "version": "0.1.0"
            }
        });

        let _result = self.send_request("initialize", Some(params))?;
        self.send_notification("notifications/initialized", None)?;
        tracing::info!("MCP server '{}' initialized", self.server_name);
        Ok(())
    }

    pub fn list_tools(&self) -> Result<Vec<McpToolDef>> {
        let result = self.send_request("tools/list", Some(serde_json::json!({})))?;
        let tools_result: ToolsListResult = serde_json::from_value(result)?;
        Ok(tools_result.tools)
    }

    pub fn call_tool(&self, name: &str, arguments: Value) -> Result<String> {
        let params = serde_json::json!({
            "name": name,
            "arguments": arguments
        });

        let result = self.send_request("tools/call", Some(params))?;
        let call_result: ToolCallResult = serde_json::from_value(result)?;

        let text: Vec<String> = call_result
            .content
            .iter()
            .filter_map(|c| c.text.clone())
            .collect();
        let output = text.join("\n");

        if call_result.is_error {
            bail!("{}", output);
        }
        Ok(output)
    }

    pub fn server_name(&self) -> &str {
        &self.server_name
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        if let Ok(child) = self._child.get_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Wraps an MCP server tool as a native `Tool` implementation.
pub struct McpToolBridge {
    tool_name: String,
    prefixed_name: String,
    description: String,
    input_schema: Value,
    client: Arc<McpClient>,
}

impl McpToolBridge {
    pub fn new(client: Arc<McpClient>, server_name: &str, tool_def: &McpToolDef) -> Self {
        McpToolBridge {
            tool_name: tool_def.name.clone(),
            prefixed_name: format!("{}_{}", server_name, tool_def.name),
            description: tool_def
                .description
                .clone()
                .unwrap_or_else(|| format!("[{}] {}", server_name, tool_def.name)),
            input_schema: tool_def.input_schema.clone(),
            client,
        }
    }
}

impl Tool for McpToolBridge {
    fn name(&self) -> &str {
        &self.prefixed_name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> Value {
        self.input_schema.clone()
    }

    fn execute(&self, input: Value) -> Result<String> {
        tracing::debug!("MCP {}:{}", self.client.server_name(), self.tool_name);
        self.client.call_tool(&self.tool_name, input)
    }
}

/// Manages connections to all configured MCP servers.
pub struct McpManager {
    clients: Vec<Arc<McpClient>>,
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}

impl McpManager {
    pub fn new() -> Self {
        McpManager {
            clients: Vec::new(),
        }
    }

    pub fn start_server(
        &mut self,
        name: &str,
        command: &[String],
        env: &HashMap<String, String>,
    ) -> Result<()> {
        let client = McpClient::start(name, command, env)?;
        client.initialize()?;
        self.clients.push(Arc::new(client));
        Ok(())
    }

    pub fn discover_tools(&self) -> Vec<Box<dyn Tool>> {
        let mut tools: Vec<Box<dyn Tool>> = Vec::new();

        for client in &self.clients {
            match client.list_tools() {
                Ok(tool_defs) => {
                    tracing::info!("MCP '{}': {} tools", client.server_name(), tool_defs.len());
                    for td in &tool_defs {
                        tools.push(Box::new(McpToolBridge::new(
                            client.clone(),
                            client.server_name(),
                            td,
                        )));
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to list tools from '{}': {}",
                        client.server_name(),
                        e
                    );
                }
            }
        }

        tools
    }

    pub fn server_count(&self) -> usize {
        self.clients.len()
    }
}
