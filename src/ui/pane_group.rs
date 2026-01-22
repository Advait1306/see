use crate::ui::pane::{Axis, Pane, PaneEvent, SplitDirection};
use gpui::prelude::*;
use gpui::*;
use std::path::PathBuf;

const MIN_PANE_SIZE: f32 = 100.0;
const DIVIDER_SIZE: f32 = 4.0;

pub enum Member {
    Pane(Entity<Pane>),
    Axis(PaneAxis),
}

impl Member {
    fn contains_pane(&self, pane: &Entity<Pane>) -> bool {
        match self {
            Member::Pane(p) => p == pane,
            Member::Axis(axis) => axis.members.iter().any(|m| m.contains_pane(pane)),
        }
    }

    fn render(&self, cx: &mut Context<PaneGroup>, group_entity: Entity<PaneGroup>) -> impl IntoElement {
        match self {
            Member::Pane(pane) => div()
                .size_full()
                .child(pane.clone())
                .into_any_element(),
            Member::Axis(axis) => axis.render(cx, group_entity).into_any_element(),
        }
    }

    pub fn first_pane(&self) -> Option<Entity<Pane>> {
        match self {
            Member::Pane(pane) => Some(pane.clone()),
            Member::Axis(axis) => axis.members.first().and_then(|m| m.first_pane()),
        }
    }

    fn collect_panes(&self, panes: &mut Vec<Entity<Pane>>) {
        match self {
            Member::Pane(pane) => panes.push(pane.clone()),
            Member::Axis(axis) => {
                for member in &axis.members {
                    member.collect_panes(panes);
                }
            }
        }
    }
}

pub struct PaneAxis {
    pub axis: Axis,
    pub members: Vec<Member>,
    pub ratios: Vec<f32>,
}

impl PaneAxis {
    pub fn new(axis: Axis, members: Vec<Member>) -> Self {
        let count = members.len();
        let ratios = vec![1.0 / count as f32; count];
        Self {
            axis,
            members,
            ratios,
        }
    }

    fn render(&self, cx: &mut Context<PaneGroup>, group_entity: Entity<PaneGroup>) -> impl IntoElement {
        let is_horizontal = self.axis == Axis::Horizontal;

        let mut children: Vec<AnyElement> = Vec::new();

        for (i, (member, &ratio)) in self.members.iter().zip(&self.ratios).enumerate() {
            if i > 0 {
                // Add divider
                let divider_index = i - 1;
                let axis = self.axis;
                children.push(
                    self.render_divider(divider_index, axis, group_entity.clone(), cx)
                        .into_any_element(),
                );
            }

            let child = member.render(cx, group_entity.clone());
            let element = if is_horizontal {
                div()
                    .h_full()
                    .flex_basis(relative(ratio))
                    .flex_shrink_0()
                    .min_w(px(MIN_PANE_SIZE))
                    .child(child)
            } else {
                div()
                    .w_full()
                    .flex_basis(relative(ratio))
                    .flex_shrink_0()
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
        &self,
        divider_index: usize,
        axis: Axis,
        group_entity: Entity<PaneGroup>,
        cx: &mut Context<PaneGroup>,
    ) -> impl IntoElement {
        let is_horizontal = axis == Axis::Horizontal;

        div()
            .id(ElementId::Name(format!("divider-{}", divider_index).into()))
            .when(is_horizontal, |el| el.w(px(DIVIDER_SIZE)).h_full().cursor_col_resize())
            .when(!is_horizontal, |el| el.h(px(DIVIDER_SIZE)).w_full().cursor_row_resize())
            .bg(rgb(0x313244))
            .hover(|el| el.bg(rgb(0x45475a)))
            .on_drag(
                DividerDrag {
                    axis,
                    divider_index,
                    initial_ratios: self.ratios.clone(),
                },
                |drag, _, _window, cx| {
                    cx.new(|_| drag.clone())
                },
            )
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DividerDrag>, _window, cx| {
                    let drag = DividerDrag {
                        axis,
                        divider_index,
                        initial_ratios: this.get_root_ratios(),
                    };
                    this.handle_divider_drag(&drag, event.event.position, divider_index, cx);
                },
            ))
    }

    fn find_and_split_pane(
        &mut self,
        target: &Entity<Pane>,
        new_pane: Entity<Pane>,
        direction: SplitDirection,
        cx: &mut App,
    ) -> bool {
        for (i, member) in self.members.iter_mut().enumerate() {
            match member {
                Member::Pane(pane) if pane == target => {
                    let split_axis = direction.axis();
                    let is_before = direction.is_before();

                    if split_axis == self.axis {
                        // Same axis - insert adjacent
                        let new_ratio = self.ratios[i] / 2.0;
                        self.ratios[i] = new_ratio;

                        let insert_index = if is_before { i } else { i + 1 };
                        self.members.insert(insert_index, Member::Pane(new_pane));
                        self.ratios.insert(insert_index, new_ratio);
                    } else {
                        // Different axis - create nested axis
                        let old_pane = std::mem::replace(member, Member::Pane(new_pane.clone()));
                        let members = if is_before {
                            vec![Member::Pane(new_pane), old_pane]
                        } else {
                            vec![old_pane, Member::Pane(new_pane)]
                        };
                        *member = Member::Axis(PaneAxis::new(split_axis, members));
                    }
                    return true;
                }
                Member::Axis(axis) => {
                    if axis.find_and_split_pane(target, new_pane.clone(), direction, cx) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn remove_pane(&mut self, target: &Entity<Pane>) -> bool {
        for i in 0..self.members.len() {
            match &mut self.members[i] {
                Member::Pane(pane) if pane == target => {
                    self.members.remove(i);
                    self.ratios.remove(i);
                    // Normalize ratios
                    if !self.ratios.is_empty() {
                        let total: f32 = self.ratios.iter().sum();
                        for ratio in &mut self.ratios {
                            *ratio /= total;
                        }
                    }
                    return true;
                }
                Member::Axis(axis) => {
                    if axis.remove_pane(target) {
                        // Collapse single-member axis
                        if axis.members.len() == 1 {
                            let remaining = axis.members.remove(0);
                            self.members[i] = remaining;
                        }
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }
}

#[derive(Clone)]
pub struct DividerDrag {
    axis: Axis,
    divider_index: usize,
    initial_ratios: Vec<f32>,
}

impl Render for DividerDrag {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .when(self.axis == Axis::Horizontal, |el| el.w(px(4.0)).h(px(40.0)))
            .when(self.axis == Axis::Vertical, |el| el.h(px(4.0)).w(px(40.0)))
            .bg(rgb(0x89b4fa))
            .rounded_sm()
    }
}

pub struct PaneGroup {
    pub root: Member,
    pub path: PathBuf,
    pub active_pane: Option<Entity<Pane>>,
    drag_start_position: Option<Point<Pixels>>,
    drag_bounds: Option<Bounds<Pixels>>,
}

pub enum PaneGroupEvent {
    PaneAdded(Entity<Pane>),
    PaneRemoved(Entity<Pane>),
    PaneFocused(Entity<Pane>),
    StateChanged,
}

impl EventEmitter<PaneGroupEvent> for PaneGroup {}

impl PaneGroup {
    pub fn new(path: PathBuf, cx: &mut Context<Self>) -> Self {
        let pane = cx.new(|cx| Pane::new(path.clone(), cx));
        log::info!("PaneGroup::new - created pane {:?}", pane.entity_id());
        Self::subscribe_to_pane(&pane, cx);

        Self {
            root: Member::Pane(pane.clone()),
            path,
            active_pane: Some(pane),
            drag_start_position: None,
            drag_bounds: None,
        }
    }

    pub fn with_pane(path: PathBuf, pane: Entity<Pane>, cx: &mut Context<Self>) -> Self {
        Self::subscribe_to_pane(&pane, cx);

        Self {
            root: Member::Pane(pane.clone()),
            path,
            active_pane: Some(pane),
            drag_start_position: None,
            drag_bounds: None,
        }
    }

    pub fn with_root(path: PathBuf, root: Member, cx: &mut Context<Self>) -> Self {
        // Subscribe to all panes in the root
        let mut panes = Vec::new();
        root.collect_panes(&mut panes);
        for pane in &panes {
            Self::subscribe_to_pane(pane, cx);
        }

        Self {
            root,
            path,
            active_pane: panes.first().cloned(),
            drag_start_position: None,
            drag_bounds: None,
        }
    }

    pub fn subscribe_to_pane_static(pane: &Entity<Pane>, cx: &mut Context<Self>) {
        Self::subscribe_to_pane(pane, cx);
    }

    fn subscribe_to_pane(pane: &Entity<Pane>, cx: &mut Context<Self>) {
        let pane_id = pane.entity_id();
        log::info!("subscribe_to_pane: subscribing to pane {:?}", pane_id);
        cx.subscribe(pane, move |this, pane, event, cx| {
            match event {
                PaneEvent::Split { direction, new_pane } => {
                    log::info!("PaneEvent::Split received from pane {:?}, new_pane {:?}", pane_id, new_pane.entity_id());
                    this.split_pane(&pane, new_pane.clone(), *direction, cx);
                }
                PaneEvent::Close => {
                    this.remove_pane(&pane, cx);
                }
                PaneEvent::TabMoved | PaneEvent::TerminalAdded | PaneEvent::TerminalClosed => {
                    // Check if pane is empty and should be removed
                    let is_empty = pane.read(cx).terminals.is_empty();
                    if is_empty {
                        this.remove_pane(&pane, cx);
                    }
                    cx.emit(PaneGroupEvent::StateChanged);
                }
                PaneEvent::Focus => {
                    this.active_pane = Some(pane.clone());
                    cx.emit(PaneGroupEvent::PaneFocused(pane.clone()));
                }
            }
        })
        .detach();
    }

    pub fn split_pane(
        &mut self,
        target: &Entity<Pane>,
        new_pane: Entity<Pane>,
        direction: SplitDirection,
        cx: &mut Context<Self>,
    ) {
        log::info!("split_pane called: direction={:?}, target={:?}, new_pane={:?}",
            direction, target.entity_id(), new_pane.entity_id());
        log::info!("  new_pane terminals: {}", new_pane.read(cx).terminals.len());

        Self::subscribe_to_pane(&new_pane, cx);

        match &mut self.root {
            Member::Pane(pane) if pane == target => {
                log::info!("  Splitting root pane");
                let split_axis = direction.axis();
                let is_before = direction.is_before();

                let old_pane = std::mem::replace(&mut self.root, Member::Pane(new_pane.clone()));
                let members = if is_before {
                    vec![Member::Pane(new_pane.clone()), old_pane]
                } else {
                    vec![old_pane, Member::Pane(new_pane.clone())]
                };
                self.root = Member::Axis(PaneAxis::new(split_axis, members));
                log::info!("  Created axis with {} members", 2);
            }
            Member::Axis(axis) => {
                log::info!("  Splitting within existing axis");
                axis.find_and_split_pane(target, new_pane.clone(), direction, cx);
            }
            _ => {
                log::info!("  No match for split target!");
            }
        }

        self.active_pane = Some(new_pane.clone());
        cx.emit(PaneGroupEvent::PaneAdded(new_pane));
        cx.emit(PaneGroupEvent::StateChanged);
        cx.notify();
    }

    pub fn remove_pane(&mut self, target: &Entity<Pane>, cx: &mut Context<Self>) {
        match &mut self.root {
            Member::Pane(pane) if pane == target => {
                // Can't remove last pane, but emit event
                cx.emit(PaneGroupEvent::PaneRemoved(target.clone()));
                return;
            }
            Member::Axis(axis) => {
                if axis.remove_pane(target) {
                    // Collapse if only one member remains
                    if axis.members.len() == 1 {
                        self.root = axis.members.remove(0);
                    }
                }
            }
            _ => {}
        }

        // Update active pane if it was removed
        if self.active_pane.as_ref() == Some(target) {
            self.active_pane = self.root.first_pane();
        }

        cx.emit(PaneGroupEvent::PaneRemoved(target.clone()));
        cx.emit(PaneGroupEvent::StateChanged);
        cx.notify();
    }

    pub fn active_pane(&self) -> Option<&Entity<Pane>> {
        self.active_pane.as_ref()
    }

    pub fn panes(&self) -> Vec<Entity<Pane>> {
        let mut panes = Vec::new();
        self.root.collect_panes(&mut panes);
        panes
    }

    pub fn pane_count(&self) -> usize {
        self.panes().len()
    }

    pub fn terminal_count(&self, cx: &App) -> usize {
        self.panes()
            .iter()
            .map(|p| p.read(cx).terminals.len())
            .sum()
    }

    fn get_root_ratios(&self) -> Vec<f32> {
        match &self.root {
            Member::Axis(axis) => axis.ratios.clone(),
            _ => vec![1.0],
        }
    }

    fn handle_divider_drag(
        &mut self,
        drag: &DividerDrag,
        position: Point<Pixels>,
        divider_index: usize,
        cx: &mut Context<Self>,
    ) {
        // Store drag start position if not set
        if self.drag_start_position.is_none() {
            self.drag_start_position = Some(position);
        }

        if let Member::Axis(axis) = &mut self.root {
            if drag.axis == axis.axis && divider_index < axis.ratios.len() - 1 {
                let start = self.drag_start_position.unwrap_or(position);
                let delta = if axis.axis == Axis::Horizontal {
                    position.x - start.x
                } else {
                    position.y - start.y
                };

                // Calculate new ratios based on drag delta
                // This is a simplified version - a full implementation would
                // need to track the initial total size
                let delta_ratio = f32::from(delta) / 1000.0; // Approximate scale

                let mut new_ratios = drag.initial_ratios.clone();
                let left_index = divider_index;
                let right_index = divider_index + 1;

                if left_index < new_ratios.len() && right_index < new_ratios.len() {
                    new_ratios[left_index] = (new_ratios[left_index] + delta_ratio).max(0.1);
                    new_ratios[right_index] = (new_ratios[right_index] - delta_ratio).max(0.1);

                    // Normalize
                    let total: f32 = new_ratios.iter().sum();
                    for ratio in &mut new_ratios {
                        *ratio /= total;
                    }

                    axis.ratios = new_ratios;
                    cx.emit(PaneGroupEvent::StateChanged);
                    cx.notify();
                }
            }
        }
    }
}

impl Render for PaneGroup {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let group_entity = cx.entity().clone();

        // Debug: log the pane structure
        self.log_structure(cx);

        div()
            .id("pane-group")
            .size_full()
            .child(self.root.render(cx, group_entity))
    }
}

impl PaneGroup {
    fn log_structure(&self, cx: &App) {
        log::info!("=== PaneGroup Structure ===");
        self.log_member(&self.root, 0, cx);
        log::info!("===========================");
    }

    fn log_member(&self, member: &Member, depth: usize, cx: &App) {
        let indent = "  ".repeat(depth);
        match member {
            Member::Pane(pane) => {
                let pane_data = pane.read(cx);
                log::info!(
                    "{}Pane: {} terminals, active_index={}",
                    indent,
                    pane_data.terminals.len(),
                    pane_data.active_index
                );
            }
            Member::Axis(axis) => {
                log::info!(
                    "{}Axis({:?}): {} members, ratios={:?}",
                    indent,
                    axis.axis,
                    axis.members.len(),
                    axis.ratios
                );
                for (i, m) in axis.members.iter().enumerate() {
                    log::info!("{}  [{}]:", indent, i);
                    self.log_member(m, depth + 2, cx);
                }
            }
        }
    }
}
