use crate::config;
use super::{Workspace, WorkspaceData, WorkspaceEvent};
use gpui::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Clone)]
pub enum WorkspaceStoreEvent {
    WorkspacesChanged,
    WorkspaceRemoved { next_workspace_id: Option<String> },
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

pub struct GlobalWorkspaceStore(pub Entity<WorkspaceStore>);

impl Global for GlobalWorkspaceStore {}

impl WorkspaceStore {
    pub fn init(cx: &mut App) {
        let store = cx.new(|cx| Self::load(cx));
        cx.set_global(GlobalWorkspaceStore(store));
    }

    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalWorkspaceStore>().0.clone()
    }

    fn load(cx: &mut Context<Self>) -> Self {
        let config: WorkspacesConfig = config::load_json(&config::workspaces_path());

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
                move |_this, _workspace, event, cx| {
                    match event {
                        WorkspaceEvent::FileTreeChanged => {
                            cx.emit(WorkspaceStoreEvent::WorkspaceUpdated(id.clone()));
                        }
                        WorkspaceEvent::PaneLayoutChanged => {
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
            move |_this, _workspace, event, cx| {
                match event {
                    WorkspaceEvent::FileTreeChanged => {
                        cx.emit(WorkspaceStoreEvent::WorkspaceUpdated(id.clone()));
                    }
                    WorkspaceEvent::PaneLayoutChanged => {
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

    /// Returns the ID of the workspace that should be selected after removing the given workspace.
    /// If the workspace is not the last one, returns the next workspace.
    /// If it is the last one, returns the previous workspace.
    /// Returns None if this is the only workspace or the workspace doesn't exist.
    pub fn next_workspace_after_removal(&self, id: &str) -> Option<String> {
        let idx = self.workspace_order.iter().position(|ws_id| ws_id == id)?;

        if self.workspace_order.len() <= 1 {
            return None;
        }

        if idx < self.workspace_order.len() - 1 {
            // Not the last workspace, select the next one
            Some(self.workspace_order[idx + 1].clone())
        } else {
            // Last workspace, select the previous one
            Some(self.workspace_order[idx - 1].clone())
        }
    }

    pub fn remove_workspace(&mut self, id: &str, cx: &mut Context<Self>) {
        // Calculate the next workspace before removal
        let next_workspace_id = self.next_workspace_after_removal(id);

        if self.workspaces.remove(id).is_some() {
            self.workspace_order.retain(|ws_id| ws_id != id);
            self.save(cx);
            cx.emit(WorkspaceStoreEvent::WorkspaceRemoved { next_workspace_id });
            cx.emit(WorkspaceStoreEvent::WorkspacesChanged);
            cx.notify();
        }
    }
}

pub struct WorkspaceRef<'a> {
    pub entity: &'a Entity<Workspace>,
}
