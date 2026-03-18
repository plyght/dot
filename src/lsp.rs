use std::collections::VecDeque;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

pub struct LspClient {
    process: Child,
    writer: BufWriter<ChildStdin>,
    rx: mpsc::Receiver<Value>,
    id: u32,
    pending: VecDeque<Value>,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub line: u32,
    pub col: u32,
    pub severity: &'static str,
    pub message: String,
    pub source: Option<String>,
}

impl LspClient {
    pub fn start(command: &[String]) -> Result<Self> {
        if command.is_empty() {
            bail!("LSP command is empty");
        }
        let mut process = Command::new(&command[0])
            .args(&command[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("starting LSP server '{}'", command[0]))?;
        let stdin = process.stdin.take().context("LSP stdin")?;
        let stdout = process.stdout.take().context("LSP stdout")?;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                match read_frame(&mut reader) {
                    Ok(msg) => {
                        if tx.send(msg).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            process,
            writer: BufWriter::new(stdin),
            rx,
            id: 0,
            pending: VecDeque::new(),
        })
    }

    pub fn initialize(&mut self, root_uri: &str) -> Result<()> {
        let id = self.next_id();
        self.send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "initialize",
            "params": {
                "processId": std::process::id(),
                "rootUri": root_uri,
                "capabilities": {
                    "textDocument": {
                        "hover": { "contentFormat": ["plaintext", "markdown"] },
                        "publishDiagnostics": { "relatedInformation": false },
                        "definition": { "linkSupport": false }
                    },
                    "workspace": {}
                }
            }
        }))?;
        self.wait_response(id, Duration::from_secs(30))?;
        self.notify("initialized", json!({}))
    }

    pub fn open(&mut self, uri: &str, text: &str, language_id: &str) -> Result<()> {
        self.notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": language_id,
                    "version": 1,
                    "text": text
                }
            }),
        )
    }

    pub fn hover(&mut self, uri: &str, line: u32, col: u32) -> Result<Option<String>> {
        let id = self.next_id();
        self.send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": col }
            }
        }))?;
        let result = self.wait_response(id, Duration::from_secs(10))?;
        if result.is_null() {
            return Ok(None);
        }
        let text = result["contents"]["value"]
            .as_str()
            .or_else(|| result["contents"].as_str())
            .map(str::to_string);
        Ok(text)
    }

    pub fn definition(&mut self, uri: &str, line: u32, col: u32) -> Result<Vec<String>> {
        let id = self.next_id();
        self.send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "textDocument/definition",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": col }
            }
        }))?;
        let result = self.wait_response(id, Duration::from_secs(10))?;
        let owned;
        let items: &[Value] = if let Some(arr) = result.as_array() {
            arr
        } else if result.is_object() {
            owned = vec![result];
            &owned
        } else {
            return Ok(vec![]);
        };
        Ok(items
            .iter()
            .filter_map(|loc| {
                let uri = loc["uri"]
                    .as_str()
                    .or_else(|| loc["targetUri"].as_str())?;
                let path = uri.strip_prefix("file://").unwrap_or(uri);
                let line = loc["range"]["start"]["line"]
                    .as_u64()
                    .or_else(|| loc["targetRange"]["start"]["line"].as_u64())?;
                Some(format!("{}:{}", path, line + 1))
            })
            .collect())
    }

    pub fn diagnostics(&mut self, uri: &str) -> Vec<Diagnostic> {
        for i in 0..self.pending.len() {
            if let Some(msg) = self.pending.get(i) {
                if msg["method"].as_str() == Some("textDocument/publishDiagnostics")
                    && msg["params"]["uri"].as_str() == Some(uri)
                {
                    let msg = self.pending.remove(i).unwrap();
                    return parse_diagnostics(&msg["params"]["diagnostics"]);
                }
            }
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match self.rx.recv_timeout(remaining) {
                Ok(msg) => {
                    if msg["method"].as_str() == Some("textDocument/publishDiagnostics")
                        && msg["params"]["uri"].as_str() == Some(uri)
                    {
                        return parse_diagnostics(&msg["params"]["diagnostics"]);
                    }
                    self.pending.push_back(msg);
                }
                Err(_) => break,
            }
        }
        vec![]
    }

    pub fn shutdown(&mut self) {
        let id = self.next_id();
        let _ = self.send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "shutdown",
            "params": null
        }));
        let _ = self.wait_response(id, Duration::from_secs(3));
        let _ = self.notify("exit", json!(null));
        let _ = self.process.wait();
    }

    fn send(&mut self, msg: &Value) -> Result<()> {
        let body = serde_json::to_string(msg)?;
        write!(
            self.writer,
            "Content-Length: {}\r\n\r\n{}",
            body.len(),
            body
        )?;
        self.writer.flush().context("flushing LSP writer")
    }

    fn notify(&mut self, method: &str, params: Value) -> Result<()> {
        self.send(&json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        }))
    }

    fn next_id(&mut self) -> u32 {
        self.id += 1;
        self.id
    }

    fn wait_response(&mut self, id: u32, timeout: Duration) -> Result<Value> {
        for i in 0..self.pending.len() {
            if let Some(msg) = self.pending.get(i) {
                if msg["id"].as_u64() == Some(id as u64) {
                    let msg = self.pending.remove(i).unwrap();
                    if let Some(err) = msg.get("error").filter(|e| !e.is_null()) {
                        bail!("LSP error: {}", err);
                    }
                    return Ok(msg["result"].clone());
                }
            }
        }
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                bail!("LSP response timeout (id={})", id);
            }
            match self.rx.recv_timeout(remaining) {
                Ok(msg) => {
                    if msg["id"].as_u64() == Some(id as u64) {
                        if let Some(err) = msg.get("error").filter(|e| !e.is_null()) {
                            bail!("LSP error: {}", err);
                        }
                        return Ok(msg["result"].clone());
                    }
                    self.pending.push_back(msg);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => bail!("LSP response timeout (id={})", id),
                Err(mpsc::RecvTimeoutError::Disconnected) => bail!("LSP reader disconnected"),
            }
        }
    }
}

fn read_frame<R: Read>(reader: &mut BufReader<R>) -> Result<Value> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            bail!("LSP: EOF reading headers");
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some(v) = trimmed.strip_prefix("Content-Length: ") {
            content_length = Some(v.trim().parse().context("parsing Content-Length")?);
        }
    }
    let len = content_length.context("LSP: no Content-Length header")?;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).context("LSP: reading body")?;
    serde_json::from_slice(&buf).context("LSP: parsing JSON body")
}

fn parse_diagnostics(arr: &Value) -> Vec<Diagnostic> {
    let Some(items) = arr.as_array() else {
        return vec![];
    };
    items
        .iter()
        .filter_map(|d| {
            let message = d["message"].as_str()?.to_string();
            let line = d["range"]["start"]["line"].as_u64().unwrap_or(0) as u32;
            let col = d["range"]["start"]["character"].as_u64().unwrap_or(0) as u32;
            let severity = match d["severity"].as_u64().unwrap_or(1) {
                2 => "warning",
                3 => "information",
                4 => "hint",
                _ => "error",
            };
            let source = d["source"].as_str().map(str::to_string);
            Some(Diagnostic {
                line,
                col,
                severity,
                message,
                source,
            })
        })
        .collect()
}
