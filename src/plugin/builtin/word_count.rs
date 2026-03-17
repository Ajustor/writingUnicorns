use crate::plugin::{Plugin, PluginCommand, PluginContext, PluginResponse, SidebarPanel};

pub struct WordCountPlugin {
    word_count: usize,
    line_count: usize,
    char_count: usize,
}

impl WordCountPlugin {
    pub fn new() -> Self {
        Self {
            word_count: 0,
            line_count: 0,
            char_count: 0,
        }
    }
}

impl Default for WordCountPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for WordCountPlugin {
    fn name(&self) -> &str {
        "Word Count"
    }

    fn commands(&self) -> Vec<PluginCommand> {
        vec![PluginCommand {
            id: "word-count.show".into(),
            title: "Word Count: Show Statistics".into(),
            keybinding: None,
        }]
    }

    fn update(&mut self, ctx: &PluginContext) -> PluginResponse {
        self.word_count = ctx.buffer_text.split_whitespace().count();
        self.line_count = ctx.buffer_text.lines().count();
        self.char_count = ctx.buffer_text.chars().count();
        PluginResponse {
            status_text: Some(format!(
                "{} words | {} lines",
                self.word_count, self.line_count
            )),
            ..Default::default()
        }
    }

    fn render_sidebar(&mut self, _panel_id: &str, ui: &mut egui::Ui) {
        ui.label(format!("Words: {}", self.word_count));
        ui.label(format!("Lines: {}", self.line_count));
        ui.label(format!("Characters: {}", self.char_count));
    }

    fn sidebar_panels(&self) -> Vec<SidebarPanel> {
        vec![SidebarPanel {
            id: "word-count.panel".into(),
            title: "Word Count".into(),
            icon: egui_phosphor::regular::CHART_BAR,
        }]
    }
}
