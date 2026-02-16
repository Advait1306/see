use super::super::git::GitStore;
use super::super::{FileStore, FileStoreEvent, PaneStore, PaneStoreEvent};
use crate::ui::pane_group::PaneGroupView;
use gpui::{App, AppContext as _, Context, Entity, EventEmitter, Subscription};
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
    file_store: Entity<FileStore>,
    git_store: Option<Entity<GitStore>>,
    pane_store: Entity<PaneStore>,
    pane_group_view: Entity<PaneGroupView>,
    _subscriptions: Vec<Subscription>,
}

impl Workspace {
    pub fn new(id: String, name: String, path: PathBuf, cx: &mut Context<Self>) -> Self {
        let file_store = cx.new(|cx| FileStore::new(id.clone(), path.clone(), cx));
        let git_store = if let Some(store) = GitStore::try_new(&path) {
            Some(cx.new(|cx| store.with_polling(cx)))
        } else {
            None
        };

        let pane_store = {
            let id = id.clone();
            let path = path.clone();
            cx.new(|cx| PaneStore::load(id, path, cx))
        };

        let pane_group_view = {
            let pane_store = pane_store.clone();
            cx.new(|cx| PaneGroupView::new(pane_store, cx))
        };

        let mut subscriptions = Vec::new();

        subscriptions.push(cx.subscribe(&file_store, |_this, _store, event, cx| match event {
            FileStoreEvent::FilesChanged
            | FileStoreEvent::ExpandedPathsChanged
            | FileStoreEvent::ScanStarted
            | FileStoreEvent::ScanCompleted => {
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
            file_store,
            git_store,
            pane_store,
            pane_group_view,
            _subscriptions: subscriptions,
        }
    }

    pub fn file_store(&self) -> &Entity<FileStore> {
        &self.file_store
    }

    pub fn git_store(&self) -> Option<&Entity<GitStore>> {
        self.git_store.as_ref()
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
