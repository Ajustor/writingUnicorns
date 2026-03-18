use egui_phosphor::regular as ph;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_expanded: bool,
    pub children: Vec<FileEntry>,
    pub depth: usize,
}

impl FileEntry {
    pub fn new(path: PathBuf, depth: usize) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let is_dir = path.is_dir();
        Self {
            name,
            path,
            is_dir,
            is_expanded: depth == 0,
            children: vec![],
            depth,
        }
    }

    pub fn load_children(&mut self) {
        if !self.is_dir {
            return;
        }
        self.children.clear();
        if let Ok(entries) = std::fs::read_dir(&self.path) {
            let mut dirs = vec![];
            let mut files = vec![];
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                if name.starts_with('.') {
                    continue;
                }
                if path.is_dir() {
                    dirs.push(path);
                } else {
                    files.push(path);
                }
            }
            dirs.sort();
            files.sort();
            for p in dirs.into_iter().chain(files) {
                self.children.push(FileEntry::new(p, self.depth + 1));
            }
        }
    }
}

pub struct FileTree {
    pub root: Option<FileEntry>,
    pub selected: Option<PathBuf>,
    /// Action requested via context menu, consumed by the caller each frame.
    pub context_action: Option<FileTreeAction>,
    /// Path being renamed (in-progress text).  Exposed so callers can pre-set it.
    pub rename_state: Option<(PathBuf, String)>,
}

#[derive(Debug, Clone)]
pub enum FileTreeAction {
    OpenFile(PathBuf),
    NewFile(PathBuf),      // parent dir
    NewFolder(PathBuf),    // parent dir
    Rename(PathBuf, String), // old path, new name
    Delete(PathBuf),
    RevealInExplorer(PathBuf),
    CopyPath(PathBuf),
}

impl FileTree {
    pub fn new() -> Self {
        Self {
            root: None,
            selected: None,
            context_action: None,
            rename_state: None,
        }
    }

    pub fn load(&mut self, path: PathBuf) {
        let mut root = FileEntry::new(path, 0);
        root.load_children();
        self.root = Some(root);
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<PathBuf> {
        let mut opened = None;
        let mut action: Option<FileTreeAction> = None;
        let mut rename_state = self.rename_state.take();
        if let Some(root) = &mut self.root {
            Self::show_entry_recursive(ui, root, &mut self.selected, &mut opened, &mut action, &mut rename_state);
        }
        self.rename_state = rename_state;
        // Store non-open actions for the caller to pick up
        if let Some(a) = action {
            match &a {
                FileTreeAction::OpenFile(p) => { opened = Some(p.clone()); }
                _ => { self.context_action = Some(a); }
            }
        }
        opened
    }

    fn show_entry_recursive(
        ui: &mut egui::Ui,
        entry: &mut FileEntry,
        selected: &mut Option<PathBuf>,
        opened: &mut Option<PathBuf>,
        action: &mut Option<FileTreeAction>,
        rename_state: &mut Option<(PathBuf, String)>,
    ) {
        let indent = entry.depth as f32 * 14.0;

        // Inline rename mode
        let is_renaming = rename_state.as_ref().map(|(p, _)| p == &entry.path).unwrap_or(false);

        let row_resp = ui.horizontal(|ui| {
            ui.add_space(indent);

            if is_renaming {
                if let Some((_, ref mut new_name)) = rename_state {
                    let resp = ui.add(
                        egui::TextEdit::singleline(new_name)
                            .desired_width(150.0),
                    );
                    resp.request_focus();
                    if resp.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let new_n = new_name.clone();
                        *action = Some(FileTreeAction::Rename(entry.path.clone(), new_n));
                        *rename_state = None;
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        *rename_state = None;
                    }
                }
                return;
            }

            if entry.is_dir {
                let arrow = if entry.is_expanded { ph::CARET_DOWN } else { ph::CARET_RIGHT };
                let folder_icon = if entry.is_expanded { ph::FOLDER_OPEN } else { ph::FOLDER };
                let color = egui::Color32::from_rgb(220, 180, 100);
                let label = egui::RichText::new(format!("{} {} {}", arrow, folder_icon, entry.name)).color(color);
                let resp = ui.selectable_label(false, label);
                if resp.clicked() {
                    entry.is_expanded = !entry.is_expanded;
                    if entry.is_expanded && entry.children.is_empty() {
                        entry.load_children();
                    }
                }
                resp.context_menu(|ui| {
                    if ui.button("New File").clicked() {
                        *action = Some(FileTreeAction::NewFile(entry.path.clone()));
                        ui.close_menu();
                    }
                    if ui.button("New Folder").clicked() {
                        *action = Some(FileTreeAction::NewFolder(entry.path.clone()));
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Rename").clicked() {
                        *rename_state = Some((entry.path.clone(), entry.name.clone()));
                        ui.close_menu();
                    }
                    if ui.button("Delete Folder").clicked() {
                        *action = Some(FileTreeAction::Delete(entry.path.clone()));
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Copy Path").clicked() {
                        *action = Some(FileTreeAction::CopyPath(entry.path.clone()));
                        ui.close_menu();
                    }
                    if ui.button("Reveal in File Manager").clicked() {
                        *action = Some(FileTreeAction::RevealInExplorer(entry.path.clone()));
                        ui.close_menu();
                    }
                });
            } else {
                let (icon, color) = file_icon(&entry.name);
                let is_selected = selected.as_ref().map(|s| s == &entry.path).unwrap_or(false);
                let icon_label = egui::RichText::new(icon).color(color);
                ui.label(icon_label);
                let resp = ui.selectable_label(
                    is_selected,
                    egui::RichText::new(&entry.name).color(egui::Color32::from_rgb(212, 212, 212)),
                );
                if resp.clicked() {
                    *selected = Some(entry.path.clone());
                    *opened = Some(entry.path.clone());
                }
                resp.context_menu(|ui| {
                    if ui.button("Open").clicked() {
                        *action = Some(FileTreeAction::OpenFile(entry.path.clone()));
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Rename").clicked() {
                        *rename_state = Some((entry.path.clone(), entry.name.clone()));
                        ui.close_menu();
                    }
                    if ui.button("Delete File").clicked() {
                        *action = Some(FileTreeAction::Delete(entry.path.clone()));
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Copy Path").clicked() {
                        *action = Some(FileTreeAction::CopyPath(entry.path.clone()));
                        ui.close_menu();
                    }
                    if ui.button("Reveal in File Manager").clicked() {
                        *action = Some(FileTreeAction::RevealInExplorer(entry.path.clone()));
                        ui.close_menu();
                    }
                });
            }
        });
        let _ = row_resp;

        if entry.is_dir && entry.is_expanded {
            for child in &mut entry.children {
                Self::show_entry_recursive(ui, child, selected, opened, action, rename_state);
            }
        }
    }
}

/// Returns (phosphor icon char, color) for a given filename.
pub fn file_icon(name: &str) -> (&'static str, egui::Color32) {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    // Phosphor Regular has dedicated icons for: rs, py, js, ts, jsx, tsx, html, css, c, cpp, sql, md, txt, lock, svg
    // For other languages, use FILE_CODE with a distinctive color
    match ext.as_str() {
        "rs" => (ph::FILE_RS, egui::Color32::from_rgb(222, 99, 52)),
        "py" => (ph::FILE_PY, egui::Color32::from_rgb(53, 114, 165)),
        "js" | "mjs" | "cjs" => (ph::FILE_JS, egui::Color32::from_rgb(240, 219, 79)),
        "ts" => (ph::FILE_TS, egui::Color32::from_rgb(49, 120, 198)),
        "jsx" => (ph::FILE_JSX, egui::Color32::from_rgb(97, 218, 251)),
        "tsx" => (ph::FILE_TSX, egui::Color32::from_rgb(97, 218, 251)),
        "json" | "jsonc" => (ph::BRACKETS_CURLY, egui::Color32::from_rgb(255, 196, 88)),
        "toml" => (ph::FILE_CODE, egui::Color32::from_rgb(156, 220, 254)),
        "yaml" | "yml" => (ph::FILE_CODE, egui::Color32::from_rgb(206, 145, 120)),
        "md" | "mdx" => (ph::FILE_MD, egui::Color32::from_rgb(100, 200, 255)),
        "html" | "htm" => (ph::FILE_HTML, egui::Color32::from_rgb(228, 79, 38)),
        "css" => (ph::FILE_CSS, egui::Color32::from_rgb(86, 156, 214)),
        "scss" | "sass" | "less" => (ph::FILE_CSS, egui::Color32::from_rgb(205, 103, 153)),
        "c" | "h" => (ph::FILE_C, egui::Color32::from_rgb(85, 144, 196)),
        "cpp" | "cc" | "cxx" | "hpp" => (ph::FILE_CPP, egui::Color32::from_rgb(85, 144, 196)),
        "sql" => (ph::FILE_SQL, egui::Color32::from_rgb(218, 160, 17)),
        "svg" => (ph::FILE_SVG, egui::Color32::from_rgb(255, 160, 40)),
        "xml" => (ph::FILE_CODE, egui::Color32::from_rgb(228, 79, 38)),
        "sh" | "bash" | "zsh" | "fish" => (ph::TERMINAL, egui::Color32::from_rgb(35, 209, 139)),
        "txt" | "log" => (ph::FILE_TXT, egui::Color32::GRAY),
        "lock" => (ph::FILE_LOCK, egui::Color32::GRAY),
        // Languages without dedicated Phosphor icon → FILE_CODE with distinct color
        "go" => (ph::FILE_CODE, egui::Color32::from_rgb(0, 173, 216)),
        "java" => (ph::FILE_CODE, egui::Color32::from_rgb(176, 114, 25)),
        "kt" | "kts" => (ph::FILE_CODE, egui::Color32::from_rgb(169, 121, 227)),
        "swift" => (ph::FILE_CODE, egui::Color32::from_rgb(240, 81, 56)),
        "rb" => (ph::FILE_CODE, egui::Color32::from_rgb(204, 52, 45)),
        "php" => (ph::FILE_CODE, egui::Color32::from_rgb(119, 123, 179)),
        "lua" => (ph::FILE_CODE, egui::Color32::from_rgb(80, 80, 228)),
        "cs" => (ph::FILE_C_SHARP, egui::Color32::from_rgb(104, 33, 122)),
        "dart" => (ph::FILE_CODE, egui::Color32::from_rgb(84, 182, 217)),
        "zig" => (ph::FILE_CODE, egui::Color32::from_rgb(247, 175, 48)),
        "ex" | "exs" => (ph::FILE_CODE, egui::Color32::from_rgb(102, 51, 153)),
        _ => (ph::FILE, egui::Color32::from_gray(160)),
    }
}
