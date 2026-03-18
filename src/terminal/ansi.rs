use super::screen_buffer::ScreenBuffer;
use vte::{Params, Perform};

pub(super) struct AnsiPerformer {
    pub(super) buf: ScreenBuffer,
}

impl AnsiPerformer {
    pub(super) fn new() -> Self {
        Self {
            buf: ScreenBuffer::new(200, 50),
        }
    }
}

impl Perform for AnsiPerformer {
    fn print(&mut self, c: char) {
        self.buf.write_char(c);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' => self.buf.line_feed(),
            b'\r' => self.buf.carriage_return(),
            b'\x08' => {
                if self.buf.cursor_col > 0 {
                    self.buf.cursor_col -= 1;
                }
            }
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, _: &[u8], _: bool, action: char) {
        let ns: Vec<u16> = params
            .iter()
            .map(|p| p.first().copied().unwrap_or(0))
            .collect();
        let n0 = ns.first().copied().unwrap_or(0);
        let n1 = ns.get(1).copied().unwrap_or(0);
        match action {
            'A' => self.buf.move_cursor('A', n0.max(1) as usize),
            'B' => self.buf.move_cursor('B', n0.max(1) as usize),
            'C' => self.buf.move_cursor('C', n0.max(1) as usize),
            'D' => self.buf.move_cursor('D', n0.max(1) as usize),
            'H' | 'f' => self.buf.set_cursor_pos(n0 as usize, n1 as usize),
            'J' => self.buf.erase_display(n0),
            'K' => self.buf.erase_line(n0),
            'm' => self.buf.set_sgr(&ns),
            'l' | 'h' => {}
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _: &[u8], _: bool, byte: u8) {
        if byte == b'M' && self.buf.cursor_row > 0 {
            self.buf.cursor_row -= 1;
        }
    }

    fn hook(&mut self, _: &Params, _: &[u8], _: bool, _: char) {}
    fn put(&mut self, _: u8) {}
    fn unhook(&mut self) {}
    fn osc_dispatch(&mut self, _: &[&[u8]], _: bool) {}
}
