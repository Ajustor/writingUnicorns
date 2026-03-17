use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crossbeam_channel::{Sender};
use serde_json::{json, Value};

use super::transport::LspTransport;

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub message: String,
    pub line: u32,
    pub col: u32,
    pub severity: DiagSeverity,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DiagSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub label: String,
    pub detail: Option<String>,
    pub kind: String,
    pub insert_text: Option<String>,
}

struct LspClientInner {
    transport: LspTransport,
    next_id: u64,
    /// Channels for callers waiting on a specific request id.
    pending: HashMap<u64, Sender<Value>>,
}

pub struct LspClient {
    pub diagnostics: HashMap<String, Vec<Diagnostic>>,
    pub completions: Vec<CompletionItem>,
    pub is_connected: bool,
    inner: Option<LspClientInner>,
}

impl LspClient {
    pub fn new() -> Self {
        Self {
            diagnostics: HashMap::new(),
            completions: vec![],
            is_connected: false,
            inner: None,
        }
    }

    fn next_id(inner: &mut LspClientInner) -> u64 {
        let id = inner.next_id;
        inner.next_id += 1;
        id
    }

    /// Spawn the LSP server process and perform the initialize handshake.
    pub fn start(&mut self, command: &str, args: &[&str], workspace: &Path) -> anyhow::Result<()> {
        let workspace_str = workspace.to_string_lossy();
        let mut transport = LspTransport::spawn(command, args, &workspace_str)?;

        let id = 1u64;
        transport.send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "initialize",
            "params": {
                "processId": std::process::id(),
                "rootUri": format!("file://{}", workspace.display()),
                "capabilities": {
                    "textDocument": {
                        "hover": { "contentFormat": ["plaintext", "markdown"] },
                        "completion": { "completionItem": { "snippetSupport": false } },
                        "publishDiagnostics": {}
                    }
                }
            }
        }))?;

        // Wait up to 5 s for the initialize response.
        let deadline =
            std::time::Instant::now() + std::time::Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            if let Ok(msg) = transport.receiver.try_recv() {
                if msg.get("id").and_then(|v| v.as_u64()) == Some(id) {
                    transport.send(&json!({
                        "jsonrpc": "2.0",
                        "method": "initialized",
                        "params": {}
                    }))?;
                    self.is_connected = true;
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        self.inner = Some(LspClientInner {
            transport,
            next_id: id + 1,
            pending: HashMap::new(),
        });
        Ok(())
    }

    /// Notify the server that a file was opened.
    pub fn did_open(&mut self, uri: &str, language_id: &str, content: &str) {
        let Some(inner) = &mut self.inner else { return };
        let _ = inner.transport.send(&json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": language_id,
                    "version": 1,
                    "text": content
                }
            }
        }));
    }

    /// Notify the server that a file changed.
    pub fn did_change(&mut self, uri: &str, version: i32, content: &str) {
        let Some(inner) = &mut self.inner else { return };
        let _ = inner.transport.send(&json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": uri, "version": version },
                "contentChanges": [{ "text": content }]
            }
        }));
    }

    /// Request hover info. Returns the request id; match it in `poll()` results.
    pub fn request_hover(&mut self, uri: &str, line: u32, character: u32) -> u64 {
        let Some(inner) = &mut self.inner else { return 0 };
        let id = Self::next_id(inner);
        let _ = inner.transport.send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
            }
        }));
        id
    }

    /// Request completion items. Returns the request id.
    pub fn request_completions(&mut self, uri: &str, line: u32, character: u32) -> u64 {
        let Some(inner) = &mut self.inner else { return 0 };
        let id = Self::next_id(inner);
        let _ = inner.transport.send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
            }
        }));
        id
    }

    /// Request go-to-definition. Returns the request id.
    pub fn request_definition(&mut self, uri: &str, line: u32, character: u32) -> u64 {
        let Some(inner) = &mut self.inner else { return 0 };
        let id = Self::next_id(inner);
        let _ = inner.transport.send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "textDocument/definition",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
            }
        }));
        id
    }

    /// Drain incoming messages from the server.
    ///
    /// Returns `(request_id, message)` pairs for responses. Notifications such
    /// as `textDocument/publishDiagnostics` are handled internally.
    pub fn poll(&mut self) -> Vec<(u64, Value)> {
        let Some(inner) = &mut self.inner else { return vec![] };
        let mut results = Vec::new();

        while let Ok(msg) = inner.transport.receiver.try_recv() {
            if let Some(id) = msg.get("id").and_then(|v| v.as_u64()) {
                results.push((id, msg));
            } else if let Some(method) = msg.get("method").and_then(|v| v.as_str()) {
                if method == "textDocument/publishDiagnostics" {
                    Self::process_diagnostics_msg(&mut self.diagnostics, &msg);
                }
            }
        }
        results
    }

    fn process_diagnostics_msg(store: &mut HashMap<String, Vec<Diagnostic>>, msg: &Value) {
        let uri = msg["params"]["uri"].as_str().unwrap_or("").to_string();
        let mut diags = Vec::new();
        if let Some(arr) = msg["params"]["diagnostics"].as_array() {
            for d in arr {
                let severity = match d["severity"].as_u64().unwrap_or(1) {
                    1 => DiagSeverity::Error,
                    2 => DiagSeverity::Warning,
                    3 => DiagSeverity::Info,
                    _ => DiagSeverity::Hint,
                };
                diags.push(Diagnostic {
                    message: d["message"].as_str().unwrap_or("").to_string(),
                    line: d["range"]["start"]["line"].as_u64().unwrap_or(0) as u32,
                    col: d["range"]["start"]["character"].as_u64().unwrap_or(0) as u32,
                    severity,
                });
            }
        }
        store.insert(uri, diags);
    }

    /// Parse a hover response into a display string.
    pub fn parse_hover(response: &Value) -> Option<String> {
        let contents = response.get("result")?.get("contents")?;
        if let Some(s) = contents.as_str() {
            return Some(s.to_string());
        }
        if let Some(obj) = contents.as_object() {
            if let Some(value) = obj.get("value").and_then(|v| v.as_str()) {
                return Some(value.to_string());
            }
        }
        if let Some(arr) = contents.as_array() {
            for item in arr {
                if let Some(s) = item.as_str() {
                    if !s.is_empty() {
                        return Some(s.to_string());
                    }
                }
                if let Some(v) = item.get("value").and_then(|v| v.as_str()) {
                    if !v.is_empty() {
                        return Some(v.to_string());
                    }
                }
            }
        }
        None
    }

    /// Parse a definition response into `(file_path, line)`.
    pub fn parse_definition(response: &Value) -> Option<(PathBuf, u32)> {
        let result = response.get("result")?;
        let loc = if result.is_array() {
            result.as_array()?.first()?
        } else {
            result
        };
        let uri = loc.get("uri").and_then(|v| v.as_str())?;
        let line = loc["range"]["start"]["line"].as_u64()? as u32;
        let path = uri.strip_prefix("file://").unwrap_or(uri);
        Some((PathBuf::from(path), line))
    }

    /// Parse a completion response into a list of items (capped at 50).
    pub fn parse_completions(response: &Value) -> Vec<CompletionItem> {
        let result = response.get("result");
        let items = result
            .and_then(|r| r.get("items"))
            .or(result)
            .and_then(|v| v.as_array());

        let mut completions = Vec::new();
        if let Some(arr) = items {
            for item in arr.iter().take(50) {
                let kind_num = item["kind"].as_u64().unwrap_or(0);
                let kind = match kind_num {
                    1 => "Text",
                    2 => "Method",
                    3 => "Function",
                    4 => "Constructor",
                    5 => "Field",
                    6 => "Variable",
                    7 => "Class",
                    8 => "Interface",
                    9 => "Module",
                    10 => "Property",
                    14 => "Keyword",
                    15 => "Snippet",
                    _ => "Value",
                };
                completions.push(CompletionItem {
                    label: item["label"].as_str().unwrap_or("").to_string(),
                    detail: item["detail"].as_str().map(|s| s.to_string()),
                    kind: kind.to_string(),
                    insert_text: item["insertText"].as_str().map(|s| s.to_string()),
                });
            }
        }
        completions
    }

    pub fn get_diagnostics(&self, path: &str) -> Vec<Diagnostic> {
        self.diagnostics.get(path).cloned().unwrap_or_default()
    }
}
