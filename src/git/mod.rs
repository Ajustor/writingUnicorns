use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct BlameEntry {
    pub commit_short: String,
    pub author: String,
    pub line: usize,
}

/// Run git blame on `path` using git2 and return per-line blame entries.
pub fn blame_file(path: &std::path::Path) -> Vec<BlameEntry> {
    let repo = match git2::Repository::discover(path) {
        Ok(r) => r,
        Err(_) => return vec![],
    };
    let workdir = match repo.workdir() {
        Some(w) => w,
        None => return vec![],
    };
    let rel = match path.strip_prefix(workdir) {
        Ok(r) => r,
        Err(_) => return vec![],
    };
    let blame = match repo.blame_file(rel, None) {
        Ok(b) => b,
        Err(_) => return vec![],
    };
    let mut entries = Vec::new();
    for hunk in blame.iter() {
        let commit_id = hunk.final_commit_id();
        let short: String = commit_id.to_string().chars().take(7).collect();
        let author = hunk.final_signature().name().unwrap_or("?").to_string();
        let start_line = hunk.final_start_line(); // 1-indexed
        let lines_in_hunk = hunk.lines_in_hunk();
        for i in 0..lines_in_hunk {
            entries.push(BlameEntry {
                commit_short: short.clone(),
                author: author.clone(),
                line: start_line + i - 1, // convert to 0-indexed
            });
        }
    }
    entries
}

#[derive(Debug, Clone, Default)]
pub struct FileStatus {
    pub path: String,
    pub status: GitFileStatus,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum GitFileStatus {
    #[default]
    Untracked,
    Modified,
    Added,
    Deleted,
    Renamed,
    Ignored,
}

pub struct GitStatus {
    pub branch: String,
    pub files: Vec<FileStatus>,
    pub repo_path: Option<PathBuf>,
}

impl GitStatus {
    pub fn new() -> Self {
        Self {
            branch: String::from("—"),
            files: vec![],
            repo_path: None,
        }
    }

    pub fn load(&mut self, path: PathBuf) {
        self.repo_path = Some(path.clone());
        if let Ok(repo) = git2::Repository::discover(&path) {
            if let Ok(head) = repo.head() {
                if let Some(name) = head.shorthand() {
                    self.branch = name.to_string();
                }
            }
            let mut opts = git2::StatusOptions::new();
            opts.include_untracked(true);
            if let Ok(statuses) = repo.statuses(Some(&mut opts)) {
                self.files = statuses
                    .iter()
                    .filter_map(|s| {
                        let path = s.path()?.to_string();
                        let st = s.status();
                        let status = if st.contains(git2::Status::WT_MODIFIED)
                            || st.contains(git2::Status::INDEX_MODIFIED)
                        {
                            GitFileStatus::Modified
                        } else if st.contains(git2::Status::WT_NEW)
                            || st.contains(git2::Status::INDEX_NEW)
                        {
                            GitFileStatus::Added
                        } else if st.contains(git2::Status::WT_DELETED)
                            || st.contains(git2::Status::INDEX_DELETED)
                        {
                            GitFileStatus::Deleted
                        } else if st.contains(git2::Status::WT_RENAMED)
                            || st.contains(git2::Status::INDEX_RENAMED)
                        {
                            GitFileStatus::Renamed
                        } else if st.contains(git2::Status::IGNORED) {
                            return None;
                        } else {
                            GitFileStatus::Untracked
                        };
                        Some(FileStatus { path, status })
                    })
                    .collect();
            }
        }
    }

    pub fn show(&self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new(format!("⎇ {}", self.branch)).strong());
        ui.separator();
        if self.files.is_empty() {
            ui.label(egui::RichText::new("No changes").color(egui::Color32::GRAY));
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for f in &self.files {
                let (icon, color) = match f.status {
                    GitFileStatus::Modified => ("M", egui::Color32::YELLOW),
                    GitFileStatus::Added | GitFileStatus::Untracked => ("U", egui::Color32::GREEN),
                    GitFileStatus::Deleted => ("D", egui::Color32::RED),
                    GitFileStatus::Renamed => ("R", egui::Color32::from_rgb(100, 150, 255)),
                    GitFileStatus::Ignored => ("I", egui::Color32::GRAY),
                };
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(icon).color(color).monospace());
                    ui.label(&f.path);
                });
            }
        });
    }

    pub fn refresh(&mut self) {
        if let Some(path) = self.repo_path.clone() {
            self.load(path);
        }
    }
}
