use crate::config;
use crate::file_watcher::FileWatcher;
use gpui::*;
use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

const SKIP_DIRS: &[&str] = &["node_modules", "target", ".git", "vendor", "dist", "build"];
const EXCLUDED_FROM_SEARCH: &[&str] = &["node_modules", "target", ".git", "vendor", "dist", "build"];

#[derive(Clone, Default)]
pub enum ScanState {
    #[default]
    Idle,
    Scanning { scanned_files: usize },
    Completed,
}

#[derive(Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_expanded: bool,
    pub is_pending: bool,
    pub depth: usize,
}

pub enum FileStoreEvent {
    ScanStarted,
    ScanCompleted,
    FilesChanged,
    ExpandedPathsChanged,
}

#[derive(Serialize, Deserialize, Default)]
struct FileStoreConfig {
    expanded_paths: HashSet<PathBuf>,
}

struct ScanBatch {
    files: Vec<PathBuf>,
    dirs: Vec<PathBuf>,
    pending: Vec<PathBuf>,
}

pub struct FileStore {
    workspace_id: String,
    workspace_path: PathBuf,
    files: HashSet<PathBuf>,
    directories: HashSet<PathBuf>,
    pending_dirs: HashSet<PathBuf>,
    expanded_paths: HashSet<PathBuf>,
    scan_state: ScanState,
    watcher: Option<FileWatcher>,
    _scan_task: Option<Task<()>>,
    _poll_task: Option<Task<()>>,
}

impl EventEmitter<FileStoreEvent> for FileStore {}

impl FileStore {
    pub fn new(workspace_id: String, workspace_path: PathBuf, cx: &mut Context<Self>) -> Self {
        let config = Self::load_config(&workspace_id);

        let mut store = Self {
            workspace_id,
            workspace_path: workspace_path.clone(),
            files: HashSet::new(),
            directories: HashSet::new(),
            pending_dirs: HashSet::new(),
            expanded_paths: config.expanded_paths,
            scan_state: ScanState::Idle,
            watcher: None,
            _scan_task: None,
            _poll_task: None,
        };

        // Ensure root is expanded
        store.expanded_paths.insert(workspace_path.clone());

        // Initialize file watcher
        if let Ok(watcher) = FileWatcher::new(workspace_path) {
            store.watcher = Some(watcher);
        }

        // Start initial background scan
        store.start_initial_scan(cx);

        // Start file watcher polling
        store.start_file_watcher(cx);

        store
    }

    fn load_config(workspace_id: &str) -> FileStoreConfig {
        let config_path = config::workspace_file_tree_path(workspace_id);
        config::load_json(&config_path)
    }

    pub fn save(&self) {
        let config = FileStoreConfig {
            expanded_paths: self.expanded_paths.clone(),
        };
        config::save_json(
            &config::workspace_file_tree_path(&self.workspace_id),
            &config,
        );
    }

    pub fn scan_state(&self) -> ScanState {
        self.scan_state.clone()
    }

    fn start_initial_scan(&mut self, cx: &mut Context<Self>) {
        self.scan_state = ScanState::Scanning { scanned_files: 0 };
        cx.emit(FileStoreEvent::ScanStarted);

        let (tx, rx) = smol::channel::unbounded::<ScanBatch>();
        let workspace_path = self.workspace_path.clone();

        cx.background_executor()
            .spawn(async move {
                scan_directory_streaming(&workspace_path, SKIP_DIRS, tx).await;
            })
            .detach();

        self._scan_task = Some(cx.spawn(async move |this, cx| {
            while let Ok(batch) = rx.recv().await {
                let _ = this.update(cx, |store, cx| {
                    store.files.extend(batch.files);
                    store.directories.extend(batch.dirs);
                    store.pending_dirs.extend(batch.pending);
                    store.scan_state = ScanState::Scanning {
                        scanned_files: store.files.len(),
                    };
                    cx.notify();
                });
            }

            let _ = this.update(cx, |store, cx| {
                store.scan_state = ScanState::Completed;
                cx.emit(FileStoreEvent::ScanCompleted);
                cx.notify();
            });
        }));
    }

    pub fn expand_pending_dir(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if !self.pending_dirs.remove(&path) {
            return;
        }

        let (tx, rx) = smol::channel::unbounded::<ScanBatch>();

        cx.background_executor()
            .spawn({
                let path = path.clone();
                async move {
                    scan_directory_streaming(&path, SKIP_DIRS, tx).await;
                }
            })
            .detach();

        cx.spawn(async move |this, cx| {
            while let Ok(batch) = rx.recv().await {
                let _ = this.update(cx, |store, cx| {
                    store.files.extend(batch.files);
                    store.directories.extend(batch.dirs);
                    store.pending_dirs.extend(batch.pending);
                    cx.notify();
                });
            }

            let _ = this.update(cx, |_store, cx| {
                cx.emit(FileStoreEvent::FilesChanged);
            });
        })
        .detach();
    }

    pub fn toggle_expanded(&mut self, path: &PathBuf, cx: &mut Context<Self>) {
        if self.expanded_paths.contains(path) {
            self.expanded_paths.remove(path);
        } else {
            self.expanded_paths.insert(path.clone());

            if self.pending_dirs.contains(path) {
                self.expand_pending_dir(path.clone(), cx);
            }
        }

        self.save();
        cx.emit(FileStoreEvent::ExpandedPathsChanged);
        cx.notify();
    }

    pub fn visible_entries(&self) -> Vec<FileEntry> {
        let mut entries = Vec::new();
        self.collect_visible_entries(&self.workspace_path, 0, &mut entries);
        entries
    }

    fn collect_visible_entries(&self, dir: &PathBuf, depth: usize, entries: &mut Vec<FileEntry>) {
        let Ok(read_dir) = std::fs::read_dir(dir) else {
            return;
        };

        let mut items: Vec<_> = read_dir
            .filter_map(|e| e.ok())
            .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
            .collect();

        items.sort_by(|a, b| {
            let a_is_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let b_is_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
            match (a_is_dir, b_is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.file_name().cmp(&b.file_name()),
            }
        });

        for item in items {
            let path = item.path();
            let is_dir = item.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let is_expanded = is_dir && self.expanded_paths.contains(&path);
            let is_pending = is_dir && self.pending_dirs.contains(&path);

            entries.push(FileEntry {
                name: item.file_name().to_string_lossy().to_string(),
                path: path.clone(),
                is_dir,
                is_expanded,
                is_pending,
                depth,
            });

            if is_expanded && !is_pending {
                self.collect_visible_entries(&path, depth + 1, entries);
            }
        }
    }

    pub fn search_files(&self, query: &str) -> Vec<PathBuf> {
        let searchable_files = self.files.iter().filter(|path| {
            !path.components().any(|component| {
                let name = component.as_os_str().to_string_lossy();
                EXCLUDED_FROM_SEARCH.contains(&name.as_ref())
            })
        });

        if query.is_empty() {
            let mut files: Vec<_> = searchable_files.cloned().collect();
            files.sort();
            return files;
        }

        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::new(
            query,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );

        let mut scored: Vec<_> = searchable_files
            .filter_map(|path| {
                let path_str = path
                    .strip_prefix(&self.workspace_path)
                    .unwrap_or(path)
                    .to_string_lossy();
                let mut buf = Vec::new();
                let score = pattern.score(nucleo_matcher::Utf32Str::new(&path_str, &mut buf), &mut matcher);
                score.map(|s| (path.clone(), s))
            })
            .collect();

        scored.sort_by(|a, b| b.1.cmp(&a.1));
        scored.into_iter().map(|(path, _)| path).collect()
    }

    fn start_file_watcher(&mut self, cx: &mut Context<Self>) {
        self._poll_task = Some(cx.spawn(async move |this, cx| {
            loop {
                smol::Timer::after(Duration::from_millis(500)).await;

                let changes = this
                    .update(cx, |store, _| {
                        if let Some(watcher) = &store.watcher {
                            watcher.poll_changes()
                        } else {
                            Vec::new()
                        }
                    })
                    .unwrap_or_default();

                if !changes.is_empty() {
                    let _ = this.update(cx, |store, cx| {
                        store.process_file_changes(changes, cx);
                    });
                }
            }
        }));
    }

    fn process_file_changes(&mut self, changes: Vec<PathBuf>, cx: &mut Context<Self>) {
        for path in changes {
            if path.exists() {
                if path.is_dir() {
                    self.directories.insert(path);
                } else {
                    self.files.insert(path);
                }
            } else {
                self.files.remove(&path);
                self.directories.remove(&path);
                self.pending_dirs.remove(&path);
            }
        }

        cx.emit(FileStoreEvent::FilesChanged);
        cx.notify();
    }
}

async fn scan_directory_streaming(
    root: &PathBuf,
    skip_dirs: &[&str],
    tx: smol::channel::Sender<ScanBatch>,
) {
    let mut stack = vec![root.clone()];

    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };

        let mut batch = ScanBatch {
            files: Vec::new(),
            dirs: Vec::new(),
            pending: Vec::new(),
        };

        for entry in entries.filter_map(|e| e.ok()) {
            let entry_path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if name.starts_with('.') {
                continue;
            }

            if entry_path.is_dir() {
                batch.dirs.push(entry_path.clone());

                if skip_dirs.contains(&name.as_str()) {
                    batch.pending.push(entry_path);
                } else {
                    stack.push(entry_path);
                }
            } else {
                batch.files.push(entry_path);
            }
        }

        let _ = tx.send(batch).await;
    }
}

