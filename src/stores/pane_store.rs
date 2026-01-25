use crate::config::{self, MemberConfig};
use crate::stores::EditorStore;
use crate::stores::TerminalStore;
use crate::types::{EditorTabConfig, TabConfig, TerminalTabConfig};
use crate::ui::pane::{Axis, Pane, PaneEvent, SplitDirection, TabItem};
use crate::ui::{EditorView, TerminalView};
use gpui::prelude::*;
use gpui::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

#[derive(Clone)]
pub enum Member {
    Pane(Entity<Pane>),
    Axis(PaneAxis),
}

impl Member {
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

#[derive(Clone)]
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
    pub axis: Axis,
    pub divider_index: usize,
    pub axis_path: Vec<usize>,
    pub container_size: f32,
}

impl DividerDrag {
    pub fn new(axis: Axis, divider_index: usize, axis_path: Vec<usize>, container_size: f32) -> Self {
        Self {
            axis,
            divider_index,
            axis_path,
            container_size,
        }
    }
}

impl Render for DividerDrag {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

pub struct PaneStore {
    workspace_id: String,
    pub root: Member,
    pub active_pane: Option<Entity<Pane>>,
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
    pub fn load(
        workspace_id: String,
        workspace_path: PathBuf,
        buffer_store: Entity<EditorStore>,
        cx: &mut Context<Self>,
    ) -> Self {
        let layout = Self::load_layout_with_migration(&workspace_id, &workspace_path);
        let member = Self::create_member_from_layout(&layout, &workspace_path, &buffer_store, cx);
        let active_pane = member.first_pane();
        let mut store = Self::with_root(workspace_id, member, cx);
        store.active_pane = active_pane;

        if let Some(pane) = &store.active_pane {
            let tabs_count = pane.read(cx).tabs.len();
            if tabs_count == 0 {
                pane.update(cx, |p, cx| {
                    p.add_terminal(cx);
                });
            }
        }

        store
    }

    fn load_layout_with_migration(workspace_id: &str, workspace_path: &PathBuf) -> LayoutNode {
        let layout_path = config::layout_path(workspace_id);
        if layout_path.exists() {
            config::load_json(&layout_path)
        } else if config::legacy_state_exists() {
            log::info!(
                "Migrating layout for workspace {} from legacy state.json",
                workspace_id
            );
            let legacy = config::load_state();
            if let Some(wc) = legacy.workspaces.iter().find(|w| w.id == workspace_id) {
                let layout = Self::convert_member_config_to_layout(&wc.layout, workspace_path);
                config::save_json(&layout_path, &layout);
                layout
            } else {
                LayoutNode::default()
            }
        } else {
            LayoutNode::default()
        }
    }

    fn create_member_from_layout(
        layout: &LayoutNode,
        path: &PathBuf,
        buffer_store: &Entity<EditorStore>,
        cx: &mut Context<Self>,
    ) -> Member {
        match layout {
            LayoutNode::Pane(pane_config) => {
                let pane = cx.new(|cx| {
                    let mut pane = Pane::new(path.clone(), cx);

                    for tab_config in &pane_config.tabs {
                        match tab_config {
                            TabConfig::Terminal(term_config) => {
                                let cwd = if term_config.cwd.exists() {
                                    term_config.cwd.clone()
                                } else {
                                    path.clone()
                                };
                                let terminal_store = TerminalStore::global(cx);
                                if let Some((_id, terminal)) =
                                    terminal_store.update(cx, |store, cx| {
                                        store.create_terminal(cwd, cx)
                                    })
                                {
                                    let terminal_view = cx.new(|cx| TerminalView::new(terminal, cx));
                                    pane.tabs.push(TabItem::Terminal(terminal_view));
                                }
                            }
                            TabConfig::Editor(editor_config) => {
                                if editor_config.path.exists() {
                                    if let Some(buffer) = buffer_store.update(cx, |store, cx| {
                                        store.open_buffer(editor_config.path.clone(), cx)
                                    }) {
                                        let editor = cx.new(|cx| {
                                            EditorView::new(buffer, editor_config.path.clone(), cx)
                                        });
                                        pane.tabs.push(TabItem::Editor(editor));
                                    }
                                }
                            }
                        }
                    }

                    if pane.tabs.is_empty() {
                        pane.add_terminal(cx);
                    }

                    pane.active_index =
                        pane_config.active_index.min(pane.tabs.len().saturating_sub(1));
                    pane
                });

                Member::Pane(pane)
            }
            LayoutNode::Split {
                axis,
                ratios,
                children,
            } => {
                let axis = Axis::from(*axis);
                let members: Vec<Member> = children
                    .iter()
                    .map(|child| Self::create_member_from_layout(child, path, buffer_store, cx))
                    .collect();

                Member::Axis(PaneAxis {
                    axis,
                    members,
                    ratios: ratios.clone(),
                })
            }
        }
    }

    fn convert_member_config_to_layout(config: &MemberConfig, default_path: &PathBuf) -> LayoutNode {
        match config {
            MemberConfig::Pane {
                terminal_count,
                active_index,
                open_files,
            } => {
                let mut tabs = Vec::new();

                for _ in 0..*terminal_count {
                    tabs.push(TabConfig::Terminal(TerminalTabConfig {
                        cwd: default_path.clone(),
                    }));
                }

                for file_path in open_files {
                    tabs.push(TabConfig::Editor(EditorTabConfig {
                        path: file_path.clone(),
                    }));
                }

                if tabs.is_empty() {
                    tabs.push(TabConfig::Terminal(TerminalTabConfig {
                        cwd: default_path.clone(),
                    }));
                }

                LayoutNode::Pane(PaneConfig {
                    tabs,
                    active_index: *active_index,
                })
            }
            MemberConfig::Axis {
                axis,
                ratios,
                members,
            } => {
                let layout_axis = match axis {
                    config::Axis::Horizontal => LayoutAxis::Horizontal,
                    config::Axis::Vertical => LayoutAxis::Vertical,
                };
                let children: Vec<LayoutNode> = members
                    .iter()
                    .map(|m| Self::convert_member_config_to_layout(m, default_path))
                    .collect();

                LayoutNode::Split {
                    axis: layout_axis,
                    ratios: ratios.clone(),
                    children,
                }
            }
        }
    }

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
            drag_start: None,
        }
    }

    fn save_layout(&self, cx: &App) {
        let layout = self.collect_layout(cx);
        config::save_json(&config::layout_path(&self.workspace_id), &layout);
    }

    fn emit_state_changed(&mut self, cx: &mut Context<Self>) {
        self.save_layout(cx);
        cx.emit(PaneStoreEvent::StateChanged);
    }

    /// Collect layout tree from current pane structure
    fn collect_layout(&self, cx: &App) -> LayoutNode {
        self.collect_member_layout(&self.root, cx)
    }

    fn collect_member_layout(&self, member: &Member, cx: &App) -> LayoutNode {
        match member {
            Member::Pane(pane) => {
                let pane = pane.read(cx);
                let tabs = pane.tabs.iter().map(|tab| tab.to_config(cx)).collect();

                LayoutNode::Pane(PaneConfig {
                    tabs,
                    active_index: pane.active_index,
                })
            }
            Member::Axis(axis) => LayoutNode::Split {
                axis: axis.axis.into(),
                ratios: axis.ratios.clone(),
                children: axis
                    .members
                    .iter()
                    .map(|m| self.collect_member_layout(m, cx))
                    .collect(),
            },
        }
    }

    fn subscribe_to_pane(pane: &Entity<Pane>, cx: &mut Context<Self>) {
        cx.subscribe(pane, move |this, pane, event, cx| match event {
            PaneEvent::Split {
                direction,
                new_pane,
            } => {
                this.split_pane(&pane, new_pane.clone(), *direction, cx);
            }
            PaneEvent::TabMoved | PaneEvent::TerminalAdded | PaneEvent::TabClosed => {
                let is_empty = pane.read(cx).tabs.is_empty();
                if is_empty {
                    this.remove_pane(&pane, cx);
                }
                this.emit_state_changed(cx);
            }
            PaneEvent::Focus => {
                this.active_pane = Some(pane.clone());
                cx.emit(PaneStoreEvent::PaneFocused);
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
        Self::subscribe_to_pane(&new_pane, cx);

        match &mut self.root {
            Member::Pane(pane) if pane == target => {
                let split_axis = direction.axis();
                let is_before = direction.is_before();

                let old_pane = std::mem::replace(&mut self.root, Member::Pane(new_pane.clone()));
                let members = if is_before {
                    vec![Member::Pane(new_pane.clone()), old_pane]
                } else {
                    vec![old_pane, Member::Pane(new_pane.clone())]
                };
                self.root = Member::Axis(PaneAxis::new(split_axis, members));

            }
            Member::Axis(axis) => {
                axis.find_and_split_pane(target, new_pane.clone(), direction, cx);
            }
            _ => {
                log::info!("  No match for split target!");
            }
        }

        self.active_pane = Some(new_pane.clone());
        cx.emit(PaneStoreEvent::PaneAdded);
        self.emit_state_changed(cx);
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
        self.emit_state_changed(cx);
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

    pub fn get_ratios_at_path(&self, path: &[usize]) -> Vec<f32> {
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

    pub fn handle_divider_drag(
        &mut self,
        drag_data: &DividerDrag,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let axis_path = &drag_data.axis_path;

        // Initialize drag state if needed
        let needs_init = match &self.drag_start {
            None => true,
            Some(ds) => {
                ds.axis_path != *axis_path || ds.divider_index != drag_data.divider_index
            }
        };

        if needs_init {
            let ratios = self.get_ratios_at_path(axis_path);
            self.drag_start = Some(DragStart {
                axis_path: axis_path.to_vec(),
                divider_index: drag_data.divider_index,
                start_position: position,
                initial_ratios: ratios,
            });
        }

        let drag_start = match &self.drag_start {
            Some(ds) => ds.clone(),
            None => return,
        };

        let pixel_delta = if drag_data.axis == Axis::Horizontal {
            position.x - drag_start.start_position.x
        } else {
            position.y - drag_start.start_position.y
        };

        let num_dividers = drag_start.initial_ratios.len().saturating_sub(1);
        let available_size = drag_data.container_size - (num_dividers as f32 * DIVIDER_SIZE);

        if available_size <= 0.0 {
            return;
        }

        let delta_ratio = f32::from(pixel_delta) / available_size;
        let min_ratio = MIN_PANE_SIZE / available_size;

        let mut new_ratios = drag_start.initial_ratios.clone();
        let left_index = drag_data.divider_index;
        let right_index = drag_data.divider_index + 1;

        if left_index < new_ratios.len() && right_index < new_ratios.len() {
            new_ratios[left_index] = (new_ratios[left_index] + delta_ratio).max(min_ratio);
            new_ratios[right_index] = (new_ratios[right_index] - delta_ratio).max(min_ratio);

            let total: f32 = new_ratios.iter().sum();
            for ratio in &mut new_ratios {
                *ratio /= total;
            }

            if let Some(axis) = self.get_axis_at_path_mut(axis_path) {
                axis.ratios = new_ratios;
                self.emit_state_changed(cx);
                cx.notify();
            }
        }
    }

    pub fn clear_drag_state(&mut self) {
        self.drag_start = None;
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
                    self.emit_state_changed(cx);
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
