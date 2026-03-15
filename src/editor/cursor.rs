use super::buffer::Buffer;

pub struct Cursor {
    pub row:        usize,
    pub col:        usize,
    pub desired_col: usize,
    pub sel_anchor: Option<(usize, usize)>,
}

impl Cursor {
    pub fn new() -> Self {
        Self { row: 0, col: 0, desired_col: 0, sel_anchor: None }
    }

    pub fn position(&self) -> (usize, usize) {
        (self.row, self.col)
    }

    pub fn set_position(&mut self, row: usize, col: usize) {
        self.row = row;
        self.col = col;
        self.desired_col = col;
    }

    pub fn start_selection(&mut self) {
        if self.sel_anchor.is_none() {
            self.sel_anchor = Some((self.row, self.col));
        }
    }

    pub fn clear_selection(&mut self) {
        self.sel_anchor = None;
    }

    pub fn has_selection(&self) -> bool {
        self.sel_anchor.is_some()
    }

    /// Returns normalized (start, end) in (row, col) order.
    pub fn selection_range(&self) -> Option<((usize, usize), (usize, usize))> {
        let anchor = self.sel_anchor?;
        let cursor = (self.row, self.col);
        if anchor <= cursor { Some((anchor, cursor)) } else { Some((cursor, anchor)) }
    }

    pub fn move_left(&mut self, buf: &Buffer) {
        self.clear_selection();
        if self.col > 0 {
            self.col -= 1;
        } else if self.row > 0 {
            self.row -= 1;
            self.col = buf.line_len(self.row);
        }
        self.desired_col = self.col;
    }

    pub fn move_right(&mut self, buf: &Buffer) {
        self.clear_selection();
        let line_len = buf.line_len(self.row);
        if self.col < line_len {
            self.col += 1;
        } else if self.row + 1 < buf.num_lines() {
            self.row += 1;
            self.col = 0;
        }
        self.desired_col = self.col;
    }

    pub fn move_up(&mut self, buf: &Buffer) {
        self.clear_selection();
        if self.row > 0 {
            self.row -= 1;
            self.col = self.desired_col.min(buf.line_len(self.row));
        }
    }

    pub fn move_down(&mut self, buf: &Buffer) {
        self.clear_selection();
        if self.row + 1 < buf.num_lines() {
            self.row += 1;
            self.col = self.desired_col.min(buf.line_len(self.row));
        }
    }

    pub fn move_left_select(&mut self, buf: &Buffer) {
        self.start_selection();
        if self.col > 0 {
            self.col -= 1;
        } else if self.row > 0 {
            self.row -= 1;
            self.col = buf.line_len(self.row);
        }
        self.desired_col = self.col;
    }

    pub fn move_right_select(&mut self, buf: &Buffer) {
        self.start_selection();
        let line_len = buf.line_len(self.row);
        if self.col < line_len {
            self.col += 1;
        } else if self.row + 1 < buf.num_lines() {
            self.row += 1;
            self.col = 0;
        }
        self.desired_col = self.col;
    }

    pub fn move_up_select(&mut self, buf: &Buffer) {
        self.start_selection();
        if self.row > 0 {
            self.row -= 1;
            self.col = self.desired_col.min(buf.line_len(self.row));
        }
    }

    pub fn move_down_select(&mut self, buf: &Buffer) {
        self.start_selection();
        if self.row + 1 < buf.num_lines() {
            self.row += 1;
            self.col = self.desired_col.min(buf.line_len(self.row));
        }
    }
}
