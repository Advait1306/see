use crate::stores::TerminalStore;
use crate::types::Tab;
use crate::ui::TerminalView;
use gpui::prelude::*;
use gpui::*;
use gpui_component::theme::ActiveTheme;
use std::path::PathBuf;

use super::{DraggedTab, PaneEvent, SplitDirection, TabItem};

pub struct Pane {
    pub tabs: Vec<TabItem>,
    pub active_index: usize,
    pub focus_handle: FocusHandle,
    pub path: PathBuf,
    terminal_counter: usize,
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
        let terminal_store = TerminalStore::global(cx);
        // TODO(fix): Terminal view is gotten added as an entity which shouldn't be happening
        let result = terminal_store.update(cx, |store, cx| {
            store.create_terminal(self.path.clone(), cx)
        });

        if let Some((_id, terminal)) = result {
            let terminal_view = cx.new(|cx| TerminalView::new(terminal, cx));
            self.tabs.push(TabItem::Terminal(terminal_view));
            self.terminal_counter += 1;
            self.active_index = self.tabs.len() - 1;
            cx.emit(PaneEvent::TabAdded);
            cx.notify();
        }
    }

    pub fn add_tab(&mut self, tab: TabItem, cx: &mut Context<Self>) {
        if matches!(tab, TabItem::Terminal(_)) {
            self.terminal_counter += 1;
        }
        self.tabs.push(tab);
        self.active_index = self.tabs.len() - 1;
        cx.emit(PaneEvent::TabAdded);
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

    pub fn focus_active_tab(&self, window: &mut Window, cx: &App) {
        if let Some(tab) = self.active_tab() {
            tab.focus(window, cx);
        }
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
            .debug_selector(|| "pane-tab-bar".into())
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
                    TabItem::Editor(editor) => editor.read(cx).label(cx),
                    TabItem::PrReview(pr) => pr.read(cx).label(cx),
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
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.select_tab(idx, cx);
                        this.focus_active_tab(window, cx);
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
                    .debug_selector(|| "pane-add-terminal".into())
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(|el| el.bg(border_color))
                    .text_color(muted_color)
                    .text_xs()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.add_terminal(cx);
                        this.focus_active_tab(window, cx);
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
            .on_mouse_down(MouseButton::Left, cx.listener(|_this, _, _window, cx| {
                cx.emit(PaneEvent::Focus);
            }))
            .child(self.render_tab_bar(cx))
            .child(
                div()
                    .id("pane-content-container")
                    .debug_selector(|| "pane-content-container".into())
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
                        Some(TabItem::PrReview(pr_review)) => el.child(pr_review),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[core::prelude::v1::test]
    fn test_pane_tab_bar_renders() {
        crate::test_helpers::run_gpui_test(|cx| {
            let _fixture = crate::test_helpers::TestFixture::new(cx);
            cx.update(|cx| gpui_component::init(cx));

            let (_view, cx) = cx.add_window_view(|_window, cx| {
                Pane::new(PathBuf::from("/tmp"), cx)
            });

            assert!(cx.debug_bounds("pane-tab-bar").is_some(), "tab bar should be rendered");
            assert!(
                cx.debug_bounds("pane-content-container").is_some(),
                "content container should be rendered"
            );
        });
    }

    #[core::prelude::v1::test]
    fn test_pane_add_terminal_button_renders() {
        crate::test_helpers::run_gpui_test(|cx| {
            let _fixture = crate::test_helpers::TestFixture::new(cx);
            cx.update(|cx| gpui_component::init(cx));

            let (_view, cx) = cx.add_window_view(|_window, cx| {
                Pane::new(PathBuf::from("/tmp"), cx)
            });

            assert!(
                cx.debug_bounds("pane-add-terminal").is_some(),
                "add terminal button should be rendered"
            );
        });
    }
}
