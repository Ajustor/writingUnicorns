use crate::dap::types::DapConfig;
use crate::editor::highlight::Token;
use crate::extension::manifest::{Capabilities, ExtensionInfo, ExtensionManifest};
use crate::plugin::{Plugin, PluginContext, PluginResponse};

pub struct RustLangExtension;

impl RustLangExtension {
    pub fn manifest() -> ExtensionManifest {
        ExtensionManifest {
            extension: ExtensionInfo {
                id: "builtin.rust-lang".to_string(),
                name: "Rust Language Support".to_string(),
                version: "0.1.0".to_string(),
                description: "Syntax highlighting for Rust source files.".to_string(),
                author: "Writing Unicorns".to_string(),
                repository: String::new(),
            },
            dependencies: Default::default(),
            capabilities: Capabilities {
                languages: vec!["rs".to_string()],
                commands: vec![],
                themes: vec![],
                ..Default::default()
            },
        }
    }
}

impl Plugin for RustLangExtension {
    fn name(&self) -> &str {
        "Rust Language Support"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn tokenize_line(&self, lang: &str, line: &str) -> Option<Vec<Token>> {
        if lang == "rs" {
            Some(crate::editor::highlight::tokenize_rust(line))
        } else {
            None
        }
    }

    fn hover_info(&self, lang: &str, word: &str, file_content: &str) -> Option<String> {
        if lang != "rs" {
            return None;
        }
        rust_hover_info(word, file_content)
    }

    fn update(&mut self, _ctx: &PluginContext) -> PluginResponse {
        PluginResponse::default()
    }

    fn dap_config(&self) -> Option<DapConfig> {
        Some(DapConfig {
            adapter_cmd: "codelldb".to_string(),
            adapter_args: vec![],
            launch_config: serde_json::json!({
                "type": "lldb",
                "request": "launch",
                "name": "Debug Rust",
                "program": "${workspaceFolder}/target/debug/app",
                "args": [],
                "cwd": "${workspaceFolder}",
                "env": {}
            }),
        })
    }
}

/// Scan Rust source text for a definition of `word` and return a code-fenced signature.
fn rust_hover_info(word: &str, file_content: &str) -> Option<String> {
    let fn_patterns = [
        format!("fn {}(", word),
        format!("fn {} (", word),
        format!("pub fn {}(", word),
        format!("pub(crate) fn {}(", word),
        format!("async fn {}(", word),
        format!("pub async fn {}(", word),
        format!("unsafe fn {}(", word),
        format!("pub unsafe fn {}(", word),
    ];
    let type_patterns = [
        format!("struct {} ", word),
        format!("struct {}{}", word, '{'),
        format!("pub struct {}", word),
        format!("enum {} ", word),
        format!("pub enum {}", word),
        format!("trait {} ", word),
        format!("pub trait {}", word),
        format!("type {} =", word),
        format!("pub type {} =", word),
    ];
    let let_patterns = [
        format!("let {}: ", word),
        format!("let mut {}: ", word),
        format!("let {} =", word),
        format!("let mut {} =", word),
        format!("const {}: ", word),
        format!("static {}: ", word),
    ];

    for line in file_content.lines() {
        let trimmed = line.trim();

        for pat in &fn_patterns {
            if trimmed.contains(pat.as_str()) {
                let sig = trimmed.trim_end_matches('{').trim_end();
                return Some(format!("```rust\n{sig}\n```"));
            }
        }
        for pat in &type_patterns {
            if trimmed.contains(pat.as_str()) || trimmed.starts_with(pat.as_str()) {
                let sig = trimmed.trim_end_matches('{').trim_end();
                return Some(format!("```rust\n{sig}\n```"));
            }
        }
        for pat in &let_patterns {
            if trimmed.starts_with(pat.as_str()) {
                let end = trimmed
                    .find('=')
                    .or_else(|| trimmed.find(';'))
                    .unwrap_or(trimmed.len());
                let sig = trimmed[..end].trim_end();
                return Some(format!("```rust\n{sig}\n```"));
            }
        }
    }
    None
}
