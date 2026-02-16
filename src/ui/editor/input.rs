//! Keyboard input handling for the editor

use super::selection::Selection;
use super::view::EditorView;
use crate::stores::EditorState;
use crate::types::SelectionPhase;
use gpui::{Context, KeyDownEvent};

/// Check if a character is a word character (alphanumeric or underscore)
pub(crate) fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// Handle a key event in the editor
pub(crate) fn handle_key(view: &mut EditorView, event: &KeyDownEvent, cx: &mut Context<EditorView>) {
    // Diff mode is read-only - ignore all input
    if view.is_diff_mode() {
        return;
    }

    // Clone buffer to avoid borrow issues - no buffer means read-only
    let Some(buffer) = view.buffer.clone() else {
        return;
    };

    // Reset cursor blink on any key press
    view.reset_cursor_blink();

    let key = &event.keystroke.key;
    let modifiers = &event.keystroke.modifiers;

    // Handle Ctrl/Cmd+S for save
    if modifiers.platform && key == "s" {
        buffer.update(cx, |buf, cx| {
            let _ = buf.save(cx);
        });
        return;
    }

    // Handle Cmd+Z for undo
    if modifiers.platform && !modifiers.shift && key == "z" {
        if let Some(state) = buffer.update(cx, |buf, cx| buf.undo(cx)) {
            view.cursor_line = state.cursor.0;
            view.cursor_col = state.cursor.1;
            // Restore selection if there was one
            if let Some((anchor_line, anchor_col, end_line, end_col)) = state.selection {
                view.selection = Some(Selection {
                    anchor_line,
                    anchor_col,
                    end_line,
                    end_col,
                });
                view.selection_phase = SelectionPhase::Ended;
            } else {
                view.clear_selection();
            }
            view.ensure_cursor_valid(cx);
            view.ensure_cursor_visible(cx);
        }
        cx.notify();
        return;
    }

    // Handle Cmd+Shift+Z for redo
    if modifiers.platform && modifiers.shift && key == "z" {
        if let Some(state) = buffer.update(cx, |buf, cx| buf.redo(cx)) {
            view.cursor_line = state.cursor.0;
            view.cursor_col = state.cursor.1;
            // Redo doesn't restore selection (it was deleted)
            view.clear_selection();
            view.ensure_cursor_valid(cx);
            view.ensure_cursor_visible(cx);
        }
        cx.notify();
        return;
    }

    // Handle Option+Arrow for word navigation
    if modifiers.alt && key == "left" {
        view.clear_selection();
        move_word_left(view, cx);
        cx.notify();
        return;
    }
    if modifiers.alt && key == "right" {
        view.clear_selection();
        move_word_right(view, cx);
        cx.notify();
        return;
    }

    // Handle navigation keys (clear selection on navigation)
    match key.as_str() {
        "up" => {
            view.clear_selection();
            if view.cursor_line > 0 {
                view.cursor_line -= 1;
                view.ensure_cursor_valid(cx);
                view.ensure_cursor_visible(cx);
                cx.notify();
            }
        }
        "down" => {
            view.clear_selection();
            let line_count = buffer.read(cx).line_count();
            if view.cursor_line + 1 < line_count {
                view.cursor_line += 1;
                view.ensure_cursor_valid(cx);
                view.ensure_cursor_visible(cx);
                cx.notify();
            }
        }
        "left" => {
            // If there's a selection, move cursor to start of selection
            if let Some(selection) = view.selection.take() {
                if !selection.is_empty() {
                    let ((start_line, start_col), _) = selection.normalized();
                    view.cursor_line = start_line;
                    view.cursor_col = start_col;
                    view.selection_phase = SelectionPhase::None;
                    view.ensure_cursor_visible(cx);
                    cx.notify();
                    return;
                }
            }
            view.selection_phase = SelectionPhase::None;
            if view.cursor_col > 0 {
                view.cursor_col -= 1;
                view.ensure_cursor_visible(cx);
                cx.notify();
            } else if view.cursor_line > 0 {
                // Move to end of previous line
                view.cursor_line -= 1;
                view.cursor_col = buffer.read(cx).line_len(view.cursor_line);
                view.ensure_cursor_visible(cx);
                cx.notify();
            }
        }
        "right" => {
            // If there's a selection, move cursor to end of selection
            if let Some(selection) = view.selection.take() {
                if !selection.is_empty() {
                    let (_, (end_line, end_col)) = selection.normalized();
                    view.cursor_line = end_line;
                    view.cursor_col = end_col;
                    view.selection_phase = SelectionPhase::None;
                    view.ensure_cursor_visible(cx);
                    cx.notify();
                    return;
                }
            }
            view.selection_phase = SelectionPhase::None;
            let line_len = buffer.read(cx).line_len(view.cursor_line);
            if view.cursor_col < line_len {
                view.cursor_col += 1;
                view.ensure_cursor_visible(cx);
                cx.notify();
            } else {
                // Move to start of next line
                let line_count = buffer.read(cx).line_count();
                if view.cursor_line + 1 < line_count {
                    view.cursor_line += 1;
                    view.cursor_col = 0;
                    view.ensure_cursor_visible(cx);
                    cx.notify();
                }
            }
        }
        "home" => {
            view.clear_selection();
            view.cursor_col = 0;
            view.ensure_cursor_visible(cx);
            cx.notify();
        }
        "end" => {
            view.clear_selection();
            view.cursor_col = buffer.read(cx).line_len(view.cursor_line);
            view.ensure_cursor_visible(cx);
            cx.notify();
        }
        "pageup" => {
            view.clear_selection();
            view.cursor_line = view.cursor_line.saturating_sub(20);
            view.ensure_cursor_valid(cx);
            view.ensure_cursor_visible(cx);
            cx.notify();
        }
        "pagedown" => {
            view.clear_selection();
            let line_count = buffer.read(cx).line_count();
            view.cursor_line = (view.cursor_line + 20).min(line_count.saturating_sub(1));
            view.ensure_cursor_valid(cx);
            view.ensure_cursor_visible(cx);
            cx.notify();
        }
        "backspace" => {
            // Delete selection if any, otherwise delete backward
            if !view.delete_selection(cx) {
                delete_backward(view, cx);
            }
        }
        "delete" => {
            // Delete selection if any, otherwise delete forward
            if !view.delete_selection(cx) {
                delete_forward(view, cx);
            }
        }
        "enter" => {
            insert_text(view, "\n", cx);
        }
        "tab" => {
            insert_text(view, "    ", cx); // 4 spaces for tab
        }
        _ => {
            // Handle regular character input
            if let Some(key_char) = &event.keystroke.key_char {
                if !key_char.is_empty() && !modifiers.control && !modifiers.platform {
                    insert_text(view, key_char, cx);
                }
            }
        }
    }
}

pub(crate) fn insert_text(view: &mut EditorView, text: &str, cx: &mut Context<EditorView>) {
    let Some(buffer) = view.buffer.clone() else {
        return;
    };

    // Delete selection if any (this also positions cursor at selection start)
    view.delete_selection(cx);

    let state_before = EditorState::new((view.cursor_line, view.cursor_col));
    let offset = buffer.read(cx).line_col_to_offset(view.cursor_line, view.cursor_col);
    buffer.update(cx, |buf, cx| {
        buf.insert_with_state(offset, text, state_before, cx);
    });

    // Move cursor forward
    for c in text.chars() {
        if c == '\n' {
            view.cursor_line += 1;
            view.cursor_col = 0;
        } else {
            view.cursor_col += 1;
        }
    }
    view.ensure_cursor_visible(cx);
    cx.notify();
}

fn delete_backward(view: &mut EditorView, cx: &mut Context<EditorView>) {
    let Some(buffer) = view.buffer.clone() else {
        return;
    };

    let state_before = EditorState::new((view.cursor_line, view.cursor_col));
    if view.cursor_col > 0 {
        let offset = buffer.read(cx).line_col_to_offset(view.cursor_line, view.cursor_col);
        buffer.update(cx, |buf, cx| {
            buf.delete_with_state(offset - 1, offset, state_before, cx);
        });
        view.cursor_col -= 1;
        cx.notify();
    } else if view.cursor_line > 0 {
        // Join with previous line
        let prev_line_len = buffer.read(cx).line_len(view.cursor_line - 1);
        let offset = buffer.read(cx).line_col_to_offset(view.cursor_line, 0);
        buffer.update(cx, |buf, cx| {
            buf.delete_with_state(offset - 1, offset, state_before, cx);
        });
        view.cursor_line -= 1;
        view.cursor_col = prev_line_len;
        view.ensure_cursor_visible(cx);
        cx.notify();
    }
}

fn delete_forward(view: &mut EditorView, cx: &mut Context<EditorView>) {
    let Some(buffer) = view.buffer.clone() else {
        return;
    };

    let state_before = EditorState::new((view.cursor_line, view.cursor_col));
    let line_len = buffer.read(cx).line_len(view.cursor_line);
    let line_count = buffer.read(cx).line_count();

    if view.cursor_col < line_len {
        let offset = buffer.read(cx).line_col_to_offset(view.cursor_line, view.cursor_col);
        buffer.update(cx, |buf, cx| {
            buf.delete_with_state(offset, offset + 1, state_before, cx);
        });
        cx.notify();
    } else if view.cursor_line + 1 < line_count {
        // Delete newline - join with next line
        let offset = buffer.read(cx).line_col_to_offset(view.cursor_line, view.cursor_col);
        buffer.update(cx, |buf, cx| {
            buf.delete_with_state(offset, offset + 1, state_before, cx);
        });
        cx.notify();
    }
}

pub(crate) fn move_word_left(view: &mut EditorView, cx: &mut Context<EditorView>) {
    let Some(buffer) = view.buffer.clone() else {
        return;
    };

    let buffer = buffer.read(cx);
    let mut offset = buffer.line_col_to_offset(view.cursor_line, view.cursor_col);

    if offset == 0 {
        return;
    }

    // Check if we're at start of line
    if view.cursor_col == 0 {
        // Move to end of previous line
        offset -= 1; // This moves past the newline to end of previous line
        let (line, _col) = buffer.offset_to_line_col(offset);
        view.cursor_line = line;
        view.cursor_col = buffer.line_len(line); // Position at end of line
        view.ensure_cursor_visible(cx);
        return;
    }

    // Move back one character first
    offset -= 1;

    // Skip whitespace/non-word characters going backwards, but stop at newline
    while offset > 0 {
        if let Some(ch) = buffer.char_at(offset) {
            if ch == '\n' {
                // Stop after the newline (at start of current line)
                offset += 1;
                break;
            }
            if is_word_char(ch) {
                break;
            }
            offset -= 1;
        } else {
            break;
        }
    }

    // Check if we landed on a newline (means we're at start of line)
    if let Some(ch) = buffer.char_at(offset) {
        if ch == '\n' {
            offset += 1; // Move to start of line
        }
    }

    // Now move to the start of the word (if we're in a word)
    if offset > 0 {
        if let Some(ch) = buffer.char_at(offset) {
            if is_word_char(ch) {
                while offset > 0 {
                    if let Some(prev_ch) = buffer.char_at(offset - 1) {
                        if !is_word_char(prev_ch) {
                            break;
                        }
                        offset -= 1;
                    } else {
                        break;
                    }
                }
            }
        }
    }

    let (line, col) = buffer.offset_to_line_col(offset);
    view.cursor_line = line;
    view.cursor_col = col;
    view.ensure_cursor_visible(cx);
}

pub(crate) fn move_word_right(view: &mut EditorView, cx: &mut Context<EditorView>) {
    let Some(buffer) = view.buffer.clone() else {
        return;
    };

    let buffer = buffer.read(cx);
    let total_chars = buffer.total_chars();
    let mut offset = buffer.line_col_to_offset(view.cursor_line, view.cursor_col);

    if offset >= total_chars {
        return;
    }

    // Check if we're at end of line (cursor at newline position)
    if let Some(ch) = buffer.char_at(offset) {
        if ch == '\n' {
            // Move past the newline to next line
            offset += 1;
            // Skip any whitespace at start of next line to find next word
            while offset < total_chars {
                if let Some(ch) = buffer.char_at(offset) {
                    if ch == '\n' || is_word_char(ch) {
                        break;
                    }
                    offset += 1;
                } else {
                    break;
                }
            }
            let (line, col) = buffer.offset_to_line_col(offset);
            view.cursor_line = line;
            view.cursor_col = col;
            view.ensure_cursor_visible(cx);
            return;
        }
    }

    // Skip current word characters
    while offset < total_chars {
        if let Some(ch) = buffer.char_at(offset) {
            if ch == '\n' || !is_word_char(ch) {
                break;
            }
            offset += 1;
        } else {
            break;
        }
    }

    // Skip whitespace/non-word characters, but stop at newline
    while offset < total_chars {
        if let Some(ch) = buffer.char_at(offset) {
            // Stop at newline - cursor stays at end of current line
            if ch == '\n' {
                break;
            }
            if is_word_char(ch) {
                break;
            }
            offset += 1;
        } else {
            break;
        }
    }

    let (line, col) = buffer.offset_to_line_col(offset);
    view.cursor_line = line;
    view.cursor_col = col;
    view.ensure_cursor_visible(cx);
}
