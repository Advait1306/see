use git2::{Repository, StatusOptions};
use gpui::*;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone)]
pub struct ChangedFile {
    pub path: PathBuf,
    pub status: FileStatus,
}

#[derive(Debug, Clone)]
pub enum GitStoreEvent {
    ChangedFilesUpdated,
}

// TODO: This could potentially be a global store which is keyed to the path
pub struct GitStore {
    repository: Option<Arc<Repository>>,
    workdir: Option<PathBuf>,
    changed_files: Vec<ChangedFile>,
    _poll_task: Option<Task<()>>,
}

impl EventEmitter<GitStoreEvent> for GitStore {}

impl GitStore {
    pub fn new(path: PathBuf, cx: &mut Context<Self>) -> Self {
        let (repository, workdir) = match Repository::discover(&path) {
            Ok(repo) => {
                let workdir = repo.workdir().map(|p| p.to_path_buf());
                (Some(Arc::new(repo)), workdir)
            }
            Err(e) => {
                log::debug!("No git repository found at {:?}: {}", path, e);
                (None, None)
            }
        };

        let mut store = Self {
            repository,
            workdir,
            changed_files: Vec::new(),
            _poll_task: None,
        };

        store.refresh_changed_files(cx);

        // Start polling for git status changes
        store._poll_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(1000))
                    .await;
                let _ = this.update(cx, |store, cx| {
                    store.refresh_changed_files(cx);
                });
            }
        }));

        store
    }

    pub fn refresh_changed_files(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = &self.repository else {
            return;
        };

        let mut opts = StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_ignored(false)
            .update_index(true);

        let mut new_files = Vec::new();

        if let Ok(statuses) = repo.statuses(Some(&mut opts)) {
            for entry in statuses.iter() {
                let status = entry.status();
                let file_status = if status.is_wt_new() || status.is_index_new() {
                    FileStatus::Added
                } else if status.is_wt_deleted() || status.is_index_deleted() {
                    FileStatus::Deleted
                } else if status.is_wt_modified()
                    || status.is_index_modified()
                    || status.is_wt_renamed()
                    || status.is_index_renamed()
                {
                    FileStatus::Modified
                } else {
                    continue;
                };

                if let Some(path) = entry.path() {
                    let full_path = if let Some(workdir) = &self.workdir {
                        workdir.join(path)
                    } else {
                        PathBuf::from(path)
                    };

                    new_files.push(ChangedFile {
                        path: full_path,
                        status: file_status,
                    });
                }
            }
        }

        // Only update and emit event if the file list changed
        let files_changed = self.changed_files.len() != new_files.len()
            || self
                .changed_files
                .iter()
                .zip(new_files.iter())
                .any(|(old, new)| old.path != new.path || old.status != new.status);

        if files_changed {
            self.changed_files = new_files;
            cx.emit(GitStoreEvent::ChangedFilesUpdated);
            cx.notify();
        }
    }

    pub fn changed_files(&self) -> &[ChangedFile] {
        &self.changed_files
    }

}
