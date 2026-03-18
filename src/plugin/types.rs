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
