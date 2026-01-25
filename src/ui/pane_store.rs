use crate::config;
use crate::types::TabConfig;
use crate::ui::pane::{Axis, Pane, PaneEvent, SplitDirection};
use gpui::prelude::*;
use gpui::*;
use gpui_component::theme::ActiveTheme;
use serde::{Deserialize, Serialize};

const MIN_PANE_SIZE: f32 = 100.0;
const DIVIDER_SIZE: f32 = 4.0;

// =============================================================================
// Layout Serialization Types
// =============================================================================

/// Serializable state for a pane (collection of tabs)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PaneConfig {
    pub tabs: Vec<TabConfig>,
    pub active_index: usize,
}

/// Serializable layout tree node
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum LayoutNode {
    Pane(PaneConfig),
    Split {
        axis: LayoutAxis,
        ratios: Vec<f32>,
        children: Vec<LayoutNode>,
    },
}

impl Default for LayoutNode {
    fn default() -> Self {
        LayoutNode::Pane(PaneConfig {
            tabs: Vec::new(),
            active_index: 0,
        })
    }
}

/// Axis for serialization (separate from internal Axis to avoid coupling)
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "lowercase")]
pub enum LayoutAxis {
    Horizontal,
    Vertical,
}

impl From<Axis> for LayoutAxis {
    fn from(axis: Axis) -> Self {
        match axis {
            Axis::Horizontal => LayoutAxis::Horizontal,
            Axis::Vertical => LayoutAxis::Vertical,
        }
    }
}

impl From<LayoutAxis> for Axis {
    fn from(axis: LayoutAxis) -> Self {
        match axis {
            LayoutAxis::Horizontal => Axis::Horizontal,
            LayoutAxis::Vertical => Axis::Vertical,
        }
    }
}

// =============================================================================
// Member and PaneAxis
// =============================================================================

pub enum Member {
    Pane(Entity<Pane>),
    Axis(PaneAxis),
}

impl Member {
    fn render(
        &self,
        cx: &mut Context<PaneStore>,
        group_entity: Entity<PaneStore>,
        path: Vec<usize>,
        container_bounds: Bounds<Pixels>,
    ) -> impl IntoElement {
        match self {
            Member::Pane(pane) => div()
                .size_full()
                .child(pane.clone())
                .into_any_element(),
            Member::Axis(axis) => axis
                .render_with_path(cx, group_entity, path, container_bounds)
                .into_any_element(),
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

    fn render_with_path(
        &self,
        cx: &mut Context<PaneStore>,
        group_entity: Entity<PaneStore>,
        path: Vec<usize>,
        container_bounds: Bounds<Pixels>,
    ) -> impl IntoElement {
        let is_horizontal = self.axis == Axis::Horizontal;
        let container_size = if is_horizontal {
            f32::from(container_bounds.size.width)
        } else {
            f32::from(container_bounds.size.height)
        };

        let mut children: Vec<AnyElement> = Vec::new();

        for (i, (member, &ratio)) in self.members.iter().zip(&self.ratios).enumerate() {
            if i > 0 {
                // Add divider
                let divider_index = i - 1;
                let axis = self.axis;
                children.push(
                    self.render_divider(
                        divider_index,
                        axis,
                        group_entity.clone(),
                        path.clone(),
                        container_size,
                        cx,
                    )
                    .into_any_element(),
                );
            }

            // Calculate child bounds for nested axes
            let child_bounds = {
                let num_dividers = self.members.len().saturating_sub(1);
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
            let child = member.render(cx, group_entity.clone(), child_path, child_bounds);
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
        &self,
        divider_index: usize,
        axis: Axis,
        _group_entity: Entity<PaneStore>,
        axis_path: Vec<usize>,
        container_size: f32,
        cx: &mut Context<PaneStore>,
    ) -> impl IntoElement {
        let is_horizontal = axis == Axis::Horizontal;
        let theme = cx.theme();
        let border_color = theme.border;
        let hover_color = theme.list_active;

        div()
            .id(ElementId::Name(
                format!("divider-{:?}-{}", axis_path, divider_index).into(),
            ))
            .flex_shrink_0()
            .when(is_horizontal, |el| {
                el.w(px(DIVIDER_SIZE)).h_full()
            })
            .when(!is_horizontal, |el| {
                el.h(px(DIVIDER_SIZE)).w_full()
            })
            .cursor(if is_horizontal {
                CursorStyle::ResizeLeftRight
            } else {
                CursorStyle::ResizeUpDown
            })
            .bg(border_color)
            .hover(|el| el.bg(hover_color))
            .on_drag(
                DividerDrag {
                    axis,
                    divider_index,
                    axis_path: axis_path.clone(),
                    container_size,
                },
                |drag, _, _window, cx| cx.new(|_| drag.clone()),
            )
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DividerDrag>, _window, cx| {
                    // Get the actual drag data from the event - this tells us WHICH divider is being dragged
                    // Clone what we need before borrowing cx mutably
                    let drag_data = event.drag(cx).clone();
                    let position = event.event.position;

                    // Check if we need to initialize drag state for this drag
                    let needs_init = match &this.drag_start {
                        None => true,
                        Some(ds) => {
                            // Different drag if identity changed
                            ds.axis_path != drag_data.axis_path
                                || ds.divider_index != drag_data.divider_index
                        }
                    };

                    if needs_init {
                        let ratios = this.get_ratios_at_path(&drag_data.axis_path);
                        log::info!(
                            "DRAG START: path={:?}, axis={:?}, idx={}",
                            drag_data.axis_path,
                            drag_data.axis,
                            drag_data.divider_index
                        );
                        this.drag_start = Some(DragStart {
                            axis_path: drag_data.axis_path.clone(),
                            divider_index: drag_data.divider_index,
                            start_position: position,
                            initial_ratios: ratios,
                        });
                    }

                    this.handle_divider_drag(&drag_data, position, cx);
                },
            ))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _event: &MouseUpEvent, _window, _cx| {
                    this.drag_start = None;
                }),
            )
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
    axis_path: Vec<usize>,          // Path to locate axis in nested structure
    container_size: f32,            // Size along axis direction
}

impl Render for DividerDrag {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Invisible drag indicator - we don't need a visible element following the cursor
        div()
    }
}

pub struct PaneStore {
    workspace_id: String,
    pub root: Member,
    pub active_pane: Option<Entity<Pane>>,
    drag_bounds: Option<Bounds<Pixels>>,
    drag_start: Option<DragStart>,
}

#[derive(Clone)]
struct DragStart {
    axis_path: Vec<usize>,
    divider_index: usize,
    start_position: Point<Pixels>,
    initial_ratios: Vec<f32>,
}

#[derive(Clone)]
pub enum PaneStoreEvent {
    PaneAdded,
    PaneRemoved,
    PaneFocused,
    StateChanged,
}

impl EventEmitter<PaneStoreEvent> for PaneStore {}

impl PaneStore {
    pub fn with_root(workspace_id: String, root: Member, cx: &mut Context<Self>) -> Self {
        let mut panes = Vec::new();
        root.collect_panes(&mut panes);
        for pane in &panes {
            Self::subscribe_to_pane(pane, cx);
        }

        Self {
            workspace_id,
            root,
            active_pane: panes.first().cloned(),
            drag_bounds: None,
            drag_start: None,
        }
    }

    pub fn save_layout(&self, cx: &App) {
        let layout = self.collect_layout(cx);
        config::save_json(&config::layout_path(&self.workspace_id), &layout);
    }

    /// Collect layout tree from current pane structure
    fn collect_layout(&self, cx: &App) -> LayoutNode {
        self.collect_member_layout(&self.root, cx)
    }

    fn collect_member_layout(&self, member: &Member, cx: &App) -> LayoutNode {
        match member {
            Member::Pane(pane) => {
                let pane = pane.read(cx);
                let tabs = pane.tabs.iter()
                    .map(|tab| tab.to_config(cx))
                    .collect();

                LayoutNode::Pane(PaneConfig {
                    tabs,
                    active_index: pane.active_index,
                })
            }
            Member::Axis(axis) => LayoutNode::Split {
                axis: axis.axis.into(),
                ratios: axis.ratios.clone(),
                children: axis.members.iter()
                    .map(|m| self.collect_member_layout(m, cx))
                    .collect(),
            },
        }
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
                PaneEvent::TabMoved | PaneEvent::TerminalAdded | PaneEvent::TabClosed => {
                    let is_empty = pane.read(cx).tabs.is_empty();
                    if is_empty {
                        this.remove_pane(&pane, cx);
                    }
                    cx.emit(PaneStoreEvent::StateChanged);
                }
                PaneEvent::Focus => {
                    this.active_pane = Some(pane.clone());
                    cx.emit(PaneStoreEvent::PaneFocused);
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
        log::info!("  new_pane tabs: {}", new_pane.read(cx).tabs.len());

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
        cx.emit(PaneStoreEvent::PaneAdded);
        cx.emit(PaneStoreEvent::StateChanged);
        cx.notify();
    }

    pub fn remove_pane(&mut self, target: &Entity<Pane>, cx: &mut Context<Self>) {
        match &mut self.root {
            Member::Pane(pane) if pane == target => {
                cx.emit(PaneStoreEvent::PaneRemoved);
                return;
            }
            Member::Axis(axis) => {
                if axis.remove_pane(target) {
                    if axis.members.len() == 1 {
                        self.root = axis.members.remove(0);
                    }
                }
            }
            _ => {}
        }

        if self.active_pane.as_ref() == Some(target) {
            self.active_pane = self.root.first_pane();
        }

        cx.emit(PaneStoreEvent::PaneRemoved);
        cx.emit(PaneStoreEvent::StateChanged);
        cx.notify();
    }

    pub fn panes(&self) -> Vec<Entity<Pane>> {
        let mut panes = Vec::new();
        self.root.collect_panes(&mut panes);
        panes
    }

    pub fn pane_count(&self) -> usize {
        self.panes().len()
    }

    fn get_root_ratios(&self) -> Vec<f32> {
        match &self.root {
            Member::Axis(axis) => axis.ratios.clone(),
            _ => vec![1.0],
        }
    }

    fn get_ratios_at_path(&self, path: &[usize]) -> Vec<f32> {
        if path.is_empty() {
            return self.get_root_ratios();
        }

        let mut current = &self.root;
        for &idx in path.iter() {
            match current {
                Member::Axis(axis) => {
                    if idx < axis.members.len() {
                        current = &axis.members[idx];
                    } else {
                        return vec![1.0];
                    }
                }
                Member::Pane(_) => return vec![1.0],
            }
        }

        match current {
            Member::Axis(axis) => axis.ratios.clone(),
            Member::Pane(_) => vec![1.0],
        }
    }

    fn get_axis_at_path_mut(&mut self, path: &[usize]) -> Option<&mut PaneAxis> {
        if path.is_empty() {
            return match &mut self.root {
                Member::Axis(axis) => Some(axis),
                _ => None,
            };
        }

        let mut current = &mut self.root;
        for &idx in path.iter() {
            match current {
                Member::Axis(axis) => {
                    if idx < axis.members.len() {
                        current = &mut axis.members[idx];
                    } else {
                        return None;
                    }
                }
                Member::Pane(_) => return None,
            }
        }

        match current {
            Member::Axis(axis) => Some(axis),
            Member::Pane(_) => None,
        }
    }

    fn handle_divider_drag(
        &mut self,
        drag_data: &DividerDrag,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        // Get the drag start state (has start position and initial ratios)
        let drag_start = match &self.drag_start {
            Some(ds) => ds.clone(),
            None => return,
        };

        // Calculate pixel delta based on axis direction
        let pixel_delta = if drag_data.axis == Axis::Horizontal {
            position.x - drag_start.start_position.x
        } else {
            position.y - drag_start.start_position.y
        };

        // Calculate available space (minus dividers)
        let num_dividers = drag_start.initial_ratios.len().saturating_sub(1);
        let available_size = drag_data.container_size - (num_dividers as f32 * DIVIDER_SIZE);

        // Avoid division by zero
        if available_size <= 0.0 {
            return;
        }

        // Convert pixel movement to ratio change
        let delta_ratio = f32::from(pixel_delta) / available_size;

        // Calculate minimum ratio based on MIN_PANE_SIZE
        let min_ratio = MIN_PANE_SIZE / available_size;

        let mut new_ratios = drag_start.initial_ratios.clone();
        let left_index = drag_data.divider_index;
        let right_index = drag_data.divider_index + 1;

        if left_index < new_ratios.len() && right_index < new_ratios.len() {
            new_ratios[left_index] = (new_ratios[left_index] + delta_ratio).max(min_ratio);
            new_ratios[right_index] = (new_ratios[right_index] - delta_ratio).max(min_ratio);

            // Normalize ratios to sum to 1.0
            let total: f32 = new_ratios.iter().sum();
            for ratio in &mut new_ratios {
                *ratio /= total;
            }

            if let Some(axis) = self.get_axis_at_path_mut(&drag_data.axis_path) {
                log::info!(
                    "APPLYING DRAG: path={:?}, axis={:?}, new_ratios={:?}",
                    drag_data.axis_path,
                    drag_data.axis,
                    new_ratios
                );
                axis.ratios = new_ratios;
                cx.emit(PaneStoreEvent::StateChanged);
                cx.notify();
            }
        }
    }

    pub fn next_pane(&mut self, cx: &mut Context<Self>) {
        if let Some(pane) = &self.active_pane {
            pane.update(cx, |p, cx| {
                if p.tabs.len() > 1 {
                    p.active_index = (p.active_index + 1) % p.tabs.len();
                    cx.notify();
                }
            });
        }
    }

    pub fn prev_pane(&mut self, cx: &mut Context<Self>) {
        if let Some(pane) = &self.active_pane {
            pane.update(cx, |p, cx| {
                if p.tabs.len() > 1 {
                    p.active_index = if p.active_index == 0 {
                        p.tabs.len() - 1
                    } else {
                        p.active_index - 1
                    };
                    cx.notify();
                }
            });
        }
    }

    pub fn close_current_pane(&mut self, cx: &mut Context<Self>) {
        if let Some(pane) = self.active_pane.clone() {
            let (should_close, pane_count) = {
                let tabs_count = pane.read(cx).tabs.len();
                if tabs_count > 1 {
                    pane.update(cx, |p, cx| {
                        p.remove_tab(p.active_index, cx);
                    });
                    (false, self.pane_count())
                } else {
                    (true, self.pane_count())
                }
            };

            if should_close && pane_count > 1 {
                self.remove_pane(&pane, cx);
            }
        }
    }
}

impl Render for PaneStore {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let group_entity = cx.entity().clone();

        let root_type = match &self.root {
            Member::Pane(_) => "Pane (single, no dividers)",
            Member::Axis(a) => {
                if a.members.len() > 1 {
                    "Axis (multiple members, should have dividers)"
                } else {
                    "Axis (single member, no dividers)"
                }
            }
        };
        log::info!("PaneStore::render - root type: {}", root_type);
        self.log_structure(cx);

        let container_bounds = self.drag_bounds.unwrap_or_else(|| window.bounds());
        log::info!(
            "PaneStore::render - container_bounds: {:?}",
            container_bounds
        );

        div()
            .id("pane-store")
            .size_full()
            .on_mouse_move(cx.listener(|this, _event: &MouseMoveEvent, window, _cx| {
                this.drag_bounds = Some(window.bounds());
            }))
            .child(self.root.render(cx, group_entity, vec![], container_bounds))
    }
}

impl PaneStore {
    fn log_structure(&self, cx: &App) {
        log::info!("=== PaneStore Structure ===");
        self.log_member(&self.root, 0, cx);
        log::info!("===========================");
    }

    fn log_member(&self, member: &Member, depth: usize, cx: &App) {
        let indent = "  ".repeat(depth);
        match member {
            Member::Pane(pane) => {
                let pane_data = pane.read(cx);
                log::info!(
                    "{}Pane: {} tabs, active_index={}",
                    indent,
                    pane_data.tabs.len(),
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
