use crate::terminal::Terminal;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
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

fn rgb_color(r: u8, g: u8, b: u8) -> Hsla {
    Rgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
    .into()
}

fn ansi_to_hsla(color: AnsiColor) -> Option<Hsla> {
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
                NamedColor::Foreground => return None,
                NamedColor::Background => return None,
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
                let idx = idx - 16;
                let r = if idx / 36 > 0 { (idx / 36) * 40 + 55 } else { 0 };
                let g = if (idx / 6) % 6 > 0 { ((idx / 6) % 6) * 40 + 55 } else { 0 };
                let b = if idx % 6 > 0 { (idx % 6) * 40 + 55 } else { 0 };
                Some(rgb_color(r, g, b))
            } else {
                let gray = (idx - 232) * 10 + 8;
                Some(rgb_color(gray, gray, gray))
            }
        }
    }
}

use crate::ui::app_view::SIDEBAR_WIDTH;

// Terminal dimensions
const CELL_WIDTH: f32 = 7.8;
const CELL_HEIGHT: f32 = 18.0;
const PADDING: f32 = 8.0;
const TERMINAL_TAB_HEIGHT: f32 = 32.0;
const TITLE_BAR_HEIGHT: f32 = 38.0;

// Default colors
fn default_fg() -> Hsla {
    rgb_color(0xcd, 0xd6, 0xf4)
}

fn default_bg() -> Hsla {
    rgb_color(0x1e, 0x1e, 0x2e)
}

fn cursor_color() -> Hsla {
    rgb_color(0xcd, 0xd6, 0xf4)
}

fn cursor_unfocused_color() -> Hsla {
    rgb_color(0x6c, 0x70, 0x86)
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
            family: "Menlo".into(),
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

// Background rectangle
struct BackgroundRect {
    line: i32,
    col: usize,
    cell_count: usize,
    color: Hsla,
}

impl BackgroundRect {
    fn paint(&self, origin: gpui::Point<Pixels>, cell_width: Pixels, line_height: Pixels, window: &mut Window) {
        let pos = gpui::Point::new(
            origin.x + px(self.col as f32 * f32::from(cell_width)),
            origin.y + px(self.line as f32 * f32::from(line_height)),
        );
        let bounds = Bounds::new(
            pos,
            Size {
                width: cell_width * self.cell_count as f32,
                height: line_height,
            },
        );
        window.paint_quad(fill(bounds, self.color));
    }
}

pub struct TerminalView {
    terminal: Arc<parking_lot::Mutex<Terminal>>,
    focus_handle: FocusHandle,
    scroll_accumulator: f32,
    last_size: Option<(u16, u16)>,
    bounds_observer_set: bool,
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
            last_size: None,
            bounds_observer_set: false,
        }
    }

    fn do_resize(&mut self, window: &Window) {
        let viewport = window.viewport_size();
        // Account for sidebar width, title bar, and terminal tab height
        let available_width = f32::from(viewport.width) - SIDEBAR_WIDTH - (PADDING * 2.0);
        let available_height = f32::from(viewport.height) - TITLE_BAR_HEIGHT - TERMINAL_TAB_HEIGHT - (PADDING * 2.0);
        let cols = (available_width / CELL_WIDTH).floor().max(1.0) as u16;
        let rows = (available_height / CELL_HEIGHT).floor().max(1.0) as u16;

        let new_size = (cols, rows);
        if self.last_size != Some(new_size) {
            self.last_size = Some(new_size);
            self.terminal.lock().resize(cols, rows, CELL_WIDTH as u16, CELL_HEIGHT as u16);
        }
    }
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus_handle = self.focus_handle.clone();
        let is_focused = focus_handle.is_focused(window);

        // Set up bounds observer on first render
        if !self.bounds_observer_set {
            self.bounds_observer_set = true;
            self.do_resize(window);
            cx.observe_window_bounds(window, |this: &mut Self, window, cx| {
                this.do_resize(window);
                cx.notify();
            })
            .detach();
        }

        // Extract terminal content for the element
        let terminal = self.terminal.clone();

        div()
            .id("terminal-wrapper")
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
            .size_full()
            .child(TerminalElement {
                terminal,
                is_focused,
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
}

struct TerminalLayoutState {
    text_runs: Vec<BatchedTextRun>,
    cursor_rect: Option<BackgroundRect>,
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
        _cx: &mut App,
    ) -> Self::PrepaintState {
        let terminal = self.terminal.lock();
        let mut text_runs: Vec<BatchedTextRun> = Vec::new();
        let mut cursor_rect: Option<BackgroundRect> = None;

        terminal.with_term(|term| {
            let grid = term.grid();
            let cols = grid.columns();
            let content = term.renderable_content();
            let display_offset = content.display_offset as i32;
            let cursor_point = content.cursor.point;

            for line_idx in 0..grid.screen_lines() {
                let grid_line = Line(line_idx as i32 - display_offset);
                let row = &grid[grid_line];
                let mut current_run: Option<BatchedTextRun> = None;

                for col_idx in 0..cols {
                    let cell = &row[Column(col_idx)];

                    if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
                        continue;
                    }

                    let c = if cell.c == '\0' { ' ' } else { cell.c };
                    let is_cursor = display_offset == 0
                        && cursor_point.line.0 == line_idx as i32
                        && cursor_point.column.0 == col_idx;

                    let (fg, bg) = if is_cursor {
                        // Cursor: inverted colors
                        let cursor_bg = if self.is_focused {
                            cursor_color()
                        } else {
                            cursor_unfocused_color()
                        };
                        cursor_rect = Some(BackgroundRect {
                            line: line_idx as i32,
                            col: col_idx,
                            cell_count: 1,
                            color: cursor_bg,
                        });
                        (default_bg(), None) // Text on cursor is dark
                    } else {
                        let fg = ansi_to_hsla(cell.fg).unwrap_or_else(default_fg);
                        let bg = ansi_to_hsla(cell.bg);
                        (fg, bg)
                    };

                    let bold = cell.flags.contains(CellFlags::BOLD);

                    // Try to extend current run or start a new one
                    if let Some(ref mut run) = current_run {
                        if run.can_append(fg, bg, bold) && !is_cursor {
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

        TerminalLayoutState { text_runs, cursor_rect }
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
        window.paint_quad(fill(bounds, default_bg()));

        // Paint cursor background
        if let Some(ref cursor) = layout.cursor_rect {
            cursor.paint(origin, cell_width, line_height, window);
        }

        // Paint text runs
        for run in &layout.text_runs {
            run.paint(origin, cell_width, line_height, window, cx);
        }
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
