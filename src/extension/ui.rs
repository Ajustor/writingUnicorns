use super::installer::{InstallJob, InstallStatus};
use super::registry::ExtensionRegistry;

pub struct ExtensionsPanel {
    pub search_query: String,
    pub install_url: String,
    pub install_job: Option<std::sync::mpsc::Receiver<InstallStatus>>,
    pub install_status: InstallStatus,
    pub local_install_job: Option<std::sync::mpsc::Receiver<InstallStatus>>,
    pub local_install_status: InstallStatus,
    pub generate_name: String,
    pub generate_path: String,
    pub template_message: Option<Result<String, String>>,
}

impl ExtensionsPanel {
    pub fn new() -> Self {
        let default_path = dirs_next::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("extensions")
            .to_string_lossy()
            .to_string();
        Self {
            search_query: String::new(),
            install_url: String::new(),
            install_job: None,
            install_status: InstallStatus::Idle,
            local_install_job: None,
            local_install_status: InstallStatus::Idle,
            generate_name: String::new(),
            generate_path: default_path,
            template_message: None,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, registry: &mut ExtensionRegistry) {
        // Poll install job
        if let Some(rx) = &self.install_job {
            if let Ok(status) = rx.try_recv() {
                let done = matches!(status, InstallStatus::Done | InstallStatus::Failed(_));
                if matches!(status, InstallStatus::Done) {
                    self.install_url.clear();
                }
                self.install_status = status;
                if done {
                    self.install_job = None;
                    registry.load_installed();
                }
            }
        }

        ui.vertical(|ui| {
            // Search bar
            ui.horizontal(|ui| {
                ui.label("🔍");
                ui.add(
                    egui::TextEdit::singleline(&mut self.search_query)
                        .hint_text("Search extensions…")
                        .desired_width(f32::INFINITY),
                );
            });
            ui.add_space(6.0);

            // ── INSTALLED ────────────────────────────────────────────────────
            ui.collapsing("INSTALLED", |ui| {
                let query = self.search_query.to_lowercase();
                let mut to_uninstall: Option<String> = None;

                let has_matches = registry.installed.iter().any(|e| {
                    query.is_empty()
                        || e.manifest.extension.name.to_lowercase().contains(&query)
                        || e.manifest.extension.id.to_lowercase().contains(&query)
                });

                if !has_matches {
                    ui.add_space(12.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("🧩")
                                .size(32.0)
                                .color(egui::Color32::from_gray(100)),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new("No extensions installed yet.")
                                .color(egui::Color32::from_gray(140)),
                        );
                        ui.label(
                            egui::RichText::new("Install one from Git or create your own below.")
                                .size(11.0)
                                .color(egui::Color32::from_gray(100)),
                        );
                    });
                    ui.add_space(12.0);
                } else {
                    let ids: Vec<String> = registry
                        .installed
                        .iter()
                        .filter(|e| {
                            query.is_empty()
                                || e.manifest.extension.name.to_lowercase().contains(&query)
                                || e.manifest.extension.id.to_lowercase().contains(&query)
                        })
                        .map(|e| e.manifest.extension.id.clone())
                        .collect();

                    for (i, id) in ids.iter().enumerate() {
                        if i > 0 {
                            ui.separator();
                        }
                        let ext = registry
                            .installed
                            .iter_mut()
                            .find(|e| &e.manifest.extension.id == id);
                        let Some(ext) = ext else { continue };

                        ui.group(|ui| {
                            ui.set_width(ui.available_width());

                            // Name + version row
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(&ext.manifest.extension.name)
                                        .strong()
                                        .color(egui::Color32::WHITE),
                                );
                                ui.label(
                                    egui::RichText::new(format!(
                                        "v{}",
                                        ext.manifest.extension.version
                                    ))
                                    .small()
                                    .color(egui::Color32::from_gray(140)),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let uninstall_btn = ui.add(
                                            egui::Button::new(
                                                egui::RichText::new("Uninstall")
                                                    .color(egui::Color32::from_rgb(240, 80, 80)),
                                            )
                                            .frame(false),
                                        );
                                        if uninstall_btn.clicked() {
                                            to_uninstall = Some(ext.manifest.extension.id.clone());
                                        }
                                    },
                                );
                            });

                            // Description
                            if !ext.manifest.extension.description.is_empty() {
                                ui.label(
                                    egui::RichText::new(&ext.manifest.extension.description)
                                        .size(11.0)
                                        .color(egui::Color32::from_gray(160)),
                                );
                            }

                            // Author + repo row
                            ui.horizontal(|ui| {
                                if !ext.manifest.extension.author.is_empty() {
                                    ui.label(
                                        egui::RichText::new(&ext.manifest.extension.author)
                                            .size(11.0)
                                            .color(egui::Color32::from_gray(120)),
                                    );
                                }
                                if !ext.manifest.extension.repository.is_empty() {
                                    ui.hyperlink_to(
                                        egui::RichText::new("repository")
                                            .size(11.0)
                                            .color(egui::Color32::from_rgb(100, 160, 220)),
                                        &ext.manifest.extension.repository,
                                    );
                                }
                            });

                            // Enable/disable
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut ext.enabled, "Enabled");
                            });
                        });
                    }
                }

                if let Some(id) = to_uninstall {
                    if let Err(e) = registry.uninstall(&id) {
                        log::error!("Uninstall failed: {e}");
                    }
                }
            });

            ui.add_space(8.0);

            // ── INSTALL FROM GIT ─────────────────────────────────────────────
            ui.collapsing("INSTALL FROM GIT", |ui| {
                ui.label("Install from Git Repository");
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.install_url)
                            .hint_text("https://github.com/user/extension")
                            .desired_width(ui.available_width() - 70.0),
                    );
                    let installing = self.install_job.is_some();
                    let install_btn = ui.add_enabled(
                        !installing && !self.install_url.is_empty(),
                        egui::Button::new(
                            egui::RichText::new("Install").color(egui::Color32::WHITE),
                        )
                        .fill(egui::Color32::from_rgb(0, 120, 212)),
                    );
                    if install_btn.clicked() {
                        let rx = InstallJob::start(
                            self.install_url.clone(),
                            ExtensionRegistry::extensions_dir(),
                        );
                        self.install_job = Some(rx);
                        self.install_status = InstallStatus::Cloning;
                    }
                });

                let (status_text, is_error) = match &self.install_status {
                    InstallStatus::Idle => (String::new(), false),
                    InstallStatus::Cloning => ("Cloning…".to_string(), false),
                    InstallStatus::Building => ("Building…".to_string(), false),
                    InstallStatus::Installing => ("Installing…".to_string(), false),
                    InstallStatus::Done => ("✓ Installed successfully!".to_string(), false),
                    InstallStatus::Failed(e) => (format!("Error: {e}"), true),
                };

                if !status_text.is_empty() {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if self.install_job.is_some() {
                            ui.spinner();
                        }
                        let color = if is_error {
                            egui::Color32::from_rgb(240, 80, 80)
                        } else if matches!(self.install_status, InstallStatus::Done) {
                            egui::Color32::from_rgb(100, 200, 100)
                        } else {
                            egui::Color32::from_gray(180)
                        };
                        ui.label(egui::RichText::new(status_text).color(color));
                    });
                }
            });

            ui.add_space(8.0);

            // ── LOAD FROM LOCAL FOLDER ────────────────────────────────────────
            ui.collapsing("📁 Load from local folder", |ui| {
                ui.label(
                    egui::RichText::new(
                        "Load an extension from a local directory.\nThe folder must contain a manifest.toml.",
                    )
                    .size(11.0)
                    .color(egui::Color32::GRAY),
                );
                ui.add_space(4.0);

                if ui.button("Browse folder…").clicked() {
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        let ext_dir = registry.extensions_dir.clone();
                        self.local_install_job = Some(
                            crate::extension::installer::install_from_folder(folder, ext_dir),
                        );
                        self.local_install_status = InstallStatus::Installing;
                    }
                }

                // Poll local install job
                if let Some(rx) = &self.local_install_job {
                    if let Ok(status) = rx.try_recv() {
                        let done = matches!(status, InstallStatus::Done | InstallStatus::Failed(_));
                        self.local_install_status = status.clone();
                        if done {
                            self.local_install_job = None;
                            if matches!(status, InstallStatus::Done) {
                                registry.load_installed();
                            }
                        }
                    }
                }

                match &self.local_install_status {
                    InstallStatus::Idle => {}
                    InstallStatus::Building => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Building…");
                        });
                    }
                    InstallStatus::Installing => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Installing…");
                        });
                    }
                    InstallStatus::Done => {
                        ui.label(
                            egui::RichText::new("✓ Installed!")
                                .color(egui::Color32::from_rgb(100, 200, 100)),
                        );
                    }
                    InstallStatus::Failed(e) => {
                        ui.label(
                            egui::RichText::new(format!("✗ {e}"))
                                .color(egui::Color32::from_rgb(240, 80, 80)),
                        );
                    }
                    _ => {}
                }
            });

            ui.add_space(8.0);
            ui.collapsing("CREATE EXTENSION", |ui| {
                ui.label("Create Extension Template");
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.generate_name)
                            .hint_text("my-extension")
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.label("Path:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.generate_path)
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.add_space(4.0);

                if ui
                    .add_enabled(
                        !self.generate_name.is_empty() && !self.generate_path.is_empty(),
                        egui::Button::new("Generate"),
                    )
                    .clicked()
                {
                    let output_dir = std::path::PathBuf::from(&self.generate_path);
                    match super::template::generate_extension_template(
                        &self.generate_name,
                        &output_dir,
                    ) {
                        Ok(()) => {
                            let path = output_dir
                                .join(&self.generate_name)
                                .to_string_lossy()
                                .to_string();
                            self.template_message = Some(Ok(path));
                        }
                        Err(e) => {
                            self.template_message = Some(Err(e.to_string()));
                        }
                    }
                }

                if let Some(result) = &self.template_message {
                    ui.add_space(4.0);
                    match result {
                        Ok(path) => {
                            ui.label(
                                egui::RichText::new(format!("✓ Created at {path}"))
                                    .color(egui::Color32::from_rgb(100, 200, 100)),
                            );
                        }
                        Err(e) => {
                            ui.label(
                                egui::RichText::new(format!("Error: {e}"))
                                    .color(egui::Color32::from_rgb(240, 80, 80)),
                            );
                        }
                    }
                    if ui.button("Dismiss").clicked() {
                        self.template_message = None;
                    }
                }
            });
        });
    }
}

impl Default for ExtensionsPanel {
    fn default() -> Self {
        Self::new()
    }
}
