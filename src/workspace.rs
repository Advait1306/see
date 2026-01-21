use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

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

pub struct WorkspaceManager {
    pub workspaces: Vec<Workspace>,
    pub active_workspace_index: Option<usize>,
}

impl WorkspaceManager {
    pub fn new() -> Self {
        Self {
            workspaces: Vec::new(),
            active_workspace_index: None,
        }
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

    pub fn active_workspace(&self) -> Option<&Workspace> {
        self.active_workspace_index
            .and_then(|i| self.workspaces.get(i))
    }

    pub fn set_active(&mut self, index: usize) {
        if index < self.workspaces.len() {
            self.active_workspace_index = Some(index);
        }
    }

    pub fn remove_workspace(&mut self, index: usize) {
        if index < self.workspaces.len() {
            self.workspaces.remove(index);
            if let Some(active) = self.active_workspace_index {
                if active >= self.workspaces.len() {
                    self.active_workspace_index = if self.workspaces.is_empty() {
                        None
                    } else {
                        Some(self.workspaces.len() - 1)
                    };
                } else if active > index {
                    self.active_workspace_index = Some(active - 1);
                }
            }
        }
    }
}
