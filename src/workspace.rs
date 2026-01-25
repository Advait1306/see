use gpui::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use crate::config;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
}

impl Workspace {
    pub fn new(name: String, path: PathBuf) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            path,
        }
    }
}

/// Serializable workspace data for persistence
#[derive(Serialize, Deserialize, Clone)]
pub struct WorkspaceData {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
}

impl From<&Workspace> for WorkspaceData {
    fn from(ws: &Workspace) -> Self {
        Self {
            id: ws.id.clone(),
            name: ws.name.clone(),
            path: ws.path.clone(),
        }
    }
}

impl From<WorkspaceData> for Workspace {
    fn from(data: WorkspaceData) -> Self {
        Self {
            id: data.id,
            name: data.name,
            path: data.path,
        }
    }
}

/// Configuration for workspaces persistence
#[derive(Serialize, Deserialize, Default)]
pub struct WorkspacesConfig {
    pub workspaces: Vec<WorkspaceData>,
    pub active_index: Option<usize>,
}

pub struct WorkspaceStore {
    pub workspaces: Vec<Workspace>,
    pub active_workspace_index: Option<usize>,
}

impl Global for WorkspaceStore {}

pub enum WorkspaceEvent {
    ActiveWorkspaceChanged,
}

impl EventEmitter<WorkspaceEvent> for WorkspaceStore {}

impl WorkspaceStore {
    pub fn init(cx: &mut App) {
        let store = Self {
            workspaces: Vec::new(),
            active_workspace_index: None,
        };
        cx.set_global(store);
    }

    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }

    /// Load workspaces from persistent storage
    pub fn load(&mut self) {
        let config: WorkspacesConfig = config::load_json(&config::workspaces_path());
        self.workspaces = config.workspaces.into_iter().map(Workspace::from).collect();
        self.active_workspace_index = config.active_index;
    }

    /// Save workspaces to persistent storage
    pub fn save(&self) {
        let config = WorkspacesConfig {
            workspaces: self.workspaces.iter().map(WorkspaceData::from).collect(),
            active_index: self.active_workspace_index,
        };
        config::save_json(&config::workspaces_path(), &config);
    }

    pub fn add_workspace(&mut self, name: String, path: PathBuf) -> &Workspace {
        let workspace = Workspace::new(name, path);
        self.workspaces.push(workspace);
        let index = self.workspaces.len() - 1;
        if self.active_workspace_index.is_none() {
            self.active_workspace_index = Some(index);
        }
        &self.workspaces[index]
    }

    /// Add a workspace with a specific ID (for restoring from config)
    pub fn add_workspace_with_id(&mut self, id: String, name: String, path: PathBuf) -> &Workspace {
        let workspace = Workspace { id, name, path };
        self.workspaces.push(workspace);
        let index = self.workspaces.len() - 1;
        if self.active_workspace_index.is_none() {
            self.active_workspace_index = Some(index);
        }
        &self.workspaces[index]
    }

    pub fn active_workspace(&self) -> Option<&Workspace> {
        self.active_workspace_index
            .and_then(|i| self.workspaces.get(i))
    }

    pub fn set_active(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.workspaces.len() && self.active_workspace_index != Some(index) {
            self.active_workspace_index = Some(index);
            cx.emit(WorkspaceEvent::ActiveWorkspaceChanged);
        }
    }

    pub fn next_workspace(&mut self, cx: &mut Context<Self>) {
        if self.workspaces.len() > 1 {
            if let Some(current) = self.active_workspace_index {
                let new_index = (current + 1) % self.workspaces.len();
                self.set_active(new_index, cx);
            }
        }
    }

    pub fn prev_workspace(&mut self, cx: &mut Context<Self>) {
        if self.workspaces.len() > 1 {
            if let Some(current) = self.active_workspace_index {
                let new_index = if current == 0 {
                    self.workspaces.len() - 1
                } else {
                    current - 1
                };
                self.set_active(new_index, cx);
            }
        }
    }
}
