//! Application-specific constants
//!
//! For colors, use `cx.theme()` from gpui-component.

/// Character cell width in pixels (monospace rendering)
pub const CELL_WIDTH: f32 = 7.8;

/// Character cell height in pixels (line height)
pub const CELL_HEIGHT: f32 = 18.0;

/// Standard padding around content areas
pub const PADDING: f32 = 8.0;

/// Width of git diff gutter markers
pub const GUTTER_MARKER_WIDTH: f32 = 3.0;

/// ANSI 16-color palette for terminal
/// Uses Catppuccin Mocha colors for terminal-specific rendering
pub const ANSI_COLORS: [(u8, u8, u8); 16] = [
    (0x1e, 0x1e, 0x2e), // 0: Black (background)
    (0xf3, 0x8b, 0xa8), // 1: Red
    (0xa6, 0xe3, 0xa1), // 2: Green
    (0xf9, 0xe2, 0xaf), // 3: Yellow
    (0x89, 0xb4, 0xfa), // 4: Blue
    (0xf5, 0xc2, 0xe7), // 5: Magenta
    (0x94, 0xe2, 0xd5), // 6: Cyan
    (0xcd, 0xd6, 0xf4), // 7: White (foreground)
    (0x58, 0x5b, 0x70), // 8: Bright Black
    (0xf3, 0x8b, 0xa8), // 9: Bright Red
    (0xa6, 0xe3, 0xa1), // 10: Bright Green
    (0xf9, 0xe2, 0xaf), // 11: Bright Yellow
    (0x89, 0xb4, 0xfa), // 12: Bright Blue
    (0xf5, 0xc2, 0xe7), // 13: Bright Magenta
    (0x94, 0xe2, 0xd5), // 14: Bright Cyan
    (0xcd, 0xd6, 0xf4), // 15: Bright White
];

/// Convert RGB tuple to Hsla
#[inline]
pub fn rgb_to_hsla(r: u8, g: u8, b: u8) -> gpui::Hsla {
    gpui::Rgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
    .into()
}
