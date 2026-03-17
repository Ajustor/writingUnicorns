pub mod autocomplete;
pub mod buffer;
pub mod cursor;
pub mod highlight;

use autocomplete::Autocomplete;
use buffer::Buffer;
use cursor::Cursor;
use highlight::Highlighter;
use std::path::PathBuf;

/// Extract the word (identifier or path-like token) at `(row, col)` in the buffer.
fn get_word_at(buffer: &Buffer, row: usize, col: usize) -> Option<String> {
    let line = buffer.line(row);
    let chars: Vec<char> = line.chars().collect();
    let c = col.min(chars.len().saturating_sub(1));
    let is_word = |ch: char| ch.is_alphanumeric() || ch == '_';
    if c >= chars.len() || !is_word(chars[c]) {
        return None;
    }
    let mut start = c;
    while start > 0 && is_word(chars[start - 1]) {
        start -= 1;
    }
    let mut end = c + 1;
    while end < chars.len() && is_word(chars[end]) {
        end += 1;
    }
    Some(chars[start..end].iter().collect())
}

/// Search for the next occurrence of `word` in the buffer starting at (from_row, from_col),
/// wrapping around to the beginning if needed. Returns (row, col) of the match start.
fn find_next_occurrence(
    buf: &Buffer,
    word: &str,
    from_row: usize,
    from_col: usize,
) -> Option<(usize, usize)> {
    let word_chars: Vec<char> = word.chars().collect();
    let word_len = word_chars.len();
    if word_len == 0 {
        return None;
    }
    let total = buf.num_lines();
    // Search from (from_row, from_col) forward, then wrap to rows before from_row.
    let row_order: Vec<usize> = (from_row..total).chain(0..from_row).collect();
    for row_idx in row_order {
        let line_chars: Vec<char> = buf.line(row_idx).chars().collect();
        let start_col = if row_idx == from_row { from_col } else { 0 };
        if line_chars.len() < word_len {
            continue;
        }
        let end = line_chars.len() - word_len;
        if start_col > end {
            continue;
        }
        for col in start_col..=end {
            if line_chars[col..col + word_len] == word_chars[..] {
                return Some((row_idx, col));
            }
        }
    }
    None
}

pub struct Editor {
    pub buffer: Buffer,
    pub cursor: Cursor,
    pub extra_cursors: Vec<Cursor>,
    pub highlighter: Highlighter,
    pub autocomplete: Autocomplete,
    pub scroll_offset: egui::Vec2,
    pub current_path: Option<PathBuf>,
    pub is_modified: bool,
    pub line_height: f32,
    pub char_width: f32,
    pub show_find: bool,
    pub find_query: String,
    find_matches: Vec<usize>,
    find_current: usize,
    pub show_goto_line: bool,
    pub goto_line_input: String,
    /// Set to true to scroll the viewport so the cursor is visible on the next frame.
    pub scroll_to_cursor: bool,
    /// Populated by Ctrl+click; consumed by the app to navigate to definition.
    pub go_to_definition_request: Option<String>,
    /// Word under the mouse when Ctrl is held: (row, start_col, end_col).
    pub ctrl_hover_word_bounds: Option<(usize, usize, usize)>,
    /// Word currently under the mouse pointer (for hover tooltip).
    hover_word: Option<String>,
    /// Screen position of the mouse (updated every frame the mouse is over the editor).
    hover_pos: egui::Pos2,
    /// When the current `hover_word` was first detected.
    hover_start: Option<std::time::Instant>,
    /// Resolved signature string to display in the tooltip (empty string = looked up, nothing found).
    hover_signature: Option<String>,
    /// Root path of the open workspace, used to search for definitions across files.
    pub workspace_path: Option<std::path::PathBuf>,
    /// Monotonically increasing counter bumped on every edit; used to detect changes for LSP didChange.
    pub content_version: i32,
    /// Set when an LSP hover request has been fired; cleared when the response arrives.
    pub hover_lsp_request_pending: bool,
    /// Cursor row when the LSP hover request was triggered.
    pub hover_row: u32,
    /// Cursor column when the LSP hover request was triggered.
    pub hover_col: u32,
    /// Diagnostics for the current file, updated by the app each frame from the LSP client.
    pub diagnostics: Vec<crate::lsp::client::Diagnostic>,
    /// Set by the keyboard handler when Ctrl+Space is pressed; consumed by the app.
    pub completion_request_pending: bool,
    /// Cursor row when the completion request was triggered.
    pub completion_trigger_row: usize,
    /// Cursor column when the completion request was triggered.
    pub completion_trigger_col: usize,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            buffer: Buffer::new(),
            cursor: Cursor::new(),
            extra_cursors: vec![],
            highlighter: Highlighter::new(),
            autocomplete: Autocomplete::new(),
            scroll_offset: egui::Vec2::ZERO,
            current_path: None,
            is_modified: false,
            line_height: 20.0,
            char_width: 8.5,
            show_find: false,
            find_query: String::new(),
            find_matches: vec![],
            find_current: 0,
            show_goto_line: false,
            goto_line_input: String::new(),
            scroll_to_cursor: false,
            go_to_definition_request: None,
            ctrl_hover_word_bounds: None,
            hover_word: None,
            hover_pos: egui::Pos2::ZERO,
            hover_start: None,
            hover_signature: None,
            workspace_path: None,
            content_version: 0,
            hover_lsp_request_pending: false,
            hover_row: 0,
            hover_col: 0,
            diagnostics: vec![],
            completion_request_pending: false,
            completion_trigger_row: 0,
            completion_trigger_col: 0,
        }
    }

    pub fn set_content(&mut self, content: String, path: Option<PathBuf>) {
        let lang = path.as_ref().and_then(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.to_string())
        });
        self.buffer = Buffer::from_str(&content);
        self.cursor = Cursor::new();
        self.extra_cursors.clear();
        self.autocomplete = Autocomplete::new();
        self.current_path = path;
        self.is_modified = false;
        self.content_version = 0;
        self.hover_lsp_request_pending = false;
        self.hover_signature = None;
        self.hover_word = None;
        self.hover_start = None;
        self.diagnostics.clear();
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

    /// The word currently under the mouse pointer (used to populate `PluginContext`).
    pub fn hovered_word(&self) -> Option<&str> {
        self.hover_word.as_deref()
    }

    /// Search the current buffer for a definition of `word` and return a short signature string.
    fn lookup_signature_in_buffer(&self, word: &str) -> Option<String> {
        let content = self.buffer.to_string();
        for raw_line in content.lines() {
            let trimmed = raw_line.trim();

            // Function definitions
            let fn_needle_paren = format!("fn {}(", word);
            let fn_needle_space = format!("fn {} (", word);
            if (trimmed.contains(&fn_needle_paren) || trimmed.contains(&fn_needle_space))
                && (trimmed.starts_with("fn ")
                    || trimmed.starts_with("pub fn ")
                    || trimmed.starts_with("async fn ")
                    || trimmed.starts_with("pub async fn ")
                    || trimmed.starts_with("pub(crate) fn ")
                    || trimmed.starts_with("unsafe fn ")
                    || trimmed.starts_with("pub unsafe fn "))
            {
                // Strip trailing `{` to keep the signature clean
                let sig = trimmed.trim_end_matches('{').trim_end();
                return Some(sig.to_string());
            }

            // Struct definitions
            if trimmed.starts_with(&format!("struct {} ", word))
                || trimmed.starts_with(&format!("struct {}{}", word, '{'))
                || trimmed.starts_with(&format!("pub struct {} ", word))
                || trimmed.starts_with(&format!("pub struct {}{}", word, '{'))
                || trimmed.starts_with(&format!("pub(crate) struct {} ", word))
            {
                let sig = trimmed.trim_end_matches('{').trim_end();
                return Some(sig.to_string());
            }

            // Enum definitions
            if trimmed.starts_with(&format!("enum {} ", word))
                || trimmed.starts_with(&format!("enum {}{}", word, '{'))
                || trimmed.starts_with(&format!("pub enum {} ", word))
                || trimmed.starts_with(&format!("pub enum {}{}", word, '{'))
            {
                let sig = trimmed.trim_end_matches('{').trim_end();
                return Some(sig.to_string());
            }

            // Type aliases
            if trimmed.starts_with(&format!("type {} ", word))
                || trimmed.starts_with(&format!("pub type {} ", word))
            {
                let sig = trimmed.trim_end_matches(';').trim_end();
                return Some(sig.to_string());
            }

            // Let bindings (typed or inferred)
            if trimmed.starts_with(&format!("let {}: ", word))
                || trimmed.starts_with(&format!("let mut {}: ", word))
                || trimmed.starts_with(&format!("let {} =", word))
                || trimmed.starts_with(&format!("let mut {} =", word))
            {
                // Return just the declaration part (up to `=` or `;`)
                let end = trimmed
                    .find('=')
                    .or_else(|| trimmed.find(';'))
                    .unwrap_or(trimmed.len());
                return Some(trimmed[..end].trim_end().to_string());
            }
        }
        None
    }

    /// Search all source files in the workspace for a definition of `word`.
    fn lookup_signature_in_workspace(&self, word: &str) -> Option<String> {
        let workspace = self.workspace_path.as_ref()?;

        let patterns = [
            format!("fn {}(", word),
            format!("fn {} (", word),
            format!("pub fn {}(", word),
            format!("struct {} ", word),
            format!("struct {}{}", word, '{'),
            format!("pub struct {}", word),
            format!("enum {} ", word),
            format!("pub enum {}", word),
            format!("type {} ", word),
        ];

        let mut stack = vec![(workspace.to_path_buf(), 0usize)];
        let mut files_checked = 0;

        while let Some((dir, depth)) = stack.pop() {
            if depth > 8 || files_checked > 1000 {
                break;
            }
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if !matches!(name, "target" | ".git" | "node_modules" | ".cargo") {
                        stack.push((path, depth + 1));
                    }
                } else if path.is_file() {
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if matches!(ext, "rs" | "ts" | "js" | "py" | "go") {
                        files_checked += 1;
                        let Ok(content) = std::fs::read_to_string(&path) else {
                            continue;
                        };
                        for line in content.lines() {
                            let trimmed = line.trim();
                            for pattern in &patterns {
                                if trimmed.contains(pattern.as_str()) {
                                    let sig = trimmed.trim_end_matches('{').trim_end();
                                    if !sig.is_empty() {
                                        return Some(sig.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn insert_char(&mut self, ch: char) {
        self.buffer.checkpoint();
        if self.cursor.has_selection() {
            self.delete_selection();
        }
        let (row, col) = self.cursor.position();
        self.buffer.insert_char(row, col, ch);
        self.cursor.move_right(&self.buffer);
        self.cursor.clear_selection();

        // Adjust extra cursors on the same row that are after the insertion point.
        for ec in &mut self.extra_cursors {
            if ec.row == row && ec.col > col {
                ec.col += 1;
                ec.desired_col = ec.col;
            }
            if let Some((ar, ac)) = ec.sel_anchor {
                if ar == row && ac > col {
                    ec.sel_anchor = Some((ar, ac + 1));
                }
            }
        }

        // Process each extra cursor in order: delete its selection first, then insert.
        let n = self.extra_cursors.len();
        for i in 0..n {
            if self.extra_cursors[i].has_selection() {
                if let Some(((sr, sc), (er, ec))) = self.extra_cursors[i].selection_range() {
                    if sr == er {
                        // Same-line selection: delete the selected characters.
                        let count = ec - sc;
                        for _ in 0..count {
                            self.buffer.delete_char(sr, sc);
                        }
                        self.extra_cursors[i].set_position(sr, sc);
                        self.extra_cursors[i].clear_selection();

                        // Adjust all subsequent extra cursors on the same row.
                        for j in (i + 1)..n {
                            if self.extra_cursors[j].row == sr && self.extra_cursors[j].col > sc {
                                self.extra_cursors[j].col = self.extra_cursors[j]
                                    .col
                                    .saturating_sub(ec)
                                    .saturating_add(sc);
                                self.extra_cursors[j].desired_col = self.extra_cursors[j].col;
                            }
                            if let Some((ar, ac)) = self.extra_cursors[j].sel_anchor {
                                if ar == sr && ac > sc {
                                    let adj = ac.saturating_sub(ec).saturating_add(sc);
                                    self.extra_cursors[j].sel_anchor = Some((ar, adj));
                                }
                            }
                        }
                    } else {
                        // Multi-line selection: move cursor to start and clear selection.
                        self.extra_cursors[i].set_position(sr, sc);
                        self.extra_cursors[i].clear_selection();
                    }
                }
            }

            // Insert the character at the (possibly adjusted) cursor position.
            let (er, ec) = self.extra_cursors[i].position();
            self.buffer.insert_char(er, ec, ch);
            self.extra_cursors[i].move_right(&self.buffer);

            // Adjust all subsequent extra cursors on the same row.
            for j in (i + 1)..n {
                if self.extra_cursors[j].row == er && self.extra_cursors[j].col >= ec {
                    self.extra_cursors[j].col += 1;
                    self.extra_cursors[j].desired_col = self.extra_cursors[j].col;
                }
                if let Some((ar, ac)) = self.extra_cursors[j].sel_anchor {
                    if ar == er && ac >= ec {
                        self.extra_cursors[j].sel_anchor = Some((ar, ac + 1));
                    }
                }
            }
        }

        self.is_modified = true;
        self.content_version = self.content_version.wrapping_add(1);
    }

    pub fn delete_char_before(&mut self) {
        self.buffer.checkpoint();
        if self.cursor.has_selection() {
            self.delete_selection();
            return;
        }
        let (row, col) = self.cursor.position();
        if col > 0 {
            self.buffer.delete_char(row, col - 1);
            self.cursor.move_left(&self.buffer);

            // Adjust extra cursors on the same row that are after the deleted column.
            for ec in &mut self.extra_cursors {
                if ec.row == row && ec.col >= col {
                    ec.col -= 1;
                    ec.desired_col = ec.col;
                }
                if let Some((ar, ac)) = ec.sel_anchor {
                    if ar == row && ac >= col {
                        ec.sel_anchor = Some((ar, ac - 1));
                    }
                }
            }
        } else if row > 0 {
            let prev_len = self.buffer.line_len(row - 1);
            self.buffer.join_lines(row);
            self.cursor.set_position(row - 1, prev_len);
        }

        // Process each extra cursor in order: delete its selection if any, else delete char before.
        let n = self.extra_cursors.len();
        for i in 0..n {
            if self.extra_cursors[i].has_selection() {
                if let Some(((sr, sc), (er, ec))) = self.extra_cursors[i].selection_range() {
                    if sr == er {
                        let count = ec - sc;
                        for _ in 0..count {
                            self.buffer.delete_char(sr, sc);
                        }
                        self.extra_cursors[i].set_position(sr, sc);
                        self.extra_cursors[i].clear_selection();

                        // Adjust all subsequent extra cursors on the same row.
                        for j in (i + 1)..n {
                            if self.extra_cursors[j].row == sr && self.extra_cursors[j].col > sc {
                                self.extra_cursors[j].col = self.extra_cursors[j]
                                    .col
                                    .saturating_sub(ec)
                                    .saturating_add(sc);
                                self.extra_cursors[j].desired_col = self.extra_cursors[j].col;
                            }
                            if let Some((ar, ac)) = self.extra_cursors[j].sel_anchor {
                                if ar == sr && ac > sc {
                                    let adj = ac.saturating_sub(ec).saturating_add(sc);
                                    self.extra_cursors[j].sel_anchor = Some((ar, adj));
                                }
                            }
                        }
                    } else {
                        self.extra_cursors[i].set_position(sr, sc);
                        self.extra_cursors[i].clear_selection();
                    }
                }
            } else {
                let (er, ec) = self.extra_cursors[i].position();
                if ec > 0 {
                    self.buffer.delete_char(er, ec - 1);
                    self.extra_cursors[i].move_left(&self.buffer);

                    // Adjust all subsequent extra cursors on the same row.
                    for j in (i + 1)..n {
                        if self.extra_cursors[j].row == er && self.extra_cursors[j].col >= ec {
                            self.extra_cursors[j].col -= 1;
                            self.extra_cursors[j].desired_col = self.extra_cursors[j].col;
                        }
                        if let Some((ar, ac)) = self.extra_cursors[j].sel_anchor {
                            if ar == er && ac >= ec {
                                self.extra_cursors[j].sel_anchor = Some((ar, ac - 1));
                            }
                        }
                    }
                } else if er > 0 {
                    let prev_len = self.buffer.line_len(er - 1);
                    self.buffer.join_lines(er);
                    self.extra_cursors[i].set_position(er - 1, prev_len);
                }
            }
        }

        self.is_modified = true;
        self.content_version = self.content_version.wrapping_add(1);
    }

    pub fn insert_newline(&mut self) {
        self.buffer.checkpoint();
        if self.cursor.has_selection() {
            self.delete_selection();
        }
        let (row, col) = self.cursor.position();
        self.buffer.split_line(row, col);
        self.cursor.set_position(row + 1, 0);

        // Process each extra cursor in order: delete its selection if any, then insert newline.
        let n = self.extra_cursors.len();
        for i in 0..n {
            if self.extra_cursors[i].has_selection() {
                if let Some(((sr, sc), (er, ec))) = self.extra_cursors[i].selection_range() {
                    if sr == er {
                        let count = ec - sc;
                        for _ in 0..count {
                            self.buffer.delete_char(sr, sc);
                        }
                        self.extra_cursors[i].set_position(sr, sc);
                        self.extra_cursors[i].clear_selection();

                        // Adjust subsequent extra cursors on the same row.
                        for j in (i + 1)..n {
                            if self.extra_cursors[j].row == sr && self.extra_cursors[j].col > sc {
                                self.extra_cursors[j].col = self.extra_cursors[j]
                                    .col
                                    .saturating_sub(ec)
                                    .saturating_add(sc);
                                self.extra_cursors[j].desired_col = self.extra_cursors[j].col;
                            }
                            if let Some((ar, ac)) = self.extra_cursors[j].sel_anchor {
                                if ar == sr && ac > sc {
                                    let adj = ac.saturating_sub(ec).saturating_add(sc);
                                    self.extra_cursors[j].sel_anchor = Some((ar, adj));
                                }
                            }
                        }
                    } else {
                        self.extra_cursors[i].set_position(sr, sc);
                        self.extra_cursors[i].clear_selection();
                    }
                }
            }

            let (er, ec) = self.extra_cursors[i].position();
            self.buffer.split_line(er, ec);
            self.extra_cursors[i].set_position(er + 1, 0);
        }

        self.is_modified = true;
        self.content_version = self.content_version.wrapping_add(1);
    }

    pub fn selected_text(&self) -> Option<String> {
        let ((sr, sc), (er, ec)) = self.cursor.selection_range()?;
        let start = self.buffer.char_index(sr, sc);
        let end = self.buffer.char_index(er, ec).min(self.buffer.rope_len());
        Some(self.buffer.rope_slice(start, end))
    }

    pub fn delete_selection(&mut self) {
        if let Some(((sr, sc), (er, ec))) = self.cursor.selection_range() {
            let start = self.buffer.char_index(sr, sc);
            let end = self.buffer.char_index(er, ec).min(self.buffer.rope_len());
            self.buffer.delete_range(start, end);
            self.cursor.set_position(sr, sc);
            self.cursor.clear_selection();
            self.is_modified = true;
            self.content_version = self.content_version.wrapping_add(1);
        }
    }

    fn update_find_matches(&mut self) {
        self.find_matches.clear();
        self.find_current = 0;
        if self.find_query.is_empty() {
            return;
        }
        let query = self.find_query.to_lowercase();
        for i in 0..self.buffer.num_lines() {
            if self.buffer.line(i).to_lowercase().contains(&query) {
                self.find_matches.push(i);
            }
        }
    }

    fn find_next(&mut self) {
        if self.find_matches.is_empty() {
            return;
        }
        self.find_current = (self.find_current + 1) % self.find_matches.len();
        let row = self.find_matches[self.find_current];
        self.cursor.set_position(row, 0);
    }

    /// Remove extra cursors that share a position with the primary cursor or with each other.
    fn dedup_cursors(&mut self) {
        let primary_pos = self.cursor.position();
        self.extra_cursors.retain(|c| c.position() != primary_pos);
        let mut seen = std::collections::HashSet::new();
        self.extra_cursors.retain(|c| seen.insert(c.position()));
    }

    /// Returns the full word under the primary cursor (extending left and right from cursor),
    /// or the existing selection text if a selection is active.
    fn current_word_full(&self) -> Option<String> {
        if let Some(text) = self.selected_text() {
            if !text.is_empty() && !text.contains('\n') {
                return Some(text);
            }
        }
        let (row, col) = self.cursor.position();
        let line = self.buffer.line(row);
        let chars: Vec<char> = line.chars().collect();
        let col = col.min(chars.len());
        let mut start = col;
        while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
            start -= 1;
        }
        let mut end = col;
        while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
            end += 1;
        }
        if start == end {
            return None;
        }
        Some(chars[start..end].iter().collect())
    }

    /// Returns (word_start_col, word) for the partial word ending at the cursor.
    fn current_word_at_cursor(&self) -> (usize, String) {
        let (row, col) = self.cursor.position();
        let line = self.buffer.line(row);
        let chars: Vec<char> = line.chars().collect();
        let col = col.min(chars.len());
        let mut start = col;
        while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
            start -= 1;
        }
        let word: String = chars[start..col].iter().collect();
        (start, word)
    }

    /// Collect all words of length ≥ 2 present in the buffer (for autocomplete suggestions).
    fn buffer_words(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        for i in 0..self.buffer.num_lines() {
            let line = self.buffer.line(i);
            let mut word = String::new();
            for ch in line.chars() {
                if ch.is_alphanumeric() || ch == '_' {
                    word.push(ch);
                } else {
                    if word.chars().count() >= 2 {
                        seen.insert(word.clone());
                    }
                    word.clear();
                }
            }
            if word.chars().count() >= 2 {
                seen.insert(word);
            }
        }
        let mut result: Vec<String> = seen.into_iter().collect();
        result.sort();
        result
    }

    /// Replace the partial word at the cursor with the confirmed autocomplete suggestion.
    fn confirm_autocomplete(&mut self) {
        if let Some(suggestion) = self.autocomplete.confirm() {
            let suggestion = suggestion.to_owned();
            let (word_start, _) = self.current_word_at_cursor();
            let (row, col) = self.cursor.position();
            let start_idx = self.buffer.char_index(row, word_start);
            let end_idx = self.buffer.char_index(row, col);
            self.buffer.checkpoint();
            self.buffer.delete_range(start_idx, end_idx);
            self.buffer.insert_str(row, word_start, &suggestion);
            self.cursor
                .set_position(row, word_start + suggestion.chars().count());
            self.is_modified = true;
            self.content_version = self.content_version.wrapping_add(1);
        }
        self.autocomplete.visible = false;
    }

    fn trigger_autocomplete_update(&mut self) {
        let (_, word) = self.current_word_at_cursor();
        let buffer_words = self.buffer_words();
        let lang = self.highlighter.language.clone();
        let keywords = highlight::keywords_for_language(&lang);
        self.autocomplete.update(&word, &buffer_words, keywords);
    }

    fn all_cursor_rows(&self) -> Vec<usize> {
        let mut rows = vec![self.cursor.row];
        for ec in &self.extra_cursors {
            rows.push(ec.row);
        }
        rows.sort_unstable();
        rows.dedup();
        rows
    }

    /// Returns the set of lines covered by the current selection, or just the cursor lines.
    fn selected_line_rows(&self) -> Vec<usize> {
        if let Some(((sr, _), (er, _))) = self.cursor.selection_range() {
            (sr..=er).collect()
        } else {
            self.all_cursor_rows()
        }
    }

    /// Returns the comment prefix for the current file based on its extension.
    fn comment_prefix(&self) -> &'static str {
        let ext = self
            .current_path
            .as_ref()
            .and_then(|p| p.extension())
            .and_then(|e| e.to_str())
            .unwrap_or("");
        match ext {
            "py" | "sh" | "bash" | "zsh" | "fish" | "toml" | "ini" | "cfg" | "conf" | "yaml"
            | "yml" | "rb" | "r" | "pl" => "# ",
            "sql" | "lua" => "-- ",
            _ => "// ",
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        config: &crate::config::Config,
        plugin_manager: &crate::plugin::manager::PluginManager,
        lsp_hover: Option<String>,
    ) {
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
                        if ui.button("✕").clicked() {
                            close_find = true;
                        }
                        let resp = ui.text_edit_singleline(&mut self.find_query);
                        if resp.changed() {
                            query_changed = true;
                        }
                        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            do_find_next = true;
                        }
                        if ui.button("▼ Next").clicked() {
                            do_find_next = true;
                        }
                        ui.label(format!("{} match(es)", self.find_matches.len()));
                    });
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        close_find = true;
                    }
                });

            if close_find {
                self.show_find = false;
                self.find_query.clear();
                self.find_matches.clear();
            }
            if query_changed {
                self.update_find_matches();
            }
            if do_find_next {
                self.find_next();
            }
        }

        // Go to line dialog
        if self.show_goto_line {
            let ctx = ui.ctx().clone();
            let mut close_goto = false;
            let mut do_goto = false;

            egui::Window::new("Go to Line")
                .collapsible(false)
                .resizable(false)
                .default_size(egui::vec2(200.0, 30.0))
                .show(&ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("✕").clicked() {
                            close_goto = true;
                        }
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut self.goto_line_input)
                                .hint_text("Line number…")
                                .desired_width(120.0),
                        );
                        resp.request_focus();
                        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            do_goto = true;
                        }
                        if ui.button("Go").clicked() {
                            do_goto = true;
                        }
                    });
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        close_goto = true;
                    }
                });

            if do_goto {
                if let Ok(n) = self.goto_line_input.trim().parse::<usize>() {
                    let row = n.saturating_sub(1);
                    let row = row.min(self.buffer.num_lines().saturating_sub(1));
                    self.cursor.set_position(row, 0);
                    self.extra_cursors.clear();
                    // Scroll to the line
                    let target_y = row as f32 * self.line_height;
                    self.scroll_offset.y = target_y;
                }
                close_goto = true;
            }
            if close_goto {
                self.show_goto_line = false;
                self.goto_line_input.clear();
            }
        }

        let bg_color = egui::Color32::from_rgb(
            config.theme.background[0],
            config.theme.background[1],
            config.theme.background[2],
        );
        let fg_color = egui::Color32::from_rgb(
            config.theme.foreground[0],
            config.theme.foreground[1],
            config.theme.foreground[2],
        );
        let accent_color = egui::Color32::from_rgb(
            config.theme.accent[0],
            config.theme.accent[1],
            config.theme.accent[2],
        );
        let line_num_color =
            egui::Color32::from_rgb(fg_color.r() / 2, fg_color.g() / 2, fg_color.b() / 2);
        let cursor_color = accent_color;
        let find_highlight = egui::Color32::from_rgba_premultiplied(255, 200, 0, 30);
        let gutter_width = if config.editor.line_numbers {
            50.0
        } else {
            8.0
        };

        let line_height = self.line_height;
        let font_id = egui::FontId::monospace(config.font.size);

        // Measure actual monospace character width from the font (replaces hardcoded 8.5)
        let char_width = ui.fonts(|f| {
            f.layout_no_wrap("M".into(), font_id.clone(), egui::Color32::WHITE)
                .size()
                .x
        });
        self.char_width = char_width;

        let total_lines = self.buffer.num_lines();
        let total_height = total_lines as f32 * line_height + line_height;

        egui::Frame::new()
            .fill(bg_color)
            .inner_margin(egui::Margin::ZERO)
            .show(ui, |ui| {
                // Remove default spacing between elements inside the editor frame
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                let available = ui.available_size();
                let (rect, response) =
                    ui.allocate_exact_size(available, egui::Sense::click_and_drag());

                // Show text cursor when hovering over the editor area; switch to pointer when Ctrl is held.
                if response.hovered() {
                    if ui.input(|i| i.modifiers.ctrl) {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        // Compute the word bounds under the mouse for the underline.
                        if let Some(hover_pos) = ui.input(|i| i.pointer.hover_pos()) {
                            let local = hover_pos - rect.min;
                            let row = ((local.y + self.scroll_offset.y) / line_height) as usize;
                            let row = row.min(self.buffer.num_lines().saturating_sub(1));
                            let x_in_text =
                                (local.x - gutter_width + self.scroll_offset.x).max(0.0);
                            let col = (x_in_text / char_width).round() as usize;
                            let col = col.min(self.buffer.line_len(row));

                            let line_chars: Vec<char> = self.buffer.line(row).chars().collect();
                            let is_sym = |ch: char| ch.is_alphanumeric() || ch == '_';
                            if col < line_chars.len() && is_sym(line_chars[col]) {
                                let mut start = col;
                                while start > 0 && is_sym(line_chars[start - 1]) {
                                    start -= 1;
                                }
                                let mut end = col + 1;
                                while end < line_chars.len() && is_sym(line_chars[end]) {
                                    end += 1;
                                }
                                self.ctrl_hover_word_bounds = Some((row, start, end));
                            } else {
                                self.ctrl_hover_word_bounds = None;
                            }
                        }
                    } else {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
                        self.ctrl_hover_word_bounds = None;
                    }
                } else {
                    self.ctrl_hover_word_bounds = None;
                }

                // Hover-word detection for signature tooltip (only when Ctrl is NOT held).
                if response.hovered() && !ui.input(|i| i.modifiers.ctrl) {
                    if let Some(mouse_pos) = ui.input(|i| i.pointer.hover_pos()) {
                        let local = mouse_pos - rect.min;
                        let row = ((local.y + self.scroll_offset.y) / line_height) as usize;
                        let row = row.min(self.buffer.num_lines().saturating_sub(1));
                        let x_in_text =
                            (local.x - gutter_width + self.scroll_offset.x).max(0.0);
                        let col = (x_in_text / char_width).round() as usize;
                        let col = col.min(self.buffer.line_len(row));

                        let word_now = get_word_at(&self.buffer, row, col);
                        let word_changed = word_now.as_deref() != self.hover_word.as_deref();
                        if word_changed {
                            self.hover_word = word_now;
                            self.hover_start = if self.hover_word.is_some() {
                                Some(std::time::Instant::now())
                            } else {
                                None
                            };
                            self.hover_signature = None;
                        }
                        self.hover_pos = mouse_pos;
                    }
                } else if !response.hovered() {
                    // Mouse left the editor — clear tooltip state.
                    if self.hover_word.is_some() {
                        self.hover_word = None;
                        self.hover_start = None;
                        self.hover_signature = None;
                    }
                }

                // If an LSP hover response has arrived, apply it (overrides regex result).
                if let Some(lsp_sig) = lsp_hover {
                    if !lsp_sig.is_empty() {
                        self.hover_signature = Some(lsp_sig);
                    }
                    self.hover_lsp_request_pending = false;
                }

                // Resolve signature once the hover timer has fired.
                if self.hover_signature.is_none() && !self.hover_lsp_request_pending {
                    if let (Some(word), Some(start)) = (self.hover_word.clone(), self.hover_start) {
                        if start.elapsed() > std::time::Duration::from_millis(500) {
                            // Mark pending so the app sends an LSP hover request.
                            self.hover_lsp_request_pending = true;
                            let (cur_row, cur_col) = self.cursor.position();
                            self.hover_row = cur_row as u32;
                            self.hover_col = cur_col as u32;
                            // Regex fallback: show something immediately while LSP responds.
                            let lang = self.highlighter.language.clone();
                            let content = self.buffer.to_string();
                            let sig = plugin_manager
                                .hover_info(&lang, &word, &content)
                                .or_else(|| self.lookup_signature_in_buffer(&word))
                                .or_else(|| self.lookup_signature_in_workspace(&word))
                                .unwrap_or_default();
                            if !sig.is_empty() {
                                self.hover_signature = Some(sig);
                            }
                        }
                    }
                }

                if response.has_focus() || ui.memory(|m| m.focused().is_none()) {
                    ui.input(|i| {
                        let mut text_typed = false;
                        let mut ac_confirm = false;
                        let mut ac_dismiss = false;
                        let mut ac_nav = false;

                        for event in &i.events {
                            match event {
                                egui::Event::Text(text) => {
                                    // Don't insert text when Ctrl is held (shortcuts)
                                    if !i.modifiers.ctrl && !i.modifiers.command {
                                        for ch in text.chars() {
                                            self.insert_char(ch);
                                        }
                                        text_typed = true;
                                    }
                                }
                                egui::Event::Paste(text) => {
                                    for ch in text.chars() {
                                        if ch == '\n' {
                                            self.insert_newline();
                                        } else {
                                            self.insert_char(ch);
                                        }
                                    }
                                    text_typed = true;
                                }
                                egui::Event::Key {
                                    key,
                                    pressed: true,
                                    modifiers,
                                    ..
                                } => {
                                    match key {
                                        egui::Key::Enter if modifiers.ctrl && modifiers.shift => {
                                            self.buffer.checkpoint();
                                            let row = self.cursor.row;
                                            self.buffer.split_line(row, 0);
                                            self.cursor.col = 0;
                                            self.cursor.clear_selection();
                                            self.extra_cursors.clear();
                                            self.is_modified = true;
                                            self.content_version = self.content_version.wrapping_add(1);
                                        }
                                        egui::Key::Enter if modifiers.ctrl => {
                                            self.buffer.checkpoint();
                                            let row = self.cursor.row;
                                            let line_len = self.buffer.line_len(row);
                                            self.buffer.split_line(row, line_len);
                                            self.cursor.row += 1;
                                            self.cursor.col = 0;
                                            self.cursor.clear_selection();
                                            self.extra_cursors.clear();
                                            self.is_modified = true;
                                            self.content_version = self.content_version.wrapping_add(1);
                                        }
                                        egui::Key::Enter => {
                                            if self.autocomplete.visible {
                                                ac_confirm = true;
                                                ac_nav = true;
                                            } else {
                                                self.insert_newline();
                                            }
                                        }
                                        egui::Key::Backspace => {
                                            self.delete_char_before();
                                            text_typed = true;
                                        }

                                        egui::Key::ArrowUp if modifiers.ctrl && modifiers.shift => {
                                            let row = self.cursor.row;
                                            if row > 0 {
                                                self.buffer.checkpoint();
                                                let current = self.buffer.line(row);
                                                let above = self.buffer.line(row - 1);
                                                self.buffer.replace_line(row, &above);
                                                self.buffer.replace_line(row - 1, &current);
                                                self.cursor.row -= 1;
                                                self.is_modified = true;
                                                self.content_version = self.content_version.wrapping_add(1);
                                            }
                                        }
                                        egui::Key::ArrowDown
                                            if modifiers.ctrl && modifiers.shift =>
                                        {
                                            let row = self.cursor.row;
                                            if row + 1 < self.buffer.num_lines() {
                                                self.buffer.checkpoint();
                                                let current = self.buffer.line(row);
                                                let below = self.buffer.line(row + 1);
                                                self.buffer.replace_line(row, &below);
                                                self.buffer.replace_line(row + 1, &current);
                                                self.cursor.row += 1;
                                                self.is_modified = true;
                                                self.content_version = self.content_version.wrapping_add(1);
                                            }
                                        }
                                        // Alt+Shift+Up — move current line up
                                        egui::Key::ArrowUp if modifiers.alt && modifiers.shift => {
                                            let row = self.cursor.row;
                                            if row > 0 {
                                                self.buffer.checkpoint();
                                                let current = self.buffer.line(row);
                                                let above = self.buffer.line(row - 1);
                                                self.buffer.replace_line(row, &above);
                                                self.buffer.replace_line(row - 1, &current);
                                                self.cursor.row -= 1;
                                                self.is_modified = true;
                                                self.content_version = self.content_version.wrapping_add(1);
                                            }
                                        }
                                        // Alt+Shift+Down — move current line down
                                        egui::Key::ArrowDown
                                            if modifiers.alt && modifiers.shift =>
                                        {
                                            let row = self.cursor.row;
                                            if row + 1 < self.buffer.num_lines() {
                                                self.buffer.checkpoint();
                                                let current = self.buffer.line(row);
                                                let below = self.buffer.line(row + 1);
                                                self.buffer.replace_line(row, &below);
                                                self.buffer.replace_line(row + 1, &current);
                                                self.cursor.row += 1;
                                                self.is_modified = true;
                                                self.content_version = self.content_version.wrapping_add(1);
                                            }
                                        }
                                        egui::Key::ArrowUp if modifiers.alt => {
                                            let mut new_extras: Vec<Cursor> = vec![];
                                            let (pr, pc) = self.cursor.position();
                                            if pr > 0 {
                                                let mut c = Cursor::new();
                                                c.set_position(
                                                    pr - 1,
                                                    pc.min(self.buffer.line_len(pr - 1)),
                                                );
                                                new_extras.push(c);
                                            }
                                            for ec in &self.extra_cursors {
                                                let (er, ec_col) = ec.position();
                                                if er > 0 {
                                                    let mut c = Cursor::new();
                                                    c.set_position(
                                                        er - 1,
                                                        ec_col.min(self.buffer.line_len(er - 1)),
                                                    );
                                                    new_extras.push(c);
                                                }
                                            }
                                            self.extra_cursors.extend(new_extras);
                                            self.dedup_cursors();
                                        }
                                        egui::Key::ArrowDown if modifiers.alt => {
                                            let mut new_extras: Vec<Cursor> = vec![];
                                            let (pr, pc) = self.cursor.position();
                                            if pr + 1 < self.buffer.num_lines() {
                                                let mut c = Cursor::new();
                                                c.set_position(
                                                    pr + 1,
                                                    pc.min(self.buffer.line_len(pr + 1)),
                                                );
                                                new_extras.push(c);
                                            }
                                            for ec in &self.extra_cursors {
                                                let (er, ec_col) = ec.position();
                                                if er + 1 < self.buffer.num_lines() {
                                                    let mut c = Cursor::new();
                                                    c.set_position(
                                                        er + 1,
                                                        ec_col.min(self.buffer.line_len(er + 1)),
                                                    );
                                                    new_extras.push(c);
                                                }
                                            }
                                            self.extra_cursors.extend(new_extras);
                                            self.dedup_cursors();
                                        }

                                        // Autocomplete navigation
                                        egui::Key::ArrowUp if self.autocomplete.visible => {
                                            self.autocomplete.move_up();
                                            ac_nav = true;
                                        }
                                        egui::Key::ArrowDown if self.autocomplete.visible => {
                                            self.autocomplete.move_down();
                                            ac_nav = true;
                                        }

                                        egui::Key::ArrowLeft if modifiers.shift => {
                                            self.cursor.move_left_select(&self.buffer);
                                            for ec in &mut self.extra_cursors {
                                                ec.move_left_select(&self.buffer);
                                            }
                                        }
                                        egui::Key::ArrowRight if modifiers.shift => {
                                            self.cursor.move_right_select(&self.buffer);
                                            for ec in &mut self.extra_cursors {
                                                ec.move_right_select(&self.buffer);
                                            }
                                        }
                                        egui::Key::ArrowUp if modifiers.shift => {
                                            self.cursor.move_up_select(&self.buffer);
                                            for ec in &mut self.extra_cursors {
                                                ec.move_up_select(&self.buffer);
                                            }
                                        }
                                        egui::Key::ArrowDown if modifiers.shift => {
                                            self.cursor.move_down_select(&self.buffer);
                                            for ec in &mut self.extra_cursors {
                                                ec.move_down_select(&self.buffer);
                                            }
                                        }

                                        egui::Key::ArrowLeft => {
                                            self.cursor.move_left(&self.buffer);
                                            for ec in &mut self.extra_cursors {
                                                ec.move_left(&self.buffer);
                                            }
                                            self.dedup_cursors();
                                            ac_dismiss = true;
                                        }
                                        egui::Key::ArrowRight => {
                                            self.cursor.move_right(&self.buffer);
                                            for ec in &mut self.extra_cursors {
                                                ec.move_right(&self.buffer);
                                            }
                                            self.dedup_cursors();
                                            ac_dismiss = true;
                                        }
                                        egui::Key::ArrowUp => {
                                            self.cursor.move_up(&self.buffer);
                                            for ec in &mut self.extra_cursors {
                                                ec.move_up(&self.buffer);
                                            }
                                            self.dedup_cursors();
                                            ac_dismiss = true;
                                        }
                                        egui::Key::ArrowDown => {
                                            self.cursor.move_down(&self.buffer);
                                            for ec in &mut self.extra_cursors {
                                                ec.move_down(&self.buffer);
                                            }
                                            self.dedup_cursors();
                                            ac_dismiss = true;
                                        }

                                        egui::Key::Home if modifiers.ctrl && modifiers.shift => {
                                            self.cursor.start_selection();
                                            self.cursor.set_position(0, 0);
                                            self.scroll_offset = egui::Vec2::ZERO;
                                            for ec in &mut self.extra_cursors {
                                                ec.start_selection();
                                                ec.set_position(0, 0);
                                            }
                                            self.dedup_cursors();
                                            ac_dismiss = true;
                                        }
                                        egui::Key::End if modifiers.ctrl && modifiers.shift => {
                                            self.cursor.start_selection();
                                            let last = self.buffer.num_lines().saturating_sub(1);
                                            self.cursor
                                                .set_position(last, self.buffer.line_len(last));
                                            for ec in &mut self.extra_cursors {
                                                ec.start_selection();
                                                ec.set_position(last, self.buffer.line_len(last));
                                            }
                                            self.dedup_cursors();
                                            ac_dismiss = true;
                                        }
                                        egui::Key::Home if modifiers.ctrl => {
                                            self.cursor.clear_selection();
                                            self.cursor.set_position(0, 0);
                                            self.scroll_offset = egui::Vec2::ZERO;
                                            for ec in &mut self.extra_cursors {
                                                ec.clear_selection();
                                                ec.set_position(0, 0);
                                            }
                                            self.dedup_cursors();
                                            ac_dismiss = true;
                                        }
                                        egui::Key::End if modifiers.ctrl => {
                                            self.cursor.clear_selection();
                                            let last = self.buffer.num_lines().saturating_sub(1);
                                            self.cursor
                                                .set_position(last, self.buffer.line_len(last));
                                            for ec in &mut self.extra_cursors {
                                                ec.clear_selection();
                                                ec.set_position(last, self.buffer.line_len(last));
                                            }
                                            self.dedup_cursors();
                                            ac_dismiss = true;
                                        }
                                        egui::Key::Home if modifiers.shift => {
                                            let (row, _) = self.cursor.position();
                                            self.cursor.start_selection();
                                            self.cursor.set_position(row, 0);
                                            for ec in &mut self.extra_cursors {
                                                let (er, _) = ec.position();
                                                ec.start_selection();
                                                ec.set_position(er, 0);
                                            }
                                            self.dedup_cursors();
                                            ac_dismiss = true;
                                        }
                                        egui::Key::End if modifiers.shift => {
                                            let (row, _) = self.cursor.position();
                                            self.cursor.start_selection();
                                            self.cursor
                                                .set_position(row, self.buffer.line_len(row));
                                            for ec in &mut self.extra_cursors {
                                                let (er, _) = ec.position();
                                                ec.start_selection();
                                                ec.set_position(er, self.buffer.line_len(er));
                                            }
                                            self.dedup_cursors();
                                            ac_dismiss = true;
                                        }
                                        egui::Key::Home => {
                                            let (row, _) = self.cursor.position();
                                            self.cursor.clear_selection();
                                            self.cursor.set_position(row, 0);
                                            for ec in &mut self.extra_cursors {
                                                let (er, _) = ec.position();
                                                ec.clear_selection();
                                                ec.set_position(er, 0);
                                            }
                                            self.dedup_cursors();
                                            ac_dismiss = true;
                                        }
                                        egui::Key::End => {
                                            let (row, _) = self.cursor.position();
                                            self.cursor.clear_selection();
                                            self.cursor
                                                .set_position(row, self.buffer.line_len(row));
                                            for ec in &mut self.extra_cursors {
                                                let (er, _) = ec.position();
                                                ec.clear_selection();
                                                ec.set_position(er, self.buffer.line_len(er));
                                            }
                                            self.dedup_cursors();
                                            ac_dismiss = true;
                                        }
                                        egui::Key::Tab => {
                                            if self.autocomplete.visible {
                                                ac_confirm = true;
                                                ac_nav = true;
                                            } else if !modifiers.shift {
                                                for _ in 0..4 {
                                                    self.insert_char(' ');
                                                }
                                                text_typed = true;
                                            }
                                        }

                                        // Save
                                        egui::Key::S if modifiers.ctrl => {
                                            let _ = self.save();
                                        }

                                        // Undo
                                        egui::Key::Z if modifiers.ctrl && !modifiers.shift => {
                                            self.buffer.undo();
                                            self.is_modified = true;
                                            self.content_version = self.content_version.wrapping_add(1);
                                        }
                                        // Redo (Ctrl+Shift+Z or Ctrl+Y)
                                        egui::Key::Z if modifiers.ctrl && modifiers.shift => {
                                            self.buffer.redo();
                                            self.is_modified = true;
                                            self.content_version = self.content_version.wrapping_add(1);
                                        }
                                        egui::Key::Y if modifiers.ctrl => {
                                            self.buffer.redo();
                                            self.is_modified = true;
                                            self.content_version = self.content_version.wrapping_add(1);
                                        }

                                        // Select All
                                        egui::Key::A if modifiers.ctrl => {
                                            let last_row =
                                                self.buffer.num_lines().saturating_sub(1);
                                            let last_col = self.buffer.line_len(last_row);
                                            self.cursor.sel_anchor = Some((0, 0));
                                            self.cursor.set_position(last_row, last_col);
                                            // Restore anchor (set_position clears desired_col but not anchor)
                                            self.cursor.sel_anchor = Some((0, 0));
                                        }

                                        // Ctrl+D — select word then find next occurrence (VSCode style)
                                        egui::Key::D if modifiers.ctrl => {
                                            if let Some(word) = self.current_word_full() {
                                                let word_len = word.chars().count();
                                                if !self.cursor.has_selection() {
                                                    // First press: select the word under cursor
                                                    let (row, col) = self.cursor.position();
                                                    let line = self.buffer.line(row);
                                                    let chars: Vec<char> = line.chars().collect();
                                                    let c = col.min(chars.len());
                                                    let mut start = c;
                                                    while start > 0
                                                        && (chars[start - 1].is_alphanumeric()
                                                            || chars[start - 1] == '_')
                                                    {
                                                        start -= 1;
                                                    }
                                                    self.cursor.set_position(row, start + word_len);
                                                    self.cursor.sel_anchor = Some((row, start));
                                                } else {
                                                    // Subsequent presses: add cursor at next occurrence
                                                    let (cur_row, cur_col) = self.cursor.position();
                                                    if let Some((match_row, match_col)) =
                                                        find_next_occurrence(
                                                            &self.buffer,
                                                            &word,
                                                            cur_row,
                                                            cur_col,
                                                        )
                                                    {
                                                        // Save current main cursor as extra, then advance main to next occurrence
                                                        let saved = self.cursor.clone();
                                                        self.extra_cursors.push(saved);
                                                        self.cursor.row = match_row;
                                                        self.cursor.col = match_col + word_len;
                                                        self.cursor.desired_col = self.cursor.col;
                                                        self.cursor.sel_anchor =
                                                            Some((match_row, match_col));
                                                        self.dedup_cursors();
                                                    }
                                                }
                                            }
                                        }

                                        // Ctrl+Shift+L — select ALL occurrences of current word/selection
                                        egui::Key::L if modifiers.ctrl && modifiers.shift => {
                                            if let Some(word) = self.current_word_full() {
                                                let word_len = word.chars().count();
                                                self.extra_cursors.clear();
                                                // Select the word with the primary cursor first
                                                let (row, col) = self.cursor.position();
                                                let line = self.buffer.line(row);
                                                let chars: Vec<char> = line.chars().collect();
                                                let c = col.min(chars.len());
                                                let mut start = c;
                                                while start > 0
                                                    && (chars[start - 1].is_alphanumeric()
                                                        || chars[start - 1] == '_')
                                                {
                                                    start -= 1;
                                                }
                                                self.cursor.set_position(row, start + word_len);
                                                self.cursor.sel_anchor = Some((row, start));
                                                // Add extra cursors for every other occurrence
                                                let mut search_row = 0;
                                                let mut search_col = 0;
                                                while let Some((mr, mc)) = find_next_occurrence(
                                                    &self.buffer,
                                                    &word,
                                                    search_row,
                                                    search_col,
                                                ) {
                                                    // Stop if we've wrapped past the starting point
                                                    if mr < search_row
                                                        || (mr == search_row
                                                            && mc < search_col
                                                            && search_row != 0)
                                                    {
                                                        break;
                                                    }
                                                    // Skip the primary cursor's occurrence
                                                    if !(mr == row && mc == start) {
                                                        let mut extra = Cursor::new();
                                                        extra.row = mr;
                                                        extra.col = mc + word_len;
                                                        extra.desired_col = extra.col;
                                                        extra.sel_anchor = Some((mr, mc));
                                                        self.extra_cursors.push(extra);
                                                    }
                                                    // Advance past this match
                                                    search_col = mc + word_len;
                                                    search_row = mr;
                                                    if search_col > self.buffer.line_len(search_row)
                                                    {
                                                        search_row += 1;
                                                        search_col = 0;
                                                        if search_row >= self.buffer.num_lines() {
                                                            break;
                                                        }
                                                    }
                                                }
                                                self.dedup_cursors();
                                            }
                                        }

                                        // Copy (Ctrl+C)
                                        egui::Key::C if modifiers.ctrl => {
                                            // handled below via output_mut – skip here since we need ui
                                        }
                                        // Cut (Ctrl+X)
                                        egui::Key::X if modifiers.ctrl => {
                                            // handled below
                                        }

                                        // Trigger LSP completions (Ctrl+Space)
                                        egui::Key::Space if modifiers.ctrl => {
                                            let (row, col) = self.cursor.position();
                                            self.completion_request_pending = true;
                                            self.completion_trigger_row = row;
                                            self.completion_trigger_col = col;
                                        }

                                        // Find
                                        egui::Key::F if modifiers.ctrl => {
                                            self.show_find = true;
                                        }

                                        // Go to line (Ctrl+G)
                                        egui::Key::G if modifiers.ctrl => {
                                            self.show_goto_line = true;
                                            self.goto_line_input.clear();
                                        }

                                        // Toggle line comment (Ctrl+/)
                                        egui::Key::Slash if modifiers.ctrl => {
                                            self.buffer.checkpoint();
                                            let prefix = self.comment_prefix();
                                            let prefix_len = prefix.chars().count();
                                            let prefix_trim = prefix.trim_end();
                                            let rows = self.selected_line_rows();
                                            let all_commented = rows.iter().all(|&row| {
                                                let line = self.buffer.line(row);
                                                let leading = line
                                                    .chars()
                                                    .take_while(|c| c.is_whitespace())
                                                    .count();
                                                let byte_offset = line
                                                    .char_indices()
                                                    .nth(leading)
                                                    .map(|(i, _)| i)
                                                    .unwrap_or(line.len());
                                                line[byte_offset..].starts_with(prefix_trim)
                                            });
                                            for row in rows.iter().rev() {
                                                let line = self.buffer.line(*row);
                                                let leading = line
                                                    .chars()
                                                    .take_while(|c| c.is_whitespace())
                                                    .count();
                                                let byte_offset = line
                                                    .char_indices()
                                                    .nth(leading)
                                                    .map(|(i, _)| i)
                                                    .unwrap_or(line.len());
                                                if all_commented {
                                                    let n = if line[byte_offset..]
                                                        .starts_with(prefix)
                                                    {
                                                        prefix_len
                                                    } else {
                                                        prefix_trim.chars().count()
                                                    };
                                                    for _ in 0..n {
                                                        self.buffer.delete_char(*row, leading);
                                                    }
                                                } else {
                                                    let pfx_chars: Vec<char> =
                                                        prefix.chars().collect();
                                                    for (i, &ch) in pfx_chars.iter().enumerate() {
                                                        self.buffer.insert_char(*row, i, ch);
                                                    }
                                                }
                                            }
                                            self.is_modified = true;
                                            self.content_version = self.content_version.wrapping_add(1);
                                        }

                                        // Delete current line(s) (Ctrl+Shift+K)
                                        egui::Key::K if modifiers.ctrl && modifiers.shift => {
                                            self.buffer.checkpoint();
                                            let mut rows = self.all_cursor_rows();
                                            rows.sort_unstable();
                                            rows.dedup();
                                            for row in rows.iter().rev() {
                                                self.buffer.delete_line(*row);
                                            }
                                            let max_row = self.buffer.num_lines().saturating_sub(1);
                                            self.cursor.row = self.cursor.row.min(max_row);
                                            self.cursor.col = 0;
                                            self.extra_cursors.clear();
                                            self.is_modified = true;
                                            self.content_version = self.content_version.wrapping_add(1);
                                        }

                                        // Indent selected/current lines (Ctrl+])
                                        egui::Key::CloseBracket if modifiers.ctrl => {
                                            self.buffer.checkpoint();
                                            let rows = self.selected_line_rows();
                                            for row in &rows {
                                                for i in 0..4 {
                                                    self.buffer.insert_char(*row, i, ' ');
                                                }
                                            }
                                            self.cursor.col = self.cursor.col.saturating_add(4);
                                            self.cursor.desired_col = self.cursor.col;
                                            self.is_modified = true;
                                            self.content_version = self.content_version.wrapping_add(1);
                                        }

                                        // Unindent selected/current lines (Ctrl+[)
                                        egui::Key::OpenBracket if modifiers.ctrl => {
                                            self.buffer.checkpoint();
                                            let rows = self.selected_line_rows();
                                            for row in &rows {
                                                let line = self.buffer.line(*row);
                                                let spaces: usize = line
                                                    .chars()
                                                    .take(4)
                                                    .take_while(|&c| c == ' ')
                                                    .count();
                                                for _ in 0..spaces {
                                                    self.buffer.delete_char(*row, 0);
                                                }
                                                if *row == self.cursor.row {
                                                    self.cursor.col =
                                                        self.cursor.col.saturating_sub(spaces);
                                                    self.cursor.desired_col = self.cursor.col;
                                                }
                                            }
                                            self.is_modified = true;
                                            self.content_version = self.content_version.wrapping_add(1);
                                        }

                                        egui::Key::Escape => {
                                            if self.autocomplete.visible {
                                                ac_dismiss = true;
                                                ac_nav = true;
                                            } else {
                                                if self.show_find {
                                                    self.show_find = false;
                                                    self.find_query.clear();
                                                    self.find_matches.clear();
                                                }
                                                if self.show_goto_line {
                                                    self.show_goto_line = false;
                                                    self.goto_line_input.clear();
                                                }
                                                self.extra_cursors.clear();
                                                self.cursor.clear_selection();
                                            }
                                        }

                                        _ => {}
                                    }
                                }
                                _ => {}
                            }
                        }

                        // Apply autocomplete state changes.
                        if ac_confirm {
                            self.confirm_autocomplete();
                        } else if ac_dismiss {
                            self.autocomplete.visible = false;
                        } else if text_typed {
                            self.trigger_autocomplete_update();
                        }
                        // ac_nav without confirm/dismiss means navigation — keep visible.
                        let _ = ac_nav;

                        // Handle copy/cut here so we have access to the full event list
                        // (these need separate ui.output_mut calls)
                    });

                    // Copy / Cut (need ui for output_mut)
                    let do_copy =
                        ui.input(|i| {
                            i.events.iter().any(|e| matches!(
                        e,
                        egui::Event::Key { key: egui::Key::C, pressed: true, modifiers, .. }
                        if modifiers.ctrl
                    ))
                        });
                    let do_cut =
                        ui.input(|i| {
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
                            self.buffer.checkpoint();
                            self.delete_selection();
                        }
                    }
                }

                let mut double_click_handled = false;

                if response.double_clicked() {
                    self.extra_cursors.clear();
                    if let Some(pos) = response.interact_pointer_pos() {
                        let (row, col) = {
                            let local = pos - rect.min;
                            let r = ((local.y + self.scroll_offset.y) / line_height) as usize;
                            let r = r.min(self.buffer.num_lines().saturating_sub(1));
                            let x_in_text =
                                (local.x - gutter_width + self.scroll_offset.x).max(0.0);
                            let c = (x_in_text / char_width).round() as usize;
                            let c = c.min(self.buffer.line_len(r));
                            (r, c)
                        };
                        let line_chars: Vec<char> = self.buffer.line(row).chars().collect();
                        let c = col.min(line_chars.len());
                        let is_word = |ch: char| ch.is_alphanumeric() || ch == '_';
                        if c < line_chars.len() && is_word(line_chars[c]) {
                            let mut start = c;
                            while start > 0 && is_word(line_chars[start - 1]) {
                                start -= 1;
                            }
                            let mut end = c;
                            while end < line_chars.len() && is_word(line_chars[end]) {
                                end += 1;
                            }
                            self.cursor.sel_anchor = Some((row, start));
                            self.cursor.set_position(row, end);
                        }
                    }
                    double_click_handled = true;
                }

                if response.drag_started() && !double_click_handled {
                    if let Some(pos) = response.interact_pointer_pos() {
                        let (row, col) = {
                            let local = pos - rect.min;
                            let r = ((local.y + self.scroll_offset.y) / line_height) as usize;
                            let r = r.min(self.buffer.num_lines().saturating_sub(1));
                            let x_in_text =
                                (local.x - gutter_width + self.scroll_offset.x).max(0.0);
                            let c = (x_in_text / char_width).round() as usize;
                            let c = c.min(self.buffer.line_len(r));
                            (r, c)
                        };
                        if ui.input(|i| i.modifiers.ctrl) {
                            let mut extra = Cursor::new();
                            extra.set_position(row, col);
                            self.extra_cursors.push(extra);
                        } else if ui.input(|i| i.modifiers.shift) {
                            if self.cursor.sel_anchor.is_none() {
                                self.cursor.sel_anchor = Some(self.cursor.position());
                            }
                            self.cursor.set_position(row, col);
                        } else {
                            self.extra_cursors.clear();
                            self.cursor.clear_selection();
                            self.cursor.set_position(row, col);
                            self.cursor.sel_anchor = Some((row, col));
                        }
                        self.autocomplete.visible = false;
                    }
                }

                if response.dragged() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        let (row, col) = {
                            let local = pos - rect.min;
                            let r = ((local.y + self.scroll_offset.y) / line_height) as usize;
                            let r = r.min(self.buffer.num_lines().saturating_sub(1));
                            let x_in_text =
                                (local.x - gutter_width + self.scroll_offset.x).max(0.0);
                            let c = (x_in_text / char_width).round() as usize;
                            let c = c.min(self.buffer.line_len(r));
                            (r, c)
                        };
                        self.cursor.set_position(row, col);
                        if self.cursor.sel_anchor.is_none() {
                            self.cursor.sel_anchor = Some((row, col));
                        }
                        // Auto-scroll when dragging near edges
                        let margin = line_height * 2.0;
                        let local_y = pos.y - rect.min.y;
                        if local_y < margin {
                            self.scroll_offset.y = (self.scroll_offset.y - line_height).max(0.0);
                        } else if local_y > rect.height() - margin {
                            self.scroll_offset.y = (self.scroll_offset.y + line_height)
                                .min((total_height - rect.height()).max(0.0));
                        }
                    }
                }

                if response.drag_stopped() && !double_click_handled {
                    if let Some(anchor) = self.cursor.sel_anchor {
                        if anchor == self.cursor.position() {
                            self.cursor.clear_selection();
                        }
                    }
                }

                if response.clicked() && !response.dragged() && !double_click_handled {
                    if let Some(pos) = response.interact_pointer_pos() {
                        let (row, col) = {
                            let local = pos - rect.min;
                            let r = ((local.y + self.scroll_offset.y) / line_height) as usize;
                            let r = r.min(self.buffer.num_lines().saturating_sub(1));
                            let x_in_text =
                                (local.x - gutter_width + self.scroll_offset.x).max(0.0);
                            let c = (x_in_text / char_width).round() as usize;
                            let c = c.min(self.buffer.line_len(r));
                            (r, c)
                        };
                        if ui.input(|i| i.modifiers.ctrl) {
                            // Ctrl+click: navigate to definition of the word under the pointer.
                            if let Some(word) = get_word_at(&self.buffer, row, col) {
                                self.go_to_definition_request = Some(word);
                            }
                        } else if ui.input(|i| i.modifiers.shift) {
                            if self.cursor.sel_anchor.is_none() {
                                self.cursor.sel_anchor = Some(self.cursor.position());
                            }
                            self.cursor.set_position(row, col);
                        } else {
                            self.extra_cursors.clear();
                            self.cursor.clear_selection();
                            self.cursor.set_position(row, col);
                        }
                        self.autocomplete.visible = false;
                    }
                }

                if response.hovered() {
                    ui.input(|i| {
                        self.scroll_offset.y -= i.smooth_scroll_delta.y;
                        self.scroll_offset.y = self
                            .scroll_offset
                            .y
                            .max(0.0)
                            .min((total_height - rect.height()).max(0.0));
                    });
                }

                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, 0.0, bg_color);

                // Scroll viewport to make the cursor visible when requested.
                if self.scroll_to_cursor {
                    let (cur_row, _) = self.cursor.position();
                    let target_y = cur_row as f32 * line_height;
                    if target_y < self.scroll_offset.y {
                        self.scroll_offset.y = target_y;
                    } else if target_y + line_height > self.scroll_offset.y + rect.height() {
                        self.scroll_offset.y = (target_y + line_height - rect.height()).max(0.0);
                    }
                    self.scroll_to_cursor = false;
                }

                let first_visible = (self.scroll_offset.y / line_height) as usize;
                let visible_count = (rect.height() / line_height) as usize + 2;

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
                            let sel_end_col = if line_idx == er {
                                ec
                            } else {
                                self.buffer.line_len(line_idx)
                            };
                            // Measure pixel positions using font layout for accuracy
                            let sx = x_start
                                + ui.fonts(|f| {
                                    let text: String = line.chars().take(sel_start_col).collect();
                                    f.layout_no_wrap(text, font_id.clone(), egui::Color32::WHITE)
                                        .size()
                                        .x
                                });
                            let ex = x_start
                                + ui.fonts(|f| {
                                    let text: String = line.chars().take(sel_end_col).collect();
                                    f.layout_no_wrap(text, font_id.clone(), egui::Color32::WHITE)
                                        .size()
                                        .x
                                });
                            if ex > sx {
                                painter.rect_filled(
                                    egui::Rect::from_min_max(
                                        egui::pos2(sx, y),
                                        egui::pos2(ex, y + line_height),
                                    ),
                                    0.0,
                                    egui::Color32::from_rgba_premultiplied(
                                        accent_color.r(),
                                        accent_color.g(),
                                        accent_color.b(),
                                        60,
                                    ),
                                );
                            }
                        }
                    }

                    // Selection highlight for extra cursors (Ctrl+D multi-selection)
                    for extra_cur in &self.extra_cursors {
                        if let Some(((esr, esc), (eer, eec))) = extra_cur.selection_range() {
                            if line_idx >= esr && line_idx <= eer {
                                let sel_start_col = if line_idx == esr { esc } else { 0 };
                                let sel_end_col = if line_idx == eer {
                                    eec
                                } else {
                                    self.buffer.line_len(line_idx)
                                };
                                let sx = x_start
                                    + ui.fonts(|f| {
                                        let text: String =
                                            line.chars().take(sel_start_col).collect();
                                        f.layout_no_wrap(
                                            text,
                                            font_id.clone(),
                                            egui::Color32::WHITE,
                                        )
                                        .size()
                                        .x
                                    });
                                let ex = x_start
                                    + ui.fonts(|f| {
                                        let text: String = line.chars().take(sel_end_col).collect();
                                        f.layout_no_wrap(
                                            text,
                                            font_id.clone(),
                                            egui::Color32::WHITE,
                                        )
                                        .size()
                                        .x
                                    });
                                if ex > sx {
                                    painter.rect_filled(
                                        egui::Rect::from_min_max(
                                            egui::pos2(sx, y),
                                            egui::pos2(ex, y + line_height),
                                        ),
                                        0.0,
                                        egui::Color32::from_rgba_premultiplied(
                                            accent_color.r(),
                                            accent_color.g(),
                                            accent_color.b(),
                                            60,
                                        ),
                                    );
                                }
                            }
                        }
                    }

                    let (cur_row, cur_col) = self.cursor.position();
                    if line_idx == cur_row {
                        // Cursor: measure actual pixel offset of char col in the line
                        let text_to_cursor: String = line.chars().take(cur_col).collect();
                        let cx = x_start
                            + ui.fonts(|f| {
                                f.layout_no_wrap(
                                    text_to_cursor,
                                    font_id.clone(),
                                    egui::Color32::WHITE,
                                )
                                .size()
                                .x
                            });
                        painter.line_segment(
                            [egui::pos2(cx, y), egui::pos2(cx, y + line_height)],
                            egui::Stroke::new(2.0, cursor_color),
                        );
                        // Update autocomplete popup anchor for this frame.
                        self.autocomplete.cursor_screen_pos = egui::pos2(cx, y + line_height);
                    }

                    // Render extra cursors
                    for extra_cur in &self.extra_cursors {
                        let (ecr, ecc) = extra_cur.position();
                        if line_idx == ecr {
                            let text_to_cur: String = line.chars().take(ecc).collect();
                            let ecx = x_start
                                + ui.fonts(|f| {
                                    f.layout_no_wrap(
                                        text_to_cur,
                                        font_id.clone(),
                                        egui::Color32::WHITE,
                                    )
                                    .size()
                                    .x
                                });
                            painter.line_segment(
                                [egui::pos2(ecx, y), egui::pos2(ecx, y + line_height)],
                                egui::Stroke::new(2.0, accent_color),
                            );
                        }
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
                                color: tok.kind.color(),
                                ..Default::default()
                            },
                        );
                    }
                    let galley = ui.fonts(|f| f.layout_job(job));
                    painter.galley(
                        egui::pos2(x_start, y + line_height * 0.15),
                        galley,
                        fg_color,
                    );

                    // Ctrl+hover underline (VSCode-style go-to-definition hint)
                    if let Some((hover_row, hover_start, hover_end)) = self.ctrl_hover_word_bounds {
                        if line_idx == hover_row {
                            let line = self.buffer.line(line_idx);
                            let underline_x_start = x_start
                                + ui.fonts(|f| {
                                    let text: String = line.chars().take(hover_start).collect();
                                    f.layout_no_wrap(text, font_id.clone(), egui::Color32::WHITE)
                                        .size()
                                        .x
                                });
                            let underline_x_end = x_start
                                + ui.fonts(|f| {
                                    let text: String = line.chars().take(hover_end).collect();
                                    f.layout_no_wrap(text, font_id.clone(), egui::Color32::WHITE)
                                        .size()
                                        .x
                                });
                            let underline_y = y + line_height - 2.0;
                            painter.line_segment(
                                [
                                    egui::pos2(underline_x_start, underline_y),
                                    egui::pos2(underline_x_end, underline_y),
                                ],
                                egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 160, 255)),
                            );
                        }
                    }

                    // Diagnostic squiggly underlines
                    for diag in &self.diagnostics {
                        if diag.line as usize == line_idx {
                            let underline_y = y + line_height - 2.0;
                            let x_diag_start = x_start + diag.col as f32 * char_width;
                            let x_diag_end = (x_diag_start + 100.0).min(rect.right());
                            let color = match diag.severity {
                                crate::lsp::client::DiagSeverity::Error => {
                                    egui::Color32::from_rgb(255, 80, 80)
                                }
                                crate::lsp::client::DiagSeverity::Warning => {
                                    egui::Color32::from_rgb(255, 200, 0)
                                }
                                _ => egui::Color32::from_rgb(100, 150, 255),
                            };
                            let amp = 1.5_f32;
                            let period = 4.0_f32;
                            let mut x = x_diag_start;
                            while x < x_diag_end {
                                let x1 = x;
                                let y1 = underline_y
                                    + amp * ((x / period * std::f32::consts::PI).sin());
                                let x2 = (x + period / 2.0).min(x_diag_end);
                                let y2 = underline_y - amp;
                                painter.line_segment(
                                    [egui::pos2(x1, y1), egui::pos2(x2, y2)],
                                    egui::Stroke::new(1.0, color),
                                );
                                x = x2;
                            }
                        }
                    }
                }

                ui.memory_mut(|m| m.request_focus(response.id));

                // Render hover signature tooltip.
                if let Some(sig) = &self.hover_signature {
                    if !sig.is_empty() {
                        let sig = sig.clone();
                        let tooltip_painter = ui.ctx().layer_painter(egui::LayerId::new(
                            egui::Order::Tooltip,
                            egui::Id::new("hover_sig_tooltip"),
                        ));
                        let galley = ui.fonts(|f| {
                            f.layout_no_wrap(
                                sig.clone(),
                                egui::FontId::monospace(13.0),
                                egui::Color32::from_rgb(212, 212, 212),
                            )
                        });
                        let padding = egui::vec2(10.0, 6.0);
                        let text_size = galley.size();
                        let box_size = text_size + padding * 2.0;

                        // Position tooltip below-right of the mouse; keep inside screen bounds.
                        let screen_rect = ui.ctx().screen_rect();
                        let mut tl = self.hover_pos + egui::vec2(12.0, 18.0);
                        if tl.x + box_size.x > screen_rect.right() - 4.0 {
                            tl.x = (screen_rect.right() - box_size.x - 4.0).max(screen_rect.left());
                        }
                        if tl.y + box_size.y > screen_rect.bottom() - 4.0 {
                            tl.y = self.hover_pos.y - box_size.y - 4.0;
                        }
                        let bg_rect = egui::Rect::from_min_size(tl, box_size);

                        tooltip_painter.rect_filled(
                            bg_rect,
                            4.0,
                            egui::Color32::from_rgb(30, 30, 30),
                        );
                        tooltip_painter.rect_stroke(
                            bg_rect,
                            4.0,
                            egui::Stroke::new(1.0, egui::Color32::from_gray(80)),
                            egui::StrokeKind::Inside,
                        );
                        tooltip_painter.galley(
                            tl + padding,
                            galley,
                            egui::Color32::from_rgb(212, 212, 212),
                        );
                    }
                }

                // Render autocomplete popup on top of editor content.
                self.autocomplete.show(ui.ctx());
            });
    }
}
