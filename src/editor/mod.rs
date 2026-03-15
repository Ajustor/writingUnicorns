pub mod buffer;
pub mod cursor;
pub mod highlight;

use std::path::PathBuf;
use buffer::Buffer;
use cursor::Cursor;
use highlight::Highlighter;

pub struct Editor {
    pub buffer:       Buffer,
    pub cursor:       Cursor,
    pub highlighter:  Highlighter,
    pub scroll_offset: egui::Vec2,
    pub current_path: Option<PathBuf>,
    pub is_modified:  bool,
    pub line_height:  f32,
    pub char_width:   f32,
    pub show_find:    bool,
    pub find_query:   String,
    find_matches:     Vec<usize>,
    find_current:     usize,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            buffer:        Buffer::new(),
            cursor:        Cursor::new(),
            highlighter:   Highlighter::new(),
            scroll_offset: egui::Vec2::ZERO,
            current_path:  None,
            is_modified:   false,
            line_height:   20.0,
            char_width:    8.5,
            show_find:     false,
            find_query:    String::new(),
            find_matches:  vec![],
            find_current:  0,
        }
    }

    pub fn set_content(&mut self, content: String, path: Option<PathBuf>) {
        let lang = path.as_ref()
            .and_then(|p| p.file_name().and_then(|n| n.to_str()).map(|n| n.to_string()));
        self.buffer = Buffer::from_str(&content);
        self.cursor = Cursor::new();
        self.current_path = path;
        self.is_modified = false;
        self.scroll_offset = egui::Vec2::ZERO;
        self.show_find = false;
        self.find_query.clear();
        self.find_matches.clear();
        if let Some(name) = lang {
            self.highlighter.set_language_from_filename(&name);
        }
    }

    pub fn save(&mut self) -> anyhow::Result<()> {
        if let Some(path) = &self.current_path {
            std::fs::write(path, self.buffer.to_string())?;
            self.is_modified = false;
        }
        Ok(())
    }

    pub fn insert_char(&mut self, ch: char) {
        if self.cursor.has_selection() { self.delete_selection(); }
        let (row, col) = self.cursor.position();
        self.buffer.insert_char(row, col, ch);
        self.cursor.move_right(&self.buffer);
        self.cursor.clear_selection();
        self.is_modified = true;
    }

    pub fn delete_char_before(&mut self) {
        if self.cursor.has_selection() {
            self.delete_selection();
            return;
        }
        let (row, col) = self.cursor.position();
        if col > 0 {
            self.buffer.delete_char(row, col - 1);
            self.cursor.move_left(&self.buffer);
        } else if row > 0 {
            let prev_len = self.buffer.line_len(row - 1);
            self.buffer.join_lines(row);
            self.cursor.set_position(row - 1, prev_len);
        }
        self.is_modified = true;
    }

    pub fn insert_newline(&mut self) {
        if self.cursor.has_selection() { self.delete_selection(); }
        let (row, col) = self.cursor.position();
        self.buffer.split_line(row, col);
        self.cursor.set_position(row + 1, 0);
        self.is_modified = true;
    }

    pub fn selected_text(&self) -> Option<String> {
        let ((sr, sc), (er, ec)) = self.cursor.selection_range()?;
        let start = self.buffer.char_index(sr, sc);
        let end   = self.buffer.char_index(er, ec).min(self.buffer.rope_len());
        Some(self.buffer.rope_slice(start, end))
    }

    pub fn delete_selection(&mut self) {
        if let Some(((sr, sc), (er, ec))) = self.cursor.selection_range() {
            let start = self.buffer.char_index(sr, sc);
            let end   = self.buffer.char_index(er, ec).min(self.buffer.rope_len());
            self.buffer.delete_range(start, end);
            self.cursor.set_position(sr, sc);
            self.cursor.clear_selection();
            self.is_modified = true;
        }
    }

    fn update_find_matches(&mut self) {
        self.find_matches.clear();
        self.find_current = 0;
        if self.find_query.is_empty() { return; }
        let query = self.find_query.to_lowercase();
        for i in 0..self.buffer.num_lines() {
            if self.buffer.line(i).to_lowercase().contains(&query) {
                self.find_matches.push(i);
            }
        }
    }

    fn find_next(&mut self) {
        if self.find_matches.is_empty() { return; }
        self.find_current = (self.find_current + 1) % self.find_matches.len();
        let row = self.find_matches[self.find_current];
        self.cursor.set_position(row, 0);
    }

    pub fn show(&mut self, ui: &mut egui::Ui, config: &crate::config::Config) {
        // Find bar (floating overlay)
        if self.show_find {
            let ctx = ui.ctx().clone();
            let mut close_find = false;
            let mut do_find_next = false;
            let mut query_changed = false;

            egui::Window::new("Find")
                .collapsible(false)
                .resizable(false)
                .default_size(egui::vec2(300.0, 30.0))
                .show(&ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("✕").clicked() { close_find = true; }
                        let resp = ui.text_edit_singleline(&mut self.find_query);
                        if resp.changed() { query_changed = true; }
                        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            do_find_next = true;
                        }
                        if ui.button("▼ Next").clicked() { do_find_next = true; }
                        ui.label(format!("{} match(es)", self.find_matches.len()));
                    });
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        close_find = true;
                    }
                });

            if close_find { self.show_find = false; self.find_query.clear(); self.find_matches.clear(); }
            if query_changed { self.update_find_matches(); }
            if do_find_next { self.find_next(); }
        }

        let bg_color       = egui::Color32::from_rgb(30, 30, 30);
        let line_num_color = egui::Color32::from_rgb(100, 100, 100);
        let cursor_color   = egui::Color32::from_rgb(86, 156, 214);
        let find_highlight = egui::Color32::from_rgba_premultiplied(255, 200, 0, 30);
        let gutter_width   = if config.editor.line_numbers { 50.0 } else { 8.0 };

        let line_height = self.line_height;
        let font_id     = egui::FontId::monospace(config.font.size);

        // Measure actual monospace character width from the font (replaces hardcoded 8.5)
        let char_width = ui.fonts(|f| {
            f.layout_no_wrap("M".into(), font_id.clone(), egui::Color32::WHITE).size().x
        });
        self.char_width = char_width;

        let total_lines  = self.buffer.num_lines();
        let total_height = total_lines as f32 * line_height + line_height;

        egui::Frame::new()
            .fill(bg_color)
            .inner_margin(egui::Margin::ZERO)
            .show(ui, |ui| {
            // Remove default spacing between elements inside the editor frame
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
            let available = ui.available_size();
            let (rect, response) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());

            // Show text cursor when hovering over the editor area
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
            }

            if response.has_focus() || ui.memory(|m| m.focused().is_none()) {
                ui.input(|i| {
                    for event in &i.events {
                        match event {
                            egui::Event::Text(text) => {
                                // Don't insert text when Ctrl is held (shortcuts)
                                if !i.modifiers.ctrl && !i.modifiers.command {
                                    for ch in text.chars() {
                                        self.insert_char(ch);
                                    }
                                }
                            }
                            egui::Event::Paste(text) => {
                                for ch in text.chars() {
                                    if ch == '\n' { self.insert_newline(); }
                                    else { self.insert_char(ch); }
                                }
                            }
                            egui::Event::Key { key, pressed: true, modifiers, .. } => {
                                match key {
                                    egui::Key::Enter => self.insert_newline(),
                                    egui::Key::Backspace => self.delete_char_before(),

                                    egui::Key::ArrowLeft if modifiers.shift => {
                                        self.cursor.move_left_select(&self.buffer);
                                    }
                                    egui::Key::ArrowRight if modifiers.shift => {
                                        self.cursor.move_right_select(&self.buffer);
                                    }
                                    egui::Key::ArrowUp if modifiers.shift => {
                                        self.cursor.move_up_select(&self.buffer);
                                    }
                                    egui::Key::ArrowDown if modifiers.shift => {
                                        self.cursor.move_down_select(&self.buffer);
                                    }

                                    egui::Key::ArrowLeft  => self.cursor.move_left(&self.buffer),
                                    egui::Key::ArrowRight => self.cursor.move_right(&self.buffer),
                                    egui::Key::ArrowUp    => self.cursor.move_up(&self.buffer),
                                    egui::Key::ArrowDown  => self.cursor.move_down(&self.buffer),

                                    egui::Key::Home if modifiers.ctrl => {
                                        self.cursor.clear_selection();
                                        self.cursor.set_position(0, 0);
                                        self.scroll_offset = egui::Vec2::ZERO;
                                    }
                                    egui::Key::End if modifiers.ctrl => {
                                        self.cursor.clear_selection();
                                        let last = self.buffer.num_lines().saturating_sub(1);
                                        self.cursor.set_position(last, self.buffer.line_len(last));
                                    }
                                    egui::Key::Home => {
                                        let (row, _) = self.cursor.position();
                                        self.cursor.clear_selection();
                                        self.cursor.set_position(row, 0);
                                    }
                                    egui::Key::End => {
                                        let (row, _) = self.cursor.position();
                                        self.cursor.clear_selection();
                                        self.cursor.set_position(row, self.buffer.line_len(row));
                                    }
                                    egui::Key::Tab => {
                                        if !modifiers.shift {
                                            for _ in 0..4 { self.insert_char(' '); }
                                        }
                                    }

                                    // Save
                                    egui::Key::S if modifiers.ctrl => { let _ = self.save(); }

                                    // Undo
                                    egui::Key::Z if modifiers.ctrl && !modifiers.shift => {
                                        self.buffer.undo();
                                        self.is_modified = true;
                                    }
                                    // Redo (Ctrl+Shift+Z or Ctrl+Y)
                                    egui::Key::Z if modifiers.ctrl && modifiers.shift => {
                                        self.buffer.redo();
                                        self.is_modified = true;
                                    }
                                    egui::Key::Y if modifiers.ctrl => {
                                        self.buffer.redo();
                                        self.is_modified = true;
                                    }

                                    // Select All
                                    egui::Key::A if modifiers.ctrl => {
                                        let last_row = self.buffer.num_lines().saturating_sub(1);
                                        let last_col = self.buffer.line_len(last_row);
                                        self.cursor.sel_anchor = Some((0, 0));
                                        self.cursor.set_position(last_row, last_col);
                                        // Restore anchor (set_position clears desired_col but not anchor)
                                        self.cursor.sel_anchor = Some((0, 0));
                                    }

                                    // Duplicate line
                                    egui::Key::D if modifiers.ctrl => {
                                        let (row, _) = self.cursor.position();
                                        let line_text = self.buffer.line(row);
                                        let col = self.buffer.line_len(row);
                                        self.buffer.split_line(row, col);
                                        self.buffer.insert_str(row + 1, 0, &line_text);
                                        self.cursor.set_position(row + 1, 0);
                                        self.is_modified = true;
                                    }

                                    // Copy (Ctrl+C)
                                    egui::Key::C if modifiers.ctrl => {
                                        // handled below via output_mut – skip here since we need ui
                                    }
                                    // Cut (Ctrl+X)
                                    egui::Key::X if modifiers.ctrl => {
                                        // handled below
                                    }

                                    // Find
                                    egui::Key::F if modifiers.ctrl => {
                                        self.show_find = true;
                                    }

                                    egui::Key::Escape => {
                                        if self.show_find {
                                            self.show_find = false;
                                            self.find_query.clear();
                                            self.find_matches.clear();
                                        }
                                        self.cursor.clear_selection();
                                    }

                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }

                    // Handle copy/cut here so we have access to the full event list
                    // (these need separate ui.output_mut calls)
                });

                // Copy / Cut (need ui for output_mut)
                let do_copy = ui.input(|i| {
                    i.events.iter().any(|e| matches!(
                        e,
                        egui::Event::Key { key: egui::Key::C, pressed: true, modifiers, .. }
                        if modifiers.ctrl
                    ))
                });
                let do_cut = ui.input(|i| {
                    i.events.iter().any(|e| matches!(
                        e,
                        egui::Event::Key { key: egui::Key::X, pressed: true, modifiers, .. }
                        if modifiers.ctrl
                    ))
                });

                if do_copy {
                    let text = self.selected_text().unwrap_or_else(|| {
                        let (row, _) = self.cursor.position();
                        self.buffer.line(row) + "\n"
                    });
                    ui.ctx().copy_text(text);
                }
                if do_cut {
                    if let Some(text) = self.selected_text() {
                        ui.ctx().copy_text(text);
                        self.delete_selection();
                    }
                }
            }

            if response.clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let local = pos - rect.min;
                    let row = ((local.y + self.scroll_offset.y) / line_height) as usize;
                    let row = row.min(self.buffer.num_lines().saturating_sub(1));
                    // Use measured char_width for accurate column hit-test
                    let x_in_text = (local.x - gutter_width + self.scroll_offset.x).max(0.0);
                    let col = (x_in_text / char_width).round() as usize;
                    let col = col.min(self.buffer.line_len(row));
                    self.cursor.clear_selection();
                    self.cursor.set_position(row, col);
                }
            }

            ui.input(|i| {
                self.scroll_offset.y -= i.smooth_scroll_delta.y;
                self.scroll_offset.y = self.scroll_offset.y
                    .max(0.0)
                    .min((total_height - rect.height()).max(0.0));
            });

            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, bg_color);

            let first_visible  = (self.scroll_offset.y / line_height) as usize;
            let visible_count  = (rect.height() / line_height) as usize + 2;

            // Determine selection range for highlight
            let sel_range = self.cursor.selection_range();

            for line_idx in first_visible..((first_visible + visible_count).min(total_lines)) {
                let y = rect.min.y + line_idx as f32 * line_height - self.scroll_offset.y;

                if config.editor.line_numbers {
                    painter.text(
                        egui::pos2(rect.min.x + gutter_width - 8.0, y + line_height * 0.5),
                        egui::Align2::RIGHT_CENTER,
                        (line_idx + 1).to_string(),
                        font_id.clone(),
                        line_num_color,
                    );
                }

                let line = self.buffer.line(line_idx);
                let x_start = rect.min.x + gutter_width;

                // Find bar match highlight
                if !self.find_query.is_empty() && self.find_matches.contains(&line_idx) {
                    painter.rect_filled(
                        egui::Rect::from_min_size(
                            egui::pos2(x_start, y),
                            egui::vec2(rect.width() - gutter_width, line_height),
                        ),
                        0.0,
                        find_highlight,
                    );
                }

                // Selection highlight (per-line)
                if let Some(((sr, sc), (er, ec))) = sel_range {
                    if line_idx >= sr && line_idx <= er {
                        let sel_start_col = if line_idx == sr { sc } else { 0 };
                        let sel_end_col   = if line_idx == er { ec } else { self.buffer.line_len(line_idx) };
                        // Measure pixel positions from font for accuracy
                        let sx = x_start + sel_start_col as f32 * char_width;
                        let ex = x_start + sel_end_col   as f32 * char_width;
                        if ex > sx {
                            painter.rect_filled(
                                egui::Rect::from_min_max(
                                    egui::pos2(sx, y),
                                    egui::pos2(ex, y + line_height),
                                ),
                                0.0,
                                egui::Color32::from_rgba_premultiplied(86, 156, 214, 60),
                            );
                        }
                    }
                }

                let (cur_row, cur_col) = self.cursor.position();
                if line_idx == cur_row {
                    // Subtle current-line highlight (use unmultiplied for correct transparency)
                    painter.rect_filled(
                        egui::Rect::from_min_size(
                            egui::pos2(rect.min.x, y),
                            egui::vec2(rect.width(), line_height),
                        ),
                        0.0,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 12),
                    );
                    // Cursor: measure actual pixel offset of char col in the line
                    let text_to_cursor: String = line.chars().take(cur_col).collect();
                    let cx = x_start + ui.fonts(|f| {
                        f.layout_no_wrap(text_to_cursor, font_id.clone(), egui::Color32::WHITE).size().x
                    });
                    painter.line_segment(
                        [egui::pos2(cx, y), egui::pos2(cx, y + line_height)],
                        egui::Stroke::new(2.0, cursor_color),
                    );
                }

                // Syntax-highlighted text
                let tokens = self.highlighter.tokenize_line(&line);
                let mut job = egui::text::LayoutJob::default();
                for tok in &tokens {
                    job.append(
                        &tok.text,
                        0.0,
                        egui::TextFormat {
                            font_id: font_id.clone(),
                            color:   tok.kind.color(),
                            ..Default::default()
                        },
                    );
                }
                let galley = ui.fonts(|f| f.layout_job(job));
                painter.galley(egui::pos2(x_start, y + line_height * 0.15), galley, egui::Color32::WHITE);
            }

            ui.memory_mut(|m| m.request_focus(response.id));
        });
    }
}
