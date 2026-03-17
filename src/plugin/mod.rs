pub mod builtin;
pub mod manager;

use crate::editor::highlight::Token;

/// A command that can be registered by a plugin and shown in the command palette.
#[derive(Clone)]
pub struct PluginCommand {
    pub id: String,
    pub title: String,
    pub keybinding: Option<String>,
}

/// A sidebar panel contributed by a plugin.
#[derive(Clone)]
pub struct SidebarPanel {
    pub id: String,
    pub title: String,
    pub icon: &'static str,
}

/// Context passed to plugins each frame.
pub struct PluginContext<'a> {
    pub buffer_text: &'a str,
    pub filename: Option<&'a str>,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub is_modified: bool,
    /// The symbol currently being hovered (if any), for hover-doc queries.
    pub hovered_word: Option<&'a str>,
}

/// What a plugin can tell the IDE to do.
#[derive(Default)]
pub struct PluginResponse {
    pub status_text: Option<String>,
    pub notifications: Vec<String>,
}

/// The Plugin trait — all plugins implement this.
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str {
        "0.1.0"
    }
    fn commands(&self) -> Vec<PluginCommand> {
        vec![]
    }
    fn sidebar_panels(&self) -> Vec<SidebarPanel> {
        vec![]
    }

    /// Called every frame with current editor state.
    fn update(&mut self, _ctx: &PluginContext) -> PluginResponse {
        PluginResponse::default()
    }

    /// Called when one of this plugin's commands is executed.
    fn execute_command(&mut self, _command_id: &str, _ctx: &PluginContext) -> PluginResponse {
        PluginResponse::default()
    }

    /// Called to render this plugin's sidebar panel (if any).
    fn render_sidebar(&mut self, _panel_id: &str, _ui: &mut egui::Ui) {}

    /// Provide syntax tokens for a line (optional — for language plugins).
    fn tokenize_line(&self, _lang: &str, _line: &str) -> Option<Vec<Token>> {
        None
    }

    /// Return hover documentation or a signature string for `word` in the given file.
    /// `lang` is the file extension (e.g. `"rs"`, `"ts"`, `"js"`).
    /// `file_content` is the full text of the current buffer.
    /// Returns a formatted string (e.g. a code-fenced signature), or `None` if not found.
    fn hover_info(&self, _lang: &str, _word: &str, _file_content: &str) -> Option<String> {
        None
    }

    /// Return the LSP server command for this language plugin.
    /// E.g. `Some(("rust-analyzer", vec![]))` for Rust.
    /// Return `None` if this plugin doesn't provide language server support.
    fn lsp_server_command(&self) -> Option<(String, Vec<String>)> {
        None
    }
}
