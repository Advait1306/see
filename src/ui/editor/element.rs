//! Editor element for efficient rendering

use super::selection::Selection;
use super::view::EditorView;
use crate::constants::{CELL_HEIGHT, CELL_WIDTH, PADDING};
use crate::editor::Buffer;
use gpui::prelude::*;
use gpui::*;
use gpui_component::theme::ActiveTheme;

/// Custom Element for efficient editor rendering
pub(crate) struct EditorElement {
    pub(crate) view: Entity<EditorView>,
    pub(crate) buffer: Entity<Buffer>,
    pub(crate) cursor_line: usize,
    pub(crate) cursor_col: usize,
    pub(crate) scroll_offset: usize,
    pub(crate) scroll_x: f32,
    pub(crate) is_focused: bool,
    pub(crate) cursor_visible: bool,
    pub(crate) selection: Option<Selection>,
}

/// Represents a selection range on a single visible line
pub(crate) struct SelectionLineRange {
    pub(crate) line_idx: usize, // Index in visible lines (0-based screen position)
    pub(crate) start_col: usize,
    pub(crate) end_col: usize,
}

pub(crate) struct EditorLayoutState {
    pub(crate) visible_lines: Vec<(usize, String)>, // (line_number, content)
    pub(crate) cursor_position: Option<gpui::Point<Pixels>>,
    pub(crate) line_number_width: f32,
    pub(crate) scroll_x: f32,
    pub(crate) selection_ranges: Vec<SelectionLineRange>,
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

        // Calculate selection ranges for visible lines
        let mut selection_ranges = Vec::new();
        if let Some(ref selection) = self.selection {
            if !selection.is_empty() {
                let ((start_line, start_col), (end_line, end_col)) = selection.normalized();

                // Iterate through visible lines
                for (idx, (line_num, content)) in visible_lines.iter().enumerate() {
                    let line = line_num - 1; // Convert back to 0-indexed

                    // Check if this line is within the selection
                    if line < start_line || line > end_line {
                        continue;
                    }

                    let line_len = content.len();

                    let sel_start_col = if line == start_line {
                        start_col
                    } else {
                        0
                    };

                    let sel_end_col = if line == end_line {
                        end_col
                    } else {
                        line_len
                    };

                    // Only add if there's something to select on this line
                    if sel_start_col < sel_end_col || (line != end_line && sel_start_col <= line_len) {
                        selection_ranges.push(SelectionLineRange {
                            line_idx: idx,
                            start_col: sel_start_col,
                            end_col: if line == end_line { sel_end_col } else { line_len.max(sel_start_col) },
                        });
                    }
                }
            }
        }

        EditorLayoutState {
            visible_lines,
            cursor_position,
            line_number_width,
            scroll_x: self.scroll_x,
            selection_ranges,
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
        // Get theme colors
        let theme = cx.theme();
        let background_color = theme.background;
        let foreground_color = theme.foreground;
        let muted_foreground_color = theme.muted_foreground;
        let selection_color = theme.selection;
        let caret_color = theme.caret;
        let sidebar_color = theme.sidebar;

        // Paint background
        window.paint_quad(fill(bounds, background_color));

        let origin = bounds.origin;

        // Paint line numbers background
        let line_numbers_bounds = Bounds {
            origin: origin + gpui::point(Pixels::ZERO, px(PADDING)),
            size: Size {
                width: px(PADDING + layout.line_number_width),
                height: bounds.size.height - px(PADDING * 2.0),
            },
        };
        window.paint_quad(fill(line_numbers_bounds, sidebar_color));

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
                color: muted_foreground_color,
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

        // Paint line content, selection, and cursor with clipping
        let cursor_pos = layout.cursor_position;
        let is_focused = self.is_focused;
        let cursor_visible = self.cursor_visible;
        let text_x_base = PADDING + layout.line_number_width - layout.scroll_x;

        window.with_content_mask(Some(ContentMask { bounds: text_area_bounds }), |window| {
            // Paint selection highlights first (behind text)
            for range in &layout.selection_ranges {
                let y = origin.y + px(PADDING + (range.line_idx as f32 * CELL_HEIGHT));
                let x = origin.x + px(text_x_base + (range.start_col as f32 * CELL_WIDTH));
                let width = ((range.end_col - range.start_col) as f32) * CELL_WIDTH;

                // Minimum width of one cell for empty line selections
                let width = if width < CELL_WIDTH { CELL_WIDTH } else { width };

                let selection_bounds = Bounds::new(
                    gpui::point(x, y),
                    Size {
                        width: px(width),
                        height: px(CELL_HEIGHT),
                    },
                );
                window.paint_quad(fill(selection_bounds, selection_color));
            }

            // Paint text
            for (idx, (_line_num, content)) in layout.visible_lines.iter().enumerate() {
                let y = origin.y + px(PADDING + (idx as f32 * CELL_HEIGHT));

                if !content.is_empty() {
                    let text_run = TextRun {
                        len: content.len(),
                        font: font.clone(),
                        color: foreground_color,
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
                    let _ = shaped.paint(
                        gpui::point(origin.x + px(text_x_base), y),
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
                        caret_color
                    } else {
                        muted_foreground_color
                    };

                    window.paint_quad(fill(cursor_bounds, color));
                }
            }
        });
    }
}
