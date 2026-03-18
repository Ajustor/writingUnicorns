use crate::editor::highlight::Token;
use crate::extension::manifest::{Capabilities, ExtensionInfo, ExtensionManifest};
use crate::plugin::{Plugin, PluginContext, PluginResponse};

pub struct DataLangExtension;

impl DataLangExtension {
    pub fn manifest() -> ExtensionManifest {
        ExtensionManifest {
            extension: ExtensionInfo {
                id: "builtin.data-lang".to_string(),
                name: "Data Language Support".to_string(),
                version: "0.1.0".to_string(),
                description: "Syntax highlighting for JSON and TOML data files.".to_string(),
                author: "Writing Unicorns".to_string(),
                repository: String::new(),
            },
            dependencies: Default::default(),
            capabilities: Capabilities {
                languages: vec!["json".to_string(), "toml".to_string()],
                commands: vec![],
                themes: vec![],
                ..Default::default()
            },
        }
    }
}

impl Plugin for DataLangExtension {
    fn name(&self) -> &str {
        "Data Language Support"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn tokenize_line(&self, lang: &str, line: &str) -> Option<Vec<Token>> {
        match lang {
            "json" => Some(crate::editor::highlight::tokenize_json(line)),
            "toml" => Some(crate::editor::highlight::tokenize_toml(line)),
            _ => None,
        }
    }

    fn update(&mut self, _ctx: &PluginContext) -> PluginResponse {
        PluginResponse::default()
    }
}
