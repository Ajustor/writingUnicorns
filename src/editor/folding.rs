use super::buffer::Buffer;

/// Compute foldable regions from buffer by indent level.
/// Returns list of `(start_line, end_line)`.
pub(super) fn compute_fold_regions(buffer: &Buffer) -> Vec<(usize, usize)> {
    let total = buffer.num_lines();
    let indent = |i: usize| -> usize {
        let line = buffer.line(i);
        if line.trim().is_empty() {
            return usize::MAX;
        }
        let tabs = line.chars().take_while(|&c| c == '\t').count();
        let spaces = line.chars().take_while(|&c| c == ' ').count();
        tabs.max(spaces / 2)
    };
    let mut regions = Vec::new();
    let mut i = 0;
    while i + 1 < total {
        let cur_ind = indent(i);
        if cur_ind == usize::MAX {
            i += 1;
            continue;
        }
        let mut end = i + 1;
        while end < total {
            let next_ind = indent(end);
            if next_ind == usize::MAX || next_ind > cur_ind {
                end += 1;
            } else {
                break;
            }
        }
        end -= 1;
        if end > i + 1 {
            regions.push((i, end));
        }
        i += 1;
    }
    regions
}
