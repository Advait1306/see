use gpui::prelude::*;
use gpui::*;
use ropey::Rope;
use std::fs;
use std::io::{self, BufReader, BufWriter};
use std::path::PathBuf;
use std::time::SystemTime;

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
}

impl EventEmitter<BufferEvent> for Buffer {}

impl Buffer {
    pub fn load(path: PathBuf, _cx: &mut Context<Self>) -> io::Result<Self> {
        let file = fs::File::open(&path)?;
        let mtime = file.metadata()?.modified().ok();
        let reader = BufReader::new(file);
        let rope = Rope::from_reader(reader)?;

        Ok(Self {
            rope,
            file_path: path,
            saved_mtime: mtime,
            is_dirty: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        })
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
        if let Ok(metadata) = fs::metadata(&self.file_path) {
            if let (Some(saved), Ok(current)) = (self.saved_mtime, metadata.modified()) {
                return current != saved;
            }
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

    pub fn file_path(&self) -> &PathBuf {
        &self.file_path
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

    /// Get a slice of text between two character offsets
    pub fn slice(&self, start: usize, end: usize) -> Option<String> {
        let start = start.min(self.rope.len_chars());
        let end = end.min(self.rope.len_chars());
        if start <= end {
            Some(self.rope.slice(start..end).to_string())
        } else {
            None
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
}
