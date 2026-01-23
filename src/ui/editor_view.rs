use crate::editor::{Buffer, BufferEvent};
use gpui::prelude::*;
use gpui::*;
use std::path::PathBuf;

// Editor dimensions (same as terminal for consistency)
const CELL_WIDTH: f32 = 7.8;
const CELL_HEIGHT: f32 = 18.0;
const PADDING: f32 = 8.0;

// Colors (Catppuccin Mocha theme)
fn default_fg() -> Hsla {
    Rgba {
        r: 0xcd as f32 / 255.0,
        g: 0xd6 as f32 / 255.0,
        b: 0xf4 as f32 / 255.0,
        a: 1.0,
    }
    .into()
}

fn default_bg() -> Hsla {
    Rgba {
        r: 0x1e as f32 / 255.0,
        g: 0x1e as f32 / 255.0,
        b: 0x2e as f32 / 255.0,
        a: 1.0,
    }
    .into()
}

fn cursor_color() -> Hsla {
    Rgba {
        r: 0xcd as f32 / 255.0,
        g: 0xd6 as f32 / 255.0,
        b: 0xf4 as f32 / 255.0,
        a: 1.0,
    }
    .into()
}

fn cursor_unfocused_color() -> Hsla {
    Rgba {
        r: 0x6c as f32 / 255.0,
        g: 0x70 as f32 / 255.0,
        b: 0x86 as f32 / 255.0,
        a: 1.0,
    }
    .into()
}

fn line_number_color() -> Hsla {
    Rgba {
        r: 0x6c as f32 / 255.0,
        g: 0x70 as f32 / 255.0,
        b: 0x86 as f32 / 255.0,
        a: 1.0,
    }
    .into()
}

pub struct EditorView {
    buffer: Entity<Buffer>,
    file_path: PathBuf,
    cursor_line: usize,
    cursor_col: usize,
    scroll_offset: usize,
    scroll_x: f32,
    focus_handle: FocusHandle,
    last_bounds: Option<Bounds<Pixels>>,
    last_line_number_width: f32,
    cursor_visible: bool,
    last_cursor_move: std::time::Instant,
    _blink_task: Task<()>,
    _subscription: Subscription,
}

impl EditorView {
    pub fn new(buffer: Entity<Buffer>, file_path: PathBuf, cx: &mut Context<Self>) -> Self {
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
            file_path,
            cursor_line: 0,
            cursor_col: 0,
            scroll_offset: 0,
            scroll_x: 0.0,
            focus_handle: cx.focus_handle(),
            last_bounds: None,
            last_line_number_width: 0.0,
            cursor_visible: true,
            last_cursor_move: std::time::Instant::now(),
            _blink_task: blink_task,
            _subscription: subscription,
        }
    }

    pub fn buffer(&self) -> &Entity<Buffer> {
        &self.buffer
    }

    pub fn file_path(&self) -> &PathBuf {
        &self.file_path
    }

    fn ensure_cursor_valid(&mut self, cx: &mut Context<Self>) {
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
    fn reset_cursor_blink(&mut self) {
        self.cursor_visible = true;
        self.last_cursor_move = std::time::Instant::now();
    }

    fn handle_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        // Reset cursor blink on any key press
        self.reset_cursor_blink();

        let key = &event.keystroke.key;
        let modifiers = &event.keystroke.modifiers;

        // Handle Ctrl/Cmd+S for save
        if modifiers.platform && key == "s" {
            self.buffer.update(cx, |buf, cx| {
                let _ = buf.save(cx);
            });
            return;
        }

        // Handle navigation keys
        match key.as_str() {
            "up" => {
                if self.cursor_line > 0 {
                    self.cursor_line -= 1;
                    self.ensure_cursor_valid(cx);
                    self.ensure_cursor_visible(cx);
                    cx.notify();
                }
            }
            "down" => {
                let line_count = self.buffer.read(cx).line_count();
                if self.cursor_line + 1 < line_count {
                    self.cursor_line += 1;
                    self.ensure_cursor_valid(cx);
                    self.ensure_cursor_visible(cx);
                    cx.notify();
                }
            }
            "left" => {
                if self.cursor_col > 0 {
                    self.cursor_col -= 1;
                    self.ensure_cursor_visible(cx);
                    cx.notify();
                } else if self.cursor_line > 0 {
                    // Move to end of previous line
                    self.cursor_line -= 1;
                    self.cursor_col = self.buffer.read(cx).line_len(self.cursor_line);
                    self.ensure_cursor_visible(cx);
                    cx.notify();
                }
            }
            "right" => {
                let line_len = self.buffer.read(cx).line_len(self.cursor_line);
                if self.cursor_col < line_len {
                    self.cursor_col += 1;
                    self.ensure_cursor_visible(cx);
                    cx.notify();
                } else {
                    // Move to start of next line
                    let line_count = self.buffer.read(cx).line_count();
                    if self.cursor_line + 1 < line_count {
                        self.cursor_line += 1;
                        self.cursor_col = 0;
                        self.ensure_cursor_visible(cx);
                        cx.notify();
                    }
                }
            }
            "home" => {
                self.cursor_col = 0;
                self.ensure_cursor_visible(cx);
                cx.notify();
            }
            "end" => {
                self.cursor_col = self.buffer.read(cx).line_len(self.cursor_line);
                self.ensure_cursor_visible(cx);
                cx.notify();
            }
            "pageup" => {
                self.cursor_line = self.cursor_line.saturating_sub(20);
                self.ensure_cursor_valid(cx);
                self.ensure_cursor_visible(cx);
                cx.notify();
            }
            "pagedown" => {
                let line_count = self.buffer.read(cx).line_count();
                self.cursor_line = (self.cursor_line + 20).min(line_count.saturating_sub(1));
                self.ensure_cursor_valid(cx);
                self.ensure_cursor_visible(cx);
                cx.notify();
            }
            "backspace" => {
                self.delete_backward(cx);
            }
            "delete" => {
                self.delete_forward(cx);
            }
            "enter" => {
                self.insert_text("\n", cx);
            }
            "tab" => {
                self.insert_text("    ", cx); // 4 spaces for tab
            }
            _ => {
                // Handle regular character input
                if let Some(key_char) = &event.keystroke.key_char {
                    if !key_char.is_empty() && !modifiers.control && !modifiers.platform {
                        self.insert_text(key_char, cx);
                    }
                }
            }
        }
    }

    fn insert_text(&mut self, text: &str, cx: &mut Context<Self>) {
        let offset = self.buffer.read(cx).line_col_to_offset(self.cursor_line, self.cursor_col);
        self.buffer.update(cx, |buf, cx| {
            buf.insert(offset, text, cx);
        });

        // Move cursor forward
        for c in text.chars() {
            if c == '\n' {
                self.cursor_line += 1;
                self.cursor_col = 0;
            } else {
                self.cursor_col += 1;
            }
        }
        self.ensure_cursor_visible(cx);
        cx.notify();
    }

    fn delete_backward(&mut self, cx: &mut Context<Self>) {
        if self.cursor_col > 0 {
            let offset = self.buffer.read(cx).line_col_to_offset(self.cursor_line, self.cursor_col);
            self.buffer.update(cx, |buf, cx| {
                buf.delete(offset - 1, offset, cx);
            });
            self.cursor_col -= 1;
            cx.notify();
        } else if self.cursor_line > 0 {
            // Join with previous line
            let prev_line_len = self.buffer.read(cx).line_len(self.cursor_line - 1);
            let offset = self.buffer.read(cx).line_col_to_offset(self.cursor_line, 0);
            self.buffer.update(cx, |buf, cx| {
                buf.delete(offset - 1, offset, cx);
            });
            self.cursor_line -= 1;
            self.cursor_col = prev_line_len;
            self.ensure_cursor_visible(cx);
            cx.notify();
        }
    }

    fn delete_forward(&mut self, cx: &mut Context<Self>) {
        let line_len = self.buffer.read(cx).line_len(self.cursor_line);
        let line_count = self.buffer.read(cx).line_count();

        if self.cursor_col < line_len {
            let offset = self.buffer.read(cx).line_col_to_offset(self.cursor_line, self.cursor_col);
            self.buffer.update(cx, |buf, cx| {
                buf.delete(offset, offset + 1, cx);
            });
            cx.notify();
        } else if self.cursor_line + 1 < line_count {
            // Delete newline - join with next line
            let offset = self.buffer.read(cx).line_col_to_offset(self.cursor_line, self.cursor_col);
            self.buffer.update(cx, |buf, cx| {
                buf.delete(offset, offset + 1, cx);
            });
            cx.notify();
        }
    }

    fn ensure_cursor_visible(&mut self, _cx: &mut Context<Self>) {
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
                this.handle_key(event, cx);
            }))
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                // Handle both vertical and horizontal scrolling
                let (delta_x, delta_y) = match event.delta {
                    ScrollDelta::Lines(lines) => (
                        f32::from(lines.x) * CELL_WIDTH * 3.0,
                        f32::from(lines.y),
                    ),
                    ScrollDelta::Pixels(pixels) => (
                        f32::from(pixels.x),
                        f32::from(pixels.y) / CELL_HEIGHT,
                    ),
                };

                // Shift+scroll converts vertical to horizontal
                let (h_delta, v_delta) = if event.modifiers.shift {
                    (delta_y * CELL_WIDTH * 3.0, 0.0)
                } else {
                    (delta_x, delta_y)
                };

                // Get content bounds for clamping
                let buffer = this.buffer.read(cx);
                let line_count = buffer.line_count();
                let max_line_len = buffer.max_line_len();
                drop(buffer);

                // Calculate visible area from stored bounds
                let (visible_lines, visible_width) = if let Some(bounds) = this.last_bounds {
                    let available_height = f32::from(bounds.size.height) - (PADDING * 2.0);
                    let visible_lines = (available_height / CELL_HEIGHT).floor() as usize;
                    let visible_width = f32::from(bounds.size.width) - PADDING - this.last_line_number_width;
                    (visible_lines, visible_width)
                } else {
                    (30, 80.0 * CELL_WIDTH) // Fallback defaults
                };

                // Horizontal scrolling
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

                // Vertical scrolling
                if v_delta.abs() > 0.1 {
                    let lines = v_delta as i32;
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

                // Use stored bounds to convert window coordinates to element-local coordinates
                if let Some(bounds) = this.last_bounds {
                    let line_number_width = this.last_line_number_width;

                    // Convert window position to element-local position
                    let local_x = f32::from(event.position.x) - f32::from(bounds.origin.x);
                    let local_y = f32::from(event.position.y) - f32::from(bounds.origin.y);

                    // Calculate clicked line (accounting for vertical scroll)
                    let click_y = local_y - PADDING;
                    let clicked_line = if click_y >= 0.0 {
                        this.scroll_offset + (click_y / CELL_HEIGHT) as usize
                    } else {
                        this.scroll_offset
                    };

                    // Calculate clicked column (accounting for horizontal scroll and gutter)
                    let text_area_x = PADDING + line_number_width;
                    let click_x = local_x - text_area_x + this.scroll_x;
                    let clicked_col = if click_x >= 0.0 {
                        (click_x / CELL_WIDTH).round() as usize
                    } else {
                        0
                    };

                    // Clamp to valid positions
                    let line_count = this.buffer.read(cx).line_count();
                    if line_count > 0 {
                        this.cursor_line = clicked_line.min(line_count - 1);
                        let line_len = this.buffer.read(cx).line_len(this.cursor_line);
                        this.cursor_col = clicked_col.min(line_len);
                    }
                }

                cx.notify();
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
            })
    }
}

impl Focusable for EditorView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

// Custom Element for efficient editor rendering
struct EditorElement {
    view: Entity<EditorView>,
    buffer: Entity<Buffer>,
    cursor_line: usize,
    cursor_col: usize,
    scroll_offset: usize,
    scroll_x: f32,
    is_focused: bool,
    cursor_visible: bool,
}

struct EditorLayoutState {
    visible_lines: Vec<(usize, String)>, // (line_number, content)
    cursor_position: Option<gpui::Point<Pixels>>,
    line_number_width: f32,
    scroll_x: f32,
}

impl IntoElement for EditorElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for EditorElement {
    type RequestLayoutState = ();
    type PrepaintState = EditorLayoutState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        let layout_id = window.request_layout(style, None, cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        // Calculate visible area
        let available_height = f32::from(bounds.size.height) - (PADDING * 2.0);
        let visible_line_count = (available_height / CELL_HEIGHT).floor() as usize;

        // Collect data from buffer (scoped to release borrow)
        let (line_count, visible_lines) = {
            let buffer = self.buffer.read(cx);
            let line_count = buffer.line_count();

            let mut visible_lines = Vec::new();
            for i in 0..visible_line_count {
                let line_idx = self.scroll_offset + i;
                if line_idx < line_count {
                    if let Some(line) = buffer.line(line_idx) {
                        // Remove trailing newline for display
                        let line = line.trim_end_matches('\n').to_string();
                        visible_lines.push((line_idx + 1, line)); // 1-indexed line numbers
                    }
                }
            }
            (line_count, visible_lines)
        };

        // Calculate line number width (4 characters minimum)
        let line_number_chars = format!("{}", line_count).len().max(4);
        let line_number_width = (line_number_chars as f32 + 1.0) * CELL_WIDTH;

        // Store bounds for click handling
        self.view.update(cx, |view, _| {
            view.last_bounds = Some(bounds);
            view.last_line_number_width = line_number_width;
        });

        // Calculate cursor position (accounting for horizontal scroll)
        let cursor_position = if self.cursor_line >= self.scroll_offset
            && self.cursor_line < self.scroll_offset + visible_line_count
        {
            let screen_line = self.cursor_line - self.scroll_offset;
            let cursor_x = PADDING + line_number_width + (self.cursor_col as f32 * CELL_WIDTH) - self.scroll_x;
            // Only show cursor if it's in the visible horizontal area
            if cursor_x >= PADDING + line_number_width - CELL_WIDTH && cursor_x < f32::from(bounds.size.width) {
                Some(gpui::point(
                    px(cursor_x),
                    px(PADDING + (screen_line as f32 * CELL_HEIGHT)),
                ))
            } else {
                None
            }
        } else {
            None
        };

        EditorLayoutState {
            visible_lines,
            cursor_position,
            line_number_width,
            scroll_x: self.scroll_x,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        layout: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Paint background
        window.paint_quad(fill(bounds, default_bg()));

        let origin = bounds.origin;

        // Paint line numbers background
        let line_numbers_bounds = Bounds {
            origin: origin + gpui::point(Pixels::ZERO, px(PADDING)),
            size: Size {
                width: px(PADDING + layout.line_number_width),
                height: bounds.size.height - px(PADDING * 2.0),
            },
        };
        let line_number_bg: Hsla = Rgba {
            r: 0x18 as f32 / 255.0,
            g: 0x18 as f32 / 255.0,
            b: 0x25 as f32 / 255.0,
            a: 1.0,
        }
        .into();
        window.paint_quad(fill(line_numbers_bounds, line_number_bg));

        let font = Font {
            family: "Paper Mono".into(),
            features: FontFeatures::default(),
            fallbacks: None,
            weight: FontWeight::NORMAL,
            style: FontStyle::Normal,
        };

        let font_size = px(13.0);

        // Paint line numbers first (outside clip region)
        for (idx, (line_num, _content)) in layout.visible_lines.iter().enumerate() {
            let y = origin.y + px(PADDING + (idx as f32 * CELL_HEIGHT));

            // Paint line number
            let line_num_str = format!("{:>4}", line_num);
            let line_num_run = TextRun {
                len: line_num_str.len(),
                font: font.clone(),
                color: line_number_color(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let shaped_num = window.text_system().shape_line(
                line_num_str.into(),
                font_size,
                &[line_num_run],
                None,
            );
            let _ = shaped_num.paint(
                gpui::point(origin.x + px(PADDING), y),
                px(CELL_HEIGHT),
                window,
                cx,
            );
        }

        // Define clip region for text content (after line number gutter)
        let text_area_bounds = Bounds {
            origin: origin + gpui::point(px(PADDING + layout.line_number_width), Pixels::ZERO),
            size: Size {
                width: bounds.size.width - px(PADDING + layout.line_number_width),
                height: bounds.size.height,
            },
        };

        // Paint line content and cursor with clipping
        let cursor_pos = layout.cursor_position;
        let is_focused = self.is_focused;
        let cursor_visible = self.cursor_visible;
        window.with_content_mask(Some(ContentMask { bounds: text_area_bounds }), |window| {
            for (idx, (_line_num, content)) in layout.visible_lines.iter().enumerate() {
                let y = origin.y + px(PADDING + (idx as f32 * CELL_HEIGHT));

                if !content.is_empty() {
                    let text_run = TextRun {
                        len: content.len(),
                        font: font.clone(),
                        color: default_fg(),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    let shaped = window.text_system().shape_line(
                        content.clone().into(),
                        font_size,
                        &[text_run],
                        None,
                    );
                    let text_x = PADDING + layout.line_number_width - layout.scroll_x;
                    let _ = shaped.paint(
                        gpui::point(origin.x + px(text_x), y),
                        px(CELL_HEIGHT),
                        window,
                        cx,
                    );
                }
            }

            // Paint cursor (inside clip region) - blink when focused
            let should_show_cursor = if is_focused { cursor_visible } else { true };
            if let Some(pos) = cursor_pos {
                if should_show_cursor {
                    let cursor_bounds = Bounds {
                        origin: origin + pos,
                        size: Size {
                            width: px(2.0), // Bar cursor
                            height: px(CELL_HEIGHT),
                        },
                    };

                    let color = if is_focused {
                        cursor_color()
                    } else {
                        cursor_unfocused_color()
                    };

                    window.paint_quad(fill(cursor_bounds, color));
                }
            }
        });
    }
}
