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

    pub fn show(&mut self, ctx: &egui::Context, keybindings: &crate::config::KeyBindings) {
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
                    row(
                        ui,
                        &keybindings.shortcuts_help.display(),
                        "Show / hide this help",
                    );
                    row(ui, &keybindings.settings.display(), "Open Settings");
                    row(ui, &keybindings.new_file.display(), "New empty file");
                    row(ui, &keybindings.open_folder.display(), "Open folder");
                    row(ui, &keybindings.open_file.display(), "Open file");
                    row(ui, &keybindings.save.display(), "Save current file");
                    row(
                        ui,
                        &keybindings.command_palette.display(),
                        "Command palette (file search)",
                    );
                    row(ui, "Ctrl+Shift+F", "Search in workspace");
                    row(ui, &keybindings.toggle_sidebar.display(), "Toggle sidebar");
                    row(
                        ui,
                        &keybindings.toggle_terminal.display(),
                        "Toggle integrated terminal",
                    );

                    ui.add_space(8.0);
                    section(ui, "Editor");
                    row(ui, "Arrow keys", "Move cursor");
                    row(ui, "Home / End", "Go to start / end of line");
                    row(ui, "Ctrl+Home", "Go to start of file");
                    row(ui, "Ctrl+End", "Go to end of file");
                    row(ui, "Backspace", "Delete character before cursor");
                    row(ui, "Enter", "Insert new line");
                    row(ui, "Ctrl+Enter", "Insert line below cursor");
                    row(ui, "Ctrl+Shift+Enter", "Insert line above cursor");
                    row(ui, "Tab", "Indent (insert 4 spaces)");
                    row(ui, "Shift + Tab", "Un-indent (remove leading spaces)");
                    row(ui, "Ctrl+/", "Toggle line comment");
                    row(ui, "Ctrl+Shift+K", "Delete current line");
                    row(ui, "Ctrl+Shift+↑", "Move line up");
                    row(ui, "Ctrl+Shift+↓", "Move line down");
                    row(ui, &keybindings.save.display(), "Save file");
                    row(ui, &keybindings.close_tab.display(), "Close tab");

                    ui.add_space(8.0);
                    section(ui, "Navigation");
                    row(
                        ui,
                        &keybindings.command_palette.display(),
                        "Go to file (fuzzy search)",
                    );
                    row(ui, "Click file tree", "Open file");
                    row(ui, "Click tab", "Switch to tab");
                    row(ui, "× on tab", "Close tab");

                    ui.add_space(8.0);
                    section(ui, "Sidebar");
                    row(ui, "Click ▸ / ▾", "Expand / collapse folder");
                    row(ui, "Explorer tab", "Browse project files");
                    row(ui, "Git tab", "View changed files & branch");

                    ui.add_space(8.0);
                    section(ui, "Terminal");
                    row(ui, "Enter", "Execute command");
                    row(
                        ui,
                        &keybindings.toggle_terminal.display(),
                        "Show / hide terminal panel",
                    );

                    ui.add_space(8.0);
                    section(ui, "Run");
                    row(ui, "F5", "Run active configuration");
                    row(ui, "Ctrl+F5", "Run without stopping");

                    ui.add_space(8.0);
                    section(ui, "Menu");
                    row(ui, "File → Open Folder", "Open a workspace folder");
                    row(ui, "File → Open File", "Open a single file");
                    row(ui, "File → Save", "Save current file");
                    row(ui, "View → …", "Toggle sidebar / terminal / palette");
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
        ui.label(egui::RichText::new(description).color(egui::Color32::from_rgb(180, 180, 180)));
    });
    ui.add_space(2.0);
}
