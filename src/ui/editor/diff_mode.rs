//! Diff mode support for the editor
//!
//! When in diff mode, the editor displays a unified diff view with:
//! - Two line number columns (old/new)
//! - Colored backgrounds for added/deleted/unchanged lines
//! - Collapsible sections for unchanged context

use crate::stores::{DiffLine, DiffLineTag};

/// What's actually displayed - either a real diff line or a collapsed indicator
#[derive(Debug, Clone)]
pub enum DiffDisplayLine {
    Line {
        line: DiffLine,
        /// Index into the buffer for syntax highlighting lookups.
        /// For file-backed diffs this is `new_line_num - 1`;
        /// for external diffs (PR review) it's a sequential index into the content buffer.
        buffer_line: Option<usize>,
    },
    Collapsed {
        start_idx: usize,
        end_idx: usize,
        count: usize,
    },
    CommentRow {
        text: String,
        is_first_line: bool,
        is_pending: bool,
    },
}

#[derive(Debug, Clone)]
pub struct InlineComment {
    pub author: String,
    pub body: String,
    pub created_at: String,
    pub is_pending: bool,
}

#[derive(Debug, Clone)]
pub struct CommentAttachment {
    pub line: u64,
    pub side: String,
    pub comments: Vec<InlineComment>,
}

/// Computes which lines to display given the full diff and expanded sections.
///
/// `buffer_line_map` provides the buffer line index for each diff line (for syntax highlighting).
/// When `None`, defaults to `new_line_num.map(|n| n - 1)` (file-backed diffs).
/// When `Some`, uses the provided mapping (for external/PR diffs with sequential indices).
pub fn compute_display_lines(
    all_lines: &[DiffLine],
    expanded_sections: &[(usize, usize)],
    context_lines: usize,
    buffer_line_map: Option<&[Option<usize>]>,
) -> Vec<DiffDisplayLine> {
    if all_lines.is_empty() {
        return Vec::new();
    }

    let buf_line = |idx: usize| -> Option<usize> {
        if let Some(map) = buffer_line_map {
            map.get(idx).copied().flatten()
        } else {
            all_lines[idx].new_line_num.map(|n| n - 1)
        }
    };

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
                .enumerate()
                .map(|(i, l)| DiffDisplayLine::Line { line: l.clone(), buffer_line: buf_line(i) })
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
                    display_items.push(DiffDisplayLine::Line { line: all_lines[i].clone(), buffer_line: buf_line(i) });
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
            display_items.push(DiffDisplayLine::Line { line: all_lines[i].clone(), buffer_line: buf_line(i) });
        }

        current_pos = end;
    }

    if current_pos < all_lines.len() {
        let collapsed_start = current_pos;
        let collapsed_end = all_lines.len();
        let is_expanded = is_section_expanded(collapsed_start, collapsed_end, expanded_sections);

        if is_expanded {
            for i in collapsed_start..collapsed_end {
                display_items.push(DiffDisplayLine::Line { line: all_lines[i].clone(), buffer_line: buf_line(i) });
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

#[cfg(test)]
fn make_diff_line(tag: DiffLineTag, old: Option<usize>, new: Option<usize>, content: &str) -> DiffLine {
    DiffLine {
        tag,
        old_line_num: old,
        new_line_num: new,
        content: content.to_string(),
    }
}

/// Injects CommentRow entries into display_lines after lines that have comments attached.
/// Matches by (line_number, side) from the original diff, not by display index.
pub fn inject_inline_comments(
    display_lines: &mut Vec<DiffDisplayLine>,
    attachments: &[CommentAttachment],
) {
    if attachments.is_empty() {
        return;
    }

    // Process attachments in reverse display order to avoid index shifting
    // First, find insertion points for each attachment
    let mut insertions: Vec<(usize, Vec<DiffDisplayLine>)> = Vec::new();

    for attachment in attachments {
        // Find the display line that matches this attachment's (line, side)
        let insert_after = display_lines.iter().enumerate().rev().find_map(|(idx, dl)| {
            if let DiffDisplayLine::Line { line, .. } = dl {
                let matches = if attachment.side == "LEFT" {
                    line.old_line_num == Some(attachment.line as usize)
                        && line.tag == DiffLineTag::Delete
                } else {
                    line.new_line_num == Some(attachment.line as usize)
                        && line.tag != DiffLineTag::Delete
                };
                if matches { Some(idx) } else { None }
            } else {
                None
            }
        });

        let Some(insert_idx) = insert_after else { continue };

        let mut rows = Vec::new();
        for comment in &attachment.comments {
            let timestamp = if comment.created_at.len() >= 10 {
                &comment.created_at[..10]
            } else {
                &comment.created_at
            };
            let author_label = if comment.is_pending {
                "You (draft)".to_string()
            } else {
                format!("@{} ({})", comment.author, timestamp)
            };

            // Header row
            rows.push(DiffDisplayLine::CommentRow {
                text: author_label.clone(),
                is_first_line: true,
                is_pending: comment.is_pending,
            });

            // Body rows — one per line
            for body_line in comment.body.lines() {
                rows.push(DiffDisplayLine::CommentRow {
                    text: format!("  {}", body_line),
                    is_first_line: false,
                    is_pending: comment.is_pending,
                });
            }
            // Handle empty body
            if comment.body.is_empty() {
                rows.push(DiffDisplayLine::CommentRow {
                    text: String::new(),
                    is_first_line: false,
                    is_pending: comment.is_pending,
                });
            }
        }

        insertions.push((insert_idx, rows));
    }

    // Sort by insertion index descending so we can insert without shifting earlier indices
    insertions.sort_by(|a, b| b.0.cmp(&a.0));

    // Deduplicate: if multiple attachments target the same line, merge them
    insertions.dedup_by(|a, b| {
        if a.0 == b.0 {
            b.1.append(&mut a.1);
            true
        } else {
            false
        }
    });

    for (idx, rows) in insertions {
        let insert_pos = idx + 1;
        for (i, row) in rows.into_iter().enumerate() {
            display_lines.insert(insert_pos + i, row);
        }
    }
}
