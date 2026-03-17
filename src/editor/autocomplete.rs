use std::collections::HashSet;

pub struct Autocomplete {
    pub visible: bool,
    pub query: String,
    pub suggestions: Vec<String>,
    pub selected: usize,
    pub cursor_screen_pos: egui::Pos2,
}

impl Autocomplete {
    pub fn new() -> Self {
        Self {
            visible: false,
            query: String::new(),
            suggestions: Vec::new(),
            selected: 0,
            cursor_screen_pos: egui::Pos2::ZERO,
        }
    }

    /// Recompute suggestions based on the partial word being typed.
    pub fn update(&mut self, word: &str, buffer_words: &[String], lang_keywords: &[&str]) {
        if word.chars().count() < 2 {
            self.visible = false;
            return;
        }

        self.query = word.to_string();
        let lower = word.to_lowercase();

        let mut seen: HashSet<String> = HashSet::new();
        let mut suggestions: Vec<String> = Vec::new();

        // Language keywords first, then buffer words.
        for &kw in lang_keywords {
            if kw.to_lowercase().starts_with(&lower) && kw != word && seen.insert(kw.to_string()) {
                suggestions.push(kw.to_string());
            }
        }
        for bw in buffer_words {
            if bw.to_lowercase().starts_with(&lower) && bw != word && seen.insert(bw.clone()) {
                suggestions.push(bw.clone());
            }
        }

        suggestions.sort();
        self.suggestions = suggestions;
        self.selected = 0;
        self.visible = !self.suggestions.is_empty();
    }

    /// Returns the suggestion to confirm, if any.
    pub fn confirm(&self) -> Option<&str> {
        if self.visible && !self.suggestions.is_empty() {
            Some(&self.suggestions[self.selected])
        } else {
            None
        }
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        if !self.suggestions.is_empty() {
            self.selected = (self.selected + 1).min(self.suggestions.len() - 1);
        }
    }

    /// Populate suggestions directly from LSP completion labels and show the popup.
    pub fn set_lsp_suggestions(&mut self, labels: Vec<String>) {
        if labels.is_empty() {
            return;
        }
        self.suggestions = labels;
        self.selected = 0;
        self.visible = true;
    }

    /// Render the popup using an egui Area (does not consume keyboard focus).
    pub fn show(&self, ctx: &egui::Context) {
        if !self.visible || self.suggestions.is_empty() {
            return;
        }

        const ITEM_HEIGHT: f32 = 20.0;
        const POPUP_WIDTH: f32 = 220.0;
        const MAX_VISIBLE: usize = 8;

        // Compute which window of suggestions to show.
        let scroll_start = if self.selected >= MAX_VISIBLE {
            self.selected + 1 - MAX_VISIBLE
        } else {
            0
        };
        let end = (scroll_start + MAX_VISIBLE).min(self.suggestions.len());
        let visible = &self.suggestions[scroll_start..end];

        // Decide whether to show below or above the cursor.
        let screen_height = ctx.screen_rect().height();
        let popup_height = visible.len() as f32 * ITEM_HEIGHT + 8.0;
        let pos = if self.cursor_screen_pos.y + popup_height > screen_height {
            egui::pos2(
                self.cursor_screen_pos.x,
                self.cursor_screen_pos.y - popup_height - ITEM_HEIGHT,
            )
        } else {
            self.cursor_screen_pos
        };

        egui::Area::new(egui::Id::new("autocomplete_popup"))
            .fixed_pos(pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(40, 44, 52))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 80, 120)))
                    .inner_margin(egui::Margin::same(2))
                    .show(ui, |ui| {
                        for (i, suggestion) in visible.iter().enumerate() {
                            let actual_idx = scroll_start + i;
                            let is_selected = actual_idx == self.selected;

                            let (rect, _) = ui.allocate_exact_size(
                                egui::vec2(POPUP_WIDTH, ITEM_HEIGHT),
                                egui::Sense::hover(),
                            );

                            if is_selected {
                                ui.painter().rect_filled(
                                    rect,
                                    2.0,
                                    egui::Color32::from_rgb(30, 80, 140),
                                );
                            }

                            ui.painter().text(
                                egui::pos2(rect.min.x + 6.0, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                suggestion,
                                egui::FontId::monospace(13.0),
                                egui::Color32::from_rgb(212, 212, 212),
                            );
                        }
                    });
            });
    }
}
