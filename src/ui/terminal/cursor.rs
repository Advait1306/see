//! Cursor rendering for terminal

use alacritty_terminal::index::Point as AlacPoint;
use gpui::{
    fill, outline, point, px, App, BorderStyle, Bounds, Font, FontFeatures, FontStyle, FontWeight,
    Hsla, Pixels, Point, Size, TextRun, Window,
};

/// Helper struct for converting between Alacritty's cursor points and screen coordinates
pub(crate) struct DisplayCursor {
    line: i32,
    col: usize,
}

impl DisplayCursor {
    pub(crate) fn from(cursor_point: AlacPoint, display_offset: usize) -> Self {
        Self {
            line: cursor_point.line.0 + display_offset as i32,
            col: cursor_point.column.0,
        }
    }

    pub(crate) fn line(&self) -> i32 {
        self.line
    }

    pub(crate) fn col(&self) -> usize {
        self.col
    }
}

/// Cursor shape for rendering
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CursorShape {
    Block,
    Underline,
    Bar,
    Hollow,
}

/// Layout information for cursor rendering
pub(crate) struct CursorLayout {
    pub(crate) origin: Point<Pixels>,
    pub(crate) block_width: Pixels,
    pub(crate) line_height: Pixels,
    pub(crate) color: Hsla,
    pub(crate) text_color: Hsla, // Color for text on block cursor
    pub(crate) shape: CursorShape,
    pub(crate) cursor_char: Option<char>,
}

impl CursorLayout {
    pub(crate) fn new(
        origin: Point<Pixels>,
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

    pub(crate) fn bounds(&self, content_origin: Point<Pixels>) -> Bounds<Pixels> {
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
                origin: origin + point(Pixels::ZERO, self.line_height - px(2.0)),
                size: Size {
                    width: self.block_width,
                    height: px(2.0),
                },
            },
        }
    }

    pub(crate) fn paint(&self, content_origin: Point<Pixels>, window: &mut Window, cx: &mut App) {
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
        if self.shape == CursorShape::Block
            && let Some(c) = self.cursor_char
                && c != ' ' && c != '\0' {
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
