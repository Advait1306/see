use crate::editor::Buffer;
use crate::terminal::Terminal;
use crate::ui::EditorView;
use crate::ui::TerminalView;
use gpui::prelude::*;
use gpui::*;
use gpui_component::theme::ActiveTheme;
use serde::{Deserialize, Serialize};
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

// =============================================================================
// Tab Trait and Config Types
// =============================================================================

/// Serializable config for a terminal tab
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TerminalTabConfig {
    pub cwd: PathBuf,
}

/// Serializable config for an editor tab
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EditorTabConfig {
    pub path: PathBuf,
    // Future: cursor position, scroll offset, etc.
}

/// Serializable state for a single tab (tagged union for JSON)
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TabConfig {
    Terminal(TerminalTabConfig),
    Editor(EditorTabConfig),
}

// =============================================================================
// TabItem Enum
// =============================================================================

/// Represents a tab item which can be either a terminal or an editor
#[derive(Clone)]
pub enum TabItem {
    Terminal(Entity<TerminalView>),
    Editor(Entity<EditorView>),
}

impl TabItem {
    pub fn label(&self, cx: &App) -> String {
        match self {
            TabItem::Terminal(_) => "Terminal".to_string(),
            TabItem::Editor(editor) => {
                let editor_view = editor.read(cx);
                let buffer = editor_view.buffer().read(cx);
                let name = buffer.file_name();
                if buffer.is_dirty() {
                    format!("{}*", name)
                } else {
                    name
                }
            }
        }
    }

    /// Serialize any tab to its config
    pub fn to_config(&self, cx: &App) -> TabConfig {
        match self {
            TabItem::Terminal(t) => {
                let cwd = t.read(cx).cwd();
                TabConfig::Terminal(TerminalTabConfig { cwd })
            }
            TabItem::Editor(e) => {
                let path = e.read(cx).buffer().read(cx).file_path().clone();
                TabConfig::Editor(EditorTabConfig { path })
            }
        }
    }

}

#[derive(Clone)]
pub struct DraggedTab {
    pub pane: Entity<Pane>,
    pub tab: TabItem,
    pub index: usize,
}

impl Render for DraggedTab {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let label = self.tab.label(cx);
        let theme = cx.theme();
        div()
            .px_3()
            .py_1()
            .bg(theme.border)
            .border_1()
            .border_color(theme.list_active)
            .rounded_md()
            .text_color(theme.foreground)
            .text_xs()
            .child(label)
    }
}

pub struct Pane {
    pub tabs: Vec<TabItem>,
    pub active_index: usize,
    pub focus_handle: FocusHandle,
    pub path: PathBuf,
    terminal_counter: usize, // For naming terminals
}

impl Pane {
    pub fn new(path: PathBuf, cx: &mut Context<Self>) -> Self {
        Self {
            tabs: Vec::new(),
            active_index: 0,
            focus_handle: cx.focus_handle(),
            path,
            terminal_counter: 0,
        }
    }

    pub fn add_terminal(&mut self, cx: &mut Context<Self>) {
        if let Ok(terminal) = Terminal::new(self.path.clone()) {
            let terminal = Arc::new(parking_lot::Mutex::new(terminal));
            let terminal_view = cx.new(|cx| TerminalView::new(terminal, cx));
            self.tabs.push(TabItem::Terminal(terminal_view));
            self.terminal_counter += 1;
            self.active_index = self.tabs.len() - 1;
            cx.notify();
        }
    }

    pub fn add_editor(&mut self, buffer: Entity<Buffer>, file_path: PathBuf, cx: &mut Context<Self>) {
        let editor_view = cx.new(|cx| EditorView::new(buffer, file_path, cx));
        self.tabs.push(TabItem::Editor(editor_view));
        self.active_index = self.tabs.len() - 1;
        cx.notify();
    }

    pub fn add_tab(&mut self, tab: TabItem, cx: &mut Context<Self>) {
        if matches!(tab, TabItem::Terminal(_)) {
            self.terminal_counter += 1;
        }
        self.tabs.push(tab);
        self.active_index = self.tabs.len() - 1;
        cx.notify();
    }

    pub fn remove_tab(&mut self, index: usize, cx: &mut Context<Self>) -> Option<TabItem> {
        if index < self.tabs.len() {
            let removed = self.tabs.remove(index);
            if self.active_index >= self.tabs.len() && !self.tabs.is_empty() {
                self.active_index = self.tabs.len() - 1;
            }
            cx.notify();
            Some(removed)
        } else {
            None
        }
    }

    pub fn active_tab(&self) -> Option<&TabItem> {
        self.tabs.get(self.active_index)
    }

    /// Returns the count of terminal tabs (for state serialization)
    pub fn terminal_count(&self) -> usize {
        self.tabs
            .iter()
            .filter(|tab| matches!(tab, TabItem::Terminal(_)))
            .count()
    }

    /// Returns the file paths of open editor tabs (for state serialization)
    pub fn open_file_paths(&self, cx: &App) -> Vec<PathBuf> {
        self.tabs
            .iter()
            .filter_map(|tab| {
                if let TabItem::Editor(editor) = tab {
                    let editor_view = editor.read(cx);
                    Some(editor_view.buffer().read(cx).file_path().clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn select_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.tabs.len() {
            self.active_index = index;
            cx.notify();
        }
    }

    fn render_tab_bar(&self, cx: &Context<Self>) -> impl IntoElement {
        let pane_entity = cx.entity().clone();
        let mut terminal_idx = 0usize;
        let theme = cx.theme();
        let tab_bar_bg = theme.tab_bar;
        let border_color = theme.border;
        let background_color = theme.background;
        let foreground_color = theme.foreground;
        let muted_color = theme.muted_foreground;

        div()
            .flex()
            .h(px(32.0))
            .bg(tab_bar_bg)
            .border_b_1()
            .border_color(border_color)
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
                let tab = dragged.tab.clone();
                let target_pane = cx.entity().clone();

                log::info!("  Moving tab from source to target");
                cx.defer(move |cx| {
                    log::info!("  Deferred tab move executing");
                    // Remove from source pane and notify it to check if empty
                    source_pane.update(cx, |source, cx| {
                        source.remove_tab(source_index, cx);
                        cx.emit(PaneEvent::TabClosed);
                    });
                    // Add to target pane
                    target_pane.update(cx, |target, cx| {
                        target.add_tab(tab, cx);
                        cx.emit(PaneEvent::TabMoved);
                    });
                });
            }))
            .children(self.tabs.iter().enumerate().map(|(idx, tab)| {
                let is_active = idx == self.active_index;
                let pane_for_drag = pane_entity.clone();
                let tab_clone = tab.clone();

                // Generate tab label
                let label = match tab {
                    TabItem::Terminal(_) => {
                        terminal_idx += 1;
                        format!("Terminal {}", terminal_idx)
                    }
                    TabItem::Editor(editor) => {
                        let editor_view = editor.read(cx);
                        let buffer = editor_view.buffer().read(cx);
                        let name = buffer.file_name();
                        if buffer.is_dirty() {
                            format!("{}*", name)
                        } else {
                            name
                        }
                    }
                };

                div()
                    .id(ElementId::Name(format!("pane-tab-{}", idx).into()))
                    .px_3()
                    .py_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .when(is_active, |el| el.bg(background_color))
                    .when(!is_active, |el| el.hover(|el| el.bg(border_color)))
                    .text_color(if is_active {
                        foreground_color
                    } else {
                        muted_color
                    })
                    .text_xs()
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        this.select_tab(idx, cx);
                    }))
                    .on_drag(
                        DraggedTab {
                            pane: pane_for_drag.clone(),
                            tab: tab_clone.clone(),
                            index: idx,
                        },
                        |dragged, _, _window, cx| cx.new(|_| dragged.clone()),
                    )
                    .child(label)
            }))
            .child(
                div()
                    .id("pane-add-terminal")
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(|el| el.bg(border_color))
                    .text_color(muted_color)
                    .text_xs()
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.add_terminal(cx);
                        cx.emit(PaneEvent::TerminalAdded);
                    }))
                    .child("+"),
            )
    }

    fn render_drop_zones(&self, cx: &Context<Self>) -> impl IntoElement {
        // Use theme primary color with alpha for drop zones
        let mut drop_color = cx.theme().primary;
        drop_color.a = 0.3;

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
                    })),
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
                    })),
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
                    })),
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
                    })),
            )
    }

    fn handle_split_drop(
        &mut self,
        dragged: &DraggedTab,
        direction: SplitDirection,
        cx: &mut Context<Self>,
    ) {
        let path = self.path.clone();
        let tab = dragged.tab.clone();
        let source_pane = dragged.pane.clone();
        let source_index = dragged.index;
        let target_pane = cx.entity().clone();

        log::info!("handle_split_drop: direction={:?}", direction);

        cx.defer(move |cx| {
            // Remove tab from source pane and notify it to check if empty
            source_pane.update(cx, |pane, cx| {
                pane.remove_tab(source_index, cx);
                // Emit event so PaneGroup can check if pane is now empty
                cx.emit(PaneEvent::TabClosed);
            });

            // Create new pane with the tab
            let new_pane = cx.new(|cx| {
                let mut pane = Pane::new(path, cx);
                pane.add_tab(tab, cx);
                pane
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
    TabMoved,
    TerminalAdded,
    TabClosed,
    Focus,
}

impl EventEmitter<PaneEvent> for Pane {}

impl Render for Pane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus_handle = self.focus_handle.clone();
        let active_tab = self.active_tab().cloned();
        let theme = cx.theme();

        div()
            .id("pane")
            .key_context("Pane")
            .track_focus(&focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .relative()
            .bg(theme.background)
            .border_1()
            .border_color(theme.border)
            .on_mouse_down(MouseButton::Left, cx.listener(|_this, _, _window, cx| {
                cx.emit(PaneEvent::Focus);
            }))
            .child(self.render_tab_bar(cx))
            .child(
                div()
                    .id("pane-content-container")
                    .flex_1()
                    .w_full()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .relative()
                    .map(|el| match active_tab {
                        Some(TabItem::Terminal(terminal)) => el.child(terminal),
                        Some(TabItem::Editor(editor)) => el.child(editor),
                        None => el,
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
