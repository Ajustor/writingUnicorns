pub(super) const DIFF_UNCHANGED: u8 = 0;
pub(super) const DIFF_ADDED: u8 = 1;
pub(super) const DIFF_MODIFIED: u8 = 2;
pub(super) const DIFF_DELETED_ABOVE: u8 = 3;

/// Compare current file content against HEAD and return per-line diff status.
pub(super) fn compute_line_diff(path: &std::path::Path, num_lines: usize) -> Vec<u8> {
    let mut result = vec![DIFF_UNCHANGED; num_lines];
    let repo = match git2::Repository::discover(path) {
        Ok(r) => r,
        Err(_) => return result,
    };
    let workdir = match repo.workdir() {
        Some(w) => w.to_path_buf(),
        None => return result,
    };
    let rel = match path.strip_prefix(&workdir) {
        Ok(r) => r,
        Err(_) => return result,
    };
    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => {
            for s in result.iter_mut() {
                *s = DIFF_ADDED;
            }
            return result;
        }
    };
    let tree = match head.peel_to_tree() {
        Ok(t) => t,
        Err(_) => return result,
    };
    let entry = match tree.get_path(rel) {
        Ok(e) => e,
        Err(_) => {
            for s in result.iter_mut() {
                *s = DIFF_ADDED;
            }
            return result;
        }
    };
    let blob = match repo.find_blob(entry.id()) {
        Ok(b) => b,
        Err(_) => return result,
    };
    let old_content = match std::str::from_utf8(blob.content()) {
        Ok(s) => s.to_string(),
        Err(_) => return result,
    };
    let old_lines: Vec<&str> = old_content.lines().collect();
    let current = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return result,
    };
    let new_lines: Vec<&str> = current.lines().collect();
    let old_set: std::collections::HashSet<&str> = old_lines.iter().copied().collect();
    for (i, &line) in new_lines.iter().enumerate() {
        if i < result.len() {
            if old_lines.get(i) == Some(&line) {
                result[i] = DIFF_UNCHANGED;
            } else if old_set.contains(line) {
                result[i] = DIFF_MODIFIED;
            } else {
                result[i] = DIFF_ADDED;
            }
        }
    }
    result
}
