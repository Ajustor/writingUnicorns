/// Detect whether a file uses spaces or tabs and the indent unit size.
/// Returns `(use_spaces, indent_size)`.
pub(super) fn detect_indent(content: &str) -> (bool, usize) {
    let mut space_lines = 0usize;
    let mut tab_lines = 0usize;
    let mut size_votes: std::collections::HashMap<usize, usize> = Default::default();
    for line in content.lines().take(200) {
        if line.starts_with('\t') {
            tab_lines += 1;
        } else if line.starts_with("  ") {
            let n = line.chars().take_while(|&c| c == ' ').count();
            space_lines += 1;
            if n > 0 {
                *size_votes.entry(n).or_default() += 1;
            }
        }
    }
    let use_spaces = space_lines >= tab_lines;
    let size = if use_spaces {
        [2usize, 4, 3, 8]
            .iter()
            .find(|&&s| size_votes.contains_key(&s))
            .copied()
            .unwrap_or(4)
    } else {
        4
    };
    (use_spaces, size)
}
