//! Editor view - main view struct and rendering

use super::diff_mode::{compute_display_lines, DiffDisplayLine, DiffLine};
use super::element::EditorElement;
use super::input::handle_key;
use super::selection::Selection;
use crate::constants::{CELL_HEIGHT, CELL_WIDTH, PADDING};
use crate::editor::{Buffer, BufferEvent, EditorState};
use crate::git::{GitStore, GitStoreEvent};
use crate::types::{EditorTabConfig, SelectionPhase, Tab, TabConfig};
use gpui::prelude::*;
use gpui::*;
use std::path::PathBuf;

const DIFF_CONTEXT_LINES: usize = 3;

/// Data for diff mode rendering
pub(crate) struct DiffModeData {
    pub(crate) all_lines: Vec<DiffLine>,
    pub(crate) display_lines: Vec<DiffDisplayLine>,
    pub(crate) expanded_sections: Vec<(usize, usize)>,
    pub(crate) max_old_line_num: usize,
    pub(crate) max_new_line_num: usize,
}

pub struct EditorView {
    pub(crate) buffer: Option<Entity<Buffer>>,
    pub(crate) file_path: PathBuf,
    pub(crate) git_store: Option<Entity<GitStore>>,
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
    pub(crate) diff_mode: Option<DiffModeData>,
    _blink_task: Option<Task<()>>,
    _buffer_subscription: Option<Subscription>,
    _git_subscription: Option<Subscription>,
}

impl EditorView {
    pub fn new(
        buffer: Entity<Buffer>,
        file_path: PathBuf,
        git_store: Entity<GitStore>,
        cx: &mut Context<Self>,
    ) -> Self {
        // Subscribe to buffer events
        let buffer_subscription = cx.subscribe(&buffer, |this, _buffer, event, cx| {
            match event {
                BufferEvent::Changed | BufferEvent::Saved | BufferEvent::ExternalChange => {
                    // Ensure cursor is still valid after buffer changes
                    this.ensure_cursor_valid(cx);
                    // Request git diff update
                    this.request_diff_update(cx);
                    cx.notify();
                }
            }
        });

        // Subscribe to git store events
        let file_path_clone = file_path.clone();
        let git_subscription = cx.subscribe(&git_store, move |this, _store, event, cx| {
            if let GitStoreEvent::DiffUpdated(path) = event {
                if path == &file_path_clone {
                    this.update_line_diffs(cx);
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

        let view = Self {
            buffer: Some(buffer),
            file_path,
            git_store: Some(git_store),
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
            diff_mode: None,
            _blink_task: Some(blink_task),
            _buffer_subscription: Some(buffer_subscription),
            _git_subscription: Some(git_subscription),
        };

        // Initial diff computation
        view.request_diff_update(cx);

        view
    }

    /// Create an editor in diff mode (read-only, shows unified diff)
    pub fn new_diff_mode(diff_lines: Vec<DiffLine>, cx: &mut Context<Self>) -> Self {
        let max_old = diff_lines
            .iter()
            .filter_map(|l| l.old_line_num)
            .max()
            .unwrap_or(0);
        let max_new = diff_lines
            .iter()
            .filter_map(|l| l.new_line_num)
            .max()
            .unwrap_or(0);

        let display_lines = compute_display_lines(&diff_lines, &[], DIFF_CONTEXT_LINES);

        Self {
            buffer: None,
            file_path: PathBuf::new(),
            git_store: None,
            cursor_line: 0,
            cursor_col: 0,
            scroll_offset: 0,
            scroll_x: 0.0,
            scroll_accumulator: 0.0,
            focus_handle: cx.focus_handle(),
            last_bounds: None,
            last_line_number_width: 0.0,
            cursor_visible: false,
            last_cursor_move: std::time::Instant::now(),
            selection: None,
            selection_phase: SelectionPhase::None,
            diff_mode: Some(DiffModeData {
                all_lines: diff_lines,
                display_lines,
                expanded_sections: Vec::new(),
                max_old_line_num: max_old,
                max_new_line_num: max_new,
            }),
            _blink_task: None,
            _buffer_subscription: None,
            _git_subscription: None,
        }
    }

    /// Expand a collapsed section in diff mode
    pub fn expand_diff_section(&mut self, start_idx: usize, end_idx: usize) {
        if let Some(ref mut diff_data) = self.diff_mode {
            diff_data.expanded_sections.push((start_idx, end_idx));
            diff_data.display_lines = compute_display_lines(
                &diff_data.all_lines,
                &diff_data.expanded_sections,
                DIFF_CONTEXT_LINES,
            );
        }
    }

    pub fn is_diff_mode(&self) -> bool {
        self.diff_mode.is_some()
    }

    pub fn diff_line_count(&self) -> usize {
        self.diff_mode
            .as_ref()
            .map(|d| d.display_lines.len())
            .unwrap_or(0)
    }

    fn request_diff_update(&self, cx: &mut Context<Self>) {
        let Some(ref buffer) = self.buffer else { return };
        let Some(ref git_store) = self.git_store else { return };
        let content = buffer.read(cx).content();
        let file_path = self.file_path.clone();
        git_store.update(cx, |store, cx| {
            store.compute_diff_for_file(&file_path, &content, cx);
        });
    }

    fn update_line_diffs(&self, cx: &mut Context<Self>) {
        let Some(ref buffer) = self.buffer else { return };
        let Some(ref git_store) = self.git_store else { return };
        let diffs = git_store
            .read(cx)
            .line_diffs_for_file(&self.file_path)
            .cloned();
        if let Some(diffs) = diffs {
            buffer.update(cx, |buffer, _cx| {
                buffer.set_line_diffs(diffs);
            });
        }
        cx.notify();
    }

    pub fn buffer(&self) -> Option<&Entity<Buffer>> {
        self.buffer.as_ref()
    }

    pub(crate) fn ensure_cursor_valid(&mut self, cx: &mut Context<Self>) {
        let Some(ref buffer) = self.buffer else { return };
        let buffer = buffer.read(cx);
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
        let (line_count, line_len) = if let Some(ref buffer) = self.buffer {
            let buf = buffer.read(cx);
            let lc = buf.line_count();
            let line = if lc > 0 { clicked_line.min(lc - 1) } else { 0 };
            (lc, if lc > 0 { buf.line_len(line) } else { 0 })
        } else if let Some(ref diff_data) = self.diff_mode {
            let lc = diff_data.display_lines.len();
            (lc, 0) // No column clamping needed for diff mode
        } else {
            (0, 0)
        };

        let line = if line_count > 0 {
            clicked_line.min(line_count - 1)
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

        let buffer = self.buffer.as_ref()?;
        let ((start_line, start_col), (end_line, end_col)) = selection.normalized();
        let buffer = buffer.read(cx);

        let start_offset = buffer.line_col_to_offset(start_line, start_col);
        let end_offset = buffer.line_col_to_offset(end_line, end_col);

        buffer.slice(start_offset, end_offset)
    }

    /// Delete selected text and return whether anything was deleted
    pub(crate) fn delete_selection(&mut self, cx: &mut Context<Self>) -> bool {
        // Diff mode is read-only
        if self.diff_mode.is_some() {
            return false;
        }

        let Some(ref buffer) = self.buffer else { return false };

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
        let start_offset = buffer.read(cx).line_col_to_offset(start_line, start_col);
        let end_offset = buffer.read(cx).line_col_to_offset(end_line, end_col);

        buffer.update(cx, |buf, cx| {
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
        let is_diff_mode = self.diff_mode.is_some();

        div()
            .id("editor-wrapper")
            .key_context("Editor")
            .track_focus(&focus_handle)
            .when(!is_diff_mode, |el| {
                el.on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                    handle_key(this, event, cx);
                }))
            })
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
                let (line_count, max_line_len) = if let Some(ref diff_data) = this.diff_mode {
                    // In diff mode, use display_lines count
                    let max_len = diff_data.display_lines.iter().map(|dl| {
                        match dl {
                            DiffDisplayLine::Line(line) => line.content.len(),
                            DiffDisplayLine::Collapsed { count, .. } => format!("··· {} lines ···", count).len(),
                        }
                    }).max().unwrap_or(0);
                    (diff_data.display_lines.len(), max_len)
                } else if let Some(ref buffer) = this.buffer {
                    let buffer = buffer.read(cx);
                    (buffer.line_count(), buffer.max_line_len())
                } else {
                    (0, 0)
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
                let (line, _col) = this.pixel_to_line_col(event.position, bounds, cx);

                // In diff mode, check if clicking on a collapsed section
                if let Some(ref diff_data) = this.diff_mode {
                    if let Some(display_line) = diff_data.display_lines.get(line) {
                        if let DiffDisplayLine::Collapsed { start_idx, end_idx, .. } = display_line {
                            // Expand this section
                            this.expand_diff_section(*start_idx, *end_idx);
                            cx.notify();
                            return;
                        }
                    }
                    // Don't allow selection in diff mode
                    return;
                }

                let (_line, col) = this.pixel_to_line_col(event.position, bounds, cx);

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
            .when(!is_diff_mode, |el| {
                el.on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
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
            })
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
        if self.diff_mode.is_some() {
            return "Diff".to_string();
        }
        let Some(ref buffer) = self.buffer else {
            return "Untitled".to_string();
        };
        let buffer = buffer.read(cx);
        let name = buffer.file_name();
        if buffer.is_dirty() {
            format!("{}*", name)
        } else {
            name
        }
    }

    fn to_config(&self, cx: &App) -> TabConfig {
        let Some(ref buffer) = self.buffer else {
            return TabConfig::Editor(EditorTabConfig { path: PathBuf::new() });
        };
        let path = buffer.read(cx).file_path().clone();
        TabConfig::Editor(EditorTabConfig { path })
    }
}
