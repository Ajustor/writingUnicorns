use crate::app::WritingUnicorns;
use crate::config::Config;
use crate::terminal::Terminal;
use crate::ui::run_panel::RunPanelAction;
use egui::{
    Align, CentralPanel, Color32, Context, Layout, RichText, SidePanel, Stroke, TopBottomPanel,
};

#[derive(Debug, Clone, PartialEq, Default)]
pub enum SidebarTab {
    #[default]
    Explorer,
    Search,
    Git,
    Extensions,
    Run,
}

pub fn render(app: &mut WritingUnicorns, ctx: &Context) {
    ctx.set_visuals(dark_visuals(&app.config));

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
                if ui.button("Settings   Ctrl+,").clicked() {
                    app.tab_manager.open_settings();
                    app.settings_panel.open = true;
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
        let panel_response = TopBottomPanel::bottom("terminal_panel")
            .resizable(true)
            .min_height(80.0)
            .default_height(app.terminal_height)
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(
                        app.config.theme.background[0],
                        app.config.theme.background[1],
                        app.config.theme.background[2],
                    ))
                    .inner_margin(egui::Margin::ZERO),
            )
            .show_separator_line(false)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

                // Tab bar — one tab per terminal instance
                let tab_bg = egui::Color32::from_rgb(
                    app.config.theme.background[0].saturating_add(7),
                    app.config.theme.background[1].saturating_add(7),
                    app.config.theme.background[2].saturating_add(7),
                );
                let tab_height = 35.0;
                let (tab_rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), tab_height),
                    egui::Sense::hover(),
                );
                ui.painter().rect_filled(tab_rect, 0.0, tab_bg);

                let mut tab_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(tab_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                );

                tab_ui.add_space(8.0);

                // Panel icon + label
                tab_ui.label(
                    egui::RichText::new(format!("{} TERMINAL", egui_phosphor::regular::TERMINAL))
                        .size(11.0)
                        .color(egui::Color32::from_gray(150)),
                );

                tab_ui.add_space(6.0);
                tab_ui.separator();
                tab_ui.add_space(4.0);

                // One clickable tab per terminal
                let mut close_idx: Option<usize> = None;
                for i in 0..app.terminals.len() {
                    let is_active = i == app.active_terminal;
                    let shell = app.terminals[i].shell_name.clone();
                    let tab_color = if is_active {
                        egui::Color32::WHITE
                    } else {
                        egui::Color32::from_gray(140)
                    };
                    let active_bar = egui::Color32::from_rgb(
                        app.config.theme.accent[0],
                        app.config.theme.accent[1],
                        app.config.theme.accent[2],
                    );

                    // Draw tab background if active
                    let tab_label_response = tab_ui.add(
                        egui::Button::new(
                            egui::RichText::new(format!(
                                "{} {}",
                                egui_phosphor::regular::TERMINAL,
                                shell
                            ))
                            .size(11.0)
                            .color(tab_color),
                        )
                        .frame(false)
                        .selected(is_active),
                    );
                    if tab_label_response.clicked() {
                        app.active_terminal = i;
                    }
                    // Draw active top-border indicator
                    if is_active {
                        let r = tab_label_response.rect;
                        tab_ui.painter().line_segment(
                            [r.left_top(), r.right_top()],
                            egui::Stroke::new(2.0, active_bar),
                        );
                    }

                    // Close button (only show if >1 terminal)
                    if app.terminals.len() > 1
                        && tab_ui
                            .add(
                                egui::Button::new(egui::RichText::new("×").size(12.0)).frame(false),
                            )
                            .clicked()
                    {
                        close_idx = Some(i);
                    }
                    tab_ui.add_space(4.0);
                }

                // Remove closed terminal after the loop
                if let Some(idx) = close_idx {
                    app.terminals.remove(idx);
                    if app.active_terminal >= app.terminals.len() {
                        app.active_terminal = app.terminals.len().saturating_sub(1);
                    }
                }

                // New terminal (+) button
                if tab_ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(egui_phosphor::regular::PLUS).size(14.0),
                        )
                        .frame(false),
                    )
                    .on_hover_text("New terminal")
                    .clicked()
                {
                    app.terminals.push(Terminal::new());
                    app.active_terminal = app.terminals.len() - 1;
                }

                // Right-aligned close-panel button
                let available = tab_ui.available_width();
                tab_ui.add_space((available - 30.0).max(0.0));
                if tab_ui
                    .add(egui::Button::new(egui::RichText::new("×").size(16.0)).frame(false))
                    .on_hover_text("Close terminal panel")
                    .clicked()
                {
                    app.show_terminal = false;
                }

                // Terminal content
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                if let Some(term) = app.terminals.get_mut(app.active_terminal) {
                    term.show_content(ui, &app.config);
                }
            });

        // Persist the panel height so it survives hide/show cycles and app restarts.
        let new_height = panel_response.response.rect.height();
        if (new_height - app.terminal_height).abs() > 0.5 {
            app.terminal_height = new_height;
            app.config.terminal_height = new_height;
            app.config.save();
        }
    }

    // Activity bar (always visible, far left)
    {
        let accent = Color32::from_rgb(
            app.config.theme.accent[0],
            app.config.theme.accent[1],
            app.config.theme.accent[2],
        );
        let bar_bg = Color32::from_rgb(
            app.config.theme.background[0].saturating_sub(5),
            app.config.theme.background[1].saturating_sub(5),
            app.config.theme.background[2].saturating_sub(5),
        );
        let hover_bg = Color32::from_rgb(
            app.config.theme.background[0].saturating_add(20),
            app.config.theme.background[1].saturating_add(20),
            app.config.theme.background[2].saturating_add(20),
        );

        SidePanel::left("activity_bar")
            .exact_width(48.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(bar_bg)
                    .inner_margin(egui::Margin::ZERO),
            )
            .show_separator_line(false)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

                struct ActivityItem {
                    icon: &'static str,
                    tooltip: &'static str,
                    tab: SidebarTab,
                }

                let items = [
                    ActivityItem {
                        icon: egui_phosphor::regular::FILES,
                        tooltip: "Explorer",
                        tab: SidebarTab::Explorer,
                    },
                    ActivityItem {
                        icon: egui_phosphor::regular::MAGNIFYING_GLASS,
                        tooltip: "Search",
                        tab: SidebarTab::Search,
                    },
                    ActivityItem {
                        icon: egui_phosphor::regular::GIT_BRANCH,
                        tooltip: "Git",
                        tab: SidebarTab::Git,
                    },
                    ActivityItem {
                        icon: egui_phosphor::regular::PUZZLE_PIECE,
                        tooltip: "Extensions",
                        tab: SidebarTab::Extensions,
                    },
                    ActivityItem {
                        icon: egui_phosphor::regular::PLAY,
                        tooltip: "Run",
                        tab: SidebarTab::Run,
                    },
                ];

                for item in &items {
                    let is_active = app.show_sidebar && app.sidebar_tab == item.tab;

                    // Allocate space first, then paint bg, then icon on top
                    let (rect, response) =
                        ui.allocate_exact_size(egui::vec2(48.0, 48.0), egui::Sense::click());
                    let response = response.on_hover_text(item.tooltip);

                    let painter = ui.painter();

                    // Hover/active background (drawn first, under the icon)
                    if response.hovered() {
                        painter.rect_filled(rect, 0.0, hover_bg);
                    }

                    // Active left border
                    if is_active {
                        painter.line_segment(
                            [rect.left_top(), rect.left_bottom()],
                            Stroke::new(2.0, accent),
                        );
                    }

                    // Icon drawn on top
                    let icon_color = if is_active {
                        Color32::WHITE
                    } else if response.hovered() {
                        Color32::from_gray(220)
                    } else {
                        Color32::from_gray(160)
                    };
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        item.icon,
                        egui::FontId::proportional(22.0),
                        icon_color,
                    );

                    if response.clicked() {
                        if app.show_sidebar && app.sidebar_tab == item.tab {
                            app.show_sidebar = false;
                        } else {
                            app.show_sidebar = true;
                            app.sidebar_tab = item.tab.clone();
                        }
                    }
                }

                // Bottom-aligned settings gear
                ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                    let (rect, response) =
                        ui.allocate_exact_size(egui::vec2(48.0, 48.0), egui::Sense::click());
                    let response = response.on_hover_text("Settings");
                    let painter = ui.painter();
                    if response.hovered() {
                        painter.rect_filled(rect, 0.0, hover_bg);
                    }
                    let gear_color = if response.hovered() {
                        Color32::from_gray(220)
                    } else {
                        Color32::from_gray(160)
                    };
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        egui_phosphor::regular::GEAR,
                        egui::FontId::proportional(22.0),
                        gear_color,
                    );
                    if response.clicked() {
                        app.tab_manager.open_settings();
                        app.settings_panel.open = true;
                    }
                });
            });
    }

    if app.show_sidebar {
        SidePanel::left("sidebar")
            .resizable(true)
            .min_width(150.0)
            .default_width(app.sidebar_width)
            .show(ctx, |ui| {
                let section_title = match app.sidebar_tab {
                    SidebarTab::Explorer => "EXPLORER",
                    SidebarTab::Search => "SEARCH",
                    SidebarTab::Git => "GIT",
                    SidebarTab::Extensions => "EXTENSIONS",
                    SidebarTab::Run => "RUN",
                };
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(section_title)
                            .size(11.0)
                            .color(Color32::from_gray(150))
                            .strong(),
                    );
                });
                ui.add_space(2.0);

                match app.sidebar_tab {
                    SidebarTab::Explorer => {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            if let Some(path) = app.file_tree.show(ui) {
                                app.open_file(path);
                            }
                        });
                    }
                    SidebarTab::Search => {
                        if let Some((path, line)) =
                            app.workspace_search.show(ui, app.workspace_path.as_ref())
                        {
                            app.open_file_at_line(path, line);
                        }
                    }
                    SidebarTab::Git => {
                        app.git_status.show(ui);
                    }
                    SidebarTab::Extensions => {
                        app.extensions_panel.show(ui, &mut app.extension_registry);
                    }
                    SidebarTab::Run => {
                        let is_running = app.runner.is_running;
                        let action: RunPanelAction = app.run_panel.show(
                            ui,
                            &mut app.runner,
                            app.workspace_path.as_ref(),
                            app.editor.current_path.as_ref(),
                            is_running,
                        );
                        if action.run_clicked {
                            app.run_active_config();
                        }
                        if action.stop_clicked {
                            app.runner.is_running = false;
                            if let Some(term) = app.terminals.get_mut(app.active_terminal) {
                                term.send_input("\x03");
                            }
                        }
                    }
                }
            });
    }

    // Use a zero-margin frame so there's no gap/padding around the editor area
    CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(egui::Color32::from_rgb(
                    app.config.theme.background[0],
                    app.config.theme.background[1],
                    app.config.theme.background[2],
                ))
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
                ui.painter().rect_filled(
                    tab_rect,
                    0.0,
                    egui::Color32::from_rgb(
                        app.config.theme.background[0].saturating_add(7),
                        app.config.theme.background[1].saturating_add(7),
                        app.config.theme.background[2].saturating_add(7),
                    ),
                );
                let mut tab_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(tab_rect)
                        .layout(*ui.layout()),
                );
                tab_ui.spacing_mut().item_spacing = egui::vec2(4.0, 0.0);
                if let Some(path) = app.tab_manager.show(&mut tab_ui) {
                    app.open_file(path);
                }
                // Keep settings_panel.open in sync with whether a settings tab exists
                app.settings_panel.open = app.tab_manager.tabs.iter().any(|t| t.is_settings);
            }

            let active_is_settings = app
                .tab_manager
                .active_tab
                .and_then(|id| app.tab_manager.tabs.iter().find(|t| t.id == id))
                .map(|t| t.is_settings)
                .unwrap_or(false);

            if active_is_settings {
                if app.settings_panel.show_inline(ui, &mut app.config) {
                    app.config.save();
                }
            } else if app.editor.current_path.is_some() || !app.editor.buffer.to_string().is_empty()
            {
                app.editor.workspace_path = app.workspace_path.clone();
                let lsp_hover = app.lsp_hover_result.take();
                app.editor.show(ui, &app.config, &app.plugin_manager, lsp_hover);
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

fn dark_visuals(config: &Config) -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    let bg = egui::Color32::from_rgb(
        config.theme.background[0],
        config.theme.background[1],
        config.theme.background[2],
    );
    let fg = egui::Color32::from_rgb(
        config.theme.foreground[0],
        config.theme.foreground[1],
        config.theme.foreground[2],
    );
    let accent = egui::Color32::from_rgb(
        config.theme.accent[0],
        config.theme.accent[1],
        config.theme.accent[2],
    );
    v.panel_fill = egui::Color32::from_rgb(
        config.theme.background[0].saturating_add(7),
        config.theme.background[1].saturating_add(7),
        config.theme.background[2].saturating_add(7),
    );
    v.window_fill = bg;
    v.override_text_color = Some(fg);
    v.selection.bg_fill =
        egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 80);
    v.selection.stroke = egui::Stroke::new(1.0, accent);
    v.hyperlink_color = accent;
    v.widgets.inactive.weak_bg_fill = egui::Color32::from_rgb(
        config.theme.background[0].saturating_add(15),
        config.theme.background[1].saturating_add(15),
        config.theme.background[2].saturating_add(15),
    );
    v.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(
        config.theme.background[0].saturating_add(30),
        config.theme.background[1].saturating_add(30),
        config.theme.background[2].saturating_add(30),
    );
    v
}
