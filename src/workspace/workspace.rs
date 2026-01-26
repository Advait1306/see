use crate::git::GitStore;
use crate::stores::{EditorStore, FileTreeStore, FileTreeStoreEvent, PaneStore, PaneStoreEvent};
use crate::ui::pane_group::PaneGroupView;
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
    git_store: Entity<GitStore>,
    pane_store: Entity<PaneStore>,
    pane_group_view: Entity<PaneGroupView>,
    _subscriptions: Vec<Subscription>,
}

impl Workspace {
    pub fn new(id: String, name: String, path: PathBuf, cx: &mut Context<Self>) -> Self {
        let buffer_store = EditorStore::global(cx);

        let file_tree_store = cx.new(|cx| FileTreeStore::new(id.clone(), path.clone(), cx));
        let git_store = cx.new(|cx| GitStore::new(path.clone(), cx));

        let pane_store = {
            let id = id.clone();
            let path = path.clone();
            let git_store = git_store.clone();
            cx.new(|cx| PaneStore::load(id, path, buffer_store, git_store, cx))
        };

        let pane_group_view = {
            let pane_store = pane_store.clone();
            cx.new(|cx| PaneGroupView::new(pane_store, cx))
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
            git_store,
            pane_store,
            pane_group_view,
            _subscriptions: subscriptions,
        }
    }

    pub fn file_tree_store(&self) -> &Entity<FileTreeStore> {
        &self.file_tree_store
    }

    pub fn git_store(&self) -> &Entity<GitStore> {
        &self.git_store
    }

    pub fn pane_store(&self) -> &Entity<PaneStore> {
        &self.pane_store
    }

    pub fn pane_group_view(&self) -> &Entity<PaneGroupView> {
        &self.pane_group_view
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
