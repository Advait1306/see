use ropey::Rope;
use std::ops::Range;
use tree_sitter::{Query, QueryCursor, StreamingIterator, Tree};

#[derive(Debug, Clone)]
pub struct HighlightSpan {
    pub byte_range: Range<usize>,
    pub capture_name: String,
}

pub fn highlights_for_range(
    tree: &Tree,
    query: &Query,
    source: &Rope,
    byte_range: Range<usize>,
) -> Vec<HighlightSpan> {
    let mut cursor = QueryCursor::new();
    cursor.set_byte_range(byte_range.clone());

    let full_source: String = source.slice(..).into();
    let source_bytes = full_source.as_bytes();

    let mut spans = Vec::new();
    let capture_names = query.capture_names();

    let mut captures = cursor.captures(query, tree.root_node(), source_bytes);
    while let Some((query_match, capture_idx)) = captures.next() {
        let capture = &query_match.captures[*capture_idx];
        let node = capture.node;
        let capture_name = capture_names[capture.index as usize].to_string();

        spans.push(HighlightSpan {
            byte_range: node.start_byte()..node.end_byte(),
            capture_name,
        });
    }

    spans.sort_by(|a, b| {
        a.byte_range
            .start
            .cmp(&b.byte_range.start)
            .then_with(|| b.capture_name.len().cmp(&a.capture_name.len()))
    });

    spans
}

pub fn highlights_for_lines(
    tree: &Tree,
    query: &Query,
    source: &Rope,
    start_line: usize,
    end_line: usize,
) -> Vec<HighlightSpan> {
    let line_count = source.len_lines();
    if start_line >= line_count {
        return Vec::new();
    }

    let actual_end = end_line.min(line_count.saturating_sub(1));

    let start_byte = source.line_to_byte(start_line);
    let end_byte = if actual_end + 1 >= line_count {
        source.len_bytes()
    } else {
        source.line_to_byte(actual_end + 1)
    };

    highlights_for_range(tree, query, source, start_byte..end_byte)
}
