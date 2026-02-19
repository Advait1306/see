use git2::{Repository, StatusOptions};
use gpui::{Context, EventEmitter, Task};
use std::path::{Path, PathBuf};
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
    repository: Repository,
    workdir: PathBuf,
    changed_files: Vec<ChangedFile>,
    _poll_task: Option<Task<()>>,
}

impl EventEmitter<GitStoreEvent> for GitStore {}

impl GitStore {
    pub fn try_new(path: &Path) -> Option<Self> {
        let repo = match Repository::discover(path) {
            Ok(repo) => repo,
            Err(e) => {
                log::debug!("No git repository found at {:?}: {}", path, e);
                return None;
            }
        };

        let workdir = repo.workdir()?.to_path_buf();

        Some(Self {
            repository: repo,
            workdir,
            changed_files: Vec::new(),
            _poll_task: None,
        })
    }

    pub fn with_polling(mut self, cx: &mut Context<Self>) -> Self {
        self.refresh_changed_files(cx);
        self._poll_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(1000))
                    .await;
                let _ = this.update(cx, |store, cx| {
                    store.refresh_changed_files(cx);
                });
            }
        }));
        self
    }

    pub fn refresh_changed_files(&mut self, cx: &mut Context<Self>) {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_ignored(false)
            .update_index(true);

        let mut new_files = Vec::new();

        if let Ok(statuses) = self.repository.statuses(Some(&mut opts)) {
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
                    let full_path = self.workdir.join(path);
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
