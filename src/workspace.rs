use gpui::*;
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

pub enum WorkspaceEvent {
    ActiveWorkspaceChanged,
}

impl EventEmitter<WorkspaceEvent> for WorkspaceManager {}

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
