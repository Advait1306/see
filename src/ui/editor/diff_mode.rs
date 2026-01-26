//! Diff mode support for the editor
//!
//! When in diff mode, the editor displays a unified diff view with:
//! - Two line number columns (old/new)
//! - Colored backgrounds for added/deleted/unchanged lines
//! - Collapsible sections for unchanged context

use similar::ChangeTag;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineTag {
    Equal,
    Insert,
    Delete,
}

impl From<ChangeTag> for DiffLineTag {
    fn from(tag: ChangeTag) -> Self {
        match tag {
            ChangeTag::Equal => DiffLineTag::Equal,
            ChangeTag::Insert => DiffLineTag::Insert,
            ChangeTag::Delete => DiffLineTag::Delete,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub tag: DiffLineTag,
    pub old_line_num: Option<usize>,
    pub new_line_num: Option<usize>,
    pub content: String,
}

/// What's actually displayed - either a real diff line or a collapsed indicator
#[derive(Debug, Clone)]
pub enum DiffDisplayLine {
    Line(DiffLine),
    Collapsed {
        start_idx: usize,
        end_idx: usize,
        count: usize,
    },
}

/// Computes which lines to display given the full diff and expanded sections
pub fn compute_display_lines(
    all_lines: &[DiffLine],
    expanded_sections: &[(usize, usize)],
    context_lines: usize,
) -> Vec<DiffDisplayLine> {
    if all_lines.is_empty() {
        return Vec::new();
    }

    // Find indices of all changed lines
    let changed_indices: Vec<usize> = all_lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.tag != DiffLineTag::Equal)
        .map(|(i, _)| i)
        .collect();

    if changed_indices.is_empty() {
        // No changes, show collapsed indicator for entire file
        if is_section_expanded(0, all_lines.len(), expanded_sections) {
            return all_lines
                .iter()
                .cloned()
                .map(DiffDisplayLine::Line)
                .collect();
        }
        return vec![DiffDisplayLine::Collapsed {
            start_idx: 0,
            end_idx: all_lines.len(),
            count: all_lines.len(),
        }];
    }

    // Build ranges of lines to show (changed lines + context)
    let mut visible_ranges: Vec<(usize, usize)> = Vec::new();

    for &idx in &changed_indices {
        let start = idx.saturating_sub(context_lines);
        let end = (idx + context_lines + 1).min(all_lines.len());

        if let Some(last) = visible_ranges.last_mut() {
            if start <= last.1 {
                last.1 = end;
                continue;
            }
        }
        visible_ranges.push((start, end));
    }

    let mut display_items: Vec<DiffDisplayLine> = Vec::new();
    let mut current_pos = 0;

    for (start, end) in visible_ranges {
        if current_pos < start {
            let collapsed_start = current_pos;
            let collapsed_end = start;
            let is_expanded = is_section_expanded(collapsed_start, collapsed_end, expanded_sections);

            if is_expanded {
                for i in collapsed_start..collapsed_end {
                    display_items.push(DiffDisplayLine::Line(all_lines[i].clone()));
                }
            } else {
                display_items.push(DiffDisplayLine::Collapsed {
                    start_idx: collapsed_start,
                    end_idx: collapsed_end,
                    count: collapsed_end - collapsed_start,
                });
            }
        }

        for i in start..end {
            display_items.push(DiffDisplayLine::Line(all_lines[i].clone()));
        }

        current_pos = end;
    }

    if current_pos < all_lines.len() {
        let collapsed_start = current_pos;
        let collapsed_end = all_lines.len();
        let is_expanded = is_section_expanded(collapsed_start, collapsed_end, expanded_sections);

        if is_expanded {
            for i in collapsed_start..collapsed_end {
                display_items.push(DiffDisplayLine::Line(all_lines[i].clone()));
            }
        } else {
            display_items.push(DiffDisplayLine::Collapsed {
                start_idx: collapsed_start,
                end_idx: collapsed_end,
                count: collapsed_end - collapsed_start,
            });
        }
    }

    display_items
}

fn is_section_expanded(start: usize, end: usize, expanded_sections: &[(usize, usize)]) -> bool {
    expanded_sections.iter().any(|(s, e)| *s == start && *e == end)
}
