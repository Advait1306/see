use crate::config;
use crate::file_watcher::FileWatcher;
use gpui::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct FileTreeStateConfig {
    pub expanded_paths: HashSet<PathBuf>,
}

#[derive(Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_expanded: bool,
    pub depth: usize,
}

pub enum FileTreeStoreEvent {
    FileSystemChanged,
    ExpandedPathsChanged,
}

pub struct FileTreeStore {
    workspace_id: String,
    workspace_path: PathBuf,
    expanded_paths: HashSet<PathBuf>,
    entries: Vec<FileEntry>,
    watcher: Option<FileWatcher>,
}

impl EventEmitter<FileTreeStoreEvent> for FileTreeStore {}

impl FileTreeStore {
    pub fn new(workspace_id: String, workspace_path: PathBuf, cx: &mut Context<Self>) -> Self {
        let mut store = Self::load_with_migration(&workspace_id, &workspace_path);

        // Ensure root is expanded
        store.expanded_paths.insert(workspace_path.clone());

        // Initialize watcher
        if let Ok(watcher) = FileWatcher::new(workspace_path.clone()) {
            store.watcher = Some(watcher);
        }

        // Initial scan
        store.refresh_entries();

        // Start polling for file changes
        cx.spawn({
            async move |this, cx| {
                loop {
                    smol::Timer::after(std::time::Duration::from_millis(500)).await;
                    let has_changes = this
                        .update(cx, |store, _cx| store.poll_watcher())
                        .unwrap_or(false);

                    if has_changes {
                        let _ = this.update(cx, |store, cx| {
                            store.refresh_entries();
                            cx.emit(FileTreeStoreEvent::FileSystemChanged);
                            cx.notify();
                        });
                    }
                }
            }
        })
        .detach();

        store
    }

    fn load_with_migration(workspace_id: &str, workspace_path: &PathBuf) -> Self {
        // Try to load from new per-workspace format first
        let config_path = config::workspace_file_tree_path(workspace_id);
        let config: FileTreeStateConfig = config::load_json(&config_path);

        // If new format is empty but legacy global file-tree-state.json exists, migrate
        if config.expanded_paths.is_empty() {
            let global_path = config::file_tree_state_path();
            if global_path.exists() {
                log::info!(
                    "Migrating file tree state for workspace {} from global file-tree-state.json",
                    workspace_id
                );
                #[derive(Deserialize, Default)]
                struct GlobalFileTreeConfig {
                    expanded_paths_by_workspace:
                        std::collections::HashMap<String, HashSet<PathBuf>>,
                }
                let global_config: GlobalFileTreeConfig = config::load_json(&global_path);
                if let Some(expanded_paths) =
                    global_config.expanded_paths_by_workspace.get(workspace_id)
                {
                    let store = Self {
                        workspace_id: workspace_id.to_string(),
                        workspace_path: workspace_path.clone(),
                        expanded_paths: expanded_paths.clone(),
                        entries: Vec::new(),
                        watcher: None,
                    };
                    store.save();
                    return store;
                }
            }

            // Also check legacy state.json for migration
            if config::legacy_state_exists() {
                log::info!(
                    "Migrating file tree state for workspace {} from legacy state.json",
                    workspace_id
                );
                let legacy = config::load_state();
                if let Some(ws_config) = legacy.workspaces.iter().find(|w| w.id == workspace_id) {
                    let store = Self {
                        workspace_id: workspace_id.to_string(),
                        workspace_path: workspace_path.clone(),
                        expanded_paths: ws_config.expanded_paths.clone(),
                        entries: Vec::new(),
                        watcher: None,
                    };
                    store.save();
                    return store;
                }
            }
        }

        Self {
            workspace_id: workspace_id.to_string(),
            workspace_path: workspace_path.clone(),
            expanded_paths: config.expanded_paths,
            entries: Vec::new(),
            watcher: None,
        }
    }

    pub fn save(&self) {
        let config = FileTreeStateConfig {
            expanded_paths: self.expanded_paths.clone(),
        };
        config::save_json(&config::workspace_file_tree_path(&self.workspace_id), &config);
    }

    fn poll_watcher(&mut self) -> bool {
        if let Some(watcher) = &self.watcher {
            let changes = watcher.poll_changes();
            !changes.is_empty()
        } else {
            false
        }
    }

    #[allow(dead_code)]
    pub fn expanded_paths(&self) -> &HashSet<PathBuf> {
        &self.expanded_paths
    }

    pub fn toggle_expanded(&mut self, path: &PathBuf, cx: &mut Context<Self>) {
        if self.expanded_paths.contains(path) {
            self.expanded_paths.remove(path);
        } else {
            self.expanded_paths.insert(path.clone());
        }

        self.save();
        self.refresh_entries();
        cx.emit(FileTreeStoreEvent::ExpandedPathsChanged);
        cx.notify();
    }

    pub fn entries(&self) -> &Vec<FileEntry> {
        &self.entries
    }

    fn refresh_entries(&mut self) {
        self.entries = Self::scan_directory(&self.workspace_path, 0, &self.expanded_paths);
    }

    fn scan_directory(
        path: &PathBuf,
        depth: usize,
        expanded_paths: &HashSet<PathBuf>,
    ) -> Vec<FileEntry> {
        let mut entries = Vec::new();

        let Ok(read_dir) = fs::read_dir(path) else {
            return entries;
        };

        let mut items: Vec<_> = read_dir
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                !name.starts_with('.')
                    && name != "node_modules"
                    && name != "target"
                    && name != "__pycache__"
            })
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

        for entry in items {
            let entry_path = entry.path();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let is_expanded = is_dir && expanded_paths.contains(&entry_path);
            let name = entry.file_name().to_string_lossy().to_string();

            entries.push(FileEntry {
                name,
                path: entry_path.clone(),
                is_dir,
                is_expanded,
                depth,
            });

            if is_expanded {
                entries.extend(Self::scan_directory(&entry_path, depth + 1, expanded_paths));
            }
        }

        entries
    }
}
