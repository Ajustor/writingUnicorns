pub struct ShortcutsHelp {
    open: bool,
}

impl ShortcutsHelp {
    pub fn new() -> Self {
        Self { open: false }
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }

        let screen = ctx.screen_rect();
        egui::Window::new("Keyboard Shortcuts")
            .open(&mut self.open)
            .resizable(false)
            .collapsible(false)
            .fixed_pos(egui::pos2(
                screen.center().x - 300.0,
                screen.center().y - 260.0,
            ))
            .fixed_size(egui::vec2(600.0, 520.0))
            .show(ctx, |ui| {
                ui.add_space(4.0);

                egui::ScrollArea::vertical().show(ui, |ui| {
                    section(ui, "General");
                    row(ui, "F1",                 "Show / hide this help");
                    row(ui, "Ctrl + N",           "New empty file");
                    row(ui, "Ctrl + O",           "Open folder");
                    row(ui, "Ctrl + Shift + O",   "Open file");
                    row(ui, "Ctrl + S",           "Save current file");
                    row(ui, "Ctrl + P",           "Command palette (file search)");
                    row(ui, "Ctrl + B",           "Toggle sidebar");
                    row(ui, "Ctrl + `",           "Toggle integrated terminal");

                    ui.add_space(8.0);
                    section(ui, "Editor");
                    row(ui, "Arrow keys",       "Move cursor");
                    row(ui, "Home / End",       "Go to start / end of line");
                    row(ui, "Backspace",        "Delete character before cursor");
                    row(ui, "Enter",            "Insert new line");
                    row(ui, "Tab",              "Indent (insert 4 spaces)");
                    row(ui, "Shift + Tab",      "Un-indent (remove leading spaces)");
                    row(ui, "Ctrl + S",         "Save file");

                    ui.add_space(8.0);
                    section(ui, "Navigation");
                    row(ui, "Ctrl + P",         "Go to file (fuzzy search)");
                    row(ui, "Click file tree",  "Open file");
                    row(ui, "Click tab",        "Switch to tab");
                    row(ui, "× on tab",         "Close tab");

                    ui.add_space(8.0);
                    section(ui, "Sidebar");
                    row(ui, "Click ▸ / ▾",     "Expand / collapse folder");
                    row(ui, "Explorer tab",     "Browse project files");
                    row(ui, "Git tab",          "View changed files & branch");

                    ui.add_space(8.0);
                    section(ui, "Terminal");
                    row(ui, "Enter",            "Execute command");
                    row(ui, "Ctrl + `",         "Show / hide terminal panel");

                    ui.add_space(8.0);
                    section(ui, "Menu");
                    row(ui, "File → Open Folder",  "Open a workspace folder");
                    row(ui, "File → Open File",    "Open a single file");
                    row(ui, "File → Save",          "Save current file");
                    row(ui, "View → …",             "Toggle sidebar / terminal / palette");
                    row(ui, "Git → Refresh Status", "Reload git file status");
                });
            });
    }
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.label(
        egui::RichText::new(title)
            .strong()
            .color(egui::Color32::from_rgb(86, 156, 214))
            .size(13.0),
    );
    ui.separator();
}

fn row(ui: &mut egui::Ui, keys: &str, description: &str) {
    ui.horizontal(|ui| {
        // Key badge
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(50, 50, 50))
            .corner_radius(egui::CornerRadius::same(3))
            .inner_margin(egui::Margin::symmetric(6, 2))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(keys)
                        .monospace()
                        .small()
                        .color(egui::Color32::from_rgb(200, 200, 200)),
                );
            });
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(description).color(egui::Color32::from_rgb(180, 180, 180)),
        );
    });
    ui.add_space(2.0);
}
