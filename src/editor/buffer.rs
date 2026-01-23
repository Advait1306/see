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

pub struct Buffer {
    rope: Rope,
    file_path: PathBuf,
    saved_mtime: Option<SystemTime>,
    is_dirty: bool,
}

impl EventEmitter<BufferEvent> for Buffer {}

impl Buffer {
    pub fn load(path: PathBuf, cx: &mut Context<Self>) -> io::Result<Self> {
        let file = fs::File::open(&path)?;
        let mtime = file.metadata()?.modified().ok();
        let reader = BufReader::new(file);
        let rope = Rope::from_reader(reader)?;

        Ok(Self {
            rope,
            file_path: path,
            saved_mtime: mtime,
            is_dirty: false,
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

    pub fn insert(&mut self, offset: usize, text: &str, cx: &mut Context<Self>) {
        let offset = offset.min(self.rope.len_chars());
        self.rope.insert(offset, text);
        self.is_dirty = true;
        cx.emit(BufferEvent::Changed);
        cx.notify();
    }

    pub fn delete(&mut self, start: usize, end: usize, cx: &mut Context<Self>) {
        let start = start.min(self.rope.len_chars());
        let end = end.min(self.rope.len_chars());
        if start < end {
            self.rope.remove(start..end);
            self.is_dirty = true;
            cx.emit(BufferEvent::Changed);
            cx.notify();
        }
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
        cx.emit(BufferEvent::Changed);
        cx.notify();
        Ok(())
    }

    pub fn text(&self) -> String {
        self.rope.to_string()
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

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
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
}
