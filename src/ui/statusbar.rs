use crate::editor::Editor;
use crate::git::GitStatus;

pub struct StatusBar {}

impl StatusBar {
    pub fn new() -> Self { Self {} }

    pub fn show(&self, ui: &mut egui::Ui, editor: &Editor, git: &GitStatus) {
        let bg = egui::Color32::from_rgb(0, 122, 204);
        egui::Frame::new().fill(bg).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("⎇ {}", git.branch))
                        .color(egui::Color32::WHITE)
                        .small(),
                );
                ui.separator();

                if let Some(path) = &editor.current_path {
                    let name = path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let modified = if editor.is_modified { " ●" } else { "" };
                    ui.label(
                        egui::RichText::new(format!("{}{}", name, modified))
                            .color(egui::Color32::WHITE)
                            .small(),
                    );
                    ui.separator();

                    let ext = path.extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("txt");
                    ui.label(
                        egui::RichText::new(ext.to_uppercase())
                            .color(egui::Color32::WHITE)
                            .small(),
                    );
                    ui.separator();
                }

                let (row, col) = editor.cursor.position();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("Ln {}, Col {}", row + 1, col + 1))
                            .color(egui::Color32::WHITE)
                            .small(),
                    );
                });
            });
        });
    }
}
