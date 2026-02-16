use crate::stores::{Member, PaneAxis, PaneStore};
use crate::ui::pane::{Axis, DividerDrag};
use gpui::{
    div, px, relative, AnyElement, AppContext as _, Bounds, Context, CursorStyle, DragMoveEvent, ElementId, Entity,
    InteractiveElement, IntoElement, MouseButton, MouseMoveEvent, MouseUpEvent, ParentElement,
    Pixels, Render, Size, StatefulInteractiveElement, Styled, Window,
};
use gpui::prelude::FluentBuilder;
use gpui_component::theme::ActiveTheme;

const MIN_PANE_SIZE: f32 = 100.0;
const DIVIDER_SIZE: f32 = 4.0;

pub struct PaneGroupView {
    pane_store: Entity<PaneStore>,
    drag_bounds: Option<Bounds<Pixels>>,
}

impl PaneGroupView {
    pub fn new(pane_store: Entity<PaneStore>, cx: &mut Context<Self>) -> Self {
        // Subscribe to pane store changes to trigger re-renders
        cx.subscribe(&pane_store, |_this, _, _event, cx| {
            cx.notify();
        })
        .detach();

        Self {
            pane_store,
            drag_bounds: None,
        }
    }
}

impl Render for PaneGroupView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let container_bounds = self.drag_bounds.unwrap_or_else(|| window.bounds());
        let pane_store = self.pane_store.clone();

        div()
            .id("pane-group-view")
            .size_full()
            .on_mouse_move(cx.listener(|this, _event: &MouseMoveEvent, window, _cx| {
                this.drag_bounds = Some(window.bounds());
            }))
            .on_drag_move(cx.listener(
                move |_this, event: &DragMoveEvent<DividerDrag>, _window, cx| {
                    let drag_data = event.drag(cx).clone();
                    let position = event.event.position;
                    pane_store.update(cx, |store, cx| {
                        store.handle_divider_drag(&drag_data, position, cx);
                    });
                },
            ))
            .child(self.render_root(cx, container_bounds))
    }
}

impl PaneGroupView {
    fn render_root(&self, cx: &mut Context<Self>, container_bounds: Bounds<Pixels>) -> AnyElement {
        let root = self.pane_store.read(cx).root.clone();
        render_member(&root, &self.pane_store, cx, vec![], container_bounds)
    }
}

fn render_member(
    member: &Member,
    pane_store: &Entity<PaneStore>,
    cx: &mut Context<PaneGroupView>,
    path: Vec<usize>,
    container_bounds: Bounds<Pixels>,
) -> AnyElement {
    match member {
        Member::Pane(pane) => div().size_full().child(pane.clone()).into_any_element(),
        Member::Axis(axis) => render_axis(axis, pane_store, cx, path, container_bounds).into_any_element(),
    }
}

fn render_axis(
    axis: &PaneAxis,
    pane_store: &Entity<PaneStore>,
    cx: &mut Context<PaneGroupView>,
    path: Vec<usize>,
    container_bounds: Bounds<Pixels>,
) -> impl IntoElement {
    let is_horizontal = axis.axis == Axis::Horizontal;
    let container_size = if is_horizontal {
        f32::from(container_bounds.size.width)
    } else {
        f32::from(container_bounds.size.height)
    };

    let mut children: Vec<AnyElement> = Vec::new();

    for (i, (member, &ratio)) in axis.members.iter().zip(&axis.ratios).enumerate() {
        if i > 0 {
            let divider_index = i - 1;
            let axis_type = axis.axis;
            children.push(
                render_divider(divider_index, axis_type, pane_store, path.clone(), container_size, cx)
                    .into_any_element(),
            );
        }

        let child_bounds = {
            let num_dividers = axis.members.len().saturating_sub(1);
            let divider_space = num_dividers as f32 * DIVIDER_SIZE;
            let available_size = container_size - divider_space;
            let child_size = available_size * ratio;

            if is_horizontal {
                Bounds {
                    origin: container_bounds.origin,
                    size: Size {
                        width: px(child_size),
                        height: container_bounds.size.height,
                    },
                }
            } else {
                Bounds {
                    origin: container_bounds.origin,
                    size: Size {
                        width: container_bounds.size.width,
                        height: px(child_size),
                    },
                }
            }
        };

        let mut child_path = path.clone();
        child_path.push(i);
        let child = render_member(member, pane_store, cx, child_path, child_bounds);
        let element = if is_horizontal {
            div()
                .h_full()
                .flex_basis(relative(ratio))
                .flex_shrink()
                .flex_grow()
                .min_w(px(MIN_PANE_SIZE))
                .child(child)
        } else {
            div()
                .w_full()
                .flex_basis(relative(ratio))
                .flex_shrink()
                .flex_grow()
                .min_h(px(MIN_PANE_SIZE))
                .child(child)
        };
        children.push(element.into_any_element());
    }

    div()
        .size_full()
        .flex()
        .when(is_horizontal, |el| el.flex_row())
        .when(!is_horizontal, |el| el.flex_col())
        .children(children)
}

fn render_divider(
    divider_index: usize,
    axis: Axis,
    pane_store: &Entity<PaneStore>,
    axis_path: Vec<usize>,
    container_size: f32,
    cx: &mut Context<PaneGroupView>,
) -> impl IntoElement {
    let is_horizontal = axis == Axis::Horizontal;
    let theme = cx.theme();
    let border_color = theme.border;
    let hover_color = theme.list_active;
    let pane_store_for_up = pane_store.clone();

    div()
        .id(ElementId::Name(
            format!("divider-{:?}-{}", axis_path, divider_index).into(),
        ))
        .flex_shrink_0()
        .when(is_horizontal, |el| el.w(px(DIVIDER_SIZE)).h_full())
        .when(!is_horizontal, |el| el.h(px(DIVIDER_SIZE)).w_full())
        .cursor(if is_horizontal {
            CursorStyle::ResizeLeftRight
        } else {
            CursorStyle::ResizeUpDown
        })
        .bg(border_color)
        .hover(|el| el.bg(hover_color))
        .on_drag(
            DividerDrag::new(axis, divider_index, axis_path.clone(), container_size),
            |drag, _, _window, cx| cx.new(|_| drag.clone()),
        )
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(move |_this, _event: &MouseUpEvent, _window, cx| {
                pane_store_for_up.update(cx, |store, _cx| {
                    store.clear_drag_state();
                });
            }),
        )
}
