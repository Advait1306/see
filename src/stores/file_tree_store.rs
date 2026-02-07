use crate::config;
use crate::file_watcher::FileWatcher;
use gpui::{Context, EventEmitter};
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
        let mut store = Self::load(&workspace_id, &workspace_path);

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

    fn load(workspace_id: &str, workspace_path: &PathBuf) -> Self {
        let config_path = config::workspace_file_tree_path(workspace_id);
        let config: FileTreeStateConfig = config::load_json(&config_path);

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

        let mut items: Vec<_> = read_dir.filter_map(|entry| entry.ok()).collect();

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

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::prelude::*;

    #[test]
    fn test_file_tree_scans_directory() {
        crate::test_helpers::run_gpui_test(|cx| {
            let fixture = crate::test_helpers::TestFixture::new(cx);
            fixture.create_tree(&[
                ("file_a.txt", "a"),
                ("file_b.txt", "b"),
                ("subdir/nested.txt", "nested"),
            ]);

            let ws_path = fixture.workspace_path();
            let store = cx.new(|cx| FileTreeStore::new("test".to_string(), ws_path, cx));

            cx.read(|cx| {
                let entries = store.read(cx).entries();
                // Should have subdir and two files at top level
                let top_level_names: Vec<&str> = entries
                    .iter()
                    .filter(|e| e.depth == 0)
                    .map(|e| e.name.as_str())
                    .collect();
                assert!(top_level_names.contains(&"subdir"));
                assert!(top_level_names.contains(&"file_a.txt"));
                assert!(top_level_names.contains(&"file_b.txt"));
            });
        });
    }

    #[test]
    fn test_file_tree_toggle_expanded() {
        crate::test_helpers::run_gpui_test(|cx| {
            let fixture = crate::test_helpers::TestFixture::new(cx);
            fixture.create_tree(&[
                ("subdir/nested.txt", "content"),
            ]);

            let ws_path = fixture.workspace_path();
            let subdir_path = ws_path.join("subdir");
            let store = cx.new(|cx| FileTreeStore::new("test".to_string(), ws_path, cx));

            // Initially subdir is not expanded, so no children visible
            cx.read(|cx| {
                let entries = store.read(cx).entries();
                let nested = entries.iter().find(|e| e.name == "nested.txt");
                assert!(nested.is_none(), "Nested file should not be visible when subdir is collapsed");
            });

            // Expand subdir
            store.update(cx, |store, cx| {
                store.toggle_expanded(&subdir_path, cx);
            });

            cx.read(|cx| {
                let entries = store.read(cx).entries();
                let nested = entries.iter().find(|e| e.name == "nested.txt");
                assert!(nested.is_some(), "Nested file should be visible after expanding subdir");
            });
        });
    }

    #[test]
    fn test_file_tree_sorts_dirs_before_files() {
        crate::test_helpers::run_gpui_test(|cx| {
            let fixture = crate::test_helpers::TestFixture::new(cx);
            fixture.create_tree(&[
                ("zebra.txt", "z"),
                ("alpha_dir/file.txt", "content"),
            ]);

            let ws_path = fixture.workspace_path();
            let store = cx.new(|cx| FileTreeStore::new("test".to_string(), ws_path, cx));

            cx.read(|cx| {
                let entries = store.read(cx).entries();
                let top_level: Vec<(&str, bool)> = entries
                    .iter()
                    .filter(|e| e.depth == 0)
                    .map(|e| (e.name.as_str(), e.is_dir))
                    .collect();
                // Directories should come before files
                if top_level.len() >= 2 {
                    assert!(top_level[0].1, "First entry should be a directory");
                    assert!(!top_level[1].1, "Second entry should be a file");
                }
            });
        });
    }

    #[test]
    fn test_file_tree_saves_expanded_state() {
        crate::test_helpers::run_gpui_test(|cx| {
            let fixture = crate::test_helpers::TestFixture::new(cx);
            fixture.create_tree(&[
                ("subdir/file.txt", "content"),
            ]);

            let ws_path = fixture.workspace_path();
            let subdir_path = ws_path.join("subdir");
            let store = cx.new(|cx| FileTreeStore::new("test-ws".to_string(), ws_path, cx));

            store.update(cx, |store, cx| {
                store.toggle_expanded(&subdir_path, cx);
            });

            // Verify the config file was written
            let config_path = crate::config::workspace_file_tree_path("test-ws");
            assert!(config_path.exists(), "Config file should have been written");
        });
    }
}
