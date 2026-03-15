use crate::filetree::FileTree;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::path::PathBuf;

pub struct CommandPalette {
    pub open: bool,
    pub query: String,
    pub results: Vec<PathBuf>,
    matcher: SkimMatcherV2,
}

impl CommandPalette {
    pub fn new() -> Self {
        Self {
            open: false,
            query: String::new(),
            results: vec![],
            matcher: SkimMatcherV2::default(),
        }
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
        if self.open {
            self.query.clear();
            self.results.clear();
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    fn collect_files(entry: &crate::filetree::FileEntry, out: &mut Vec<PathBuf>) {
        if !entry.is_dir {
            out.push(entry.path.clone());
        }
        for child in &entry.children {
            Self::collect_files(child, out);
        }
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        file_tree: &mut FileTree,
        _workspace: &mut Option<PathBuf>,
    ) {
        let mut close = false;
        let mut opened: Option<PathBuf> = None;

        egui::Window::new("Command Palette")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .fixed_pos(egui::pos2(
                ctx.screen_rect().center().x - 280.0,
                ctx.screen_rect().top() + 60.0,
            ))
            .fixed_size(egui::vec2(560.0, 400.0))
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.query)
                            .desired_width(ui.available_width())
                            .hint_text("Search files…")
                            .font(egui::TextStyle::Monospace),
                    );
                    response.request_focus();

                    let mut all_files = vec![];
                    if let Some(root) = &file_tree.root {
                        Self::collect_files(root, &mut all_files);
                    }

                    if self.query.is_empty() {
                        self.results = all_files.into_iter().take(20).collect();
                    } else {
                        let q = self.query.clone();
                        let mut scored: Vec<(i64, PathBuf)> = all_files
                            .into_iter()
                            .filter_map(|p| {
                                let name = p.file_name()?.to_string_lossy().to_string();
                                let score = self.matcher.fuzzy_match(&name, &q)?;
                                Some((score, p))
                            })
                            .collect();
                        scored.sort_by(|a, b| b.0.cmp(&a.0));
                        self.results = scored.into_iter().map(|(_, p)| p).take(20).collect();
                    }

                    ui.separator();

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for path in &self.results {
                            let name = path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            let dir = path
                                .parent()
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_default();
                            if ui
                                .selectable_label(false, format!("{}\n  {}", name, dir))
                                .clicked()
                            {
                                opened = Some(path.clone());
                                close = true;
                            }
                        }
                    });

                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        close = true;
                    }
                });
            });

        if close {
            self.open = false;
        }
        let _ = opened;
    }
}
