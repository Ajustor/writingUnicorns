use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Tab {
    pub id: usize,
    pub path: PathBuf,
    pub title: String,
    pub is_modified: bool,
    pub is_settings: bool,
}

pub struct TabManager {
    pub tabs: Vec<Tab>,
    pub active_tab: Option<usize>,
    next_id: usize,
}

impl TabManager {
    pub fn new() -> Self {
        Self {
            tabs: vec![],
            active_tab: None,
            next_id: 0,
        }
    }

    pub fn open(&mut self, path: PathBuf, _content: String) -> usize {
        if let Some(tab) = self.tabs.iter().find(|t| t.path == path) {
            let id = tab.id;
            self.active_tab = Some(id);
            return id;
        }
        let title = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled".to_string());
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.push(Tab {
            id,
            path,
            title,
            is_modified: false,
            is_settings: false,
        });
        self.active_tab = Some(id);
        id
    }

    pub fn open_untitled(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        let title = format!("untitled-{}", id + 1);
        let path = PathBuf::from(format!("untitled-{}", id + 1));
        self.tabs.push(Tab {
            id,
            path,
            title,
            is_modified: true,
            is_settings: false,
        });
        self.active_tab = Some(id);
        id
    }

    pub fn open_settings(&mut self) -> usize {
        if let Some(existing) = self.tabs.iter().find(|t| t.is_settings) {
            let id = existing.id;
            self.active_tab = Some(id);
            return id;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.push(Tab {
            id,
            path: PathBuf::from("__settings__"),
            title: "Settings".to_string(),
            is_modified: false,
            is_settings: true,
        });
        self.active_tab = Some(id);
        id
    }

    pub fn close(&mut self, id: usize) {
        self.tabs.retain(|t| t.id != id);
        if self.active_tab == Some(id) {
            self.active_tab = self.tabs.last().map(|t| t.id);
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<PathBuf> {
        let mut to_open: Option<PathBuf> = None;
        let mut activate_id: Option<usize> = None;
        let mut to_close: Option<usize> = None;
        let active_tab = self.active_tab;

        let tabs_data: Vec<(usize, PathBuf, String, bool, bool)> = self
            .tabs
            .iter()
            .map(|t| {
                (
                    t.id,
                    t.path.clone(),
                    t.title.clone(),
                    t.is_modified,
                    t.is_settings,
                )
            })
            .collect();

        ui.horizontal(|ui| {
            ui.style_mut().spacing.item_spacing.x = 0.0;
            for (tab_id, tab_path, tab_title, tab_modified, tab_is_settings) in &tabs_data {
                let is_active = active_tab == Some(*tab_id);
                let bg = if is_active {
                    egui::Color32::from_rgb(30, 30, 30)
                } else {
                    egui::Color32::from_rgb(45, 45, 45)
                };
                egui::Frame::new().fill(bg).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if *tab_is_settings {
                            ui.label(
                                egui::RichText::new(egui_phosphor::regular::GEAR)
                                    .color(egui::Color32::from_gray(160))
                                    .size(14.0),
                            );
                        } else {
                            let (icon, icon_color) = crate::filetree::file_icon(tab_title);
                            ui.label(egui::RichText::new(icon).color(icon_color).size(14.0));
                        }
                        ui.add_space(2.0);
                        let tab_label = if *tab_modified {
                            egui::RichText::new(format!("● {}", tab_title))
                                .color(egui::Color32::from_rgb(255, 180, 50))
                        } else {
                            egui::RichText::new(tab_title.as_str()).color(if is_active {
                                egui::Color32::WHITE
                            } else {
                                egui::Color32::from_rgb(160, 160, 160)
                            })
                        };
                        if ui.selectable_label(is_active, tab_label).clicked() {
                            if *tab_is_settings {
                                activate_id = Some(*tab_id);
                            } else {
                                to_open = Some(tab_path.clone());
                            }
                        }
                        if ui.small_button("×").clicked() {
                            to_close = Some(*tab_id);
                        }
                    });
                });
                ui.separator();
            }
        });

        if let Some(id) = activate_id {
            self.active_tab = Some(id);
        }

        if let Some(ref path) = to_open {
            if let Some(tab) = self.tabs.iter().find(|t| &t.path == path) {
                self.active_tab = Some(tab.id);
            }
        }

        if let Some(id) = to_close {
            let was_active = self.active_tab == Some(id);
            self.close(id);
            // If the active tab was closed, load the new active tab's file
            if was_active && to_open.is_none() {
                to_open = self
                    .active_tab
                    .and_then(|new_id| self.tabs.iter().find(|t| t.id == new_id))
                    .map(|t| t.path.clone());
            }
        }
        to_open
    }
}
