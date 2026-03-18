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
    /// Uses `cmd`/`args` if provided; falls back to built-in defaults.
    /// Silently does nothing if the server binary is not found.
    pub fn ensure_started(&mut self, ext: &str, workspace: &Path) {
        let (cmd, args): (&str, &[&str]) = match ext {
            "rs" => ("rust-analyzer", &[]),
            "ts" | "tsx" => ("typescript-language-server", &["--stdio"]),
            "js" | "jsx" | "mjs" => ("typescript-language-server", &["--stdio"]),
            "py" | "pyw" => ("pylsp", &[]),
            "go" => ("gopls", &[]),
            "vue" => ("vue-language-server", &["--stdio"]),
            "svelte" => ("svelte-language-server", &["--stdio"]),
            _ => return,
        };
        self.start_client(ext, cmd, args, workspace);
    }

    /// Ensure an LSP server is running using an explicit command from the plugin system.
    pub fn ensure_started_with_cmd(
        &mut self,
        ext: &str,
        cmd: &str,
        args: &[String],
        workspace: &Path,
    ) {
        if self.clients.contains_key(ext) {
            return;
        }
        let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        self.start_client(ext, cmd, &args_ref, workspace);
    }

    fn start_client(&mut self, ext: &str, cmd: &str, args: &[&str], workspace: &Path) {
        if self.clients.contains_key(ext) {
            return;
        }
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
    /// Also drives crash detection + auto-restart for disconnected clients.
    /// Returns `(responses, reconnected_exts)` where `reconnected_exts` is the
    /// list of file extensions whose LSP server just reconnected this frame.
    pub fn poll_all(&mut self) -> (HashMap<String, Vec<(u64, Value)>>, Vec<String>) {
        let mut results = HashMap::new();
        let mut reconnected = Vec::new();
        for (ext, client) in &mut self.clients {
            let was_connected = client.is_connected;
            let msgs = client.poll();
            if !msgs.is_empty() {
                results.insert(ext.clone(), msgs);
            }
            // Attempt non-blocking restart if the client just crashed.
            if !client.is_connected {
                client.try_restart();
            }
            // Detect successful reconnect this frame.
            if !was_connected && client.is_connected {
                reconnected.push(ext.clone());
            }
        }
        (results, reconnected)
    }
}
