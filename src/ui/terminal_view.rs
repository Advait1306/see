use crate::constants::{ANSI_COLORS, CELL_HEIGHT, CELL_WIDTH, PADDING, rgb_to_hsla};
use crate::terminal::Terminal;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point as AlacPoint, Side};
use alacritty_terminal::selection::SelectionType;
use alacritty_terminal::term::cell::Flags as CellFlags;
use alacritty_terminal::term::TermMode;
use alacritty_terminal::vte::ansi::{Color as AnsiColor, CursorShape as AlacCursorShape, NamedColor};
use gpui::prelude::*;
use gpui::*;
use gpui_component::theme::ActiveTheme;
use std::sync::Arc;
use std::time::Duration;

fn ansi_to_hsla(color: AnsiColor, default_fg: Hsla, default_bg: Hsla) -> Option<Hsla> {
    match color {
        AnsiColor::Named(named) => {
            let idx = match named {
                NamedColor::Black => 0,
                NamedColor::Red => 1,
                NamedColor::Green => 2,
                NamedColor::Yellow => 3,
                NamedColor::Blue => 4,
                NamedColor::Magenta => 5,
                NamedColor::Cyan => 6,
                NamedColor::White => 7,
                NamedColor::BrightBlack => 8,
                NamedColor::BrightRed => 9,
                NamedColor::BrightGreen => 10,
                NamedColor::BrightYellow => 11,
                NamedColor::BrightBlue => 12,
                NamedColor::BrightMagenta => 13,
                NamedColor::BrightCyan => 14,
                NamedColor::BrightWhite => 15,
                NamedColor::Foreground => return Some(default_fg),
                NamedColor::Background => return Some(default_bg),
                _ => return None,
            };
            let (r, g, b) = ANSI_COLORS[idx];
            Some(rgb_to_hsla(r, g, b))
        }
        AnsiColor::Spec(c) => Some(rgb_to_hsla(c.r, c.g, c.b)),
        AnsiColor::Indexed(idx) => {
            if idx < 16 {
                let (r, g, b) = ANSI_COLORS[idx as usize];
                Some(rgb_to_hsla(r, g, b))
            } else if idx < 232 {
                let idx = idx - 16;
                let r = if idx / 36 > 0 { (idx / 36) * 40 + 55 } else { 0 };
                let g = if (idx / 6) % 6 > 0 { ((idx / 6) % 6) * 40 + 55 } else { 0 };
                let b = if idx % 6 > 0 { (idx % 6) * 40 + 55 } else { 0 };
                Some(rgb_to_hsla(r, g, b))
            } else {
                let gray = (idx - 232) * 10 + 8;
                Some(rgb_to_hsla(gray, gray, gray))
            }
        }
    }
}

/// Helper struct for converting between Alacritty's cursor points and screen coordinates
struct DisplayCursor {
    line: i32,
    col: usize,
}

impl DisplayCursor {
    fn from(cursor_point: AlacPoint, display_offset: usize) -> Self {
        Self {
            line: cursor_point.line.0 + display_offset as i32,
            col: cursor_point.column.0,
        }
    }

    fn line(&self) -> i32 {
        self.line
    }

    fn col(&self) -> usize {
        self.col
    }
}

/// Cursor shape for rendering
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CursorShape {
    Block,
    Underline,
    Bar,
    Hollow,
}

/// Layout information for cursor rendering
struct CursorLayout {
    origin: gpui::Point<Pixels>,
    block_width: Pixels,
    line_height: Pixels,
    color: Hsla,
    text_color: Hsla, // Color for text on block cursor
    shape: CursorShape,
    cursor_char: Option<char>,
}

impl CursorLayout {
    fn new(
        origin: gpui::Point<Pixels>,
        block_width: Pixels,
        line_height: Pixels,
        color: Hsla,
        text_color: Hsla,
        shape: CursorShape,
        cursor_char: Option<char>,
    ) -> Self {
        Self {
            origin,
            block_width,
            line_height,
            color,
            text_color,
            shape,
            cursor_char,
        }
    }

    fn bounds(&self, content_origin: gpui::Point<Pixels>) -> Bounds<Pixels> {
        let origin = self.origin + content_origin;
        match self.shape {
            CursorShape::Bar => Bounds {
                origin,
                size: Size {
                    width: px(2.0),
                    height: self.line_height,
                },
            },
            CursorShape::Block | CursorShape::Hollow => Bounds {
                origin,
                size: Size {
                    width: self.block_width,
                    height: self.line_height,
                },
            },
            CursorShape::Underline => Bounds {
                origin: origin + gpui::point(Pixels::ZERO, self.line_height - px(2.0)),
                size: Size {
                    width: self.block_width,
                    height: px(2.0),
                },
            },
        }
    }

    fn paint(&self, content_origin: gpui::Point<Pixels>, window: &mut Window, cx: &mut App) {
        let bounds = self.bounds(content_origin);

        // Draw cursor shape
        if matches!(self.shape, CursorShape::Hollow) {
            // Hollow cursor: just an outline
            window.paint_quad(outline(bounds, self.color, BorderStyle::Solid));
        } else {
            // Filled cursor
            window.paint_quad(fill(bounds, self.color));
        }

        // For block cursor, draw the character on top with inverted colors
        if self.shape == CursorShape::Block {
            if let Some(c) = self.cursor_char {
                if c != ' ' && c != '\0' {
                    let font = Font {
                        family: "Paper Mono".into(),
                        features: FontFeatures::default(),
                        fallbacks: None,
                        weight: FontWeight::NORMAL,
                        style: FontStyle::Normal,
                    };

                    let text_run = TextRun {
                        len: c.len_utf8(),
                        font,
                        color: self.text_color,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };

                    let shaped = window.text_system().shape_line(
                        c.to_string().into(),
                        px(13.0),
                        &[text_run],
                        Some(self.block_width),
                    );
                    let _ = shaped.paint(
                        self.origin + content_origin,
                        self.line_height,
                        window,
                        cx,
                    );
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SelectionPhase {
    None,
    Selecting,
    Ended,
}

/// Convert pixel position to grid point and side
fn pixel_to_grid_point(
    position: gpui::Point<Pixels>,
    origin: gpui::Point<Pixels>,
    cols: usize,
    rows: usize,
    display_offset: usize,
) -> (AlacPoint, Side) {
    // Adjust for padding
    let x = f32::from(position.x - origin.x - px(PADDING));
    let y = f32::from(position.y - origin.y - px(PADDING));

    // Calculate column
    let mut col = (x / CELL_WIDTH) as i32;
    let cell_x = if x >= 0.0 { x % CELL_WIDTH } else { 0.0 };
    let half_cell_width = CELL_WIDTH / 2.0;
    let mut side = if cell_x > half_cell_width {
        Side::Right
    } else {
        Side::Left
    };

    // Clamp column
    if col < 0 {
        col = 0;
        side = Side::Left;
    } else if col >= cols as i32 {
        col = (cols - 1) as i32;
        side = Side::Right;
    }

    // Calculate line
    let mut line = (y / CELL_HEIGHT) as i32;
    if line < 0 {
        line = 0;
        side = Side::Left;
    } else if line >= rows as i32 {
        line = (rows - 1) as i32;
        side = Side::Right;
    }

    (
        AlacPoint::new(Line(line - display_offset as i32), Column(col as usize)),
        side,
    )
}

// Batched text run - combines consecutive characters with same style
struct BatchedTextRun {
    line: i32,
    col: usize,
    text: String,
    cell_count: usize,
    color: Hsla,
    background: Option<Hsla>,
    bold: bool,
}

impl BatchedTextRun {
    fn new(line: i32, col: usize, c: char, color: Hsla, background: Option<Hsla>, bold: bool) -> Self {
        Self {
            line,
            col,
            text: c.to_string(),
            cell_count: 1,
            color,
            background,
            bold,
        }
    }

    fn can_append(&self, color: Hsla, background: Option<Hsla>, bold: bool) -> bool {
        self.color == color && self.background == background && self.bold == bold
    }

    fn append(&mut self, c: char) {
        self.text.push(c);
        self.cell_count += 1;
    }

    fn paint(&self, origin: gpui::Point<Pixels>, cell_width: Pixels, line_height: Pixels, window: &mut Window, cx: &mut App) {
        let pos = gpui::Point::new(
            origin.x + px(self.col as f32 * f32::from(cell_width)),
            origin.y + px(self.line as f32 * f32::from(line_height)),
        );

        // Paint background if set
        if let Some(bg) = self.background {
            let bounds = Bounds::new(
                pos,
                Size {
                    width: cell_width * self.cell_count as f32,
                    height: line_height,
                },
            );
            window.paint_quad(fill(bounds, bg));
        }

        // Paint text
        let font = Font {
            family: "Paper Mono".into(),
            features: FontFeatures::default(),
            fallbacks: None,
            weight: if self.bold { FontWeight::BOLD } else { FontWeight::NORMAL },
            style: FontStyle::Normal,
        };

        let text_run = TextRun {
            len: self.text.len(),
            font,
            color: self.color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };

        let font_size = px(13.0);
        let shaped = window.text_system().shape_line(
            self.text.clone().into(),
            font_size,
            &[text_run],
            Some(cell_width),
        );
        let _ = shaped.paint(pos, line_height, window, cx);
    }
}

pub struct TerminalView {
    terminal: Arc<parking_lot::Mutex<Terminal>>,
    focus_handle: FocusHandle,
    scroll_accumulator: f32,
    selection_phase: SelectionPhase,
    content_bounds: Arc<parking_lot::Mutex<Option<Bounds<Pixels>>>>,
    last_size: Arc<parking_lot::Mutex<Option<(u16, u16)>>>,
}

impl TerminalView {
    pub fn new(terminal: Arc<parking_lot::Mutex<Terminal>>, cx: &mut Context<Self>) -> Self {
        let term = terminal.clone();

        // Poll for terminal output every 30ms
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(30))
                    .await;

                let has_updates = term.lock().drain_events();

                if has_updates {
                    let _ = cx.update(|cx| {
                        let _ = this.update(cx, |_, cx| {
                            cx.notify();
                        });
                    });
                }
            }
        })
        .detach();

        Self {
            terminal,
            focus_handle: cx.focus_handle(),
            scroll_accumulator: 0.0,
            selection_phase: SelectionPhase::None,
            content_bounds: Arc::new(parking_lot::Mutex::new(None)),
            last_size: Arc::new(parking_lot::Mutex::new(None)),
        }
    }

    pub fn write(&self, input: &str) {
        self.terminal.lock().write(input.as_bytes());
    }
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus_handle = self.focus_handle.clone();
        let is_focused = focus_handle.is_focused(window);

        // Extract terminal content for the element
        let terminal = self.terminal.clone();

        div()
            .id("terminal-wrapper")
            .key_context("Terminal")
            .track_focus(&focus_handle)
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _window, cx| {
                // Handle Cmd+C for copy
                if event.keystroke.modifiers.platform && event.keystroke.key == "c" {
                    if let Some(text) = this.terminal.lock().selection_to_string() {
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                    }
                    return;
                }

                // Handle Cmd+V for paste
                if event.keystroke.modifiers.platform && event.keystroke.key == "v" {
                    if let Some(item) = cx.read_from_clipboard() {
                        if let Some(text) = item.text() {
                            this.terminal.lock().write(text.as_bytes());
                            cx.notify();
                        }
                    }
                    return;
                }

                let terminal = this.terminal.lock();
                let mode = terminal.mode();
                let input = key_to_input(event, &mode);
                if !input.is_empty() {
                    terminal.write(input.as_bytes());
                    cx.notify();
                }
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, event: &MouseDownEvent, window, cx| {
                cx.focus_self(window);

                let Some(bounds) = *this.content_bounds.lock() else { return };
                let terminal = this.terminal.lock();
                let (cols, rows, display_offset) = terminal.with_term(|term| {
                    (term.grid().columns(), term.grid().screen_lines(), term.grid().display_offset())
                });

                let (point, side) = pixel_to_grid_point(
                    event.position,
                    bounds.origin,
                    cols,
                    rows,
                    display_offset,
                );

                let selection_type = match event.click_count {
                    0 => return, // Release
                    1 => SelectionType::Simple,
                    2 => SelectionType::Semantic,
                    3 => SelectionType::Lines,
                    _ => return,
                };

                // Shift+click extends selection
                if selection_type == SelectionType::Simple && event.modifiers.shift {
                    if terminal.has_selection() {
                        terminal.update_selection(point, side);
                        this.selection_phase = SelectionPhase::Selecting;
                        cx.notify();
                        return;
                    }
                }

                terminal.start_selection(selection_type, point, side);
                this.selection_phase = SelectionPhase::Selecting;
                cx.notify();
            }))
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                let pixel_delta = match event.delta {
                    ScrollDelta::Lines(lines) => f32::from(lines.y) * CELL_HEIGHT,
                    ScrollDelta::Pixels(pixels) => f32::from(pixels.y),
                };

                this.scroll_accumulator += pixel_delta;

                let lines = (this.scroll_accumulator / CELL_HEIGHT) as i32;
                if lines != 0 {
                    this.scroll_accumulator -= lines as f32 * CELL_HEIGHT;
                    this.terminal.lock().scroll(lines);
                    cx.notify();
                }
            }))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                // Only update selection while dragging with left mouse button
                if this.selection_phase != SelectionPhase::Selecting {
                    return;
                }
                if event.pressed_button != Some(MouseButton::Left) {
                    return;
                }

                let Some(bounds) = *this.content_bounds.lock() else { return };
                let terminal = this.terminal.lock();
                let (cols, rows, display_offset) = terminal.with_term(|term| {
                    (term.grid().columns(), term.grid().screen_lines(), term.grid().display_offset())
                });

                let (point, side) = pixel_to_grid_point(
                    event.position,
                    bounds.origin,
                    cols,
                    rows,
                    display_offset,
                );

                terminal.update_selection(point, side);
                cx.notify();
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _event: &MouseUpEvent, _, cx| {
                if this.selection_phase == SelectionPhase::Selecting {
                    this.selection_phase = SelectionPhase::Ended;
                    cx.notify();
                }
            }))
            .size_full()
            .child(TerminalElement {
                terminal,
                is_focused,
                bounds_out: self.content_bounds.clone(),
                last_size: self.last_size.clone(),
            })
    }
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

// Custom Element for efficient terminal rendering
struct TerminalElement {
    terminal: Arc<parking_lot::Mutex<Terminal>>,
    is_focused: bool,
    bounds_out: Arc<parking_lot::Mutex<Option<Bounds<Pixels>>>>,
    last_size: Arc<parking_lot::Mutex<Option<(u16, u16)>>>,
}

/// Represents a selection range on a single line
struct SelectionLineRange {
    line: i32,
    start_col: usize,
    end_col: usize,
}

struct TerminalLayoutState {
    text_runs: Vec<BatchedTextRun>,
    cursor: Option<CursorLayout>,
    selection_ranges: Vec<SelectionLineRange>,
    // Theme colors for paint
    background_color: Hsla,
    selection_color: Hsla,
}

impl IntoElement for TerminalElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TerminalElement {
    type RequestLayoutState = ();
    type PrepaintState = TerminalLayoutState;

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
        // Get theme colors
        let theme = cx.theme();
        let default_fg = theme.foreground;
        let default_bg = theme.background;
        let selection_color = theme.selection;
        let caret_color = theme.caret;
        let muted_foreground = theme.muted_foreground;

        // Store bounds for mouse event coordinate conversion
        *self.bounds_out.lock() = Some(bounds);

        // Calculate terminal size from actual element bounds
        let available_width = f32::from(bounds.size.width) - (PADDING * 2.0);
        let available_height = f32::from(bounds.size.height) - (PADDING * 2.0);
        let cols = (available_width / CELL_WIDTH).floor().max(1.0) as u16;
        let rows = (available_height / CELL_HEIGHT).floor().max(1.0) as u16;
        let new_size = (cols, rows);

        // Resize terminal if size changed
        {
            let mut last_size = self.last_size.lock();
            if *last_size != Some(new_size) {
                *last_size = Some(new_size);
                self.terminal.lock().resize(cols, rows, CELL_WIDTH as u16, CELL_HEIGHT as u16);
            }
        }

        let terminal = self.terminal.lock();
        let mut text_runs: Vec<BatchedTextRun> = Vec::new();
        let mut cursor: Option<CursorLayout> = None;
        let mut selection_ranges: Vec<SelectionLineRange> = Vec::new();

        terminal.with_term(|term| {
            let grid = term.grid();
            let cols = grid.columns();
            let content = term.renderable_content();
            let display_offset = content.display_offset;
            let alac_cursor = &content.cursor;
            let screen_lines = grid.screen_lines();

            // Convert cursor position to screen coordinates using DisplayCursor
            let display_cursor = DisplayCursor::from(alac_cursor.point, display_offset);
            let cursor_screen_line = display_cursor.line();
            let cursor_col = display_cursor.col();

            // Only show cursor if it's within visible screen and not hidden
            // Note: Don't check >= 0 as cursor_screen_line is i32 and negative values
            // will wrap to large usize values that fail the < screen_lines check anyway
            if (cursor_screen_line as usize) < screen_lines
                && !matches!(alac_cursor.shape, AlacCursorShape::Hidden)
            {
                // Determine cursor shape based on focus and alacritty shape
                let shape = if !self.is_focused {
                    CursorShape::Hollow
                } else {
                    match alac_cursor.shape {
                        AlacCursorShape::Block => CursorShape::Block,
                        AlacCursorShape::Underline => CursorShape::Underline,
                        AlacCursorShape::Beam => CursorShape::Bar,
                        AlacCursorShape::HollowBlock => CursorShape::Hollow,
                        AlacCursorShape::Hidden => CursorShape::Block, // Won't reach here
                    }
                };

                let cursor_color = if self.is_focused {
                    caret_color
                } else {
                    muted_foreground
                };

                // Get the character under the cursor for block cursor text rendering
                let cursor_char = if shape == CursorShape::Block {
                    let grid_line = alac_cursor.point.line;
                    let cell = &grid[grid_line][Column(cursor_col)];
                    let c = cell.c;
                    if c != '\0' && c != ' ' { Some(c) } else { None }
                } else {
                    None
                };

                // Calculate pixel position for cursor
                let cursor_origin = gpui::point(
                    px(cursor_col as f32 * CELL_WIDTH),
                    px(cursor_screen_line as f32 * CELL_HEIGHT),
                );

                cursor = Some(CursorLayout::new(
                    cursor_origin,
                    px(CELL_WIDTH),
                    px(CELL_HEIGHT),
                    cursor_color,
                    default_bg, // Text color on block cursor
                    shape,
                    cursor_char,
                ));
            }

            // Extract selection ranges from the terminal
            if let Some(ref selection) = term.selection {
                if let Some(range) = selection.to_range(term) {
                    let start = range.start;
                    let end = range.end;

                    // Iterate through visible lines and extract selection ranges
                    for line_idx in 0..screen_lines {
                        let grid_line = Line(line_idx as i32 - display_offset as i32);

                        // Check if this line is within the selection
                        if grid_line.0 < start.line.0 || grid_line.0 > end.line.0 {
                            continue;
                        }

                        let start_col = if grid_line.0 == start.line.0 {
                            start.column.0
                        } else {
                            0
                        };

                        let end_col = if grid_line.0 == end.line.0 {
                            end.column.0
                        } else {
                            cols - 1
                        };

                        selection_ranges.push(SelectionLineRange {
                            line: line_idx as i32,
                            start_col,
                            end_col,
                        });
                    }
                }
            }

            for line_idx in 0..screen_lines {
                let grid_line = Line(line_idx as i32 - display_offset as i32);
                let row = &grid[grid_line];
                let mut current_run: Option<BatchedTextRun> = None;

                for col_idx in 0..cols {
                    let cell = &row[Column(col_idx)];

                    if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
                        continue;
                    }

                    let c = if cell.c == '\0' { ' ' } else { cell.c };

                    // Check if this is the cursor position
                    let is_block_cursor = cursor_screen_line == line_idx as i32
                        && cursor_col == col_idx
                        && cursor.as_ref().map_or(false, |c| c.shape == CursorShape::Block);

                    // For block cursor, skip rendering this character (CursorLayout handles it)
                    // For other cursor shapes, render normally
                    let (fg, bg) = {
                        let mut fg = cell.fg;
                        let mut bg = cell.bg;

                        // Handle INVERSE flag - swap foreground and background colors
                        // This is how TUI apps like Claude Code render their own cursor
                        if cell.flags.contains(CellFlags::INVERSE) {
                            std::mem::swap(&mut fg, &mut bg);
                        }

                        let fg = ansi_to_hsla(fg, default_fg, default_bg).unwrap_or(default_fg);
                        // Only set background if it's not the default background
                        // (so selection highlights can show through)
                        let bg = if matches!(bg, AnsiColor::Named(NamedColor::Background)) {
                            None
                        } else {
                            ansi_to_hsla(bg, default_fg, default_bg)
                        };
                        (fg, bg)
                    };

                    let bold = cell.flags.contains(CellFlags::BOLD);

                    // Skip rendering at block cursor position (CursorLayout handles it)
                    if is_block_cursor {
                        if let Some(run) = current_run.take() {
                            text_runs.push(run);
                        }
                        continue;
                    }

                    // Try to extend current run or start a new one
                    if let Some(ref mut run) = current_run {
                        if run.can_append(fg, bg, bold) {
                            run.append(c);
                        } else {
                            text_runs.push(current_run.take().unwrap());
                            current_run = Some(BatchedTextRun::new(line_idx as i32, col_idx, c, fg, bg, bold));
                        }
                    } else {
                        current_run = Some(BatchedTextRun::new(line_idx as i32, col_idx, c, fg, bg, bold));
                    }
                }

                if let Some(run) = current_run.take() {
                    text_runs.push(run);
                }
            }
        });

        TerminalLayoutState {
            text_runs,
            cursor,
            selection_ranges,
            background_color: default_bg,
            selection_color,
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
        let cell_width = px(CELL_WIDTH);
        let line_height = px(CELL_HEIGHT);
        let origin = bounds.origin + gpui::point(px(PADDING), px(PADDING));

        // Paint background
        window.paint_quad(fill(bounds, layout.background_color));

        // Paint selection highlights (before text so it appears behind)
        for range in &layout.selection_ranges {
            let pos = gpui::Point::new(
                origin.x + px(range.start_col as f32 * CELL_WIDTH),
                origin.y + px(range.line as f32 * CELL_HEIGHT),
            );
            let width = ((range.end_col - range.start_col + 1) as f32) * CELL_WIDTH;
            let bounds = Bounds::new(
                pos,
                Size {
                    width: px(width),
                    height: line_height,
                },
            );
            window.paint_quad(fill(bounds, layout.selection_color));
        }

        // Paint text runs
        for run in &layout.text_runs {
            run.paint(origin, cell_width, line_height, window, cx);
        }

        // Paint cursor (after text so it appears on top)
        if let Some(ref cursor) = layout.cursor {
            cursor.paint(origin, window, cx);
        }
    }
}

fn key_to_input(event: &KeyDownEvent, mode: &TermMode) -> String {
    let key = &event.keystroke.key;
    let modifiers = &event.keystroke;
    let app_cursor = mode.contains(TermMode::APP_CURSOR);

    if modifiers.modifiers.control {
        if key.len() == 1 {
            let c = key.chars().next().unwrap();
            if c.is_ascii_alphabetic() {
                let ctrl_char = (c.to_ascii_lowercase() as u8 - b'a' + 1) as char;
                return ctrl_char.to_string();
            }
        }
    }

    // Handle special keys first
    // Arrow keys and cursor keys change based on APP_CURSOR mode
    match key.as_str() {
        "enter" => return "\r".to_string(),
        "backspace" => {
            // Option+Backspace deletes word (send ESC + DEL)
            if modifiers.modifiers.alt {
                return "\x1b\x7f".to_string();
            }
            return "\x7f".to_string();
        }
        "tab" => {
            if modifiers.modifiers.shift {
                return "\x1b[Z".to_string();
            }
            return "\t".to_string();
        }
        "escape" => return "\x1b".to_string(),
        "up" => {
            if app_cursor {
                return "\x1bOA".to_string();
            }
            return "\x1b[A".to_string();
        }
        "down" => {
            if app_cursor {
                return "\x1bOB".to_string();
            }
            return "\x1b[B".to_string();
        }
        "right" => {
            // Option+Right: forward word (ESC + f)
            if modifiers.modifiers.alt {
                return "\x1bf".to_string();
            }
            if app_cursor {
                return "\x1bOC".to_string();
            }
            return "\x1b[C".to_string();
        }
        "left" => {
            // Option+Left: backward word (ESC + b)
            if modifiers.modifiers.alt {
                return "\x1bb".to_string();
            }
            if app_cursor {
                return "\x1bOD".to_string();
            }
            return "\x1b[D".to_string();
        }
        "home" => {
            if app_cursor {
                return "\x1bOH".to_string();
            }
            return "\x1b[H".to_string();
        }
        "end" => {
            if app_cursor {
                return "\x1bOF".to_string();
            }
            return "\x1b[F".to_string();
        }
        "pageup" => return "\x1b[5~".to_string(),
        "pagedown" => return "\x1b[6~".to_string(),
        "delete" => return "\x1b[3~".to_string(),
        "insert" => return "\x1b[2~".to_string(),
        "space" => return " ".to_string(),
        // Function keys
        "f1" => return "\x1bOP".to_string(),
        "f2" => return "\x1bOQ".to_string(),
        "f3" => return "\x1bOR".to_string(),
        "f4" => return "\x1bOS".to_string(),
        "f5" => return "\x1b[15~".to_string(),
        "f6" => return "\x1b[17~".to_string(),
        "f7" => return "\x1b[18~".to_string(),
        "f8" => return "\x1b[19~".to_string(),
        "f9" => return "\x1b[20~".to_string(),
        "f10" => return "\x1b[21~".to_string(),
        "f11" => return "\x1b[23~".to_string(),
        "f12" => return "\x1b[24~".to_string(),
        _ => {}
    }

    // Use key_char for actual typed character (handles shift for uppercase, etc.)
    if let Some(key_char) = &event.keystroke.key_char {
        if !key_char.is_empty() {
            return key_char.clone();
        }
    }

    // Fallback to key if it's a single character
    if key.len() == 1 {
        return key.clone();
    }

    String::new()
}
