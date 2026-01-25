mod workspace;

pub use workspace::{Workspace, WorkspaceData, WorkspaceEvent};

use crate::config::{self, MemberConfig};
use crate::editor::EditorStore;
use crate::file_tree_store::FileTreeStore;
use crate::terminal_store::TerminalStore;
use crate::types::{EditorTabConfig, TabConfig, TerminalTabConfig};
use crate::ui::pane::{Axis, Pane, TabItem};
use crate::ui::pane_store::{LayoutAxis, LayoutNode, Member, PaneAxis, PaneConfig, PaneStore};
use crate::ui::{EditorView, TerminalView};
use gpui::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Clone)]
#[allow(dead_code)]
pub enum WorkspaceStoreEvent {
    WorkspacesChanged,
    WorkspaceUpdated(String),
    PaneLayoutChanged(String),
}

impl EventEmitter<WorkspaceStoreEvent> for WorkspaceStore {}

#[derive(Serialize, Deserialize, Default)]
pub struct WorkspacesConfig {
    pub workspaces: Vec<WorkspaceData>,
}

pub struct WorkspaceStore {
    workspaces: HashMap<String, Entity<Workspace>>,
    workspace_order: Vec<String>,
    _subscriptions: Vec<Subscription>,
}

#[allow(dead_code)]
pub struct GlobalWorkspaceStore(pub Entity<WorkspaceStore>);

impl Global for GlobalWorkspaceStore {}

impl WorkspaceStore {
    pub fn init(cx: &mut App) -> Entity<Self> {
        let store = cx.new(|cx| Self::load_with_migration(cx));
        cx.set_global(GlobalWorkspaceStore(store.clone()));
        store
    }

    #[allow(dead_code)]
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalWorkspaceStore>().0.clone()
    }

    fn load_with_migration(cx: &mut Context<Self>) -> Self {
        let config: WorkspacesConfig = config::load_json(&config::workspaces_path());

        // If new format is empty but legacy exists, migrate
        if config.workspaces.is_empty() && config::legacy_state_exists() {
            log::info!("Migrating workspaces from legacy state.json");
            let legacy = config::load_state();
            let workspace_data: Vec<WorkspaceData> = legacy
                .workspaces
                .into_iter()
                .map(|wc| WorkspaceData {
                    id: wc.id,
                    name: wc.name,
                    path: wc.path,
                })
                .collect();

            let (workspaces, workspace_order, subscriptions) =
                Self::create_workspaces_from_data(&workspace_data, cx);

            let store = Self {
                workspaces,
                workspace_order,
                _subscriptions: subscriptions,
            };

            store.save(cx);
            return store;
        }

        let (workspaces, workspace_order, subscriptions) =
            Self::create_workspaces_from_data(&config.workspaces, cx);

        Self {
            workspaces,
            workspace_order,
            _subscriptions: subscriptions,
        }
    }

    fn create_workspaces_from_data(
        data: &[WorkspaceData],
        cx: &mut Context<Self>,
    ) -> (HashMap<String, Entity<Workspace>>, Vec<String>, Vec<Subscription>) {
        let mut workspaces = HashMap::new();
        let mut workspace_order = Vec::new();
        let mut subscriptions = Vec::new();

        let buffer_store = EditorStore::global(cx);

        for ws_data in data {
            let id = ws_data.id.clone();
            let path = ws_data.path.clone();
            let name = ws_data.name.clone();

            let file_tree_store =
                cx.new(|cx| FileTreeStore::new(id.clone(), path.clone(), cx));

            let pane_store = Self::create_pane_store(&id, &path, &buffer_store, cx);

            let workspace = cx.new(|cx| Workspace::new(id.clone(), name, path, file_tree_store, pane_store, cx));

            let sub = cx.subscribe(&workspace, {
                let id = id.clone();
                move |this, workspace, event, cx| {
                    match event {
                        WorkspaceEvent::Updated => {
                            this.save(cx);
                            cx.emit(WorkspaceStoreEvent::WorkspaceUpdated(id.clone()));
                        }
                        WorkspaceEvent::FileTreeChanged => {
                            cx.emit(WorkspaceStoreEvent::WorkspaceUpdated(id.clone()));
                        }
                        WorkspaceEvent::PaneLayoutChanged => {
                            workspace.read(cx).pane_store().read(cx).save_layout(cx);
                            cx.emit(WorkspaceStoreEvent::PaneLayoutChanged(id.clone()));
                        }
                    }
                }
            });

            workspaces.insert(id.clone(), workspace);
            workspace_order.push(id);
            subscriptions.push(sub);
        }

        (workspaces, workspace_order, subscriptions)
    }

    fn create_pane_store(
        workspace_id: &str,
        path: &PathBuf,
        buffer_store: &Entity<EditorStore>,
        cx: &mut Context<Self>,
    ) -> Entity<PaneStore> {
        let layout_path = config::layout_path(workspace_id);
        let layout: LayoutNode = if layout_path.exists() {
            config::load_json(&layout_path)
        } else if config::legacy_state_exists() {
            log::info!("Migrating layout for workspace {} from legacy state.json", workspace_id);
            let legacy = config::load_state();
            if let Some(wc) = legacy.workspaces.iter().find(|w| w.id == workspace_id) {
                let layout = Self::convert_member_config_to_layout(&wc.layout, path);
                config::save_json(&layout_path, &layout);
                layout
            } else {
                LayoutNode::default()
            }
        } else {
            LayoutNode::default()
        };

        let path_clone = path.clone();
        let workspace_id_owned = workspace_id.to_string();
        let buffer_store_clone = buffer_store.clone();

        cx.new(|cx| {
            let member = Self::create_member_from_layout(&layout, &path_clone, &buffer_store_clone, cx);
            let active_pane = member.first_pane();
            let mut store = PaneStore::with_root(workspace_id_owned, member, cx);
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
        })
    }

    fn create_member_from_layout(
        layout: &LayoutNode,
        path: &PathBuf,
        buffer_store: &Entity<EditorStore>,
        cx: &mut Context<PaneStore>,
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
                                if let Some((_id, terminal)) = terminal_store.update(cx, |store, cx| {
                                    store.create_terminal(cwd, cx)
                                }) {
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

                    pane.active_index = pane_config.active_index.min(pane.tabs.len().saturating_sub(1));
                    pane
                });

                Member::Pane(pane)
            }
            LayoutNode::Split { axis, ratios, children } => {
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
            MemberConfig::Axis { axis, ratios, members } => {
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

    pub fn save(&self, cx: &App) {
        let workspaces: Vec<WorkspaceData> = self
            .workspace_order
            .iter()
            .filter_map(|id| self.workspaces.get(id))
            .map(|ws| WorkspaceData::from_entity(ws, cx))
            .collect();

        let config = WorkspacesConfig { workspaces };
        config::save_json(&config::workspaces_path(), &config);
    }

    pub fn add_workspace(
        &mut self,
        name: String,
        path: PathBuf,
        cx: &mut Context<Self>,
    ) -> Entity<Workspace> {
        let id = Uuid::new_v4().to_string();
        self.add_workspace_with_id(id, name, path, cx)
    }

    pub fn add_workspace_with_id(
        &mut self,
        id: String,
        name: String,
        path: PathBuf,
        cx: &mut Context<Self>,
    ) -> Entity<Workspace> {
        let buffer_store = EditorStore::global(cx);

        let file_tree_store =
            cx.new(|cx| FileTreeStore::new(id.clone(), path.clone(), cx));

        let pane_store = Self::create_pane_store(&id, &path, &buffer_store, cx);

        let workspace = cx.new(|cx| Workspace::new(id.clone(), name, path, file_tree_store, pane_store, cx));

        let sub = cx.subscribe(&workspace, {
            let id = id.clone();
            move |this, workspace, event, cx| {
                match event {
                    WorkspaceEvent::Updated => {
                        this.save(cx);
                        cx.emit(WorkspaceStoreEvent::WorkspaceUpdated(id.clone()));
                    }
                    WorkspaceEvent::FileTreeChanged => {
                        cx.emit(WorkspaceStoreEvent::WorkspaceUpdated(id.clone()));
                    }
                    WorkspaceEvent::PaneLayoutChanged => {
                        workspace.read(cx).pane_store().read(cx).save_layout(cx);
                        cx.emit(WorkspaceStoreEvent::PaneLayoutChanged(id.clone()));
                    }
                }
            }
        });

        self.workspaces.insert(id.clone(), workspace.clone());
        self.workspace_order.push(id);
        self._subscriptions.push(sub);

        self.save(cx);
        cx.emit(WorkspaceStoreEvent::WorkspacesChanged);
        cx.notify();

        workspace
    }

    #[allow(dead_code)]
    pub fn remove_workspace(&mut self, id: &str, cx: &mut Context<Self>) -> bool {
        if self.workspaces.len() <= 1 {
            return false;
        }

        if self.workspaces.remove(id).is_some() {
            self.workspace_order.retain(|ws_id| ws_id != id);
            self.save(cx);
            cx.emit(WorkspaceStoreEvent::WorkspacesChanged);
            cx.notify();
            true
        } else {
            false
        }
    }

    pub fn get_workspace(&self, id: &str) -> Option<&Entity<Workspace>> {
        self.workspaces.get(id)
    }

    pub fn workspaces(&self) -> impl Iterator<Item = WorkspaceRef<'_>> {
        self.workspace_order
            .iter()
            .filter_map(|id| self.workspaces.get(id).map(|ws| WorkspaceRef { id, entity: ws }))
    }

    pub fn workspace_ids(&self) -> impl Iterator<Item = &String> {
        self.workspace_order.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.workspaces.is_empty()
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.workspaces.len()
    }

    pub fn first_workspace_id(&self) -> Option<&String> {
        self.workspace_order.first()
    }
}

#[allow(dead_code)]
pub struct WorkspaceRef<'a> {
    pub id: &'a String,
    pub entity: &'a Entity<Workspace>,
}
