use crate::editor::highlight::Token;
use crate::extension::manifest::{Capabilities, ExtensionInfo, ExtensionManifest};
use crate::plugin::{Plugin, PluginContext, PluginResponse};

pub struct DockerLangExtension;

impl DockerLangExtension {
    pub fn manifest() -> ExtensionManifest {
        ExtensionManifest {
            extension: ExtensionInfo {
                id: "builtin.docker-lang".to_string(),
                name: "Dockerfile Language Support".to_string(),
                version: "0.1.0".to_string(),
                description: "Syntax highlighting for Dockerfile.".to_string(),
                author: "Writing Unicorns".to_string(),
                repository: String::new(),
            },
            capabilities: Capabilities {
                languages: vec!["dockerfile".to_string()],
                commands: vec![],
                themes: vec![],
            },
        }
    }
}

impl Plugin for DockerLangExtension {
    fn name(&self) -> &str {
        "Dockerfile Language Support"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn tokenize_line(&self, lang: &str, line: &str) -> Option<Vec<Token>> {
        if lang == "dockerfile" {
            Some(crate::editor::highlight::tokenize_dockerfile(line))
        } else {
            None
        }
    }

    fn update(&mut self, _ctx: &PluginContext) -> PluginResponse {
        PluginResponse::default()
    }
}
