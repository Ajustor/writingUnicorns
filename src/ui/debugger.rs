use std::path::PathBuf;

use egui::{Color32, RichText, ScrollArea};

use crate::dap::manager::DapManager;
use crate::dap::types::DebugSessionState;

pub struct DebuggerPanel {
    pub open: bool,
}

impl DebuggerPanel {
    pub fn new() -> Self {
        Self { open: false }
    }
}

/// Action requested by the debugger panel UI.
#[derive(Default)]
pub struct DebugPanelAction {
    /// User clicked "Start" or "Continue".
    pub start_or_continue: bool,
    /// User clicked "Stop".
    pub stop: bool,
    /// User clicked "Step Over".
    pub step_over: bool,
    /// User clicked "Step In".
    pub step_in: bool,
    /// User clicked "Step Out".
    pub step_out: bool,
    /// User clicked "Pause".
    pub pause: bool,
    /// Navigate to this file/line (e.g. from call stack click).
    pub navigate_to: Option<(PathBuf, usize)>,
}

impl DebuggerPanel {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        dap: &DapManager,
    ) -> DebugPanelAction {
        let mut action = DebugPanelAction::default();
        let state = dap.session_state();
        let is_active = dap.is_active();
        let is_paused = dap.is_paused();
        let is_running = dap.is_running();

        // ── Toolbar ──────────────────────────────────────────────────────────
        ui.horizontal(|ui| {
            let start_label = if is_paused { "▶ Continue (F5)" } else { "▶ Start (F5)" };
            let start_enabled = !is_active || is_paused;
            if ui.add_enabled(start_enabled, egui::Button::new(start_label)).clicked() {
                action.start_or_continue = true;
            }
            if ui.add_enabled(is_active, egui::Button::new("⏸ Pause")).clicked() {
                action.pause = true;
            }
            if ui.add_enabled(is_paused, egui::Button::new("⤵ Over (F10)")).clicked() {
                action.step_over = true;
            }
            if ui.add_enabled(is_paused, egui::Button::new("↓ In (F11)")).clicked() {
                action.step_in = true;
            }
            if ui.add_enabled(is_paused, egui::Button::new("↑ Out (⇧F11)")).clicked() {
                action.step_out = true;
            }
            if ui.add_enabled(is_active, egui::Button::new("■ Stop")).clicked() {
                action.stop = true;
            }
        });

        ui.separator();

        // ── Status ────────────────────────────────────────────────────────────
        let status_text = match &state {
            DebugSessionState::Idle => "Idle — press F5 to start",
            DebugSessionState::Launching => "Launching…",
            DebugSessionState::Running => "Running",
            DebugSessionState::Paused { .. } => "Paused",
            DebugSessionState::Terminated => "Terminated",
        };
        let status_color = match &state {
            DebugSessionState::Running => Color32::from_rgb(80, 200, 80),
            DebugSessionState::Paused { .. } => Color32::from_rgb(255, 200, 50),
            DebugSessionState::Terminated => Color32::from_rgb(200, 80, 80),
            _ => Color32::GRAY,
        };
        ui.label(RichText::new(status_text).color(status_color).size(11.0));
        ui.separator();

        // ── Call Stack ────────────────────────────────────────────────────────
        let frames = dap.call_stack();
        if !frames.is_empty() {
            ui.label(
                RichText::new("CALL STACK")
                    .size(10.0)
                    .color(Color32::from_gray(130))
                    .strong(),
            );
            ScrollArea::vertical()
                .id_salt("dap_call_stack")
                .max_height(120.0)
                .show(ui, |ui| {
                    for (i, frame) in frames.iter().enumerate() {
                        let label = format!(
                            "{}  {}:{}",
                            frame.name,
                            frame.file.as_ref().and_then(|f| f.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                            frame.line
                        );
                        let is_top = i == 0;
                        let response = ui.selectable_label(
                            is_top,
                            RichText::new(&label)
                                .size(11.0)
                                .color(if is_top { Color32::WHITE } else { Color32::from_gray(180) }),
                        );
                        if response.clicked() {
                            if let Some(ref file) = frame.file {
                                action.navigate_to = Some((file.clone(), frame.line.saturating_sub(1)));
                            }
                        }
                    }
                });
            ui.separator();
        }

        // ── Variables ────────────────────────────────────────────────────────
        let vars = dap.variables();
        if !vars.is_empty() {
            ui.label(
                RichText::new("VARIABLES")
                    .size(10.0)
                    .color(Color32::from_gray(130))
                    .strong(),
            );
            ScrollArea::vertical()
                .id_salt("dap_variables")
                .max_height(150.0)
                .show(ui, |ui| {
                    for v in vars.iter().take(50) {
                        let type_hint = v.var_type.as_deref().unwrap_or("");
                        let label = if type_hint.is_empty() {
                            format!("{}: {}", v.name, v.value)
                        } else {
                            format!("{}: {} ({})", v.name, v.value, type_hint)
                        };
                        ui.label(RichText::new(label).size(11.0).monospace());
                    }
                });
            ui.separator();
        }

        // ── Output log ────────────────────────────────────────────────────────
        let log = dap.output_log();
        if !log.is_empty() || is_active {
            ui.label(
                RichText::new("OUTPUT")
                    .size(10.0)
                    .color(Color32::from_gray(130))
                    .strong(),
            );
            ScrollArea::vertical()
                .id_salt("dap_output")
                .stick_to_bottom(true)
                .max_height(150.0)
                .show(ui, |ui| {
                    for line in log.iter().rev().take(200).collect::<Vec<_>>().iter().rev() {
                        ui.label(RichText::new(line.as_str()).size(11.0).monospace());
                    }
                });
        }

        if !is_active && !is_running {
            ui.add_space(8.0);
            ui.label(
                RichText::new("Set breakpoints by clicking in the gutter (F9),\nthen press F5 to start debugging.")
                    .color(Color32::GRAY)
                    .size(11.0),
            );
        }

        action
    }
}
