use crate::app::WritingUnicorns;
use egui::{CentralPanel, Context, SidePanel, TopBottomPanel};

#[derive(Debug, Clone, PartialEq, Default)]
pub enum SidebarTab {
    #[default]
    Explorer,
    Git,
}

pub fn render(app: &mut WritingUnicorns, ctx: &Context) {
    ctx.set_visuals(dark_visuals());

    // Drain pending folder/file picked by dialog threads
    if let Some(rx) = app.folder_pending.take() {
        match rx.try_recv() {
            Ok(path) => app.open_folder(path),
            Err(std::sync::mpsc::TryRecvError::Empty) => app.folder_pending = Some(rx),
            Err(_) => {}
        }
    }
    if let Some(rx) = app.file_pending.take() {
        match rx.try_recv() {
            Ok(path) => app.open_file(path),
            Err(std::sync::mpsc::TryRecvError::Empty) => app.file_pending = Some(rx),
            Err(_) => {}
        }
    }

    // Sync editor modified state → active tab
    if let Some(active_id) = app.tab_manager.active_tab {
        if let Some(tab) = app.tab_manager.tabs.iter_mut().find(|t| t.id == active_id) {
            tab.is_modified = app.editor.is_modified;
        }
    }

    // Update window title to show modified indicator (● prefix)
    {
        let filename = app
            .editor
            .current_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Writing Unicorns".to_string());
        let title = if app.editor.is_modified {
            format!("● {} — Writing Unicorns", filename)
        } else {
            format!("{} — Writing Unicorns", filename)
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
    }

    TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New File          Ctrl+N").clicked() {
                    app.open_new_file();
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Open Folder…     Ctrl+O").clicked() {
                    app.folder_pending = Some(app.trigger_open_folder());
                    ui.close_menu();
                }
                if ui.button("Open File…  Ctrl+Shift+O").clicked() {
                    app.file_pending = Some(app.trigger_open_file());
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Save              Ctrl+S").clicked() {
                    let _ = app.editor.save();
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });

            ui.menu_button("View", |ui| {
                if ui.button("Toggle Sidebar  Ctrl+B").clicked() {
                    app.show_sidebar = !app.show_sidebar;
                    ui.close_menu();
                }
                if ui.button("Toggle Terminal  Ctrl+`").clicked() {
                    app.show_terminal = !app.show_terminal;
                    ui.close_menu();
                }
                if ui.button("Command Palette  Ctrl+P").clicked() {
                    app.command_palette.toggle();
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Keyboard Shortcuts  F1").clicked() {
                    app.shortcuts_help.toggle();
                    ui.close_menu();
                }
            });

            ui.menu_button("Git", |ui| {
                if ui.button("Refresh Status").clicked() {
                    app.git_status.refresh();
                    ui.close_menu();
                }
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format!("⎇ {}", app.git_status.branch))
                        .color(egui::Color32::from_rgb(150, 200, 150))
                        .small(),
                );
            });
        });
    });

    TopBottomPanel::bottom("status_bar")
        .exact_height(22.0)
        .show(ctx, |ui| {
            app.status_bar.show(ui, &app.editor, &app.git_status);
        });

    if app.show_terminal {
        TopBottomPanel::bottom("terminal_panel")
            .resizable(true)
            .min_height(80.0)
            .default_height(app.terminal_height)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("TERMINAL")
                            .small()
                            .strong()
                            .color(egui::Color32::GRAY),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("×").clicked() {
                            app.show_terminal = false;
                        }
                    });
                });
                ui.separator();
                app.terminal.show(ui);
            });
    }

    if app.show_sidebar {
        SidePanel::left("sidebar")
            .resizable(true)
            .min_width(150.0)
            .default_width(app.sidebar_width)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let explorer_label = egui::RichText::new(format!(
                        "{} Explorer",
                        egui_phosphor::regular::FOLDER_SIMPLE
                    ));
                    let git_label =
                        egui::RichText::new(format!("{} Git", egui_phosphor::regular::GIT_BRANCH));
                    ui.selectable_value(&mut app.sidebar_tab, SidebarTab::Explorer, explorer_label);
                    ui.selectable_value(&mut app.sidebar_tab, SidebarTab::Git, git_label);
                });
                ui.separator();

                match app.sidebar_tab {
                    SidebarTab::Explorer => {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            if let Some(path) = app.file_tree.show(ui) {
                                app.open_file(path);
                            }
                        });
                    }
                    SidebarTab::Git => {
                        app.git_status.show(ui);
                    }
                }
            });
    }

    // Use a zero-margin frame so there's no gap/padding around the editor area
    CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(egui::Color32::from_rgb(30, 30, 30))
                .inner_margin(egui::Margin::ZERO),
        )
        .show(ctx, |ui| {
            // Remove default item spacing to avoid gaps between tab bar and editor
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

            if !app.tab_manager.tabs.is_empty() {
                // Draw the tab bar background explicitly to fill the full allocated height
                let (tab_rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 32.0),
                    egui::Sense::hover(),
                );
                ui.painter()
                    .rect_filled(tab_rect, 0.0, egui::Color32::from_rgb(37, 37, 38));
                let mut tab_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(tab_rect)
                        .layout(*ui.layout()),
                );
                tab_ui.spacing_mut().item_spacing = egui::vec2(4.0, 0.0);
                if let Some(path) = app.tab_manager.show(&mut tab_ui) {
                    app.open_file(path);
                }
            }

            if app.editor.current_path.is_some() || !app.editor.buffer.to_string().is_empty() {
                app.editor.show(ui, &app.config);
            } else {
                welcome_screen(ui);
            }
        });
}

fn welcome_screen(ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(80.0);
        ui.label(
            egui::RichText::new("🦄 Writing Unicorns")
                .size(32.0)
                .color(egui::Color32::from_rgb(180, 130, 255))
                .strong(),
        );
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new("A lightweight IDE")
                .size(16.0)
                .color(egui::Color32::GRAY),
        );
        ui.add_space(40.0);
        ui.label(egui::RichText::new("Ctrl+P  — Command Palette").color(egui::Color32::GRAY));
        ui.label(egui::RichText::new("Ctrl+B  — Toggle Sidebar").color(egui::Color32::GRAY));
        ui.label(egui::RichText::new("Ctrl+`  — Toggle Terminal").color(egui::Color32::GRAY));
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new("File → Open Folder to get started")
                .color(egui::Color32::from_rgb(150, 200, 150)),
        );
    });
}

fn dark_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.panel_fill = egui::Color32::from_rgb(37, 37, 38);
    v.window_fill = egui::Color32::from_rgb(30, 30, 30);
    v.override_text_color = Some(egui::Color32::from_rgb(212, 212, 212));
    v
}
