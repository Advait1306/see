use crate::terminal::Terminal;
use crate::ui::TerminalView;
use gpui::prelude::*;
use gpui::*;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Up,
    Down,
    Left,
    Right,
}

impl SplitDirection {
    pub fn axis(&self) -> Axis {
        match self {
            SplitDirection::Up | SplitDirection::Down => Axis::Vertical,
            SplitDirection::Left | SplitDirection::Right => Axis::Horizontal,
        }
    }

    pub fn is_before(&self) -> bool {
        matches!(self, SplitDirection::Up | SplitDirection::Left)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Clone)]
pub struct DraggedTab {
    pub pane: Entity<Pane>,
    pub terminal: Entity<TerminalView>,
    pub index: usize,
}

impl Render for DraggedTab {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py_1()
            .bg(rgb(0x313244))
            .border_1()
            .border_color(rgb(0x45475a))
            .rounded_md()
            .text_color(rgb(0xcdd6f4))
            .text_xs()
            .child(format!("Terminal {}", self.index + 1))
    }
}

pub struct Pane {
    pub terminals: Vec<Entity<TerminalView>>,
    pub active_index: usize,
    pub focus_handle: FocusHandle,
    pub path: PathBuf,
}

impl Pane {
    pub fn new(path: PathBuf, cx: &mut Context<Self>) -> Self {
        Self {
            terminals: Vec::new(),
            active_index: 0,
            focus_handle: cx.focus_handle(),
            path,
        }
    }

    pub fn with_terminal(mut self, terminal: Entity<TerminalView>) -> Self {
        self.terminals.push(terminal);
        self
    }

    pub fn add_terminal(&mut self, cx: &mut Context<Self>) {
        if let Ok(terminal) = Terminal::new(self.path.clone()) {
            let terminal = Arc::new(parking_lot::Mutex::new(terminal));
            let terminal_view = cx.new(|cx| TerminalView::new(terminal, cx));
            self.terminals.push(terminal_view);
            self.active_index = self.terminals.len() - 1;
            cx.notify();
        }
    }

    pub fn add_terminal_view(&mut self, terminal: Entity<TerminalView>, cx: &mut Context<Self>) {
        self.terminals.push(terminal);
        self.active_index = self.terminals.len() - 1;
        cx.notify();
    }

    pub fn remove_terminal(&mut self, index: usize, cx: &mut Context<Self>) -> Option<Entity<TerminalView>> {
        if index < self.terminals.len() {
            let removed = self.terminals.remove(index);
            if self.active_index >= self.terminals.len() && !self.terminals.is_empty() {
                self.active_index = self.terminals.len() - 1;
            }
            cx.notify();
            Some(removed)
        } else {
            None
        }
    }

    pub fn active_terminal(&self) -> Option<&Entity<TerminalView>> {
        self.terminals.get(self.active_index)
    }

    pub fn select_terminal(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.terminals.len() {
            self.active_index = index;
            cx.notify();
        }
    }

    fn render_tab_bar(&self, cx: &Context<Self>) -> impl IntoElement {
        let pane_entity = cx.entity().clone();

        div()
            .flex()
            .h(px(32.0))
            .bg(rgb(0x11111b))
            .border_b_1()
            .border_color(rgb(0x313244))
            .items_center()
            .px_2()
            .gap_1()
            .on_drop(cx.listener(move |_this, dragged: &DraggedTab, _window, cx| {
                log::info!("Tab bar on_drop triggered");
                // Drop on tab bar = move tab to this pane (no split)
                if dragged.pane == cx.entity() {
                    log::info!("  Same pane, returning");
                    // Same pane - reordering not implemented yet
                    return;
                }
                // Defer the update to avoid re-entrancy
                let source_pane = dragged.pane.clone();
                let source_index = dragged.index;
                let terminal = dragged.terminal.clone();
                let target_pane = cx.entity().clone();

                log::info!("  Moving tab from source to target");
                cx.defer(move |cx| {
                    log::info!("  Deferred tab move executing");
                    // Remove from source pane and notify it to check if empty
                    source_pane.update(cx, |source, cx| {
                        source.remove_terminal(source_index, cx);
                        cx.emit(PaneEvent::TerminalClosed);
                    });
                    // Add to target pane
                    target_pane.update(cx, |target, cx| {
                        target.add_terminal_view(terminal, cx);
                        cx.emit(PaneEvent::TabMoved);
                    });
                });
            }))
            .children(self.terminals.iter().enumerate().map(|(idx, terminal)| {
                let is_active = idx == self.active_index;
                let pane_for_drag = pane_entity.clone();
                let terminal_clone = terminal.clone();

                div()
                    .id(ElementId::Name(format!("pane-tab-{}", idx).into()))
                    .px_3()
                    .py_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .when(is_active, |el| el.bg(rgb(0x1e1e2e)))
                    .when(!is_active, |el| el.hover(|el| el.bg(rgb(0x313244))))
                    .text_color(if is_active {
                        rgb(0xcdd6f4)
                    } else {
                        rgb(0x6c7086)
                    })
                    .text_xs()
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        this.select_terminal(idx, cx);
                    }))
                    .on_drag(DraggedTab {
                        pane: pane_for_drag.clone(),
                        terminal: terminal_clone.clone(),
                        index: idx,
                    }, |dragged, _, _window, cx| {
                        cx.new(|_| dragged.clone())
                    })
                    .child(format!("Terminal {}", idx + 1))
            }))
            .child(
                div()
                    .id("pane-add-terminal")
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(|el| el.bg(rgb(0x313244)))
                    .text_color(rgb(0x6c7086))
                    .text_xs()
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.add_terminal(cx);
                        cx.emit(PaneEvent::TerminalAdded);
                    }))
                    .child("+"),
            )
    }

    fn render_drop_zones(&self, cx: &Context<Self>) -> impl IntoElement {
        let drop_color = rgba(0x89b4fa4d); // Blue with ~30% opacity

        // Create invisible edge zones that show highlight on drag-over
        div()
            .id("drop-zones")
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            // Left edge zone (20% width)
            .child(
                div()
                    .id("drop-zone-left")
                    .absolute()
                    .top_0()
                    .left_0()
                    .bottom_0()
                    .w_1_5()
                    .drag_over::<DraggedTab>(move |style, _, _, _| style.bg(drop_color))
                    .on_drop(cx.listener(|this, dragged: &DraggedTab, _window, cx| {
                        this.handle_split_drop(dragged, SplitDirection::Left, cx);
                    }))
            )
            // Right edge zone (20% width)
            .child(
                div()
                    .id("drop-zone-right")
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .w_1_5()
                    .drag_over::<DraggedTab>(move |style, _, _, _| style.bg(drop_color))
                    .on_drop(cx.listener(|this, dragged: &DraggedTab, _window, cx| {
                        this.handle_split_drop(dragged, SplitDirection::Right, cx);
                    }))
            )
            // Top edge zone (20% height, but not overlapping left/right)
            .child(
                div()
                    .id("drop-zone-top")
                    .absolute()
                    .top_0()
                    .left(relative(0.2))
                    .right(relative(0.2))
                    .h_1_5()
                    .drag_over::<DraggedTab>(move |style, _, _, _| style.bg(drop_color))
                    .on_drop(cx.listener(|this, dragged: &DraggedTab, _window, cx| {
                        this.handle_split_drop(dragged, SplitDirection::Up, cx);
                    }))
            )
            // Bottom edge zone (20% height, but not overlapping left/right)
            .child(
                div()
                    .id("drop-zone-bottom")
                    .absolute()
                    .bottom_0()
                    .left(relative(0.2))
                    .right(relative(0.2))
                    .h_1_5()
                    .drag_over::<DraggedTab>(move |style, _, _, _| style.bg(drop_color))
                    .on_drop(cx.listener(|this, dragged: &DraggedTab, _window, cx| {
                        this.handle_split_drop(dragged, SplitDirection::Down, cx);
                    }))
            )
    }

    fn handle_split_drop(&mut self, dragged: &DraggedTab, direction: SplitDirection, cx: &mut Context<Self>) {
        let path = self.path.clone();
        let terminal = dragged.terminal.clone();
        let source_pane = dragged.pane.clone();
        let source_index = dragged.index;
        let target_pane = cx.entity().clone();

        log::info!("handle_split_drop: direction={:?}", direction);

        cx.defer(move |cx| {
            // Remove terminal from source pane and notify it to check if empty
            source_pane.update(cx, |pane, cx| {
                pane.remove_terminal(source_index, cx);
                // Emit event so PaneGroup can check if pane is now empty
                cx.emit(PaneEvent::TerminalClosed);
            });

            // Create new pane with the terminal
            let new_pane = cx.new(|cx| {
                Pane::new(path, cx).with_terminal(terminal)
            });

            target_pane.update(cx, |_, cx| {
                cx.emit(PaneEvent::Split { direction, new_pane });
            });
        });
    }
}

pub enum PaneEvent {
    Split {
        direction: SplitDirection,
        new_pane: Entity<Pane>,
    },
    Close,
    TabMoved,
    TerminalAdded,
    TerminalClosed,
    Focus,
}

impl EventEmitter<PaneEvent> for Pane {}

impl Render for Pane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus_handle = self.focus_handle.clone();
        let active_terminal = self.active_terminal().cloned();

        div()
            .id("pane")
            .key_context("Pane")
            .track_focus(&focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .relative()
            .bg(rgb(0x1e1e2e))
            .border_1()
            .border_color(rgb(0x313244))
            .on_mouse_down(MouseButton::Left, cx.listener(|_this, _, _window, cx| {
                cx.emit(PaneEvent::Focus);
            }))
            .child(self.render_tab_bar(cx))
            .child(
                div()
                    .id("pane-terminal-container")
                    .flex_1()
                    .w_full()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .relative()
                    .map(|el| {
                        if let Some(terminal) = active_terminal {
                            el.child(terminal)
                        } else {
                            el
                        }
                    })
                    .child(self.render_drop_zones(cx)),
            )
    }
}

impl Focusable for Pane {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
