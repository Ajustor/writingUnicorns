use crate::dap::types::DapConfig;
use crate::editor::highlight::Token;
use crate::extension::manifest::{Capabilities, ExtensionInfo, ExtensionManifest};
use crate::plugin::{Plugin, PluginContext, PluginResponse};

pub struct WebLangExtension;

impl WebLangExtension {
    pub fn manifest() -> ExtensionManifest {
        ExtensionManifest {
            extension: ExtensionInfo {
                id: "builtin.web-lang".to_string(),
                name: "Web Language Support".to_string(),
                version: "0.1.0".to_string(),
                description: "Syntax highlighting for JavaScript, TypeScript, JSX, and TSX."
                    .to_string(),
                author: "Writing Unicorns".to_string(),
                repository: String::new(),
            },
            dependencies: Default::default(),
            capabilities: Capabilities {
                languages: vec![
                    "js".to_string(),
                    "ts".to_string(),
                    "jsx".to_string(),
                    "tsx".to_string(),
                    "mjs".to_string(),
                ],
                commands: vec![],
                themes: vec![],
                ..Default::default()
            },
        }
    }
}

impl Plugin for WebLangExtension {
    fn name(&self) -> &str {
        "Web Language Support"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn tokenize_line(&self, lang: &str, line: &str) -> Option<Vec<Token>> {
        match lang {
            "js" | "ts" | "jsx" | "tsx" | "mjs" => {
                Some(crate::editor::highlight::tokenize_js_ts(line))
            }
            _ => None,
        }
    }

    fn hover_info(&self, lang: &str, word: &str, file_content: &str) -> Option<String> {
        match lang {
            "js" | "jsx" | "mjs" => js_hover_info(word, file_content),
            "ts" | "tsx" => ts_hover_info(word, file_content),
            _ => None,
        }
    }

    fn update(&mut self, _ctx: &PluginContext) -> PluginResponse {
        PluginResponse::default()
    }

    fn dap_config(&self) -> Option<DapConfig> {
        // Node.js debugging via node --inspect protocol bridged through a DAP adapter.
        // Requires `@vscode/js-debug` or `node-debug2` to be installed.
        Some(DapConfig {
            adapter_cmd: "node".to_string(),
            adapter_args: vec!["--require".to_string(), "ts-node/register".to_string()],
            launch_config: serde_json::json!({
                "type": "node",
                "request": "launch",
                "name": "Debug Node.js",
                "program": "${file}",
                "cwd": "${workspaceFolder}",
                "console": "integratedTerminal",
                "runtimeExecutable": "node"
            }),
        })
    }
}

/// Scan JavaScript source text for a definition of `word` and return a code-fenced signature.
fn js_hover_info(word: &str, file_content: &str) -> Option<String> {
    let fn_patterns = [
        format!("function {}(", word),
        format!("function {} (", word),
        format!("async function {}(", word),
        format!("async function {} (", word),
    ];
    let arrow_patterns = [
        format!("const {} = (", word),
        format!("const {} = async (", word),
        format!("let {} = (", word),
        format!("var {} = (", word),
    ];
    let decl_patterns = [
        format!("class {} ", word),
        format!("class {}{}", word, '{'),
        format!("const {}", word),
        format!("let {}", word),
        format!("var {}", word),
    ];

    for line in file_content.lines() {
        let trimmed = line.trim();
        for pat in &fn_patterns {
            if trimmed.contains(pat.as_str()) {
                let sig = trimmed.trim_end_matches('{').trim_end();
                return Some(format!("```js\n{sig}\n```"));
            }
        }
        for pat in &arrow_patterns {
            if trimmed.starts_with(pat.as_str()) {
                let sig = trimmed.trim_end_matches('{').trim_end();
                return Some(format!("```js\n{sig}\n```"));
            }
        }
        for pat in &decl_patterns {
            if trimmed.starts_with(pat.as_str()) {
                let end = trimmed
                    .find('=')
                    .or_else(|| trimmed.find(';'))
                    .unwrap_or(trimmed.len());
                let sig = trimmed[..end].trim_end();
                return Some(format!("```js\n{sig}\n```"));
            }
        }
    }
    None
}

/// Scan TypeScript source text for a definition of `word` and return a code-fenced signature.
fn ts_hover_info(word: &str, file_content: &str) -> Option<String> {
    let fn_patterns = [
        format!("function {}(", word),
        format!("function {} (", word),
        format!("async function {}(", word),
        format!("async function {} (", word),
    ];
    let arrow_patterns = [
        format!("const {} = (", word),
        format!("const {} = async (", word),
        format!("let {} = (", word),
        format!("var {} = (", word),
    ];
    let decl_patterns = [
        format!("class {} ", word),
        format!("class {}{}", word, '{'),
        format!("interface {} ", word),
        format!("interface {}{}", word, '{'),
        format!("type {} =", word),
        format!("const {}", word),
        format!("let {}", word),
        format!("var {}", word),
    ];

    for line in file_content.lines() {
        let trimmed = line.trim();
        for pat in &fn_patterns {
            if trimmed.contains(pat.as_str()) {
                let sig = trimmed.trim_end_matches('{').trim_end();
                return Some(format!("```ts\n{sig}\n```"));
            }
        }
        for pat in &arrow_patterns {
            if trimmed.starts_with(pat.as_str()) {
                let sig = trimmed.trim_end_matches('{').trim_end();
                return Some(format!("```ts\n{sig}\n```"));
            }
        }
        for pat in &decl_patterns {
            if trimmed.starts_with(pat.as_str()) {
                let end = trimmed
                    .find('=')
                    .or_else(|| trimmed.find(';'))
                    .unwrap_or(trimmed.len());
                let sig = trimmed[..end].trim_end();
                return Some(format!("```ts\n{sig}\n```"));
            }
        }
    }
    None
}
