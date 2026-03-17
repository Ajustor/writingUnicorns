use crate::editor::highlight::Token;
use crate::extension::manifest::{Capabilities, ExtensionInfo, ExtensionManifest};
use crate::plugin::{Plugin, PluginContext, PluginResponse};

pub struct PythonLangExtension;

impl PythonLangExtension {
    pub fn manifest() -> ExtensionManifest {
        ExtensionManifest {
            extension: ExtensionInfo {
                id: "builtin.python-lang".to_string(),
                name: "Python Language Support".to_string(),
                version: "0.1.0".to_string(),
                description: "Syntax highlighting for Python source files.".to_string(),
                author: "Writing Unicorns".to_string(),
                repository: String::new(),
            },
            capabilities: Capabilities {
                languages: vec!["py".to_string()],
                commands: vec![],
                themes: vec![],
            },
        }
    }
}

impl Plugin for PythonLangExtension {
    fn name(&self) -> &str {
        "Python Language Support"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn tokenize_line(&self, lang: &str, line: &str) -> Option<Vec<Token>> {
        if lang == "py" {
            Some(crate::editor::highlight::tokenize_python(line))
        } else {
            None
        }
    }

    fn update(&mut self, _ctx: &PluginContext) -> PluginResponse {
        PluginResponse::default()
    }
}
