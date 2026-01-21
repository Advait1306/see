use crate::terminal::Terminal;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::cell::Flags as CellFlags;
use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor};
use gpui::prelude::*;
use gpui::*;
use std::sync::Arc;
use std::time::Duration;

// ANSI 256 color palette
const ANSI_COLORS: [(u8, u8, u8); 16] = [
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

fn rgb_color(r: u8, g: u8, b: u8) -> Rgba {
    Rgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
}

fn ansi_to_rgb(color: AnsiColor) -> Option<Rgba> {
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
                NamedColor::Foreground => return None, // Use default
                NamedColor::Background => return None, // Use default
                _ => return None,
            };
            let (r, g, b) = ANSI_COLORS[idx];
            Some(rgb_color(r, g, b))
        }
        AnsiColor::Spec(c) => Some(rgb_color(c.r, c.g, c.b)),
        AnsiColor::Indexed(idx) => {
            if idx < 16 {
                let (r, g, b) = ANSI_COLORS[idx as usize];
                Some(rgb_color(r, g, b))
            } else if idx < 232 {
                // 216 color cube (6x6x6)
                let idx = idx - 16;
                let r = if idx / 36 > 0 { (idx / 36) * 40 + 55 } else { 0 };
                let g = if (idx / 6) % 6 > 0 { ((idx / 6) % 6) * 40 + 55 } else { 0 };
                let b = if idx % 6 > 0 { (idx % 6) * 40 + 55 } else { 0 };
                Some(rgb_color(r, g, b))
            } else {
                // Grayscale (24 shades)
                let gray = (idx - 232) * 10 + 8;
                Some(rgb_color(gray, gray, gray))
            }
        }
    }
}

pub struct TerminalView {
    terminal: Arc<parking_lot::Mutex<Terminal>>,
    focus_handle: FocusHandle,
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

                // Check if terminal has new output
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
        }
    }
}

#[derive(Clone)]
struct StyledCell {
    c: char,
    fg: Option<Rgba>,
    bg: Option<Rgba>,
    bold: bool,
}

struct TerminalLine {
    cells: Vec<StyledCell>,
    cursor_col: Option<usize>,
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let terminal = self.terminal.lock();

        // Build lines with color info
        let mut lines: Vec<TerminalLine> = Vec::new();
        let mut cursor_point: Option<Point> = None;

        terminal.with_term(|term| {
            let grid = term.grid();
            let cols = grid.columns();
            let content = term.renderable_content();
            cursor_point = Some(content.cursor.point);

            for line_idx in 0..grid.screen_lines() {
                let mut cells: Vec<StyledCell> = Vec::with_capacity(cols);
                let row = &grid[Line(line_idx as i32)];

                for col_idx in 0..cols {
                    let cell = &row[Column(col_idx)];

                    // Handle wide characters and placeholders
                    if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
                        continue;
                    }

                    let c = if cell.c == '\0' { ' ' } else { cell.c };
                    let fg = ansi_to_rgb(cell.fg);
                    let bg = ansi_to_rgb(cell.bg);
                    let bold = cell.flags.contains(CellFlags::BOLD);

                    cells.push(StyledCell { c, fg, bg, bold });
                }

                // Determine if cursor is on this line
                let cursor_col = cursor_point.and_then(|cp| {
                    if cp.line.0 == line_idx as i32 {
                        Some(cp.column.0)
                    } else {
                        None
                    }
                });

                lines.push(TerminalLine { cells, cursor_col });
            }
        });

        drop(terminal); // Release lock before building UI

        let focus_handle = self.focus_handle.clone();
        let is_focused = focus_handle.is_focused(window);

        div()
            .id("terminal")
            .key_context("Terminal")
            .track_focus(&focus_handle)
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _window, cx| {
                let input = key_to_input(event);
                if !input.is_empty() {
                    this.terminal.lock().write(input.as_bytes());
                    cx.notify();
                }
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(|_this, _, window, cx| {
                cx.focus_self(window);
            }))
            .size_full()
            .bg(rgb(0x1e1e2e))
            .p_2()
            .font_family("Menlo")
            .text_size(px(13.0))
            .text_color(rgb(0xcdd6f4))
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_0()
                    .children(lines.into_iter().enumerate().map(|(_line_idx, line)| {
                        render_terminal_line(line, is_focused)
                    })),
            )
    }
}

// Character width for Menlo 13px (monospace)
const CHAR_WIDTH: f32 = 7.8;

fn render_terminal_line(line: TerminalLine, is_focused: bool) -> Div {
    // If line is empty, add a space
    if line.cells.is_empty() {
        return div().line_height(px(18.0)).child(" ");
    }

    // Render each character in a fixed-width cell for proper alignment
    div()
        .line_height(px(18.0))
        .flex()
        .flex_row()
        .children(line.cells.into_iter().enumerate().map(|(col_idx, cell)| {
            let is_cursor = line.cursor_col == Some(col_idx);

            let mut el = div()
                .w(px(CHAR_WIDTH))
                .flex_shrink_0()
                .child(cell.c.to_string());

            if is_cursor {
                el = el
                    .bg(if is_focused {
                        rgb(0xcdd6f4)
                    } else {
                        rgb(0x6c7086)
                    })
                    .text_color(rgb(0x1e1e2e));
            } else {
                if let Some(fg_color) = cell.fg {
                    el = el.text_color(fg_color);
                }
                if let Some(bg_color) = cell.bg {
                    el = el.bg(bg_color);
                }
            }

            if cell.bold {
                el = el.font_weight(FontWeight::BOLD);
            }

            el
        }))
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

fn key_to_input(event: &KeyDownEvent) -> String {
    let key = &event.keystroke.key;
    let modifiers = &event.keystroke;

    if modifiers.modifiers.control {
        if key.len() == 1 {
            let c = key.chars().next().unwrap();
            if c.is_ascii_alphabetic() {
                let ctrl_char = (c.to_ascii_lowercase() as u8 - b'a' + 1) as char;
                return ctrl_char.to_string();
            }
        }
    }

    match key.as_str() {
        "enter" => "\r".to_string(),
        "backspace" => "\x7f".to_string(),
        "tab" => "\t".to_string(),
        "escape" => "\x1b".to_string(),
        "up" => "\x1b[A".to_string(),
        "down" => "\x1b[B".to_string(),
        "right" => "\x1b[C".to_string(),
        "left" => "\x1b[D".to_string(),
        "home" => "\x1b[H".to_string(),
        "end" => "\x1b[F".to_string(),
        "pageup" => "\x1b[5~".to_string(),
        "pagedown" => "\x1b[6~".to_string(),
        "delete" => "\x1b[3~".to_string(),
        "insert" => "\x1b[2~".to_string(),
        "space" => " ".to_string(),
        _ if key.len() == 1 => key.clone(),
        _ => String::new(),
    }
}
