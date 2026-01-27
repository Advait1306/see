//! Editor element for efficient rendering using GPUI's Element trait.
//!
//! # Rendering Pipeline
//!
//! The editor uses GPUI's three-phase rendering:
//!
//! 1. **request_layout**: Returns a layout ID requesting full available space.
//!
//! 2. **prepaint**: Calculates what needs to be drawn:
//!    - Determines visible lines based on scroll offset and viewport height
//!    - Calculates line number gutter width based on total line count
//!    - Computes cursor position accounting for horizontal scroll
//!    - Builds selection ranges for visible lines
//!    - Stores bounds in the view for mouse click handling
//!
//! 3. **paint**: Actually draws the editor:
//!    - Fills background
//!    - Paints line number gutter (outside clip region)
//!    - Within a clipped text area:
//!      - Paints selection highlights
//!      - Paints text content with monospace font
//!      - Paints blinking cursor
//!
//! # Coordinate System
//!
//! - `scroll_offset`: Vertical scroll in lines (0 = top of document)
//! - `scroll_x`: Horizontal scroll in pixels
//! - `cursor_line`, `cursor_col`: 0-indexed position in the document
//! - Line numbers display as 1-indexed

use super::diff_mode::DiffDisplayLine;
use super::selection::Selection;
use super::view::EditorView;
use crate::constants::{CELL_HEIGHT, CELL_WIDTH, GUTTER_MARKER_WIDTH, PADDING};
use crate::stores::{Buffer, DiffLineTag, LineDiff};
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

/// Visible line data for diff mode
pub(crate) enum DiffVisibleLine {
    Line {
        old_num: Option<usize>,
        new_num: Option<usize>,
        tag: DiffLineTag,
        content: String,
    },
    Collapsed {
        count: usize,
    },
}

pub(crate) struct EditorLayoutState {
    pub(crate) visible_lines: Vec<(usize, String)>, // (line_number, content) for normal mode
    pub(crate) cursor_position: Option<gpui::Point<Pixels>>,
    pub(crate) line_number_width: f32,
    pub(crate) scroll_x: f32,
    pub(crate) selection_ranges: Vec<SelectionLineRange>,
    pub(crate) line_diffs: Vec<LineDiff>, // Diff status for visible lines (normal mode)
    pub(crate) diff_mode_lines: Vec<DiffVisibleLine>, // Diff mode visible lines
    pub(crate) diff_old_num_width: f32,
    pub(crate) diff_new_num_width: f32,
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
        // Both normal and diff mode fill available height - content is virtualized internally
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

        // Check if in diff mode
        let diff_mode_data = self.view.read(cx).diff_mode.as_ref().map(|d| {
            (
                d.display_lines.clone(),
                d.max_old_line_num,
                d.max_new_line_num,
            )
        });

        if let Some((display_lines, max_old, max_new)) = diff_mode_data {
            // Diff mode prepaint
            let total_lines = display_lines.len();

            let mut diff_visible_lines = Vec::new();
            for i in 0..visible_line_count {
                let line_idx = self.scroll_offset + i;
                if line_idx < total_lines {
                    match &display_lines[line_idx] {
                        DiffDisplayLine::Line(line) => {
                            diff_visible_lines.push(DiffVisibleLine::Line {
                                old_num: line.old_line_num,
                                new_num: line.new_line_num,
                                tag: line.tag,
                                content: line.content.clone(),
                            });
                        }
                        DiffDisplayLine::Collapsed { count, .. } => {
                            diff_visible_lines.push(DiffVisibleLine::Collapsed { count: *count });
                        }
                    }
                }
            }

            // Calculate line number widths for diff mode
            let old_num_chars = format!("{}", max_old).len().max(3);
            let new_num_chars = format!("{}", max_new).len().max(3);
            let diff_old_num_width = (old_num_chars as f32 + 1.0) * CELL_WIDTH;
            let diff_new_num_width = (new_num_chars as f32 + 1.0) * CELL_WIDTH;
            let line_number_width = diff_old_num_width + diff_new_num_width + CELL_WIDTH; // +1 cell for prefix

            // Store bounds for click handling
            self.view.update(cx, |view, _| {
                view.last_bounds = Some(bounds);
                view.last_line_number_width = line_number_width;
            });

            return EditorLayoutState {
                visible_lines: Vec::new(),
                cursor_position: None,
                line_number_width,
                scroll_x: self.scroll_x,
                selection_ranges: Vec::new(),
                line_diffs: Vec::new(),
                diff_mode_lines: diff_visible_lines,
                diff_old_num_width,
                diff_new_num_width,
            };
        }

        // Normal mode prepaint
        let buffer = self.buffer.read(cx);
        let line_count = buffer.line_count();

        let mut visible_lines = Vec::new();
        let mut line_diffs = Vec::new();
        for i in 0..visible_line_count {
            let line_idx = self.scroll_offset + i;
            if line_idx < line_count {
                if let Some(line) = buffer.line(line_idx) {
                    // Remove trailing newline for display
                    let line = line.trim_end_matches('\n').to_string();
                    visible_lines.push((line_idx + 1, line)); // 1-indexed line numbers
                    line_diffs.push(buffer.line_diff(line_idx));
                }
            }
        }

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
            line_diffs,
            diff_mode_lines: Vec::new(),
            diff_old_num_width: 0.0,
            diff_new_num_width: 0.0,
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
        let added_color = theme.success;
        let modified_color = theme.warning;
        let deleted_color = theme.danger;

        let font = Font {
            family: "Paper Mono".into(),
            features: FontFeatures::default(),
            fallbacks: None,
            weight: FontWeight::NORMAL,
            style: FontStyle::Normal,
        };

        let font_size = px(13.0);
        let origin = bounds.origin;

        // Check if in diff mode
        if !layout.diff_mode_lines.is_empty() {
            self.paint_diff_mode(
                bounds,
                layout,
                window,
                cx,
                &font,
                font_size,
                background_color,
                foreground_color,
                muted_foreground_color,
                sidebar_color,
                added_color,
                deleted_color,
            );
            return;
        }

        // Normal mode painting
        // Paint background
        window.paint_quad(fill(bounds, background_color));

        // Paint line numbers background
        let line_numbers_bounds = Bounds {
            origin: origin + gpui::point(Pixels::ZERO, px(PADDING)),
            size: Size {
                width: px(PADDING + layout.line_number_width),
                height: bounds.size.height - px(PADDING * 2.0),
            },
        };
        window.paint_quad(fill(line_numbers_bounds, sidebar_color));

        // Paint git diff gutter markers
        for (idx, diff) in layout.line_diffs.iter().enumerate() {
            let marker_color = match diff {
                LineDiff::Unchanged => continue,
                LineDiff::Added => added_color,
                LineDiff::Modified => modified_color,
                LineDiff::DeletedAbove => deleted_color,
            };

            let y = origin.y + px(PADDING + (idx as f32 * CELL_HEIGHT));
            let marker_height = if *diff == LineDiff::DeletedAbove {
                GUTTER_MARKER_WIDTH
            } else {
                CELL_HEIGHT
            };

            let marker_bounds = Bounds {
                origin: gpui::point(origin.x + px(PADDING - GUTTER_MARKER_WIDTH - 1.0), y),
                size: Size {
                    width: px(GUTTER_MARKER_WIDTH),
                    height: px(marker_height),
                },
            };
            window.paint_quad(fill(marker_bounds, marker_color));
        }

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

            // Paint cursor (inside clip region) - only show when focused, with blink
            let should_show_cursor = is_focused && cursor_visible;
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

impl EditorElement {
    #[allow(clippy::too_many_arguments)]
    fn paint_diff_mode(
        &self,
        bounds: Bounds<Pixels>,
        layout: &EditorLayoutState,
        window: &mut Window,
        cx: &mut App,
        font: &Font,
        font_size: Pixels,
        background_color: Hsla,
        foreground_color: Hsla,
        muted_foreground_color: Hsla,
        sidebar_color: Hsla,
        added_color: Hsla,
        deleted_color: Hsla,
    ) {
        let origin = bounds.origin;

        // Paint background
        window.paint_quad(fill(bounds, background_color));

        // Diff line colors with transparency
        let added_bg = Hsla {
            h: added_color.h,
            s: added_color.s,
            l: added_color.l,
            a: 0.15,
        };
        let deleted_bg = Hsla {
            h: deleted_color.h,
            s: deleted_color.s,
            l: deleted_color.l,
            a: 0.15,
        };

        // Paint gutter background
        let gutter_width = PADDING + layout.diff_old_num_width + layout.diff_new_num_width + CELL_WIDTH;
        let gutter_bounds = Bounds {
            origin: origin + gpui::point(Pixels::ZERO, px(PADDING)),
            size: Size {
                width: px(gutter_width),
                height: bounds.size.height - px(PADDING * 2.0),
            },
        };
        window.paint_quad(fill(gutter_bounds, sidebar_color));

        // Calculate old/new line number column positions
        let old_num_x = PADDING;
        let new_num_x = PADDING + layout.diff_old_num_width;
        let prefix_x = PADDING + layout.diff_old_num_width + layout.diff_new_num_width;
        let content_x = prefix_x + CELL_WIDTH;

        // Calculate format widths
        let old_num_chars = (layout.diff_old_num_width / CELL_WIDTH) as usize - 1;
        let new_num_chars = (layout.diff_new_num_width / CELL_WIDTH) as usize - 1;

        for (idx, line) in layout.diff_mode_lines.iter().enumerate() {
            let y = origin.y + px(PADDING + (idx as f32 * CELL_HEIGHT));

            match line {
                DiffVisibleLine::Line { old_num, new_num, tag, content } => {
                    // Paint line background based on tag
                    let (line_bg, prefix, prefix_color) = match tag {
                        DiffLineTag::Insert => (Some(added_bg), "+", added_color),
                        DiffLineTag::Delete => (Some(deleted_bg), "-", deleted_color),
                        DiffLineTag::Equal => (None, " ", muted_foreground_color),
                    };

                    // Paint background for the entire line
                    if let Some(bg) = line_bg {
                        let line_bounds = Bounds {
                            origin: gpui::point(origin.x, y),
                            size: Size {
                                width: bounds.size.width,
                                height: px(CELL_HEIGHT),
                            },
                        };
                        window.paint_quad(fill(line_bounds, bg));
                    }

                    // Paint old line number
                    let old_num_str = old_num
                        .map(|n| format!("{:>width$}", n, width = old_num_chars))
                        .unwrap_or_else(|| " ".repeat(old_num_chars));
                    let old_num_run = TextRun {
                        len: old_num_str.len(),
                        font: font.clone(),
                        color: muted_foreground_color,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    let shaped_old = window.text_system().shape_line(
                        old_num_str.into(),
                        font_size,
                        &[old_num_run],
                        None,
                    );
                    let _ = shaped_old.paint(
                        gpui::point(origin.x + px(old_num_x), y),
                        px(CELL_HEIGHT),
                        window,
                        cx,
                    );

                    // Paint new line number
                    let new_num_str = new_num
                        .map(|n| format!("{:>width$}", n, width = new_num_chars))
                        .unwrap_or_else(|| " ".repeat(new_num_chars));
                    let new_num_run = TextRun {
                        len: new_num_str.len(),
                        font: font.clone(),
                        color: muted_foreground_color,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    let shaped_new = window.text_system().shape_line(
                        new_num_str.into(),
                        font_size,
                        &[new_num_run],
                        None,
                    );
                    let _ = shaped_new.paint(
                        gpui::point(origin.x + px(new_num_x), y),
                        px(CELL_HEIGHT),
                        window,
                        cx,
                    );

                    // Paint prefix (+/-/space)
                    let prefix_run = TextRun {
                        len: prefix.len(),
                        font: font.clone(),
                        color: prefix_color,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    let shaped_prefix = window.text_system().shape_line(
                        prefix.into(),
                        font_size,
                        &[prefix_run],
                        None,
                    );
                    let _ = shaped_prefix.paint(
                        gpui::point(origin.x + px(prefix_x), y),
                        px(CELL_HEIGHT),
                        window,
                        cx,
                    );

                    // Paint content (clipped to content area)
                    if !content.is_empty() {
                        let content_area_bounds = Bounds {
                            origin: origin + gpui::point(px(content_x), Pixels::ZERO),
                            size: Size {
                                width: bounds.size.width - px(content_x),
                                height: bounds.size.height,
                            },
                        };
                        window.with_content_mask(Some(ContentMask { bounds: content_area_bounds }), |window| {
                            let content_run = TextRun {
                                len: content.len(),
                                font: font.clone(),
                                color: foreground_color,
                                background_color: None,
                                underline: None,
                                strikethrough: None,
                            };
                            let shaped_content = window.text_system().shape_line(
                                content.clone().into(),
                                font_size,
                                &[content_run],
                                None,
                            );
                            let _ = shaped_content.paint(
                                gpui::point(origin.x + px(content_x - layout.scroll_x), y),
                                px(CELL_HEIGHT),
                                window,
                                cx,
                            );
                        });
                    }
                }
                DiffVisibleLine::Collapsed { count } => {
                    // Paint collapsed section indicator
                    let collapsed_text = format!("··· {} lines ···", count);
                    let collapsed_run = TextRun {
                        len: collapsed_text.len(),
                        font: font.clone(),
                        color: muted_foreground_color,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    let shaped = window.text_system().shape_line(
                        collapsed_text.into(),
                        font_size,
                        &[collapsed_run],
                        None,
                    );

                    // Center the collapsed indicator
                    let text_width = shaped.width;
                    let center_x = (f32::from(bounds.size.width) - f32::from(text_width)) / 2.0;

                    // Paint a subtle background
                    let collapsed_bounds = Bounds {
                        origin: gpui::point(origin.x, y),
                        size: Size {
                            width: bounds.size.width,
                            height: px(CELL_HEIGHT),
                        },
                    };
                    window.paint_quad(fill(collapsed_bounds, sidebar_color));

                    let _ = shaped.paint(
                        gpui::point(origin.x + px(center_x), y),
                        px(CELL_HEIGHT),
                        window,
                        cx,
                    );
                }
            }
        }
    }
}
