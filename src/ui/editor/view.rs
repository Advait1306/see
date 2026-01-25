//! Editor view - main view struct and rendering

use super::element::EditorElement;
use super::input::handle_key;
use super::selection::Selection;
use crate::constants::{CELL_HEIGHT, CELL_WIDTH, PADDING};
use crate::editor::{Buffer, BufferEvent, EditorState};
use crate::types::{EditorTabConfig, SelectionPhase, Tab, TabConfig};
use gpui::prelude::*;
use gpui::*;
use std::path::PathBuf;

pub struct EditorView {
    pub(crate) buffer: Entity<Buffer>,
    pub(crate) cursor_line: usize,
    pub(crate) cursor_col: usize,
    pub(crate) scroll_offset: usize,
    pub(crate) scroll_x: f32,
    pub(crate) scroll_accumulator: f32, // For smooth partial scrolling
    pub(crate) focus_handle: FocusHandle,
    pub(crate) last_bounds: Option<Bounds<Pixels>>,
    pub(crate) last_line_number_width: f32,
    pub(crate) cursor_visible: bool,
    pub(crate) last_cursor_move: std::time::Instant,
    pub(crate) selection: Option<Selection>,
    pub(crate) selection_phase: SelectionPhase,
    _blink_task: Task<()>,
    _subscription: Subscription,
}

impl EditorView {
    pub fn new(buffer: Entity<Buffer>, _file_path: PathBuf, cx: &mut Context<Self>) -> Self {
        // Subscribe to buffer events
        let subscription = cx.subscribe(&buffer, |this, _buffer, event, cx| {
            match event {
                BufferEvent::Changed | BufferEvent::Saved | BufferEvent::ExternalChange => {
                    // Ensure cursor is still valid after buffer changes
                    this.ensure_cursor_valid(cx);
                    cx.notify();
                }
            }
        });

        // Start cursor blink timer
        let blink_task = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(530))
                    .await;

                let result = cx.update(|cx| {
                    this.update(cx, |this, cx| {
                        // Only blink if 0.5 seconds has passed since last cursor movement
                        let elapsed = this.last_cursor_move.elapsed();
                        if elapsed >= std::time::Duration::from_millis(500) {
                            this.cursor_visible = !this.cursor_visible;
                            cx.notify();
                        }
                    })
                });

                if result.is_err() {
                    break; // Entity was dropped, stop blinking
                }
            }
        });

        Self {
            buffer,
            cursor_line: 0,
            cursor_col: 0,
            scroll_offset: 0,
            scroll_x: 0.0,
            scroll_accumulator: 0.0,
            focus_handle: cx.focus_handle(),
            last_bounds: None,
            last_line_number_width: 0.0,
            cursor_visible: true,
            last_cursor_move: std::time::Instant::now(),
            selection: None,
            selection_phase: SelectionPhase::None,
            _blink_task: blink_task,
            _subscription: subscription,
        }
    }

    pub fn buffer(&self) -> &Entity<Buffer> {
        &self.buffer
    }

    pub(crate) fn ensure_cursor_valid(&mut self, cx: &mut Context<Self>) {
        let buffer = self.buffer.read(cx);
        let line_count = buffer.line_count();

        if line_count == 0 {
            self.cursor_line = 0;
            self.cursor_col = 0;
            return;
        }

        // Ensure cursor line is valid
        if self.cursor_line >= line_count {
            self.cursor_line = line_count - 1;
        }

        // Ensure cursor col is valid for the line
        let line_len = buffer.line_len(self.cursor_line);
        if self.cursor_col > line_len {
            self.cursor_col = line_len;
        }
    }

    /// Reset cursor to visible and restart blink delay (called on user interaction)
    pub(crate) fn reset_cursor_blink(&mut self) {
        self.cursor_visible = true;
        self.last_cursor_move = std::time::Instant::now();
    }

    /// Convert pixel position to (line, col) in editor coordinates
    pub(crate) fn pixel_to_line_col(
        &self,
        position: gpui::Point<Pixels>,
        bounds: Bounds<Pixels>,
        cx: &App,
    ) -> (usize, usize) {
        let line_number_width = self.last_line_number_width;

        // Convert window position to element-local position
        let local_x = f32::from(position.x) - f32::from(bounds.origin.x);
        let local_y = f32::from(position.y) - f32::from(bounds.origin.y);

        // Calculate clicked line (accounting for vertical scroll)
        let click_y = local_y - PADDING;
        let clicked_line = if click_y >= 0.0 {
            self.scroll_offset + (click_y / CELL_HEIGHT) as usize
        } else {
            self.scroll_offset
        };

        // Calculate clicked column (accounting for horizontal scroll and gutter)
        let text_area_x = PADDING + line_number_width;
        let click_x = local_x - text_area_x + self.scroll_x;
        let clicked_col = if click_x >= 0.0 {
            (click_x / CELL_WIDTH).round() as usize
        } else {
            0
        };

        // Clamp to valid positions
        let buffer = self.buffer.read(cx);
        let line_count = buffer.line_count();
        let line = if line_count > 0 {
            clicked_line.min(line_count - 1)
        } else {
            0
        };
        let line_len = if line_count > 0 {
            buffer.line_len(line)
        } else {
            0
        };
        let col = clicked_col.min(line_len);

        (line, col)
    }

    /// Clear selection
    pub(crate) fn clear_selection(&mut self) {
        self.selection = None;
        self.selection_phase = SelectionPhase::None;
    }

    /// Get selected text from buffer
    #[allow(dead_code)]
    pub(crate) fn selection_to_string(&self, cx: &App) -> Option<String> {
        let selection = self.selection.as_ref()?;
        if selection.is_empty() {
            return None;
        }

        let ((start_line, start_col), (end_line, end_col)) = selection.normalized();
        let buffer = self.buffer.read(cx);

        let start_offset = buffer.line_col_to_offset(start_line, start_col);
        let end_offset = buffer.line_col_to_offset(end_line, end_col);

        buffer.slice(start_offset, end_offset)
    }

    /// Delete selected text and return whether anything was deleted
    pub(crate) fn delete_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let selection = match self.selection.take() {
            Some(s) if !s.is_empty() => s,
            _ => return false,
        };

        // Store editor state before deletion for undo (including selection)
        let state_before = EditorState::with_selection(
            (self.cursor_line, self.cursor_col),
            (selection.anchor_line, selection.anchor_col, selection.end_line, selection.end_col),
        );

        let ((start_line, start_col), (end_line, end_col)) = selection.normalized();
        let start_offset = self.buffer.read(cx).line_col_to_offset(start_line, start_col);
        let end_offset = self.buffer.read(cx).line_col_to_offset(end_line, end_col);

        self.buffer.update(cx, |buf, cx| {
            buf.delete_with_state(start_offset, end_offset, state_before, cx);
        });

        // Move cursor to start of selection
        self.cursor_line = start_line;
        self.cursor_col = start_col;
        self.selection_phase = SelectionPhase::None;

        true
    }

    pub(crate) fn ensure_cursor_visible(&mut self, _cx: &mut Context<Self>) {
        // Adjust scroll offset to keep cursor visible
        // Assuming ~30 visible lines and ~80 visible columns (will be calculated properly in paint)
        let visible_lines = 30usize;
        let visible_cols = 80.0f32;

        // Vertical scrolling
        if self.cursor_line < self.scroll_offset {
            self.scroll_offset = self.cursor_line;
        } else if self.cursor_line >= self.scroll_offset + visible_lines {
            self.scroll_offset = self.cursor_line - visible_lines + 1;
        }

        // Horizontal scrolling - cursor position in pixels relative to text start
        let cursor_x = self.cursor_col as f32 * CELL_WIDTH;
        let visible_width = visible_cols * CELL_WIDTH;

        if cursor_x < self.scroll_x {
            // Cursor is left of visible area
            self.scroll_x = cursor_x;
        } else if cursor_x >= self.scroll_x + visible_width - CELL_WIDTH {
            // Cursor is right of visible area (leave room for cursor itself)
            self.scroll_x = cursor_x - visible_width + CELL_WIDTH * 2.0;
        }

        // Don't scroll past the start
        if self.scroll_x < 0.0 {
            self.scroll_x = 0.0;
        }
    }
}

impl Render for EditorView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus_handle = self.focus_handle.clone();
        let is_focused = focus_handle.is_focused(window);
        let buffer = self.buffer.clone();

        div()
            .id("editor-wrapper")
            .key_context("Editor")
            .track_focus(&focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                handle_key(this, event, cx);
            }))
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                // Handle both vertical and horizontal scrolling with smooth accumulation
                let (h_pixel_delta, v_pixel_delta) = match event.delta {
                    ScrollDelta::Lines(lines) => (
                        f32::from(lines.x) * CELL_WIDTH * 3.0,
                        f32::from(lines.y) * CELL_HEIGHT,
                    ),
                    ScrollDelta::Pixels(pixels) => (
                        f32::from(pixels.x),
                        f32::from(pixels.y),
                    ),
                };

                // Shift+scroll converts vertical to horizontal
                let (h_delta, v_delta) = if event.modifiers.shift {
                    (v_pixel_delta, 0.0)
                } else {
                    (h_pixel_delta, v_pixel_delta)
                };

                // Get content bounds for clamping
                let (line_count, max_line_len) = {
                    let buffer = this.buffer.read(cx);
                    (buffer.line_count(), buffer.max_line_len())
                };

                // Calculate visible area from stored bounds
                let (visible_lines, visible_width) = if let Some(bounds) = this.last_bounds {
                    let available_height = f32::from(bounds.size.height) - (PADDING * 2.0);
                    let visible_lines = (available_height / CELL_HEIGHT).floor() as usize;
                    let visible_width = f32::from(bounds.size.width) - PADDING - this.last_line_number_width;
                    (visible_lines, visible_width)
                } else {
                    (30, 80.0 * CELL_WIDTH) // Fallback defaults
                };

                // Horizontal scrolling (pixel-based, smooth)
                if h_delta.abs() > 0.1 {
                    this.scroll_x -= h_delta;
                    if this.scroll_x < 0.0 {
                        this.scroll_x = 0.0;
                    }
                    // Limit so last char reaches right edge (not left edge)
                    let content_width = max_line_len as f32 * CELL_WIDTH;
                    let max_scroll_x = (content_width - visible_width).max(0.0);
                    if this.scroll_x > max_scroll_x {
                        this.scroll_x = max_scroll_x;
                    }
                }

                // Vertical scrolling with accumulator (like terminal)
                this.scroll_accumulator += v_delta;
                let lines = (this.scroll_accumulator / CELL_HEIGHT) as i32;
                if lines != 0 {
                    this.scroll_accumulator -= lines as f32 * CELL_HEIGHT;

                    if lines < 0 {
                        this.scroll_offset = this.scroll_offset.saturating_add((-lines) as usize);
                    } else {
                        this.scroll_offset = this.scroll_offset.saturating_sub(lines as usize);
                    }
                    // Limit so last line reaches bottom (not top)
                    let max_scroll_offset = line_count.saturating_sub(visible_lines);
                    if this.scroll_offset > max_scroll_offset {
                        this.scroll_offset = max_scroll_offset;
                    }
                }

                cx.notify();
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, event: &MouseDownEvent, window, cx| {
                cx.focus_self(window);
                this.reset_cursor_blink();

                let Some(bounds) = this.last_bounds else { return };
                let (line, col) = this.pixel_to_line_col(event.position, bounds, cx);

                // Shift+click extends selection
                if event.modifiers.shift {
                    if let Some(ref mut selection) = this.selection {
                        selection.update(line, col);
                        this.selection_phase = SelectionPhase::Selecting;
                        this.cursor_line = line;
                        this.cursor_col = col;
                        cx.notify();
                        return;
                    }
                }

                // Start new selection
                this.selection = Some(Selection::new(line, col));
                this.selection_phase = SelectionPhase::Selecting;
                this.cursor_line = line;
                this.cursor_col = col;

                cx.notify();
            }))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                // Only update selection while dragging with left mouse button
                if this.selection_phase != SelectionPhase::Selecting {
                    return;
                }
                if event.pressed_button != Some(MouseButton::Left) {
                    return;
                }

                let Some(bounds) = this.last_bounds else { return };
                let (line, col) = this.pixel_to_line_col(event.position, bounds, cx);

                if let Some(ref mut selection) = this.selection {
                    selection.update(line, col);
                }
                this.cursor_line = line;
                this.cursor_col = col;

                cx.notify();
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _event: &MouseUpEvent, _, cx| {
                if this.selection_phase == SelectionPhase::Selecting {
                    // If selection is empty, clear it
                    if let Some(ref selection) = this.selection {
                        if selection.is_empty() {
                            this.selection = None;
                        }
                    }
                    this.selection_phase = SelectionPhase::Ended;
                    cx.notify();
                }
            }))
            .size_full()
            .child(EditorElement {
                view: cx.entity().clone(),
                buffer,
                cursor_line: self.cursor_line,
                cursor_col: self.cursor_col,
                scroll_offset: self.scroll_offset,
                scroll_x: self.scroll_x,
                is_focused,
                cursor_visible: self.cursor_visible,
                selection: self.selection,
            })
    }
}

impl Focusable for EditorView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Tab for EditorView {
    fn label(&self, cx: &App) -> String {
        let buffer = self.buffer.read(cx);
        let name = buffer.file_name();
        if buffer.is_dirty() {
            format!("{}*", name)
        } else {
            name
        }
    }

    fn to_config(&self, cx: &App) -> TabConfig {
        let path = self.buffer.read(cx).file_path().clone();
        TabConfig::Editor(EditorTabConfig { path })
    }
}
