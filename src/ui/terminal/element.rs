//! Terminal element for efficient rendering using GPUI's Element trait.
//!
//! # Architecture
//!
//! The terminal uses Alacritty's terminal emulator (`alacritty_terminal`) for PTY
//! management and VT parsing. This element handles rendering the terminal grid to
//! the screen.
//!
//! # Rendering Pipeline
//!
//! 1. **request_layout**: Returns a layout ID requesting full available space.
//!
//! 2. **prepaint**: Prepares rendering data:
//!    - Calculates terminal grid size (cols x rows) from viewport dimensions
//!    - Resizes the PTY if the terminal size changed
//!    - Iterates through the terminal grid to build `BatchedTextRun`s
//!    - Handles cell flags (BOLD, INVERSE, WIDE_CHAR_SPACER)
//!    - Computes cursor position and shape
//!    - Extracts selection ranges for highlighting
//!    - Stores bounds for mouse coordinate conversion
//!
//! 3. **paint**: Draws the terminal:
//!    - Fills background
//!    - Paints selection highlights
//!    - Paints batched text runs (optimized to group adjacent cells with same style)
//!    - Paints cursor (block, bar, underline, or hollow)
//!
//! # Text Batching
//!
//! Adjacent cells with the same foreground color, background, and bold state are
//! grouped into `BatchedTextRun`s to reduce draw calls. Each run is shaped and
//! painted as a single text string.
//!
//! # Cursor Handling
//!
//! The cursor is rendered separately from text. For block cursors, the character
//! under the cursor is skipped during text rendering and drawn by `CursorLayout`
//! with inverted colors.

use super::colors::ansi_to_hsla;
use super::cursor::{CursorLayout, CursorShape, DisplayCursor};
use super::text_batch::BatchedTextRun;
use crate::constants::{CELL_HEIGHT, CELL_WIDTH, PADDING};
use crate::terminal::{TerminalEventListener, TerminalInner};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags as CellFlags;
use alacritty_terminal::vte::ansi::{Color as AnsiColor, CursorShape as AlacCursorShape, NamedColor};
use gpui::prelude::*;
use gpui::*;
use gpui_component::theme::ActiveTheme;
use std::sync::Arc;

/// Custom Element for efficient terminal rendering
pub(crate) struct TerminalElement {
    pub(crate) inner: Arc<parking_lot::Mutex<TerminalInner>>,
    pub(crate) is_focused: bool,
    pub(crate) bounds_out: Arc<parking_lot::Mutex<Option<Bounds<Pixels>>>>,
    pub(crate) last_size: Arc<parking_lot::Mutex<Option<(u16, u16)>>>,
}

/// Represents a selection range on a single line
pub(crate) struct SelectionLineRange {
    pub(crate) line: i32,
    pub(crate) start_col: usize,
    pub(crate) end_col: usize,
}

pub(crate) struct TerminalLayoutState {
    pub(crate) text_runs: Vec<BatchedTextRun>,
    pub(crate) cursor: Option<CursorLayout>,
    pub(crate) selection_ranges: Vec<SelectionLineRange>,
    // Theme colors for paint
    pub(crate) background_color: Hsla,
    pub(crate) selection_color: Hsla,
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
                self.inner.lock().resize(cols, rows, CELL_WIDTH as u16, CELL_HEIGHT as u16);
            }
        }

        let inner = self.inner.lock();
        let mut text_runs: Vec<BatchedTextRun> = Vec::new();
        let mut cursor: Option<CursorLayout> = None;
        let mut selection_ranges: Vec<SelectionLineRange> = Vec::new();

        inner.with_term(|term| {
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
                if let Some(range) = selection.to_range::<TerminalEventListener>(term) {
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
