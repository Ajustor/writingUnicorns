use crate::dap::types::DapConfig;
use crate::editor::highlight::Token;
use crate::extension::manifest::{Capabilities, ExtensionInfo, ExtensionManifest};
use crate::plugin::{Plugin, PluginContext, PluginResponse};

pub struct ShellLangExtension;

impl ShellLangExtension {
    pub fn manifest() -> ExtensionManifest {
        ExtensionManifest {
            extension: ExtensionInfo {
                id: "builtin.shell-lang".to_string(),
                name: "Shell Language Support".to_string(),
                version: "0.1.0".to_string(),
                description: "Syntax highlighting for shell scripts (sh, bash, zsh).".to_string(),
                author: "Writing Unicorns".to_string(),
                repository: String::new(),
            },
            dependencies: Default::default(),
            capabilities: Capabilities {
                languages: vec!["sh".to_string(), "bash".to_string(), "zsh".to_string()],
                commands: vec![],
                themes: vec![],
                ..Default::default()
            },
        }
    }
}

impl Plugin for ShellLangExtension {
    fn name(&self) -> &str {
        "Shell Language Support"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn tokenize_line(&self, lang: &str, line: &str) -> Option<Vec<Token>> {
        match lang {
            "sh" | "bash" | "zsh" => Some(crate::editor::highlight::tokenize_shell(line)),
            _ => None,
        }
    }

    fn update(&mut self, _ctx: &PluginContext) -> PluginResponse {
        PluginResponse::default()
    }

    fn dap_config(&self) -> Option<DapConfig> {
        // Requires bash-debug-adapter (install: npm install -g bash-debug)
        Some(DapConfig {
            adapter_cmd: "bash-debug-adapter".to_string(),
            adapter_args: vec![],
            launch_config: serde_json::json!({
                "type": "bashdb",
                "request": "launch",
                "name": "Debug Shell Script",
                "program": "${file}",
                "args": [],
                "cwd": "${workspaceFolder}",
                "env": {},
                "terminalKind": "integrated"
            }),
        })
    }
}
