use crate::stores::{DiffLine as EditorDiffLine, DiffLineTag};

#[derive(Debug, Clone, PartialEq)]
pub enum DiffLineKind {
    Context,
    Addition,
    Deletion,
    Header,
}

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub content: String,
    pub line_number_old: Option<u64>,
    pub line_number_new: Option<u64>,
}

pub fn parse_patch(patch: &str) -> Vec<DiffLine> {
    let mut lines = Vec::new();
    let mut old_line: u64 = 0;
    let mut new_line: u64 = 0;

    for raw_line in patch.lines() {
        if raw_line.starts_with("@@") {
            if let Some((old_start, new_start)) = parse_hunk_header(raw_line) {
                old_line = old_start;
                new_line = new_start;
            }
            lines.push(DiffLine {
                kind: DiffLineKind::Header,
                content: raw_line.to_string(),
                line_number_old: None,
                line_number_new: None,
            });
        } else if let Some(rest) = raw_line.strip_prefix('+') {
            lines.push(DiffLine {
                kind: DiffLineKind::Addition,
                content: rest.to_string(),
                line_number_old: None,
                line_number_new: Some(new_line),
            });
            new_line += 1;
        } else if let Some(rest) = raw_line.strip_prefix('-') {
            lines.push(DiffLine {
                kind: DiffLineKind::Deletion,
                content: rest.to_string(),
                line_number_old: Some(old_line),
                line_number_new: None,
            });
            old_line += 1;
        } else {
            let content = raw_line.strip_prefix(' ').unwrap_or(raw_line);
            lines.push(DiffLine {
                kind: DiffLineKind::Context,
                content: content.to_string(),
                line_number_old: Some(old_line),
                line_number_new: Some(new_line),
            });
            old_line += 1;
            new_line += 1;
        }
    }

    lines
}

fn parse_hunk_header(header: &str) -> Option<(u64, u64)> {
    // Format: @@ -old_start,old_count +new_start,new_count @@
    let stripped = header.strip_prefix("@@ -")?;
    let at_pos = stripped.find(" @@")?;
    let range_part = &stripped[..at_pos];

    let mut parts = range_part.split(' ');
    let old_range = parts.next()?;
    let new_range = parts.next()?.strip_prefix('+')?;

    let old_start: u64 = old_range.split(',').next()?.parse().ok()?;
    let new_start: u64 = new_range.split(',').next()?.parse().ok()?;

    Some((old_start, new_start))
}

/// Convert a GitHub patch string into editor-compatible diff data.
///
/// Returns:
/// - `content`: All line contents joined by newlines (for creating an in-memory Buffer)
/// - `diff_lines`: Lines in the editor's DiffLine format (Header lines skipped)
/// - `buffer_line_map`: Sequential buffer line indices for syntax highlighting
pub fn patch_to_editor_diff_lines(
    patch: &str,
) -> (String, Vec<EditorDiffLine>, Vec<Option<usize>>) {
    let parsed = parse_patch(patch);

    let mut content_lines: Vec<String> = Vec::new();
    let mut diff_lines: Vec<EditorDiffLine> = Vec::new();
    let mut buffer_line_map: Vec<Option<usize>> = Vec::new();
    let mut buffer_idx: usize = 0;

    for line in &parsed {
        if line.kind == DiffLineKind::Header {
            continue;
        }

        let tag = match line.kind {
            DiffLineKind::Context => DiffLineTag::Equal,
            DiffLineKind::Addition => DiffLineTag::Insert,
            DiffLineKind::Deletion => DiffLineTag::Delete,
            DiffLineKind::Header => unreachable!(),
        };

        diff_lines.push(EditorDiffLine {
            tag,
            old_line_num: line.line_number_old.map(|n| n as usize),
            new_line_num: line.line_number_new.map(|n| n as usize),
            content: line.content.clone(),
        });

        content_lines.push(line.content.clone());
        buffer_line_map.push(Some(buffer_idx));
        buffer_idx += 1;
    }

    let content = content_lines.join("\n");
    (content, diff_lines, buffer_line_map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[core::prelude::v1::test]
    fn test_parse_patch_basic() {
        let patch = "@@ -1,3 +1,4 @@\n context\n-removed\n+added\n+new line\n context end";
        let lines = parse_patch(patch);

        assert_eq!(lines.len(), 6);
        assert_eq!(lines[0].kind, DiffLineKind::Header);
        assert_eq!(lines[1].kind, DiffLineKind::Context);
        assert_eq!(lines[1].content, "context");
        assert_eq!(lines[2].kind, DiffLineKind::Deletion);
        assert_eq!(lines[2].content, "removed");
        assert_eq!(lines[3].kind, DiffLineKind::Addition);
        assert_eq!(lines[3].content, "added");
        assert_eq!(lines[4].kind, DiffLineKind::Addition);
        assert_eq!(lines[4].content, "new line");
        assert_eq!(lines[5].kind, DiffLineKind::Context);
        assert_eq!(lines[5].content, "context end");
    }

    #[core::prelude::v1::test]
    fn test_parse_patch_line_numbers() {
        let patch = "@@ -10,2 +20,3 @@\n context\n+added";
        let lines = parse_patch(patch);

        assert_eq!(lines[1].line_number_old, Some(10));
        assert_eq!(lines[1].line_number_new, Some(20));
        assert_eq!(lines[2].line_number_old, None);
        assert_eq!(lines[2].line_number_new, Some(21));
    }

    #[core::prelude::v1::test]
    fn test_parse_hunk_header() {
        assert_eq!(parse_hunk_header("@@ -1,3 +1,4 @@"), Some((1, 1)));
        assert_eq!(parse_hunk_header("@@ -10,5 +20,7 @@ fn main()"), Some((10, 20)));
        assert_eq!(parse_hunk_header("not a header"), None);
    }

    #[core::prelude::v1::test]
    fn test_parse_patch_empty() {
        let lines = parse_patch("");
        assert!(lines.is_empty() || lines.len() == 1);
    }

    #[core::prelude::v1::test]
    fn test_parse_patch_multiple_hunks() {
        let patch = "@@ -1,2 +1,2 @@\n-old\n+new\n@@ -10,2 +10,2 @@\n-old2\n+new2";
        let lines = parse_patch(patch);

        let headers: Vec<_> = lines
            .iter()
            .filter(|l| l.kind == DiffLineKind::Header)
            .collect();
        assert_eq!(headers.len(), 2);
    }
}
