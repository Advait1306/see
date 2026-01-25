use crate::config;
use super::workspace::{Workspace, WorkspaceData, WorkspaceEvent};
use gpui::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Clone)]
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

// Stored as global for future use (pattern matches EditorStore, TerminalStore)
#[allow(dead_code)]
struct GlobalWorkspaceStore(Entity<WorkspaceStore>);

impl Global for GlobalWorkspaceStore {}

impl WorkspaceStore {
    pub fn init(cx: &mut App) -> Entity<Self> {
        let store = cx.new(|cx| Self::load_with_migration(cx));
        cx.set_global(GlobalWorkspaceStore(store.clone()));
        store
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

        for ws_data in data {
            let id = ws_data.id.clone();
            let path = ws_data.path.clone();
            let name = ws_data.name.clone();

            let workspace = cx.new(|cx| Workspace::new(id.clone(), name, path, cx));

            let sub = cx.subscribe(&workspace, {
                let id = id.clone();
                move |_this, workspace, event, cx| {
                    match event {
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
        let workspace = cx.new(|cx| Workspace::new(id.clone(), name, path, cx));

        let sub = cx.subscribe(&workspace, {
            let id = id.clone();
            move |_this, workspace, event, cx| {
                match event {
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

    pub fn get_workspace(&self, id: &str) -> Option<&Entity<Workspace>> {
        self.workspaces.get(id)
    }

    pub fn workspaces(&self) -> impl Iterator<Item = WorkspaceRef<'_>> {
        self.workspace_order
            .iter()
            .filter_map(|id| self.workspaces.get(id).map(|ws| WorkspaceRef { entity: ws }))
    }

    pub fn workspace_ids(&self) -> impl Iterator<Item = &String> {
        self.workspace_order.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.workspaces.is_empty()
    }

    pub fn first_workspace_id(&self) -> Option<&String> {
        self.workspace_order.first()
    }
}

pub struct WorkspaceRef<'a> {
    pub entity: &'a Entity<Workspace>,
}
