//! Batched text rendering for efficient terminal display

use gpui::{
    fill, px, App, Bounds, Font, FontFeatures, FontStyle, FontWeight, Hsla, Pixels, Point, Size,
    TextRun, Window,
};

/// Batched text run - combines consecutive characters with same style
pub(crate) struct BatchedTextRun {
    pub(crate) line: i32,
    pub(crate) col: usize,
    pub(crate) text: String,
    pub(crate) cell_count: usize,
    pub(crate) color: Hsla,
    pub(crate) background: Option<Hsla>,
    pub(crate) bold: bool,
}

impl BatchedTextRun {
    pub(crate) fn new(line: i32, col: usize, c: char, color: Hsla, background: Option<Hsla>, bold: bool) -> Self {
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

    pub(crate) fn can_append(&self, color: Hsla, background: Option<Hsla>, bold: bool) -> bool {
        self.color == color && self.background == background && self.bold == bold
    }

    pub(crate) fn append(&mut self, c: char) {
        self.text.push(c);
        self.cell_count += 1;
    }

    pub(crate) fn paint(&self, origin: Point<Pixels>, cell_width: Pixels, line_height: Pixels, window: &mut Window, cx: &mut App) {
        let pos = Point::new(
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
