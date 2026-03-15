use std::io::{Read, Write};
use crossbeam_channel::{Receiver, Sender, unbounded};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use vte::{Params, Parser, Perform};

#[derive(Clone)]
struct Span {
    text: String,
    fg:   egui::Color32,
}

const DEFAULT_FG: egui::Color32 = egui::Color32::from_rgb(212, 212, 212);

struct AnsiPerformer {
    spans:       Vec<Span>,
    current_fg:  egui::Color32,
    current_buf: String,
}

impl AnsiPerformer {
    fn new() -> Self {
        Self {
            spans: vec![Span { text: String::new(), fg: DEFAULT_FG }],
            current_fg: DEFAULT_FG,
            current_buf: String::new(),
        }
    }

    fn push_text(&mut self, text: &str) {
        if text.is_empty() { return; }
        if let Some(last) = self.spans.last_mut() {
            if last.fg == self.current_fg {
                last.text.push_str(text);
                return;
            }
        }
        self.spans.push(Span { text: text.to_string(), fg: self.current_fg });
    }

    fn flush_buf(&mut self) {
        let buf = std::mem::take(&mut self.current_buf);
        self.push_text(&buf);
    }

    fn set_color(&mut self, code: u16) {
        self.current_fg = match code {
            0  => DEFAULT_FG,
            1  => egui::Color32::WHITE,
            30 => egui::Color32::from_rgb(  0,   0,   0),
            31 => egui::Color32::from_rgb(205,  49,  49),
            32 => egui::Color32::from_rgb( 13, 188, 121),
            33 => egui::Color32::from_rgb(229, 229,  16),
            34 => egui::Color32::from_rgb( 36, 114, 200),
            35 => egui::Color32::from_rgb(188,  63, 188),
            36 => egui::Color32::from_rgb( 17, 168, 205),
            37 => egui::Color32::from_rgb(229, 229, 229),
            90 => egui::Color32::from_rgb(102, 102, 102),
            91 => egui::Color32::from_rgb(241,  76,  76),
            92 => egui::Color32::from_rgb( 35, 209, 139),
            93 => egui::Color32::from_rgb(245, 245,  67),
            94 => egui::Color32::from_rgb( 59, 142, 234),
            95 => egui::Color32::from_rgb(214, 112, 214),
            96 => egui::Color32::from_rgb( 41, 184, 219),
            97 => egui::Color32::WHITE,
            _  => return,
        };
    }
}

impl Perform for AnsiPerformer {
    fn print(&mut self, c: char) { self.current_buf.push(c); }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n'   => { self.current_buf.push('\n'); self.flush_buf(); }
            b'\r'   => {}
            b'\x08' => { self.current_buf.pop(); }
            _       => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, _: &[u8], _: bool, action: char) {
        if action == 'm' {
            self.flush_buf();
            for param in params.iter() {
                if !param.is_empty() { self.set_color(param[0]); }
            }
        }
    }

    fn hook(&mut self, _: &Params, _: &[u8], _: bool, _: char) {}
    fn put(&mut self, _: u8) {}
    fn unhook(&mut self) {}
    fn osc_dispatch(&mut self, _: &[&[u8]], _: bool) {}
    fn esc_dispatch(&mut self, _: &[u8], _: bool, _: u8) {}
}

pub struct Terminal {
    pub input_buf: String,
    spans:         Vec<Span>,
    rx:            Option<Receiver<Vec<u8>>>,
    writer:        Option<Box<dyn Write + Send>>,
    parser: Parser,
    performer:     AnsiPerformer,
    _child:        Option<Box<dyn portable_pty::Child + Send + Sync>>,
}

impl Terminal {
    pub fn new() -> Self {
        let (rx, writer, child) = Self::spawn_shell();
        let mut performer = AnsiPerformer::new();
        performer.push_text("Terminal ready. Type commands below.\n");
        let spans = performer.spans.clone();
        Self {
            input_buf: String::new(),
            spans,
            rx,
            writer,
            parser: Parser::new(),
            performer,
            _child: child,
        }
    }

    fn spawn_shell() -> (
        Option<Receiver<Vec<u8>>>,
        Option<Box<dyn Write + Send>>,
        Option<Box<dyn portable_pty::Child + Send + Sync>>,
    ) {
        let pty_system = native_pty_system();
        let size = PtySize { rows: 24, cols: 200, pixel_width: 0, pixel_height: 0 };

        let pair = match pty_system.openpty(size) {
            Ok(p)  => p,
            Err(_) => return (None, None, None),
        };

        let shell = if cfg!(windows) {
            "cmd.exe".to_string()
        } else {
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
        };

        let mut cmd = CommandBuilder::new(&shell);
        cmd.env("TERM", "xterm-256color");

        let child = match pair.slave.spawn_command(cmd) {
            Ok(c)  => c,
            Err(_) => return (None, None, None),
        };

        let reader = match pair.master.try_clone_reader() {
            Ok(r)  => r,
            Err(_) => return (None, None, None),
        };
        let writer = match pair.master.take_writer() {
            Ok(w)  => w,
            Err(_) => return (None, None, None),
        };

        let (tx, rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = unbounded();
        std::thread::spawn(move || {
            let mut reader = reader;
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => { if tx.send(buf[..n].to_vec()).is_err() { break; } }
                }
            }
        });

        (Some(rx), Some(writer), Some(child))
    }

    fn update(&mut self) {
        let mut bytes_to_process: Vec<Vec<u8>> = Vec::new();
        if let Some(rx) = &self.rx {
            while let Ok(chunk) = rx.try_recv() {
                bytes_to_process.push(chunk);
            }
        }
        if !bytes_to_process.is_empty() {
            for chunk in bytes_to_process {
                for byte in chunk {
                    self.parser.advance(&mut self.performer, byte);
                }
            }
            self.performer.flush_buf();
            self.spans = self.performer.spans.clone();
        }
    }

    pub fn send_input(&mut self, input: &str) {
        if let Some(w) = &mut self.writer {
            let _ = w.write_all(input.as_bytes());
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.update();

        let bg = egui::Color32::from_rgb(20, 20, 20);
        egui::Frame::new().fill(bg).show(ui, |ui| {
            ui.vertical(|ui| {
                let output_height = (ui.available_height() - 30.0).max(40.0);

                egui::ScrollArea::vertical()
                    .max_height(output_height)
                    .stick_to_bottom(true)
                    .id_salt("term_scroll")
                    .show(ui, |ui| {
                        let mut job = egui::text::LayoutJob::default();
                        for span in &self.spans {
                            job.append(
                                &span.text,
                                0.0,
                                egui::TextFormat {
                                    font_id: egui::FontId::monospace(13.0),
                                    color: span.fg,
                                    ..Default::default()
                                },
                            );
                        }
                        ui.label(job);
                    });

                ui.separator();

                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("❯")
                            .color(egui::Color32::from_rgb(35, 209, 139))
                            .monospace(),
                    );
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.input_buf)
                            .frame(false)
                            .desired_width(ui.available_width())
                            .font(egui::TextStyle::Monospace),
                    );
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let line = self.input_buf.clone() + "\n";
                        self.send_input(&line);
                        self.input_buf.clear();
                        resp.request_focus();
                    }
                });
            });
        });
    }
}
