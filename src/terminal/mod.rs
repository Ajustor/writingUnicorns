use crossbeam_channel::{unbounded, Receiver, Sender};
use egui::Color32;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use vte::{Params, Parser, Perform};

const DEFAULT_FG: Color32 = Color32::from_rgb(212, 212, 212);

// ─── Cell & ScreenBuffer ──────────────────────────────────────────────────────

#[derive(Clone)]
struct Cell {
    ch: char,
    fg: Color32,
    bold: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: DEFAULT_FG,
            bold: false,
        }
    }
}

struct ScreenBuffer {
    /// Visible rows — fixed to terminal dimensions.
    rows: Vec<Vec<Cell>>,
    /// Lines that have scrolled off the top.
    scrollback: Vec<Vec<Cell>>,
    cursor_row: usize,
    cursor_col: usize,
    cols: usize,
    term_rows: usize,
    current_fg: Color32,
    current_bold: bool,
    max_scrollback: usize,
}

impl ScreenBuffer {
    fn new(cols: usize, rows: usize) -> Self {
        Self {
            rows: (0..rows).map(|_| vec![Cell::default(); cols]).collect(),
            scrollback: Vec::new(),
            cursor_row: 0,
            cursor_col: 0,
            cols,
            term_rows: rows,
            current_fg: DEFAULT_FG,
            current_bold: false,
            max_scrollback: 10_000,
        }
    }

    fn write_char(&mut self, ch: char) {
        if self.cursor_col >= self.cols {
            self.line_feed();
            self.cursor_col = 0;
        }
        if let Some(row) = self.rows.get_mut(self.cursor_row) {
            if let Some(cell) = row.get_mut(self.cursor_col) {
                *cell = Cell {
                    ch,
                    fg: self.current_fg,
                    bold: self.current_bold,
                };
            }
        }
        self.cursor_col += 1;
    }

    fn carriage_return(&mut self) {
        self.cursor_col = 0;
    }

    fn line_feed(&mut self) {
        if self.cursor_row + 1 >= self.term_rows {
            // Scroll: push top visible row into scrollback.
            let top = self.rows.remove(0);
            self.scrollback.push(top);
            if self.scrollback.len() > self.max_scrollback {
                let excess = self.scrollback.len() - self.max_scrollback;
                self.scrollback.drain(0..excess);
            }
            self.rows.push(vec![Cell::default(); self.cols]);
        } else {
            self.cursor_row += 1;
        }
    }

    fn move_cursor(&mut self, dir: char, n: usize) {
        match dir {
            'A' => self.cursor_row = self.cursor_row.saturating_sub(n),
            'B' => {
                self.cursor_row = (self.cursor_row + n).min(self.term_rows.saturating_sub(1));
            }
            'C' => {
                self.cursor_col = (self.cursor_col + n).min(self.cols.saturating_sub(1));
            }
            'D' => self.cursor_col = self.cursor_col.saturating_sub(n),
            _ => {}
        }
    }

    /// Set cursor position from a 1-indexed CSI H/f sequence (0 treated as 1).
    fn set_cursor_pos(&mut self, row: usize, col: usize) {
        let r = row.max(1) - 1;
        let c = col.max(1) - 1;
        self.cursor_row = r.min(self.term_rows.saturating_sub(1));
        self.cursor_col = c.min(self.cols.saturating_sub(1));
    }

    /// Erase display: param 0 = below cursor, 1 = above cursor, 2/3 = whole screen.
    fn erase_display(&mut self, param: u16) {
        let (crow, ccol) = (self.cursor_row, self.cursor_col);
        match param {
            0 => {
                for c in ccol..self.cols {
                    self.rows[crow][c] = Cell::default();
                }
                for r in (crow + 1)..self.term_rows {
                    self.rows[r] = vec![Cell::default(); self.cols];
                }
            }
            1 => {
                for r in 0..crow {
                    self.rows[r] = vec![Cell::default(); self.cols];
                }
                for c in 0..=ccol.min(self.cols.saturating_sub(1)) {
                    self.rows[crow][c] = Cell::default();
                }
            }
            2 | 3 => {
                for r in 0..self.term_rows {
                    self.rows[r] = vec![Cell::default(); self.cols];
                }
                self.cursor_row = 0;
                self.cursor_col = 0;
            }
            _ => {}
        }
    }

    /// Erase line: param 0 = to end, 1 = to start, 2 = entire line.
    fn erase_line(&mut self, param: u16) {
        let (crow, ccol) = (self.cursor_row, self.cursor_col);
        match param {
            0 => {
                for c in ccol..self.cols {
                    self.rows[crow][c] = Cell::default();
                }
            }
            1 => {
                for c in 0..=ccol.min(self.cols.saturating_sub(1)) {
                    self.rows[crow][c] = Cell::default();
                }
            }
            2 => {
                self.rows[crow] = vec![Cell::default(); self.cols];
            }
            _ => {}
        }
    }

    fn set_sgr(&mut self, params: &[u16]) {
        let mut i = 0;
        while i < params.len() {
            match params[i] {
                0 => {
                    self.current_fg = DEFAULT_FG;
                    self.current_bold = false;
                }
                1 => self.current_bold = true,
                22 => self.current_bold = false,
                39 => self.current_fg = DEFAULT_FG,
                30..=37 => self.current_fg = ansi_color(params[i] - 30, false),
                90..=97 => self.current_fg = ansi_color(params[i] - 90, true),
                38 if params.get(i + 1) == Some(&5) => {
                    if let Some(&n) = params.get(i + 2) {
                        self.current_fg = color_256(n);
                        i += 2;
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }
}

// ─── Color helpers ────────────────────────────────────────────────────────────

fn ansi_color(idx: u16, bright: bool) -> Color32 {
    match (idx, bright) {
        (0, false) => Color32::from_rgb(0, 0, 0),
        (1, false) => Color32::from_rgb(205, 49, 49),
        (2, false) => Color32::from_rgb(13, 188, 121),
        (3, false) => Color32::from_rgb(229, 229, 16),
        (4, false) => Color32::from_rgb(36, 114, 200),
        (5, false) => Color32::from_rgb(188, 63, 188),
        (6, false) => Color32::from_rgb(17, 168, 205),
        (7, false) => Color32::from_rgb(229, 229, 229),
        (0, true) => Color32::from_rgb(102, 102, 102),
        (1, true) => Color32::from_rgb(241, 76, 76),
        (2, true) => Color32::from_rgb(35, 209, 139),
        (3, true) => Color32::from_rgb(245, 245, 67),
        (4, true) => Color32::from_rgb(59, 142, 234),
        (5, true) => Color32::from_rgb(214, 112, 214),
        (6, true) => Color32::from_rgb(41, 184, 219),
        (7, true) => Color32::from_rgb(229, 229, 229),
        _ => Color32::from_rgb(212, 212, 212),
    }
}

fn color_256(n: u16) -> Color32 {
    match n {
        0..=7 => ansi_color(n, false),
        8..=15 => ansi_color(n - 8, true),
        16..=231 => {
            let n = n - 16;
            let b = n % 6;
            let g = (n / 6) % 6;
            let r = n / 36;
            let c = |x: u16| -> u8 {
                if x == 0 {
                    0
                } else {
                    (55 + x * 40) as u8
                }
            };
            Color32::from_rgb(c(r), c(g), c(b))
        }
        232..=255 => {
            let v = (8 + (n - 232) * 10) as u8;
            Color32::from_rgb(v, v, v)
        }
        _ => Color32::from_rgb(212, 212, 212),
    }
}

// ─── AnsiPerformer ────────────────────────────────────────────────────────────

struct AnsiPerformer {
    buf: ScreenBuffer,
}

impl AnsiPerformer {
    fn new() -> Self {
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
            'l' | 'h' => {} // private modes (bracketed paste etc.) — ignore
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _: &[u8], _: bool, byte: u8) {
        // Reverse index: scroll up one line
        if byte == b'M' && self.buf.cursor_row > 0 {
            self.buf.cursor_row -= 1;
        }
    }

    fn hook(&mut self, _: &Params, _: &[u8], _: bool, _: char) {}
    fn put(&mut self, _: u8) {}
    fn unhook(&mut self) {}
    fn osc_dispatch(&mut self, _: &[&[u8]], _: bool) {}
}

// ─── Shell resolution ─────────────────────────────────────────────────────────

/// Returns `(shell_path, extra_args)` for the current OS.
fn resolve_shell() -> (String, Vec<String>) {
    #[cfg(windows)]
    {
        let ps =
            std::path::Path::new("C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe");
        if ps.exists() {
            return (
                "powershell.exe".to_string(),
                vec!["powershell.exe".to_string(), "-NoExit".to_string()],
            );
        }
        return (
            "cmd.exe".to_string(),
            vec!["cmd.exe".to_string(), "/K".to_string()],
        );
    }

    #[cfg(not(windows))]
    {
        let shell = std::env::var("SHELL")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| {
                if std::path::Path::new("/bin/bash").exists() {
                    Some("/bin/bash".to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "/bin/sh".to_string());
        let args = vec![shell.clone(), "-l".to_string()];
        (shell, args)
    }
}

// ─── Terminal ─────────────────────────────────────────────────────────────────

pub struct Terminal {
    pub input_buf: String,
    pub shell_name: String,
    performer: AnsiPerformer,
    rx: Option<Receiver<Vec<u8>>>,
    writer: Option<Box<dyn Write + Send>>,
    parser: Parser,
    _child: Option<Box<dyn portable_pty::Child + Send + Sync>>,
    /// Set to true when new output arrives — triggers a one-shot scroll to bottom.
    needs_scroll: bool,
}

impl Terminal {
    pub fn new() -> Self {
        let (rx, writer, child, shell_name) = Self::spawn_shell();
        let mut parser = Parser::new();
        let mut performer = AnsiPerformer::new();
        for byte in "Terminal ready. Type commands below.\r\n".bytes() {
            parser.advance(&mut performer, byte);
        }
        Self {
            input_buf: String::new(),
            shell_name,
            performer,
            rx,
            writer,
            parser,
            _child: child,
            needs_scroll: true,
        }
    }

    #[allow(clippy::type_complexity)]
    fn spawn_shell() -> (
        Option<Receiver<Vec<u8>>>,
        Option<Box<dyn Write + Send>>,
        Option<Box<dyn portable_pty::Child + Send + Sync>>,
        String,
    ) {
        let (shell_path, shell_args) = resolve_shell();
        let shell_name = std::path::Path::new(&shell_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("shell")
            .to_string();

        let pty_system = native_pty_system();
        let size = PtySize {
            rows: 50,
            cols: 200,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = match pty_system.openpty(size) {
            Ok(p) => p,
            Err(_) => return (None, None, None, shell_name),
        };

        let mut cmd = CommandBuilder::new(&shell_path);
        for arg in shell_args.iter().skip(1) {
            cmd.arg(arg);
        }
        cmd.env("TERM", "xterm-256color");

        let child = match pair.slave.spawn_command(cmd) {
            Ok(c) => c,
            Err(_) => return (None, None, None, shell_name),
        };

        let reader = match pair.master.try_clone_reader() {
            Ok(r) => r,
            Err(_) => return (None, None, None, shell_name),
        };
        let writer = match pair.master.take_writer() {
            Ok(w) => w,
            Err(_) => return (None, None, None, shell_name),
        };

        let (tx, rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = unbounded();
        std::thread::spawn(move || {
            let mut reader = reader;
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        (Some(rx), Some(writer), Some(child), shell_name)
    }

    fn update(&mut self) {
        let mut got_bytes = false;
        if let Some(rx) = &self.rx {
            while let Ok(chunk) = rx.try_recv() {
                for byte in chunk {
                    self.parser.advance(&mut self.performer, byte);
                }
                got_bytes = true;
            }
        }
        if got_bytes {
            self.needs_scroll = true;
        }
    }

    pub fn send_input(&mut self, input: &str) {
        if let Some(w) = &mut self.writer {
            let _ = w.write_all(input.as_bytes());
        }
    }

    /// Signals the terminal to scroll to the bottom on the next render frame.
    pub fn scroll_to_bottom(&mut self) {
        self.needs_scroll = true;
    }

    /// Renders just the terminal output and input area (no header/tab bar).
    pub fn show_content(&mut self, ui: &mut egui::Ui, config: &crate::config::Config) {
        self.update();

        let term_bg = egui::Color32::from_rgb(
            config.theme.background[0],
            config.theme.background[1],
            config.theme.background[2],
        );
        let default_fg = egui::Color32::from_rgb(
            config.theme.foreground[0],
            config.theme.foreground[1],
            config.theme.foreground[2],
        );

        let scroll_to_bottom = self.needs_scroll;
        self.needs_scroll = false;

        // Collect keyboard state before rendering closures to avoid borrow conflicts.
        let (want_ctrl_c, want_ctrl_d, want_up, want_down) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::C) && i.modifiers.ctrl,
                i.key_pressed(egui::Key::D) && i.modifiers.ctrl,
                i.key_pressed(egui::Key::ArrowUp) && !i.modifiers.any(),
                i.key_pressed(egui::Key::ArrowDown) && !i.modifiers.any(),
            )
        });

        let mut input_submitted = false;
        let mut want_tab = false;
        let mut text_edit_focused = false;

        egui::Frame::new()
            .fill(term_bg)
            .inner_margin(egui::Margin {
                left: 8,
                right: 8,
                top: 4,
                bottom: 4,
            })
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    let output_height = (ui.available_height() - 32.0).max(40.0);
                    const LINE_HEIGHT: f32 = 13.5;

                    egui::ScrollArea::vertical()
                        .max_height(output_height)
                        .auto_shrink([false, false])
                        .id_salt("term_scroll")
                        .stick_to_bottom(scroll_to_bottom)
                        .show(ui, |ui| {
                            ui.style_mut().spacing.item_spacing.y = 0.0;

                            // Find last non-empty row in the visible screen.
                            let last_screen_row = self
                                .performer
                                .buf
                                .rows
                                .iter()
                                .rposition(|row| {
                                    row.iter()
                                        .any(|c| c.ch != ' ' || c.fg != DEFAULT_FG || c.bold)
                                })
                                .map_or(0, |i| i + 1);

                            for row in &self.performer.buf.scrollback {
                                render_row(ui, row, LINE_HEIGHT, default_fg, term_bg);
                            }
                            for row in self.performer.buf.rows[..last_screen_row].iter() {
                                render_row(ui, row, LINE_HEIGHT, default_fg, term_bg);
                            }
                        });

                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("❯")
                                .color(egui::Color32::from_rgb(35, 209, 139))
                                .size(13.5)
                                .monospace(),
                        );
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut self.input_buf)
                                .frame(false)
                                .desired_width(ui.available_width())
                                .font(egui::FontId::monospace(13.0)),
                        );
                        text_edit_focused = resp.has_focus();

                        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            input_submitted = true;
                            resp.request_focus();
                        }

                        if resp.has_focus() {
                            ui.input_mut(|i| {
                                if i.key_pressed(egui::Key::Tab) {
                                    i.events.retain(|e| {
                                        !matches!(
                                            e,
                                            egui::Event::Key {
                                                key: egui::Key::Tab,
                                                pressed: true,
                                                ..
                                            }
                                        )
                                    });
                                    want_tab = true;
                                }
                            });
                        }
                    });
                });
            });

        // Process queued actions after all closures have run.
        if input_submitted {
            let line = self.input_buf.clone() + "\n";
            self.send_input(&line);
            self.input_buf.clear();
        }
        if want_tab {
            self.send_input("\t");
        }
        if want_ctrl_c {
            self.send_input("\x03");
            self.input_buf.clear();
        }
        if want_ctrl_d {
            self.send_input("\x04");
        }
        if want_up && text_edit_focused {
            self.send_input("\x1b[A");
        }
        if want_down && text_edit_focused {
            self.send_input("\x1b[B");
        }
    }
}

// ─── Rendering helper ─────────────────────────────────────────────────────────

/// Render one row of the screen buffer as a single egui label.
/// Groups consecutive cells with the same fg/bold into text runs.
fn render_row(
    ui: &mut egui::Ui,
    row: &[Cell],
    line_height: f32,
    default_fg: Color32,
    term_bg: Color32,
) {
    // Find the last non-blank cell to avoid trailing whitespace allocations.
    let last = row
        .iter()
        .rposition(|c| c.ch != ' ' || c.fg != DEFAULT_FG || c.bold)
        .map_or(0, |i| i + 1);

    if last == 0 {
        ui.add_space(line_height);
        return;
    }

    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;

    let mut i = 0;
    while i < last {
        let fg = row[i].fg;
        let bold = row[i].bold;
        let mut j = i + 1;
        while j < last && row[j].fg == fg && row[j].bold == bold {
            j += 1;
        }
        let text: String = row[i..j].iter().map(|c| c.ch).collect();
        let color = if fg == DEFAULT_FG { default_fg } else { fg };
        job.append(
            &text,
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::monospace(13.5),
                color,
                background: term_bg,
                ..Default::default()
            },
        );
        i = j;
    }
    ui.label(job);
}
