//! ANSI color conversion for terminal rendering

use crate::constants::{ANSI_COLORS, rgb_to_hsla};
use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor};
use gpui::Hsla;

/// Convert ANSI color to GPUI Hsla color
pub(crate) fn ansi_to_hsla(color: AnsiColor, default_fg: Hsla, default_bg: Hsla) -> Option<Hsla> {
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
