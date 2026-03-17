use std::path::{Path, PathBuf};
use std::sync::mpsc;

#[derive(Clone)]
pub struct SearchMatch {
    pub file_path: PathBuf,
    pub line_number: usize,
    pub line_text: String,
    pub match_start: usize,
    pub match_end: usize,
}

pub struct SearchResults {
    pub query: String,
    pub matches: Vec<SearchMatch>,
    pub searched_files: usize,
    pub elapsed_ms: u64,
}

pub struct WorkspaceSearch {
    pub query: String,
    pub case_sensitive: bool,
    pub results: Option<SearchResults>,
    pub is_searching: bool,
    rx: Option<mpsc::Receiver<SearchResults>>,
    pub selected_match: Option<usize>,
}

impl WorkspaceSearch {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            case_sensitive: false,
            results: None,
            is_searching: false,
            rx: None,
            selected_match: None,
        }
    }

    pub fn start_search(&mut self, workspace: PathBuf) {
        if self.query.is_empty() {
            return;
        }
        self.is_searching = true;
        self.results = None;
        let query = self.query.clone();
        let case_sensitive = self.case_sensitive;
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);

        std::thread::spawn(move || {
            let start = std::time::Instant::now();
            let mut matches = Vec::new();
            let mut searched = 0;
            search_dir(
                &workspace,
                &query,
                case_sensitive,
                &mut matches,
                &mut searched,
            );
            let _ = tx.send(SearchResults {
                query,
                matches,
                searched_files: searched,
                elapsed_ms: start.elapsed().as_millis() as u64,
            });
        });
    }

    pub fn poll(&mut self) {
        if let Some(rx) = &self.rx {
            if let Ok(results) = rx.try_recv() {
                self.results = Some(results);
                self.is_searching = false;
                self.rx = None;
            }
        }
    }

    /// Render the search panel. Returns `Some((path, line))` when a result is clicked.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        workspace: Option<&PathBuf>,
    ) -> Option<(PathBuf, usize)> {
        self.poll();
        let mut open_file: Option<(PathBuf, usize)> = None;

        ui.horizontal(|ui| {
            ui.label("🔍");
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .hint_text("Search in workspace…")
                    .desired_width(ui.available_width() - 40.0),
            );
            if (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                || resp.changed()
            {
                if let Some(ws) = workspace {
                    if !self.query.is_empty() {
                        self.start_search(ws.clone());
                    }
                }
            }
        });

        ui.horizontal(|ui| {
            ui.checkbox(&mut self.case_sensitive, "Aa");
            ui.label(
                egui::RichText::new("Case sensitive")
                    .size(11.0)
                    .color(egui::Color32::GRAY),
            );
            if ui.small_button("Search").clicked() {
                if let Some(ws) = workspace {
                    if !self.query.is_empty() {
                        self.start_search(ws.clone());
                    }
                }
            }
        });

        ui.add_space(4.0);

        if self.is_searching {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(
                    egui::RichText::new("Searching…")
                        .color(egui::Color32::GRAY)
                        .size(11.0),
                );
            });
        } else if let Some(results) = &self.results {
            let count = results.matches.len();
            ui.label(
                egui::RichText::new(format!(
                    "{} result{} in {} file{} ({} ms)",
                    count,
                    if count == 1 { "" } else { "s" },
                    results.searched_files,
                    if results.searched_files == 1 { "" } else { "s" },
                    results.elapsed_ms,
                ))
                .size(11.0)
                .color(egui::Color32::GRAY),
            );
        } else if workspace.is_none() {
            ui.label(
                egui::RichText::new("Open a folder to search")
                    .color(egui::Color32::GRAY)
                    .size(11.0),
            );
        }

        ui.add_space(4.0);
        ui.separator();

        if let Some(results) = &self.results {
            // Group matches by file while preserving order
            let mut by_file: Vec<(PathBuf, Vec<&SearchMatch>)> = Vec::new();
            for m in &results.matches {
                if let Some(entry) = by_file.iter_mut().find(|(p, _)| p == &m.file_path) {
                    entry.1.push(m);
                } else {
                    by_file.push((m.file_path.clone(), vec![m]));
                }
            }

            egui::ScrollArea::vertical().show(ui, |ui| {
                for (file_path, file_matches) in &by_file {
                    let file_name = file_path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| file_path.to_string_lossy().to_string());

                    ui.collapsing(
                        egui::RichText::new(format!("{} ({})", file_name, file_matches.len()))
                            .size(12.0)
                            .color(egui::Color32::from_rgb(100, 160, 255)),
                        |ui| {
                            for m in file_matches {
                                let text = format!("{}: {}", m.line_number, m.line_text.trim());
                                let truncated = if text.len() > 80 { &text[..80] } else { &text };

                                let resp = ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(truncated)
                                            .monospace()
                                            .size(11.0)
                                            .color(egui::Color32::from_rgb(200, 200, 200)),
                                    )
                                    .sense(egui::Sense::click()),
                                );
                                if resp.clicked() {
                                    open_file = Some((
                                        m.file_path.clone(),
                                        m.line_number.saturating_sub(1),
                                    ));
                                }
                                if resp.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                            }
                        },
                    );
                }
            });
        }

        open_file
    }
}

fn search_dir(
    dir: &Path,
    query: &str,
    case_sensitive: bool,
    matches: &mut Vec<SearchMatch>,
    searched: &mut usize,
) {
    let dir_entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in dir_entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if name_str.starts_with('.') {
            continue;
        }
        if matches!(
            name_str.as_ref(),
            "target" | "node_modules" | ".git" | "dist" | "build"
        ) {
            continue;
        }

        if path.is_dir() {
            search_dir(&path, query, case_sensitive, matches, searched);
        } else if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !is_text_extension(ext) {
                continue;
            }
            search_file(&path, query, case_sensitive, matches);
            *searched += 1;
            if matches.len() >= 1000 {
                return;
            }
        }
    }
}

fn search_file(path: &Path, query: &str, case_sensitive: bool, matches: &mut Vec<SearchMatch>) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };

    for (line_idx, line) in content.lines().enumerate() {
        let haystack = if case_sensitive {
            line.to_string()
        } else {
            line.to_lowercase()
        };
        let needle = if case_sensitive {
            query.to_string()
        } else {
            query.to_lowercase()
        };

        if let Some(pos) = haystack.find(&needle) {
            matches.push(SearchMatch {
                file_path: path.to_path_buf(),
                line_number: line_idx + 1,
                line_text: line.to_string(),
                match_start: pos,
                match_end: pos + needle.len(),
            });
        }
    }
}

fn is_text_extension(ext: &str) -> bool {
    matches!(
        ext.to_lowercase().as_str(),
        "rs" | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "mjs"
            | "py"
            | "json"
            | "toml"
            | "yaml"
            | "yml"
            | "md"
            | "txt"
            | "sh"
            | "bash"
            | "zsh"
            | "html"
            | "css"
            | "scss"
            | "less"
            | "vue"
            | "svelte"
            | "go"
            | "java"
            | "kt"
            | "c"
            | "cpp"
            | "h"
            | "hpp"
            | "cs"
            | "rb"
            | "php"
            | "swift"
            | "lua"
            | "r"
            | "ex"
            | "exs"
            | "xml"
            | "ini"
            | "cfg"
            | "conf"
            | "env"
            | "gitignore"
            | "dockerfile"
            | "lock"
    )
}

impl Default for WorkspaceSearch {
    fn default() -> Self {
        Self::new()
    }
}
