//! PaneStore manages the layout of panes within a workspace.
//!
//! # Architecture
//!
//! The pane layout is a tree structure where:
//! - Leaf nodes are `Member::Pane` containing a single `Pane` entity with tabs
//! - Internal nodes are `Member::Axis` containing multiple children split along an axis
//!
//! # Layout Persistence
//!
//! Layouts are saved to `layouts/{workspace_id}.json` and restored on workspace load.
//! The `LayoutNode` enum is the serializable representation of the tree.
//!
//! # Splitting and Removing Panes
//!
//! When a pane is split, the tree is modified to insert a new `PaneAxis` node.
//! When a pane is removed and its parent axis has only one child left, the axis
//! is collapsed to simplify the tree.

use crate::config;
use crate::stores::TerminalStore;
use crate::types::TabConfig;
use crate::ui::pane::{Axis, DividerDrag, Pane, PaneEvent, SplitDirection, TabItem};
use crate::ui::{EditorView, TerminalView};
use gpui::{App, AppContext as _, Context, Entity, EventEmitter, Pixels, Point};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const MIN_PANE_SIZE: f32 = 100.0;
const DIVIDER_SIZE: f32 = 4.0;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PaneConfig {
    pub tabs: Vec<TabConfig>,
    pub active_index: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum LayoutNode {
    Pane(PaneConfig),
    Split {
        axis: Axis,
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
        cx: &mut Context<Self>,
    ) -> Self {
        let layout_path = config::layout_path(&workspace_id);
        let layout: LayoutNode = config::load_json(&layout_path);

        let member = Self::create_member_from_layout(&layout, &workspace_path, cx);
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

    fn create_member_from_layout(
        layout: &LayoutNode,
        path: &PathBuf,
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
                                let editor = cx.new(|cx| {
                                    EditorView::new(
                                        editor_config.path.clone(),
                                        Default::default(),
                                        cx,
                                    )
                                });
                                pane.tabs.push(TabItem::Editor(editor));
                            }
                            TabConfig::PrReview(_) => {
                                // PR review tabs require a GitHubStore and are not
                                // restored from layout — they're opened via the PR list
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
                let members: Vec<Member> = children
                    .iter()
                    .map(|child| Self::create_member_from_layout(child, path, cx))
                    .collect();

                Member::Axis(PaneAxis {
                    axis: *axis,
                    members,
                    ratios: ratios.clone(),
                })
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
                axis: axis.axis,
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
            PaneEvent::TabMoved | PaneEvent::TabAdded | PaneEvent::TabClosed => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    fn make_pane(cx: &mut TestAppContext) -> Entity<Pane> {
        cx.new(|cx| Pane::new(PathBuf::from("/tmp"), cx))
    }

    #[test]
    fn test_pane_store_split_root_pane() {
        crate::test_helpers::run_gpui_test(|cx| {
            let fixture = crate::test_helpers::TestFixture::new(cx);
            let _ = &fixture; // keep alive for config isolation

            let pane1 = make_pane(cx);
            let store = cx.new(|cx| {
                PaneStore::with_root("test".to_string(), Member::Pane(pane1.clone()), cx)
            });

            let new_pane = make_pane(cx);
            store.update(cx, |store, cx| {
                store.split_pane(&pane1, new_pane.clone(), SplitDirection::Right, cx);
            });

            cx.read(|cx| {
                let s = store.read(cx);
                assert_eq!(s.pane_count(), 2);
                assert!(matches!(s.root, Member::Axis(_)));
            });
        });
    }

    #[test]
    fn test_pane_store_remove_pane_collapses_axis() {
        crate::test_helpers::run_gpui_test(|cx| {
            let fixture = crate::test_helpers::TestFixture::new(cx);
            let _ = &fixture;

            let pane1 = make_pane(cx);
            let store = cx.new(|cx| {
                PaneStore::with_root("test".to_string(), Member::Pane(pane1.clone()), cx)
            });

            let pane2 = make_pane(cx);
            store.update(cx, |store, cx| {
                store.split_pane(&pane1, pane2.clone(), SplitDirection::Right, cx);
            });

            store.update(cx, |store, cx| {
                store.remove_pane(&pane2, cx);
            });

            cx.read(|cx| {
                let s = store.read(cx);
                assert_eq!(s.pane_count(), 1);
                assert!(matches!(s.root, Member::Pane(_)));
            });
        });
    }

    #[test]
    fn test_pane_store_active_pane_updates_on_split() {
        crate::test_helpers::run_gpui_test(|cx| {
            let fixture = crate::test_helpers::TestFixture::new(cx);
            let _ = &fixture;

            let pane1 = make_pane(cx);
            let store = cx.new(|cx| {
                PaneStore::with_root("test".to_string(), Member::Pane(pane1.clone()), cx)
            });

            let pane2 = make_pane(cx);
            store.update(cx, |store, cx| {
                store.split_pane(&pane1, pane2.clone(), SplitDirection::Right, cx);
            });

            cx.read(|cx| {
                let s = store.read(cx);
                // After split, active pane should be the new pane
                assert_eq!(s.active_pane.as_ref().unwrap().entity_id(), pane2.entity_id());
            });
        });
    }

    #[test]
    fn test_pane_store_remove_active_pane_falls_back() {
        crate::test_helpers::run_gpui_test(|cx| {
            let fixture = crate::test_helpers::TestFixture::new(cx);
            let _ = &fixture;

            let pane1 = make_pane(cx);
            let store = cx.new(|cx| {
                PaneStore::with_root("test".to_string(), Member::Pane(pane1.clone()), cx)
            });

            let pane2 = make_pane(cx);
            store.update(cx, |store, cx| {
                store.split_pane(&pane1, pane2.clone(), SplitDirection::Right, cx);
            });

            // Active pane is pane2 after split. Remove it.
            store.update(cx, |store, cx| {
                store.remove_pane(&pane2, cx);
            });

            cx.read(|cx| {
                let s = store.read(cx);
                // Should fall back to remaining pane
                assert!(s.active_pane.is_some());
                assert_eq!(s.active_pane.as_ref().unwrap().entity_id(), pane1.entity_id());
            });
        });
    }

    #[test]
    fn test_pane_axis_equal_ratios() {
        crate::test_helpers::run_gpui_test(|cx| {
            let fixture = crate::test_helpers::TestFixture::new(cx);
            let _ = &fixture;

            let pane1 = make_pane(cx);
            let pane2 = make_pane(cx);
            let pane3 = make_pane(cx);

            let axis = PaneAxis::new(
                Axis::Horizontal,
                vec![
                    Member::Pane(pane1),
                    Member::Pane(pane2),
                    Member::Pane(pane3),
                ],
            );

            let expected = 1.0 / 3.0;
            for ratio in &axis.ratios {
                assert!((ratio - expected).abs() < 0.001);
            }
        });
    }

    #[test]
    fn test_pane_axis_remove_normalizes_ratios() {
        crate::test_helpers::run_gpui_test(|cx| {
            let fixture = crate::test_helpers::TestFixture::new(cx);
            let _ = &fixture;

            let pane1 = make_pane(cx);
            let pane2 = make_pane(cx);
            let pane3 = make_pane(cx);

            let mut axis = PaneAxis::new(
                Axis::Horizontal,
                vec![
                    Member::Pane(pane1),
                    Member::Pane(pane2.clone()),
                    Member::Pane(pane3),
                ],
            );

            axis.remove_pane(&pane2);

            assert_eq!(axis.members.len(), 2);
            assert_eq!(axis.ratios.len(), 2);
            let total: f32 = axis.ratios.iter().sum();
            assert!((total - 1.0).abs() < 0.001, "Ratios should sum to 1.0, got {}", total);
        });
    }
}
