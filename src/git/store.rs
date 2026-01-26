use super::diff::{compute_hunks, compute_line_diffs, DiffHunk, LineDiff};
use git2::{Repository, StatusOptions};
use gpui::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

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
    DiffUpdated(PathBuf),
    ChangedFilesUpdated,
}

pub struct GitStore {
    repository: Option<Arc<Repository>>,
    workdir: Option<PathBuf>,
    changed_files: Vec<ChangedFile>,
    file_diffs: HashMap<PathBuf, Vec<DiffHunk>>,
    line_diffs: HashMap<PathBuf, Vec<LineDiff>>,
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
            file_diffs: HashMap::new(),
            line_diffs: HashMap::new(),
        };

        store.refresh_changed_files(cx);
        store
    }

    pub fn refresh_changed_files(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = &self.repository else {
            return;
        };

        let mut opts = StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_ignored(false);

        self.changed_files.clear();

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

                    self.changed_files.push(ChangedFile {
                        path: full_path,
                        status: file_status,
                    });
                }
            }
        }

        cx.emit(GitStoreEvent::ChangedFilesUpdated);
        cx.notify();
    }

    pub fn compute_diff_for_file(
        &mut self,
        file_path: &PathBuf,
        current_content: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = &self.repository else {
            return;
        };

        let relative_path = if let Some(workdir) = &self.workdir {
            file_path
                .strip_prefix(workdir)
                .ok()
                .map(|p| p.to_path_buf())
        } else {
            None
        };

        let Some(rel_path) = relative_path else {
            return;
        };

        let old_content = self.get_head_content(repo, &rel_path).unwrap_or_default();
        let hunks = compute_hunks(&old_content, current_content);
        let line_count = current_content.lines().count().max(1);
        let line_diffs = compute_line_diffs(&hunks, line_count);

        self.file_diffs.insert(file_path.clone(), hunks);
        self.line_diffs.insert(file_path.clone(), line_diffs);

        cx.emit(GitStoreEvent::DiffUpdated(file_path.clone()));
        cx.notify();
    }

    fn get_head_content(&self, repo: &Repository, path: &PathBuf) -> Option<String> {
        let head = repo.head().ok()?;
        let tree = head.peel_to_tree().ok()?;
        let entry = tree.get_path(path).ok()?;
        let blob = repo.find_blob(entry.id()).ok()?;

        if blob.is_binary() {
            return None;
        }

        String::from_utf8(blob.content().to_vec()).ok()
    }

    pub fn get_head_content_for_path(&self, path: &std::path::Path) -> String {
        let Some(repo) = &self.repository else {
            return String::new();
        };
        self.get_head_content(repo, &path.to_path_buf())
            .unwrap_or_default()
    }

    pub fn line_diffs_for_file(&self, path: &PathBuf) -> Option<&Vec<LineDiff>> {
        self.line_diffs.get(path)
    }

    pub fn changed_files(&self) -> &[ChangedFile] {
        &self.changed_files
    }

}
