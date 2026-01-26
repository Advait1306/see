use gpui::*;
use serde::{Deserialize, Serialize};

use crate::config;
use crate::workspace::Workspace;
use super::{WorkspaceStore, WorkspaceStoreEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RightSidebarPanel {
    #[default]
    Hidden,
    FileTree,
    DiffList,
}

#[derive(Clone)]
pub enum WindowStoreEvent {
    ActiveWorkspaceChanged,
    UiStateChanged,
}

#[derive(Serialize, Deserialize, Default)]
pub struct WindowUiState {
    pub active_workspace_id: Option<String>,
    pub sidebar_collapsed: bool,
    pub right_sidebar: RightSidebarPanel,
}

pub struct WindowStore {
    workspace_store: Entity<WorkspaceStore>,
    active_workspace_id: Option<String>,
    sidebar_collapsed: bool,
    right_sidebar: RightSidebarPanel,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<WindowStoreEvent> for WindowStore {}

impl WindowStore {
    pub fn new(workspace_store: Entity<WorkspaceStore>, cx: &mut Context<Self>) -> Self {
        let ui_state: WindowUiState = config::load_json(&config::ui_state_path());

        // Determine initial active workspace:
        // 1. Try saved active_workspace_id if it still exists
        // 2. Fall back to first workspace if available
        let active_workspace_id = {
            let store = workspace_store.read(cx);
            if let Some(saved_id) = &ui_state.active_workspace_id {
                if store.get_workspace(saved_id).is_some() {
                    Some(saved_id.clone())
                } else {
                    store.first_workspace_id().cloned()
                }
            } else {
                store.first_workspace_id().cloned()
            }
        };

        // Subscribe to workspace store events
        let sub = cx.subscribe(&workspace_store, |this, _, event, cx| {
            this.on_workspace_event(event, cx);
        });

        Self {
            workspace_store,
            active_workspace_id,
            sidebar_collapsed: ui_state.sidebar_collapsed,
            right_sidebar: ui_state.right_sidebar,
            _subscriptions: vec![sub],
        }
    }

    fn on_workspace_event(&mut self, event: &WorkspaceStoreEvent, cx: &mut Context<Self>) {
        match event {
            WorkspaceStoreEvent::WorkspacesChanged => {
                if let Some(id) = &self.active_workspace_id {
                    let store = self.workspace_store.read(cx);
                    if store.get_workspace(id).is_none() {
                        self.active_workspace_id = store.first_workspace_id().cloned();
                        self.save();
                        cx.emit(WindowStoreEvent::ActiveWorkspaceChanged);
                    }
                }
                cx.notify();
            }
            WorkspaceStoreEvent::WorkspaceUpdated(id) => {
                if self.active_workspace_id.as_ref() == Some(id) {
                    cx.notify();
                }
            }
            WorkspaceStoreEvent::PaneLayoutChanged(id) => {
                if self.active_workspace_id.as_ref() == Some(id) {
                    cx.notify();
                }
            }
        }
    }

    pub fn active_workspace_id(&self) -> Option<&String> {
        self.active_workspace_id.as_ref()
    }

    pub fn active_workspace<'a>(&'a self, cx: &'a App) -> Option<&'a Entity<Workspace>> {
        self.active_workspace_id.as_ref().and_then(|id| {
            self.workspace_store.read(cx).get_workspace(id)
        })
    }

    pub fn set_active_workspace(&mut self, id: String, cx: &mut Context<Self>) {
        if self.active_workspace_id.as_ref() != Some(&id) {
            // Verify workspace exists
            if self.workspace_store.read(cx).get_workspace(&id).is_some() {
                self.active_workspace_id = Some(id);
                self.save();
                cx.emit(WindowStoreEvent::ActiveWorkspaceChanged);
                cx.notify();
            }
        }
    }

    pub fn next_workspace(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<String> = self
            .workspace_store
            .read(cx)
            .workspace_ids()
            .cloned()
            .collect();
        if ids.len() > 1 {
            if let Some(current_id) = &self.active_workspace_id {
                if let Some(current_idx) = ids.iter().position(|id| id == current_id) {
                    let new_idx = (current_idx + 1) % ids.len();
                    self.set_active_workspace(ids[new_idx].clone(), cx);
                }
            }
        }
    }

    pub fn prev_workspace(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<String> = self
            .workspace_store
            .read(cx)
            .workspace_ids()
            .cloned()
            .collect();
        if ids.len() > 1 {
            if let Some(current_id) = &self.active_workspace_id {
                if let Some(current_idx) = ids.iter().position(|id| id == current_id) {
                    let new_idx = if current_idx == 0 {
                        ids.len() - 1
                    } else {
                        current_idx - 1
                    };
                    self.set_active_workspace(ids[new_idx].clone(), cx);
                }
            }
        }
    }

    pub fn sidebar_collapsed(&self) -> bool {
        self.sidebar_collapsed
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        self.save();
        cx.emit(WindowStoreEvent::UiStateChanged);
        cx.notify();
    }

    pub fn right_sidebar(&self) -> RightSidebarPanel {
        self.right_sidebar
    }

    pub fn toggle_file_tree(&mut self, cx: &mut Context<Self>) {
        self.right_sidebar = match self.right_sidebar {
            RightSidebarPanel::FileTree => RightSidebarPanel::Hidden,
            _ => RightSidebarPanel::FileTree,
        };
        self.save();
        cx.emit(WindowStoreEvent::UiStateChanged);
        cx.notify();
    }

    pub fn toggle_diff_list(&mut self, cx: &mut Context<Self>) {
        self.right_sidebar = match self.right_sidebar {
            RightSidebarPanel::DiffList => RightSidebarPanel::Hidden,
            _ => RightSidebarPanel::DiffList,
        };
        self.save();
        cx.emit(WindowStoreEvent::UiStateChanged);
        cx.notify();
    }

    fn save(&self) {
        let state = WindowUiState {
            active_workspace_id: self.active_workspace_id.clone(),
            sidebar_collapsed: self.sidebar_collapsed,
            right_sidebar: self.right_sidebar,
        };
        config::save_json(&config::ui_state_path(), &state);
    }
}
