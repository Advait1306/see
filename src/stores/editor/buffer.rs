use super::super::git::{compute_hunks, compute_line_diffs, LineDiff};
use crate::syntax::{highlights_for_lines, HighlightSpan, Language, LanguageRegistry};
use git2::{Oid, Repository};
use gpui::{Context, EventEmitter};
use ropey::Rope;
use similar::{ChangeTag, TextDiff};
use std::fs;
use std::io::{self, BufReader, BufWriter};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use tree_sitter::{Parser, Tree};

/// A line in a unified diff view
#[derive(Debug, Clone)]
pub struct DiffLine {
    pub tag: DiffLineTag,
    pub old_line_num: Option<usize>,
    pub new_line_num: Option<usize>,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineTag {
    Equal,
    Insert,
    Delete,
}

#[derive(Debug, Clone)]
pub enum BufferEvent {
    Changed,
    Saved,
    ExternalChange,
}

/// Editor state to restore on undo (cursor position and optional selection)
#[derive(Debug, Clone, Default)]
pub struct EditorState {
    pub cursor: (usize, usize), // (line, col)
    /// Selection as (anchor_line, anchor_col, end_line, end_col), None if no selection
    pub selection: Option<(usize, usize, usize, usize)>,
}

impl EditorState {
    pub fn new(cursor: (usize, usize)) -> Self {
        Self { cursor, selection: None }
    }

    pub fn with_selection(cursor: (usize, usize), selection: (usize, usize, usize, usize)) -> Self {
        Self { cursor, selection: Some(selection) }
    }
}

/// Represents a single undoable operation
#[derive(Debug, Clone)]
enum UndoOperation {
    /// Text was inserted at offset
    Insert {
        offset: usize,
        text: String,
        state_before: EditorState,
    },
    /// Text was deleted from start..end
    Delete {
        offset: usize,
        text: String, // The deleted text (for restoring)
        state_before: EditorState,
    },
}

pub struct Buffer {
    rope: Rope,
    file_path: PathBuf,
    saved_mtime: Option<SystemTime>,
    is_dirty: bool,
    undo_stack: Vec<UndoOperation>,
    redo_stack: Vec<UndoOperation>,
    /// Per-line diff status for gutter indicators
    line_diffs: Vec<LineDiff>,
    /// Full diff lines for unified diff view
    diff_lines: Vec<DiffLine>,
    /// Cached git repository for this file
    repository: Option<Arc<Repository>>,
    /// Tracked HEAD commit for detecting when diffs need recomputing
    head_oid: Option<Oid>,
    /// Language for syntax highlighting
    language: Option<Arc<Language>>,
    /// Parsed syntax tree
    syntax_tree: Option<Tree>,
    /// Tree-sitter parser instance
    parser: Option<Parser>,
}

impl EventEmitter<BufferEvent> for Buffer {}

impl Buffer {
    pub fn load(path: PathBuf, cx: &mut Context<Self>) -> io::Result<Self> {
        let file = fs::File::open(&path)?;
        let mtime = file.metadata()?.modified().ok();
        let reader = BufReader::new(file);
        let rope = Rope::from_reader(reader)?;

        // Try to discover git repository for this file
        let repository = Repository::discover(&path).ok().map(Arc::new);

        let head_oid = repository
            .as_ref()
            .and_then(|repo| repo.head().ok()?.peel_to_commit().ok())
            .map(|c| c.id());

        let mut buffer = Self {
            rope,
            file_path: path.clone(),
            saved_mtime: mtime,
            is_dirty: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            line_diffs: Vec::new(),
            diff_lines: Vec::new(),
            repository,
            head_oid,
            language: None,
            syntax_tree: None,
            parser: None,
        };

        // Compute initial diffs
        buffer.recompute_diffs();

        // Detect and set language based on file extension
        let registry = LanguageRegistry::global(cx);
        if let Some(lang) = registry.read(cx).language_for_path(&path) {
            buffer.set_language(lang);
        }

        // Start polling for external changes
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(500))
                    .await;

                let should_stop = this
                    .update(cx, |buffer, cx| {
                        buffer.check_and_reload_if_changed(cx);
                    })
                    .is_err();

                if should_stop {
                    break;
                }
            }
        })
        .detach();

        Ok(buffer)
    }

    fn check_and_reload_if_changed(&mut self, cx: &mut Context<Self>) {
        if self.check_external_changes() {
            cx.emit(BufferEvent::ExternalChange);
            if !self.is_dirty {
                let _ = self.reload(cx);
            }
        }

        // Check if HEAD changed (e.g., after a commit) and recompute diffs
        if self.check_head_changed() {
            self.recompute_diffs();
            cx.notify();
        }
    }

    fn check_head_changed(&mut self) -> bool {
        let current_head = self
            .repository
            .as_ref()
            .and_then(|repo| repo.head().ok()?.peel_to_commit().ok())
            .map(|c| c.id());

        if current_head != self.head_oid {
            self.head_oid = current_head;
            return true;
        }
        false
    }

    pub fn save(&mut self, cx: &mut Context<Self>) -> io::Result<()> {
        let file = fs::File::create(&self.file_path)?;
        let writer = BufWriter::new(file);
        self.rope.write_to(writer)?;

        // Update mtime after save
        if let Ok(metadata) = fs::metadata(&self.file_path) {
            self.saved_mtime = metadata.modified().ok();
        }

        self.is_dirty = false;
        cx.emit(BufferEvent::Saved);
        cx.notify();
        Ok(())
    }

    /// Insert text at offset with editor state for undo
    pub fn insert_with_state(
        &mut self,
        offset: usize,
        text: &str,
        state_before: EditorState,
        cx: &mut Context<Self>,
    ) {
        let offset = offset.min(self.rope.len_chars());

        // Record for undo
        self.undo_stack.push(UndoOperation::Insert {
            offset,
            text: text.to_string(),
            state_before,
        });
        // Clear redo stack on new operation
        self.redo_stack.clear();

        self.rope.insert(offset, text);
        self.is_dirty = true;
        self.parse_syntax();
        cx.emit(BufferEvent::Changed);
        cx.notify();
    }

    /// Delete text from start..end with editor state for undo
    pub fn delete_with_state(
        &mut self,
        start: usize,
        end: usize,
        state_before: EditorState,
        cx: &mut Context<Self>,
    ) {
        let start = start.min(self.rope.len_chars());
        let end = end.min(self.rope.len_chars());
        if start < end {
            // Save the text being deleted for undo
            let deleted_text = self.rope.slice(start..end).to_string();

            // Record for undo
            self.undo_stack.push(UndoOperation::Delete {
                offset: start,
                text: deleted_text,
                state_before,
            });
            // Clear redo stack on new operation
            self.redo_stack.clear();

            self.rope.remove(start..end);
            self.is_dirty = true;
            self.parse_syntax();
            cx.emit(BufferEvent::Changed);
            cx.notify();
        }
    }

    /// Undo the last operation, returns editor state to restore if successful
    pub fn undo(&mut self, cx: &mut Context<Self>) -> Option<EditorState> {
        let op = self.undo_stack.pop()?;

        let state = match &op {
            UndoOperation::Insert { offset, text, state_before } => {
                // Undo insert = delete the inserted text
                let end = offset + text.chars().count();
                self.rope.remove(*offset..end);
                state_before.clone()
            }
            UndoOperation::Delete { offset, text, state_before } => {
                // Undo delete = re-insert the deleted text
                self.rope.insert(*offset, text);
                state_before.clone()
            }
        };

        // Push to redo stack (inverted operation)
        self.redo_stack.push(op);

        self.is_dirty = true;
        cx.emit(BufferEvent::Changed);
        cx.notify();

        Some(state)
    }

    /// Redo the last undone operation, returns editor state if successful
    pub fn redo(&mut self, cx: &mut Context<Self>) -> Option<EditorState> {
        let op = self.redo_stack.pop()?;

        let state = match &op {
            UndoOperation::Insert { offset, text, .. } => {
                // Redo insert = insert the text again
                self.rope.insert(*offset, text);
                // Position cursor at end of inserted text
                let end_offset = offset + text.chars().count();
                EditorState::new(self.offset_to_line_col(end_offset))
            }
            UndoOperation::Delete { offset, text, .. } => {
                // Redo delete = delete the text again
                let end = offset + text.chars().count();
                self.rope.remove(*offset..end);
                // Position cursor at deletion point
                EditorState::new(self.offset_to_line_col(*offset))
            }
        };

        // Push back to undo stack
        self.undo_stack.push(op);

        self.is_dirty = true;
        cx.emit(BufferEvent::Changed);
        cx.notify();

        Some(state)
    }

    pub fn check_external_changes(&self) -> bool {
        if let Ok(metadata) = fs::metadata(&self.file_path)
            && let (Some(saved), Ok(current)) = (self.saved_mtime, metadata.modified()) {
                return current != saved;
            }
        false
    }

    pub fn reload(&mut self, cx: &mut Context<Self>) -> io::Result<()> {
        let file = fs::File::open(&self.file_path)?;
        let mtime = file.metadata()?.modified().ok();
        let reader = BufReader::new(file);
        let rope = Rope::from_reader(reader)?;

        self.rope = rope;
        self.saved_mtime = mtime;
        self.is_dirty = false;
        // Clear undo/redo history on reload
        self.undo_stack.clear();
        self.redo_stack.clear();
        // Recompute diffs
        self.recompute_diffs();
        cx.emit(BufferEvent::Changed);
        cx.notify();
        Ok(())
    }

    pub fn line(&self, line_idx: usize) -> Option<String> {
        if line_idx < self.rope.len_lines() {
            Some(self.rope.line(line_idx).to_string())
        } else {
            None
        }
    }

    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    /// Returns the length of the longest line in the buffer (excluding newline)
    pub fn max_line_len(&self) -> usize {
        let mut max_len = 0;
        for i in 0..self.rope.len_lines() {
            let line = self.rope.line(i);
            let len = line.len_chars().saturating_sub(
                if line.to_string().ends_with('\n') { 1 } else { 0 }
            );
            if len > max_len {
                max_len = len;
            }
        }
        max_len
    }

    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    pub fn file_name(&self) -> String {
        self.file_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled".to_string())
    }

    /// Convert a (line, col) position to a character offset
    pub fn line_col_to_offset(&self, line: usize, col: usize) -> usize {
        if line >= self.rope.len_lines() {
            return self.rope.len_chars();
        }
        let line_start = self.rope.line_to_char(line);
        let line_len = self.rope.line(line).len_chars();
        // Subtract 1 for newline if not last line
        let effective_line_len = if line + 1 < self.rope.len_lines() {
            line_len.saturating_sub(1)
        } else {
            line_len
        };
        line_start + col.min(effective_line_len)
    }

    /// Convert a character offset to (line, col) position
    pub fn offset_to_line_col(&self, offset: usize) -> (usize, usize) {
        let offset = offset.min(self.rope.len_chars());
        let line = self.rope.char_to_line(offset);
        let line_start = self.rope.line_to_char(line);
        let col = offset - line_start;
        (line, col)
    }

    /// Get the length of a line (excluding newline)
    pub fn line_len(&self, line: usize) -> usize {
        if line >= self.rope.len_lines() {
            return 0;
        }
        let line_content = self.rope.line(line);
        let len = line_content.len_chars();
        // Subtract 1 for newline if not last line
        if line + 1 < self.rope.len_lines() {
            len.saturating_sub(1)
        } else {
            len
        }
    }

    /// Get the character at a given offset
    pub fn char_at(&self, offset: usize) -> Option<char> {
        if offset < self.rope.len_chars() {
            Some(self.rope.char(offset))
        } else {
            None
        }
    }

    /// Get the total number of characters in the buffer
    pub fn total_chars(&self) -> usize {
        self.rope.len_chars()
    }

    /// Get the diff status for a specific line
    pub fn line_diff(&self, line: usize) -> LineDiff {
        self.line_diffs.get(line).copied().unwrap_or(LineDiff::Unchanged)
    }

    /// Get the unified diff lines for diff mode display
    pub fn diff_lines(&self) -> &[DiffLine] {
        &self.diff_lines
    }

    /// Recompute diffs by comparing current content with HEAD
    pub fn recompute_diffs(&mut self) {
        let Some(repo) = &self.repository else {
            self.line_diffs.clear();
            self.diff_lines.clear();
            return;
        };

        // For new files not in HEAD, treat HEAD content as empty (all lines are added)
        let head_content = self.get_head_content(repo).unwrap_or_default();
        let current_content = self.rope.to_string();

        // Compute line diffs for gutter indicators
        let hunks = compute_hunks(&head_content, &current_content);
        let line_count = self.rope.len_lines().max(1);
        self.line_diffs = compute_line_diffs(&hunks, line_count);

        // Compute unified diff lines for diff mode
        self.diff_lines = self.compute_unified_diff(&head_content, &current_content);
    }

    fn get_head_content(&self, repo: &Repository) -> Option<String> {
        let workdir = repo.workdir()?;
        let relative_path = self.file_path.strip_prefix(workdir).ok()?;

        let head = repo.head().ok()?;
        let tree = head.peel_to_tree().ok()?;
        let entry = tree.get_path(relative_path).ok()?;
        let blob = repo.find_blob(entry.id()).ok()?;

        if blob.is_binary() {
            return None;
        }

        String::from_utf8(blob.content().to_vec()).ok()
    }

    fn compute_unified_diff(&self, old_content: &str, new_content: &str) -> Vec<DiffLine> {
        let diff = TextDiff::from_lines(old_content, new_content);
        let mut lines = Vec::new();
        let mut old_line = 1usize;
        let mut new_line = 1usize;

        for change in diff.iter_all_changes() {
            let content = change.value().trim_end_matches('\n').to_string();
            match change.tag() {
                ChangeTag::Equal => {
                    lines.push(DiffLine {
                        tag: DiffLineTag::Equal,
                        old_line_num: Some(old_line),
                        new_line_num: Some(new_line),
                        content,
                    });
                    old_line += 1;
                    new_line += 1;
                }
                ChangeTag::Delete => {
                    lines.push(DiffLine {
                        tag: DiffLineTag::Delete,
                        old_line_num: Some(old_line),
                        new_line_num: None,
                        content,
                    });
                    old_line += 1;
                }
                ChangeTag::Insert => {
                    lines.push(DiffLine {
                        tag: DiffLineTag::Insert,
                        old_line_num: None,
                        new_line_num: Some(new_line),
                        content,
                    });
                    new_line += 1;
                }
            }
        }

        lines
    }

    pub fn set_language(&mut self, lang: Arc<Language>) {
        let mut parser = Parser::new();
        parser.set_language(&lang.grammar()).ok();
        self.parser = Some(parser);
        self.language = Some(lang);
        self.parse_syntax();
    }

    fn parse_syntax(&mut self) {
        let Some(ref mut parser) = self.parser else { return };
        let source: String = self.rope.slice(..).into();
        self.syntax_tree = parser.parse(source.as_bytes(), None);
    }

    pub fn line_to_byte(&self, line: usize) -> usize {
        if line >= self.rope.len_lines() {
            return self.rope.len_bytes();
        }
        self.rope.line_to_byte(line)
    }

    pub fn highlights_for_visible_lines(&self, start_line: usize, end_line: usize) -> Option<Vec<HighlightSpan>> {
        let tree = self.syntax_tree.as_ref()?;
        let lang = self.language.as_ref()?;
        Some(highlights_for_lines(tree, &lang.highlights_query, &self.rope, start_line, end_line))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::AppContext as _;

    fn make_buffer_from_text(text: &str) -> Buffer {
        Buffer {
            rope: Rope::from_str(text),
            file_path: PathBuf::from("test.txt"),
            saved_mtime: None,
            is_dirty: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            line_diffs: Vec::new(),
            diff_lines: Vec::new(),
            repository: None,
            head_oid: None,
            language: None,
            syntax_tree: None,
            parser: None,
        }
    }

    #[test]
    fn test_line_col_to_offset_basic() {
        let buf = make_buffer_from_text("hello\nworld\n");
        assert_eq!(buf.line_col_to_offset(0, 0), 0);
        assert_eq!(buf.line_col_to_offset(0, 3), 3);
        assert_eq!(buf.line_col_to_offset(1, 0), 6);
        assert_eq!(buf.line_col_to_offset(1, 2), 8);
    }

    #[test]
    fn test_line_col_to_offset_clamped() {
        let buf = make_buffer_from_text("hi\nbye\n");
        // Col beyond line length should clamp
        assert_eq!(buf.line_col_to_offset(0, 100), 2);
        // Line beyond buffer should return end
        assert_eq!(buf.line_col_to_offset(99, 0), buf.total_chars());
    }

    #[test]
    fn test_offset_to_line_col_roundtrip() {
        let buf = make_buffer_from_text("abc\ndef\nghi\n");
        for offset in 0..buf.total_chars() {
            let (line, col) = buf.offset_to_line_col(offset);
            let back = buf.line_col_to_offset(line, col);
            assert_eq!(back, offset, "Roundtrip failed for offset {}", offset);
        }
    }

    #[test]
    fn test_line_len() {
        let buf = make_buffer_from_text("hello\nhi\n\n");
        assert_eq!(buf.line_len(0), 5);
        assert_eq!(buf.line_len(1), 2);
        assert_eq!(buf.line_len(2), 0); // empty line before trailing newline
    }

    #[test]
    fn test_line_count() {
        let buf = make_buffer_from_text("a\nb\nc\n");
        assert_eq!(buf.line_count(), 4); // ropey counts trailing empty line
    }

    #[test]
    fn test_char_at() {
        let buf = make_buffer_from_text("abc");
        assert_eq!(buf.char_at(0), Some('a'));
        assert_eq!(buf.char_at(2), Some('c'));
        assert_eq!(buf.char_at(3), None);
    }

    #[test]
    fn test_total_chars() {
        let buf = make_buffer_from_text("hello");
        assert_eq!(buf.total_chars(), 5);
    }

    #[test]
    fn test_max_line_len() {
        let buf = make_buffer_from_text("short\nlonger line\nhi\n");
        assert_eq!(buf.max_line_len(), 11); // "longer line"
    }

    #[test]
    fn test_file_name() {
        let mut buf = make_buffer_from_text("");
        assert_eq!(buf.file_name(), "test.txt");
        buf.file_path = PathBuf::from("/some/path/main.rs");
        assert_eq!(buf.file_name(), "main.rs");
    }

    #[test]
    fn test_buffer_load_from_file() {
        crate::test_helpers::run_gpui_test(|cx| {
            let fixture = crate::test_helpers::TestFixture::new(cx);
            let path = fixture.create_file("hello.txt", "line one\nline two\nline three\n");

            let buffer = cx.new(|cx| Buffer::load(path, cx).unwrap());

            cx.read(|cx| {
                let buf = buffer.read(cx);
                assert_eq!(buf.line_count(), 4); // ropey trailing empty line
                assert_eq!(buf.line(0), Some("line one\n".to_string()));
                assert!(!buf.is_dirty());
            });
        });
    }

    #[test]
    fn test_buffer_insert_with_state() {
        crate::test_helpers::run_gpui_test(|cx| {
            let fixture = crate::test_helpers::TestFixture::new(cx);
            let path = fixture.create_file("test.txt", "hello");

            let buffer = cx.new(|cx| Buffer::load(path, cx).unwrap());

            buffer.update(cx, |buf, cx| {
                buf.insert_with_state(5, " world", EditorState::new((0, 5)), cx);
            });

            cx.read(|cx| {
                let buf = buffer.read(cx);
                assert_eq!(buf.line(0), Some("hello world".to_string()));
                assert!(buf.is_dirty());
            });
        });
    }

    #[test]
    fn test_buffer_delete_with_state() {
        crate::test_helpers::run_gpui_test(|cx| {
            let fixture = crate::test_helpers::TestFixture::new(cx);
            let path = fixture.create_file("test.txt", "hello world");

            let buffer = cx.new(|cx| Buffer::load(path, cx).unwrap());

            buffer.update(cx, |buf, cx| {
                buf.delete_with_state(5, 11, EditorState::new((0, 5)), cx);
            });

            cx.read(|cx| {
                let buf = buffer.read(cx);
                assert_eq!(buf.line(0), Some("hello".to_string()));
                assert!(buf.is_dirty());
            });
        });
    }

    #[test]
    fn test_buffer_undo() {
        crate::test_helpers::run_gpui_test(|cx| {
            let fixture = crate::test_helpers::TestFixture::new(cx);
            let path = fixture.create_file("test.txt", "original");

            let buffer = cx.new(|cx| Buffer::load(path, cx).unwrap());

            buffer.update(cx, |buf, cx| {
                buf.insert_with_state(8, " added", EditorState::new((0, 8)), cx);
            });

            cx.read(|cx| {
                assert_eq!(buffer.read(cx).line(0), Some("original added".to_string()));
            });

            buffer.update(cx, |buf, cx| {
                let state = buf.undo(cx);
                assert!(state.is_some());
            });

            cx.read(|cx| {
                assert_eq!(buffer.read(cx).line(0), Some("original".to_string()));
            });
        });
    }

    #[test]
    fn test_buffer_redo() {
        crate::test_helpers::run_gpui_test(|cx| {
            let fixture = crate::test_helpers::TestFixture::new(cx);
            let path = fixture.create_file("test.txt", "original");

            let buffer = cx.new(|cx| Buffer::load(path, cx).unwrap());

            buffer.update(cx, |buf, cx| {
                buf.insert_with_state(8, " added", EditorState::new((0, 8)), cx);
            });

            buffer.update(cx, |buf, cx| {
                buf.undo(cx);
            });

            buffer.update(cx, |buf, cx| {
                let state = buf.redo(cx);
                assert!(state.is_some());
            });

            cx.read(|cx| {
                assert_eq!(buffer.read(cx).line(0), Some("original added".to_string()));
            });
        });
    }

    #[test]
    fn test_buffer_undo_redo_clears_redo_on_new_edit() {
        crate::test_helpers::run_gpui_test(|cx| {
            let fixture = crate::test_helpers::TestFixture::new(cx);
            let path = fixture.create_file("test.txt", "abc");

            let buffer = cx.new(|cx| Buffer::load(path, cx).unwrap());

            buffer.update(cx, |buf, cx| {
                buf.insert_with_state(3, "d", EditorState::new((0, 3)), cx);
            });

            buffer.update(cx, |buf, cx| {
                buf.undo(cx);
            });

            // New edit should clear redo stack
            buffer.update(cx, |buf, cx| {
                buf.insert_with_state(3, "x", EditorState::new((0, 3)), cx);
            });

            buffer.update(cx, |buf, cx| {
                let state = buf.redo(cx);
                assert!(state.is_none(), "Redo should be empty after new edit");
            });
        });
    }

    #[test]
    fn test_buffer_save() {
        crate::test_helpers::run_gpui_test(|cx| {
            let fixture = crate::test_helpers::TestFixture::new(cx);
            let path = fixture.create_file("test.txt", "before");

            let buffer = cx.new(|cx| Buffer::load(path.clone(), cx).unwrap());

            buffer.update(cx, |buf, cx| {
                buf.insert_with_state(6, " after", EditorState::new((0, 6)), cx);
                buf.save(cx).unwrap();
            });

            cx.read(|cx| {
                assert!(!buffer.read(cx).is_dirty());
            });

            let saved_content = std::fs::read_to_string(&path).unwrap();
            assert_eq!(saved_content, "before after");
        });
    }
}
