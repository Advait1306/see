//! Terminal view - main view struct and rendering

use super::element::TerminalElement;
use super::input::key_to_input;
use crate::commands::{SendShiftTabToTerminal, SendTabToTerminal};
use crate::constants::{CELL_HEIGHT, CELL_WIDTH, PADDING};
use crate::terminal::Terminal;
use crate::types::SelectionPhase;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point as AlacPoint, Side};
use alacritty_terminal::selection::SelectionType;
use gpui::prelude::*;
use gpui::*;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

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

    /// Get the working directory this terminal was started in
    pub fn cwd(&self) -> PathBuf {
        self.terminal.lock().working_directory().clone()
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
            .on_action(cx.listener(|this, _: &SendTabToTerminal, _window, cx| {
                this.write("\t");
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SendShiftTabToTerminal, _window, cx| {
                this.write("\x1b[Z"); // Escape sequence for shift-tab
                cx.notify();
            }))
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
