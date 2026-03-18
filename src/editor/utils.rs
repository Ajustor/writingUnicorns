use super::buffer::Buffer;

/// Extract the word (identifier or path-like token) at `(row, col)` in the buffer.
pub(super) fn get_word_at(buffer: &Buffer, row: usize, col: usize) -> Option<String> {
    let line = buffer.line(row);
    let chars: Vec<char> = line.chars().collect();
    let c = col.min(chars.len().saturating_sub(1));
    let is_word = |ch: char| ch.is_alphanumeric() || ch == '_';
    if c >= chars.len() || !is_word(chars[c]) {
        return None;
    }
    let mut start = c;
    while start > 0 && is_word(chars[start - 1]) {
        start -= 1;
    }
    let mut end = c + 1;
    while end < chars.len() && is_word(chars[end]) {
        end += 1;
    }
    Some(chars[start..end].iter().collect())
}

/// Search for the next occurrence of `word` in the buffer starting at (from_row, from_col),
/// wrapping around to the beginning if needed. Returns (row, col) of the match start.
pub(super) fn find_next_occurrence(
    buf: &Buffer,
    word: &str,
    from_row: usize,
    from_col: usize,
) -> Option<(usize, usize)> {
    let word_chars: Vec<char> = word.chars().collect();
    let word_len = word_chars.len();
    if word_len == 0 {
        return None;
    }
    let total = buf.num_lines();
    let row_order: Vec<usize> = (from_row..total).chain(0..from_row).collect();
    for row_idx in row_order {
        let line_chars: Vec<char> = buf.line(row_idx).chars().collect();
        let start_col = if row_idx == from_row { from_col } else { 0 };
        if line_chars.len() < word_len {
            continue;
        }
        let end = line_chars.len() - word_len;
        if start_col > end {
            continue;
        }
        for col in start_col..=end {
            if line_chars[col..col + word_len] == word_chars[..] {
                return Some((row_idx, col));
            }
        }
    }
    None
}
