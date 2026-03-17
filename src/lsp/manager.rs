use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

use super::client::LspClient;

/// Maps file extension to a running LSP client.
pub struct LspManager {
    clients: HashMap<String, LspClient>,
}

impl LspManager {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    /// Ensure an LSP server is running for the given file extension.
    /// Silently does nothing if the server binary is not found.
    pub fn ensure_started(&mut self, ext: &str, workspace: &Path) {
        if self.clients.contains_key(ext) {
            return;
        }

        let (cmd, args): (&str, &[&str]) = match ext {
            "rs" => ("rust-analyzer", &[]),
            "ts" | "tsx" => ("typescript-language-server", &["--stdio"]),
            "js" | "jsx" => ("typescript-language-server", &["--stdio"]),
            "py" => ("pylsp", &[]),
            _ => return,
        };

        let mut client = LspClient::new();
        if client.start(cmd, args, workspace).is_ok() {
            self.clients.insert(ext.to_string(), client);
        }
    }

    pub fn get_mut(&mut self, ext: &str) -> Option<&mut LspClient> {
        self.clients.get_mut(ext)
    }

    pub fn get(&self, ext: &str) -> Option<&LspClient> {
        self.clients.get(ext)
    }

    /// Poll all active clients and return their pending responses.
    pub fn poll_all(&mut self) -> HashMap<String, Vec<(u64, Value)>> {
        let mut results = HashMap::new();
        for (ext, client) in &mut self.clients {
            let msgs = client.poll();
            if !msgs.is_empty() {
                results.insert(ext.clone(), msgs);
            }
        }
        results
    }
}
