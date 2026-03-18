pub mod entry;
pub mod icons;

pub use entry::FileEntry;
pub use icons::file_icon;

use egui_phosphor::regular as ph;
use std::path::PathBuf;

pub struct FileTree {
    pub root: Option<FileEntry>,
    pub selected: Option<PathBuf>,
    /// Action requested via context menu, consumed by the caller each frame.
    pub context_action: Option<FileTreeAction>,
    /// Path being renamed (in-progress text).  Exposed so callers can pre-set it.
    pub rename_state: Option<(PathBuf, String)>,
}

#[derive(Debug, Clone)]
pub enum FileTreeAction {
    OpenFile(PathBuf),
    NewFile(PathBuf),        // parent dir
    NewFolder(PathBuf),      // parent dir
    Rename(PathBuf, String), // old path, new name
    Delete(PathBuf),
    RevealInExplorer(PathBuf),
    CopyPath(PathBuf),
}

impl FileTree {
    pub fn new() -> Self {
        Self {
            root: None,
            selected: None,
            context_action: None,
            rename_state: None,
        }
    }

    pub fn load(&mut self, path: PathBuf) {
        let mut root = FileEntry::new(path, 0);
        root.load_children();
        self.root = Some(root);
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<PathBuf> {
        let mut opened = None;
        let mut action: Option<FileTreeAction> = None;
        let mut rename_state = self.rename_state.take();
        if let Some(root) = &mut self.root {
            Self::show_entry_recursive(
                ui,
                root,
                &mut self.selected,
                &mut opened,
                &mut action,
                &mut rename_state,
            );
        }
        self.rename_state = rename_state;
        // Store non-open actions for the caller to pick up
        if let Some(a) = action {
            match &a {
                FileTreeAction::OpenFile(p) => {
                    opened = Some(p.clone());
                }
                _ => {
                    self.context_action = Some(a);
                }
            }
        }
        opened
    }

    fn show_entry_recursive(
        ui: &mut egui::Ui,
        entry: &mut FileEntry,
        selected: &mut Option<PathBuf>,
        opened: &mut Option<PathBuf>,
        action: &mut Option<FileTreeAction>,
        rename_state: &mut Option<(PathBuf, String)>,
    ) {
        let indent = entry.depth as f32 * 14.0;

        // Inline rename mode
        let is_renaming = rename_state
            .as_ref()
            .map(|(p, _)| p == &entry.path)
            .unwrap_or(false);

        let row_resp = ui.horizontal(|ui| {
            ui.add_space(indent);

            if is_renaming {
                if let Some((_, ref mut new_name)) = rename_state {
                    let resp = ui.add(egui::TextEdit::singleline(new_name).desired_width(150.0));
                    resp.request_focus();
                    if resp.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let new_n = new_name.clone();
                        *action = Some(FileTreeAction::Rename(entry.path.clone(), new_n));
                        *rename_state = None;
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        *rename_state = None;
                    }
                }
                return;
            }

            if entry.is_dir {
                let arrow = if entry.is_expanded {
                    ph::CARET_DOWN
                } else {
                    ph::CARET_RIGHT
                };
                let folder_icon = if entry.is_expanded {
                    ph::FOLDER_OPEN
                } else {
                    ph::FOLDER
                };
                let color = egui::Color32::from_rgb(220, 180, 100);
                let label =
                    egui::RichText::new(format!("{} {} {}", arrow, folder_icon, entry.name))
                        .color(color);
                let resp = ui.selectable_label(false, label);
                if resp.clicked() {
                    entry.is_expanded = !entry.is_expanded;
                    if entry.is_expanded && entry.children.is_empty() {
                        entry.load_children();
                    }
                }
                resp.context_menu(|ui| {
                    if ui.button("New File").clicked() {
                        *action = Some(FileTreeAction::NewFile(entry.path.clone()));
                        ui.close_menu();
                    }
                    if ui.button("New Folder").clicked() {
                        *action = Some(FileTreeAction::NewFolder(entry.path.clone()));
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Rename").clicked() {
                        *rename_state = Some((entry.path.clone(), entry.name.clone()));
                        ui.close_menu();
                    }
                    if ui.button("Delete Folder").clicked() {
                        *action = Some(FileTreeAction::Delete(entry.path.clone()));
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Copy Path").clicked() {
                        *action = Some(FileTreeAction::CopyPath(entry.path.clone()));
                        ui.close_menu();
                    }
                    if ui.button("Reveal in File Manager").clicked() {
                        *action = Some(FileTreeAction::RevealInExplorer(entry.path.clone()));
                        ui.close_menu();
                    }
                });
            } else {
                let (icon, color) = file_icon(&entry.name);
                let is_selected = selected.as_ref().map(|s| s == &entry.path).unwrap_or(false);
                let icon_label = egui::RichText::new(icon).color(color);
                ui.label(icon_label);
                let resp = ui.selectable_label(
                    is_selected,
                    egui::RichText::new(&entry.name).color(egui::Color32::from_rgb(212, 212, 212)),
                );
                if resp.clicked() {
                    *selected = Some(entry.path.clone());
                    *opened = Some(entry.path.clone());
                }
                resp.context_menu(|ui| {
                    if ui.button("Open").clicked() {
                        *action = Some(FileTreeAction::OpenFile(entry.path.clone()));
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Rename").clicked() {
                        *rename_state = Some((entry.path.clone(), entry.name.clone()));
                        ui.close_menu();
                    }
                    if ui.button("Delete File").clicked() {
                        *action = Some(FileTreeAction::Delete(entry.path.clone()));
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Copy Path").clicked() {
                        *action = Some(FileTreeAction::CopyPath(entry.path.clone()));
                        ui.close_menu();
                    }
                    if ui.button("Reveal in File Manager").clicked() {
                        *action = Some(FileTreeAction::RevealInExplorer(entry.path.clone()));
                        ui.close_menu();
                    }
                });
            }
        });
        let _ = row_resp;

        if entry.is_dir && entry.is_expanded {
            for child in &mut entry.children {
                Self::show_entry_recursive(ui, child, selected, opened, action, rename_state);
            }
        }
    }
}
