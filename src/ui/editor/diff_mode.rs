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

#[cfg(test)]
mod tests {
    use super::*;

    fn to_display_lines(lines: &[DiffLine]) -> Vec<DiffDisplayLine> {
        lines
            .iter()
            .map(|l| DiffDisplayLine::Line {
                line: l.clone(),
                buffer_line: l.new_line_num.map(|n| n - 1),
            })
            .collect()
    }

    fn make_comment(author: &str, body: &str, is_pending: bool) -> InlineComment {
        InlineComment {
            author: author.to_string(),
            body: body.to_string(),
            created_at: "2024-01-15T12:00:00Z".to_string(),
            is_pending,
        }
    }

    fn make_attachment(line: u64, side: &str, comments: Vec<InlineComment>) -> CommentAttachment {
        CommentAttachment {
            line,
            side: side.to_string(),
            comments,
        }
    }

    fn count_comment_rows(display_lines: &[DiffDisplayLine]) -> usize {
        display_lines
            .iter()
            .filter(|dl| matches!(dl, DiffDisplayLine::CommentRow { .. }))
            .count()
    }

    // ---- inject_inline_comments tests ----

    #[core::prelude::v1::test]
    fn test_inject_comment_right_side() {
        let lines = vec![
            make_diff_line(DiffLineTag::Equal, Some(1), Some(1), "context"),
            make_diff_line(DiffLineTag::Insert, None, Some(2), "added"),
            make_diff_line(DiffLineTag::Equal, Some(2), Some(3), "context2"),
        ];
        let mut display = to_display_lines(&lines);
        let attachments = vec![make_attachment(
            2,
            "RIGHT",
            vec![make_comment("alice", "nice addition", false)],
        )];
        inject_inline_comments(&mut display, &attachments);

        // Should have 3 original lines + 1 header + 1 body = 5
        assert_eq!(display.len(), 5);
        // Comment rows should appear after index 1 (the Insert line)
        assert!(matches!(&display[2], DiffDisplayLine::CommentRow { is_first_line: true, .. }));
        assert!(matches!(&display[3], DiffDisplayLine::CommentRow { is_first_line: false, .. }));
    }

    #[core::prelude::v1::test]
    fn test_inject_comment_left_side() {
        let lines = vec![
            make_diff_line(DiffLineTag::Equal, Some(1), Some(1), "context"),
            make_diff_line(DiffLineTag::Delete, Some(2), None, "removed"),
            make_diff_line(DiffLineTag::Equal, Some(3), Some(2), "context2"),
        ];
        let mut display = to_display_lines(&lines);
        let attachments = vec![make_attachment(
            2,
            "LEFT",
            vec![make_comment("bob", "why remove?", false)],
        )];
        inject_inline_comments(&mut display, &attachments);

        assert_eq!(display.len(), 5);
        assert!(matches!(&display[2], DiffDisplayLine::CommentRow { is_first_line: true, .. }));
    }

    #[core::prelude::v1::test]
    fn test_inject_multiline_body() {
        let lines = vec![make_diff_line(DiffLineTag::Insert, None, Some(1), "added")];
        let mut display = to_display_lines(&lines);
        let attachments = vec![make_attachment(
            1,
            "RIGHT",
            vec![make_comment("alice", "line1\nline2\nline3", false)],
        )];
        inject_inline_comments(&mut display, &attachments);

        // 1 original + 1 header + 3 body lines = 5
        assert_eq!(display.len(), 5);
        assert_eq!(count_comment_rows(&display), 4);
    }

    #[core::prelude::v1::test]
    fn test_inject_empty_body() {
        let lines = vec![make_diff_line(DiffLineTag::Insert, None, Some(1), "added")];
        let mut display = to_display_lines(&lines);
        let attachments = vec![make_attachment(
            1,
            "RIGHT",
            vec![make_comment("alice", "", false)],
        )];
        inject_inline_comments(&mut display, &attachments);

        // 1 original + 1 header + 1 empty body = 3
        assert_eq!(display.len(), 3);
    }

    #[core::prelude::v1::test]
    fn test_inject_pending_header() {
        let lines = vec![make_diff_line(DiffLineTag::Insert, None, Some(1), "added")];
        let mut display = to_display_lines(&lines);
        let attachments = vec![make_attachment(
            1,
            "RIGHT",
            vec![make_comment("me", "draft comment", true)],
        )];
        inject_inline_comments(&mut display, &attachments);

        if let DiffDisplayLine::CommentRow { text, is_pending, .. } = &display[1] {
            assert_eq!(text, "You (draft)");
            assert!(*is_pending);
        } else {
            panic!("Expected CommentRow at index 1");
        }
    }

    #[core::prelude::v1::test]
    fn test_inject_existing_header() {
        let lines = vec![make_diff_line(DiffLineTag::Insert, None, Some(1), "added")];
        let mut display = to_display_lines(&lines);
        let attachments = vec![make_attachment(
            1,
            "RIGHT",
            vec![make_comment("alice", "looks good", false)],
        )];
        inject_inline_comments(&mut display, &attachments);

        if let DiffDisplayLine::CommentRow { text, is_pending, .. } = &display[1] {
            assert_eq!(text, "@alice (2024-01-15)");
            assert!(!*is_pending);
        } else {
            panic!("Expected CommentRow at index 1");
        }
    }

    #[core::prelude::v1::test]
    fn test_inject_no_match_skips() {
        let lines = vec![make_diff_line(DiffLineTag::Insert, None, Some(1), "added")];
        let mut display = to_display_lines(&lines);
        let attachments = vec![make_attachment(
            99,
            "RIGHT",
            vec![make_comment("alice", "no match", false)],
        )];
        inject_inline_comments(&mut display, &attachments);

        assert_eq!(display.len(), 1);
        assert_eq!(count_comment_rows(&display), 0);
    }

    #[core::prelude::v1::test]
    fn test_inject_empty_attachments() {
        let lines = vec![
            make_diff_line(DiffLineTag::Equal, Some(1), Some(1), "line1"),
            make_diff_line(DiffLineTag::Insert, None, Some(2), "added"),
        ];
        let mut display = to_display_lines(&lines);
        let original_len = display.len();
        inject_inline_comments(&mut display, &[]);

        assert_eq!(display.len(), original_len);
    }

    #[core::prelude::v1::test]
    fn test_inject_multiple_comments_same_line() {
        let lines = vec![make_diff_line(DiffLineTag::Insert, None, Some(1), "added")];
        let mut display = to_display_lines(&lines);
        let attachments = vec![make_attachment(
            1,
            "RIGHT",
            vec![
                make_comment("alice", "first", false),
                make_comment("bob", "second", false),
            ],
        )];
        inject_inline_comments(&mut display, &attachments);

        // 1 original + 2*(header + body) = 5
        assert_eq!(display.len(), 5);
        assert_eq!(count_comment_rows(&display), 4);
    }

    #[core::prelude::v1::test]
    fn test_inject_comments_different_lines() {
        let lines = vec![
            make_diff_line(DiffLineTag::Insert, None, Some(1), "line1"),
            make_diff_line(DiffLineTag::Insert, None, Some(2), "line2"),
            make_diff_line(DiffLineTag::Insert, None, Some(3), "line3"),
        ];
        let mut display = to_display_lines(&lines);
        let attachments = vec![
            make_attachment(1, "RIGHT", vec![make_comment("alice", "on line 1", false)]),
            make_attachment(3, "RIGHT", vec![make_comment("bob", "on line 3", false)]),
        ];
        inject_inline_comments(&mut display, &attachments);

        // 3 original + 2*(header + body) = 7
        assert_eq!(display.len(), 7);
    }

    #[core::prelude::v1::test]
    fn test_inject_preserves_line_order() {
        let lines = vec![
            make_diff_line(DiffLineTag::Equal, Some(1), Some(1), "first"),
            make_diff_line(DiffLineTag::Insert, None, Some(2), "added"),
            make_diff_line(DiffLineTag::Equal, Some(2), Some(3), "last"),
        ];
        let mut display = to_display_lines(&lines);
        let attachments = vec![make_attachment(
            2,
            "RIGHT",
            vec![make_comment("alice", "comment", false)],
        )];
        inject_inline_comments(&mut display, &attachments);

        let line_contents: Vec<&str> = display
            .iter()
            .filter_map(|dl| {
                if let DiffDisplayLine::Line { line, .. } = dl {
                    Some(line.content.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(line_contents, vec!["first", "added", "last"]);
    }

    // ---- compute_display_lines tests ----

    #[core::prelude::v1::test]
    fn test_compute_display_all_equal_collapsed() {
        let lines = vec![
            make_diff_line(DiffLineTag::Equal, Some(1), Some(1), "a"),
            make_diff_line(DiffLineTag::Equal, Some(2), Some(2), "b"),
            make_diff_line(DiffLineTag::Equal, Some(3), Some(3), "c"),
        ];
        let display = compute_display_lines(&lines, &[], 3, None);
        assert_eq!(display.len(), 1);
        assert!(matches!(
            &display[0],
            DiffDisplayLine::Collapsed { count: 3, .. }
        ));
    }

    #[core::prelude::v1::test]
    fn test_compute_display_all_equal_expanded() {
        let lines = vec![
            make_diff_line(DiffLineTag::Equal, Some(1), Some(1), "a"),
            make_diff_line(DiffLineTag::Equal, Some(2), Some(2), "b"),
            make_diff_line(DiffLineTag::Equal, Some(3), Some(3), "c"),
        ];
        let display = compute_display_lines(&lines, &[(0, 3)], 3, None);
        assert_eq!(display.len(), 3);
        assert!(display
            .iter()
            .all(|dl| matches!(dl, DiffDisplayLine::Line { .. })));
    }

    #[core::prelude::v1::test]
    fn test_compute_display_context_around_change() {
        // 10 equal lines, then 1 change, then 10 equal lines = 21 total
        let mut lines = Vec::new();
        for i in 1..=10 {
            lines.push(make_diff_line(DiffLineTag::Equal, Some(i), Some(i), &format!("eq{}", i)));
        }
        lines.push(make_diff_line(DiffLineTag::Insert, None, Some(11), "added"));
        for i in 11..=20 {
            lines.push(make_diff_line(DiffLineTag::Equal, Some(i), Some(i + 1), &format!("eq{}", i)));
        }

        let context = 3;
        let display = compute_display_lines(&lines, &[], context, None);

        // Should have: collapsed(0..7) + 3 context + 1 change + 3 context + collapsed(14..21)
        let line_count = display
            .iter()
            .filter(|dl| matches!(dl, DiffDisplayLine::Line { .. }))
            .count();
        assert_eq!(line_count, 7); // 3 before + 1 change + 3 after

        let collapsed_count = display
            .iter()
            .filter(|dl| matches!(dl, DiffDisplayLine::Collapsed { .. }))
            .count();
        assert_eq!(collapsed_count, 2);
    }

    #[core::prelude::v1::test]
    fn test_compute_display_multiple_changes_merge_context() {
        // Two changes close together — context should merge
        let lines = vec![
            make_diff_line(DiffLineTag::Equal, Some(1), Some(1), "eq1"),
            make_diff_line(DiffLineTag::Insert, None, Some(2), "add1"),
            make_diff_line(DiffLineTag::Equal, Some(2), Some(3), "eq2"),
            make_diff_line(DiffLineTag::Equal, Some(3), Some(4), "eq3"),
            make_diff_line(DiffLineTag::Insert, None, Some(5), "add2"),
            make_diff_line(DiffLineTag::Equal, Some(4), Some(6), "eq4"),
        ];
        let display = compute_display_lines(&lines, &[], 3, None);

        // All lines are within context of changes — should all be visible, no collapsed
        let line_count = display
            .iter()
            .filter(|dl| matches!(dl, DiffDisplayLine::Line { .. }))
            .count();
        assert_eq!(line_count, 6);

        let collapsed_count = display
            .iter()
            .filter(|dl| matches!(dl, DiffDisplayLine::Collapsed { .. }))
            .count();
        assert_eq!(collapsed_count, 0);
    }

    #[core::prelude::v1::test]
    fn test_compute_display_with_buffer_line_map() {
        let lines = vec![
            make_diff_line(DiffLineTag::Insert, None, Some(1), "added"),
            make_diff_line(DiffLineTag::Delete, Some(1), None, "removed"),
        ];
        let buffer_map = vec![Some(0), Some(1)];
        let display = compute_display_lines(&lines, &[], 3, Some(&buffer_map));

        if let DiffDisplayLine::Line { buffer_line, .. } = &display[0] {
            assert_eq!(*buffer_line, Some(0));
        } else {
            panic!("Expected Line");
        }
        if let DiffDisplayLine::Line { buffer_line, .. } = &display[1] {
            assert_eq!(*buffer_line, Some(1));
        } else {
            panic!("Expected Line");
        }
    }

    #[core::prelude::v1::test]
    fn test_compute_display_empty_input() {
        let display = compute_display_lines(&[], &[], 3, None);
        assert!(display.is_empty());
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
