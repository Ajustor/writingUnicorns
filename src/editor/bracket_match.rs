use super::buffer::Buffer;

/// Find the matching bracket for the char at `(row, col)` in the buffer.
/// Returns `(open_row, open_col, close_row, close_col)` or `None`.
pub(super) fn find_matching_bracket(
    buffer: &Buffer,
    row: usize,
    col: usize,
) -> Option<(usize, usize, usize, usize)> {
    let line = buffer.line(row);
    let chars: Vec<char> = line.chars().collect();

    for check_col in [col, col.wrapping_sub(1)] {
        if check_col >= chars.len() {
            continue;
        }
        let ch = chars[check_col];
        match ch {
            '{' | '(' | '[' => {
                let close = match ch {
                    '{' => '}',
                    '(' => ')',
                    _ => ']',
                };
                let mut depth = 1usize;
                let mut r = row;
                let mut c = check_col + 1;
                loop {
                    let line_chars: Vec<char> = buffer.line(r).chars().collect();
                    while c < line_chars.len() {
                        if line_chars[c] == ch {
                            depth += 1;
                        } else if line_chars[c] == close {
                            depth -= 1;
                            if depth == 0 {
                                return Some((row, check_col, r, c));
                            }
                        }
                        c += 1;
                    }
                    r += 1;
                    if r >= buffer.num_lines() {
                        break;
                    }
                    c = 0;
                }
            }
            '}' | ')' | ']' => {
                let open = match ch {
                    '}' => '{',
                    ')' => '(',
                    _ => '[',
                };
                let mut depth = 1usize;
                let mut r = row;
                let mut c = check_col;
                loop {
                    let line_chars: Vec<char> = buffer.line(r).chars().collect();
                    let start = if r == row { c } else { line_chars.len() };
                    let scan_range: Vec<usize> = (0..start).rev().collect();
                    for sc in scan_range {
                        if line_chars[sc] == ch {
                            depth += 1;
                        } else if line_chars[sc] == open {
                            depth -= 1;
                            if depth == 0 {
                                return Some((r, sc, row, check_col));
                            }
                        }
                    }
                    if r == 0 {
                        break;
                    }
                    r -= 1;
                    c = buffer.line(r).chars().count();
                }
            }
            _ => {}
        }
    }
    None
}
