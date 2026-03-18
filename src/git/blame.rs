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
