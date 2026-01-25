use crate::editor::EditorStore;
use crate::file_tree_store::{FileTreeStore, FileTreeStoreEvent};
use crate::ui::pane_store::{PaneStore, PaneStoreEvent};
use gpui::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone)]
pub enum WorkspaceEvent {
    FileTreeChanged,
    PaneLayoutChanged,
}

impl EventEmitter<WorkspaceEvent> for Workspace {}

pub struct Workspace {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    file_tree_store: Entity<FileTreeStore>,
    pane_store: Entity<PaneStore>,
    _subscriptions: Vec<Subscription>,
}

impl Workspace {
    pub fn new(id: String, name: String, path: PathBuf, cx: &mut Context<Self>) -> Self {
        let buffer_store = EditorStore::global(cx);

        let file_tree_store = cx.new(|cx| FileTreeStore::new(id.clone(), path.clone(), cx));

        let pane_store = {
            let id = id.clone();
            let path = path.clone();
            cx.new(|cx| PaneStore::load(id, path, buffer_store, cx))
        };

        let mut subscriptions = Vec::new();

        subscriptions.push(cx.subscribe(&file_tree_store, |_this, _store, event, cx| match event {
            FileTreeStoreEvent::FileSystemChanged | FileTreeStoreEvent::ExpandedPathsChanged => {
                cx.emit(WorkspaceEvent::FileTreeChanged);
            }
        }));

        subscriptions.push(cx.subscribe(&pane_store, |_this, _store, event, cx| match event {
            PaneStoreEvent::StateChanged
            | PaneStoreEvent::PaneAdded
            | PaneStoreEvent::PaneRemoved
            | PaneStoreEvent::PaneFocused => {
                cx.emit(WorkspaceEvent::PaneLayoutChanged);
            }
        }));

        Self {
            id,
            name,
            path,
            file_tree_store,
            pane_store,
            _subscriptions: subscriptions,
        }
    }

    pub fn file_tree_store(&self) -> &Entity<FileTreeStore> {
        &self.file_tree_store
    }

    pub fn pane_store(&self) -> &Entity<PaneStore> {
        &self.pane_store
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct WorkspaceData {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
}

impl WorkspaceData {
    pub fn from_entity(workspace: &Entity<Workspace>, cx: &App) -> Self {
        let ws = workspace.read(cx);
        Self {
            id: ws.id.clone(),
            name: ws.name.clone(),
            path: ws.path.clone(),
        }
    }
}
