use similar::{DiffTag, TextDiff};
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffHunkStatus {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub buffer_range: Range<usize>,
    pub status: DiffHunkStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineDiff {
    #[default]
    Unchanged,
    Added,
    Modified,
    DeletedAbove,
}

pub fn compute_hunks(old: &str, new: &str) -> Vec<DiffHunk> {
    let diff = TextDiff::from_lines(old, new);
    let mut hunks = Vec::new();

    for group in diff.grouped_ops(3) {
        let mut buffer_start = None;
        let mut buffer_end = 0;
        let mut has_deletes = false;
        let mut has_inserts = false;

        for op in &group {
            match op.tag() {
                DiffTag::Delete => {
                    has_deletes = true;
                    if buffer_start.is_none() {
                        buffer_start = Some(op.new_range().start);
                    }
                }
                DiffTag::Insert => {
                    has_inserts = true;
                    if buffer_start.is_none() {
                        buffer_start = Some(op.new_range().start);
                    }
                    buffer_end = op.new_range().end;
                }
                DiffTag::Equal => {
                    if buffer_start.is_none() {
                        buffer_start = Some(op.new_range().start);
                    }
                    buffer_end = op.new_range().end;
                }
                DiffTag::Replace => {
                    has_deletes = true;
                    has_inserts = true;
                    if buffer_start.is_none() {
                        buffer_start = Some(op.new_range().start);
                    }
                    buffer_end = op.new_range().end;
                }
            }
        }

        let status = match (has_deletes, has_inserts) {
            (true, true) => DiffHunkStatus::Modified,
            (true, false) => DiffHunkStatus::Deleted,
            (false, true) => DiffHunkStatus::Added,
            (false, false) => continue,
        };

        let start = buffer_start.unwrap_or(0);
        let end = if status == DiffHunkStatus::Deleted {
            start
        } else {
            buffer_end
        };

        hunks.push(DiffHunk {
            buffer_range: start..end,
            status,
        });
    }

    hunks
}

pub fn compute_line_diffs(hunks: &[DiffHunk], line_count: usize) -> Vec<LineDiff> {
    let mut line_diffs = vec![LineDiff::Unchanged; line_count];

    for hunk in hunks {
        match hunk.status {
            DiffHunkStatus::Added => {
                for line in hunk.buffer_range.clone() {
                    if line < line_count {
                        line_diffs[line] = LineDiff::Added;
                    }
                }
            }
            DiffHunkStatus::Modified => {
                for line in hunk.buffer_range.clone() {
                    if line < line_count {
                        line_diffs[line] = LineDiff::Modified;
                    }
                }
            }
            DiffHunkStatus::Deleted => {
                let line = hunk.buffer_range.start;
                if line < line_count {
                    line_diffs[line] = LineDiff::DeletedAbove;
                }
            }
        }
    }

    line_diffs
}
