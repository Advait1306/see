use crate::file_watcher::FileWatcher;
use gpui::prelude::*;
use gpui::*;
use gpui_component::list::{List, ListDelegate, ListEvent, ListItem, ListState};
use gpui_component::theme::ActiveTheme;
use gpui_component::{Icon, IconName, IndexPath, Selectable, Sizable};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

/// A wrapper around ListItem that ignores selection styling.
/// This allows clicks to still work for directory toggling while
/// preventing the visual selection highlight.
pub struct NonSelectableItem(ListItem);

impl Selectable for NonSelectableItem {
    fn selected(self, _selected: bool) -> Self {
        self // Ignore selection
    }

    fn is_selected(&self) -> bool {
        false
    }

    fn secondary_selected(self, _selected: bool) -> Self {
        self // Ignore secondary selection
    }
}

impl IntoElement for NonSelectableItem {
    type Element = <ListItem as IntoElement>::Element;

    fn into_element(self) -> Self::Element {
        self.0.into_element()
    }
}

#[derive(Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub depth: usize,
}

pub struct FileTreeDelegate {
    entries: Vec<FileEntry>,
    expanded_paths: HashSet<PathBuf>,
    selected_index: Option<usize>,
}

impl ListDelegate for FileTreeDelegate {
    type Item = NonSelectableItem;

    fn items_count(&self, _section: usize, _cx: &App) -> usize {
        self.entries.len()
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let entry = self.entries.get(ix.row)?;
        let is_expanded = self.expanded_paths.contains(&entry.path);
        let depth = entry.depth;
        let is_dir = entry.is_dir;
        let name = entry.name.clone();

        let theme = cx.theme();
        let muted_color = theme.muted_foreground;
        let blue_color = theme.primary;
        let foreground_color = theme.foreground;

        Some(NonSelectableItem(
            ListItem::new(ix)
                .py_0()
                .px_0()
                .child(
                    div()
                        .h(px(24.0))
                        .w_full()
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .pl(px(8.0 + (depth as f32 * 16.0)))
                        .pr(px(8.0))
                        .child(
                            div()
                                .w(px(16.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .when(is_dir, |el| {
                                    let chevron_icon = if is_expanded {
                                        IconName::ChevronDown
                                    } else {
                                        IconName::ChevronRight
                                    };
                                    el.child(
                                        Icon::new(chevron_icon)
                                            .xsmall()
                                            .text_color(muted_color),
                                    )
                                }),
                        )
                        .child(div().flex().items_center().child(if is_dir {
                            let folder_icon = if is_expanded {
                                IconName::FolderOpen
                            } else {
                                IconName::Folder
                            };
                            Icon::new(folder_icon)
                                .small()
                                .text_color(blue_color)
                        } else {
                            Icon::new(IconName::File)
                                .small()
                                .text_color(muted_color)
                        }))
                        .child(
                            div()
                                .text_sm()
                                .text_color(foreground_color)
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(name),
                        ),
                ),
        ))
    }

    fn render_empty(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        let muted_color = cx.theme().muted_foreground;
        div()
            .p(px(12.0))
            .text_sm()
            .text_color(muted_color)
            .child("No files")
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) {
        self.selected_index = ix.map(|i| i.row);
    }
}

// Events emitted by the file tree
pub enum FileTreeEvent {
    ToggleDirectory(PathBuf),
    OpenFile(PathBuf),
}

pub struct FileTree {
    root_path: PathBuf,
    expanded_paths: HashSet<PathBuf>,
    watcher: Option<FileWatcher>,
    list_state: Option<Entity<ListState<FileTreeDelegate>>>,
    focus_handle: FocusHandle,
}

impl EventEmitter<FileTreeEvent> for FileTree {}

impl FileTree {
    pub fn new(root_path: PathBuf, expanded_paths: HashSet<PathBuf>, cx: &mut Context<Self>) -> Self {
        let watcher = FileWatcher::new(root_path.clone()).ok();

        let mut actual_expanded = expanded_paths;
        // Always expand root
        actual_expanded.insert(root_path.clone());

        let tree = Self {
            root_path,
            expanded_paths: actual_expanded,
            watcher,
            list_state: None,
            focus_handle: cx.focus_handle(),
        };

        // Set up polling for file changes
        cx.spawn(async move |this, cx| {
            loop {
                smol::Timer::after(std::time::Duration::from_millis(500)).await;
                let should_refresh = this
                    .update(cx, |tree, _cx| {
                        if let Some(watcher) = &tree.watcher {
                            let changes = watcher.poll_changes();
                            !changes.is_empty()
                        } else {
                            false
                        }
                    })
                    .unwrap_or(false);

                if should_refresh {
                    let _ = this.update(cx, |tree, cx| {
                        tree.refresh_entries(cx);
                        cx.notify();
                    });
                }
            }
        })
        .detach();

        tree
    }

    pub fn set_expanded_paths(&mut self, expanded_paths: HashSet<PathBuf>, cx: &mut Context<Self>) {
        self.expanded_paths = expanded_paths;
        self.expanded_paths.insert(self.root_path.clone());
        self.refresh_entries(cx);
        cx.notify();
    }

    fn ensure_list_state(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.list_state.is_none() {
            let entries = Self::scan_directory_static(&self.root_path, 0, &self.expanded_paths);
            let delegate = FileTreeDelegate {
                entries,
                expanded_paths: self.expanded_paths.clone(),
                selected_index: None,
            };
            let list_state = cx.new(|cx| ListState::new(delegate, window, cx));

            // Subscribe to click events (Confirm) to handle directory toggling and file opening
            cx.subscribe(&list_state, |_this, list_entity, event: &ListEvent, cx| {
                if let ListEvent::Confirm(ix) = event {
                    let entry = list_entity.read(cx).delegate().entries.get(ix.row).cloned();
                    if let Some(entry) = entry {
                        if entry.is_dir {
                            cx.emit(FileTreeEvent::ToggleDirectory(entry.path));
                        } else {
                            cx.emit(FileTreeEvent::OpenFile(entry.path));
                        }
                    }
                }
            })
            .detach();

            self.list_state = Some(list_state);
        }
    }

    fn refresh_entries(&mut self, cx: &mut Context<Self>) {
        let entries = Self::scan_directory_static(&self.root_path, 0, &self.expanded_paths);
        let expanded_paths = self.expanded_paths.clone();
        if let Some(list_state) = &self.list_state {
            list_state.update(cx, |state, _cx| {
                state.delegate_mut().entries = entries;
                state.delegate_mut().expanded_paths = expanded_paths;
            });
        }
    }

    fn scan_directory_static(
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
            let name = entry.file_name().to_string_lossy().to_string();

            entries.push(FileEntry {
                name,
                path: entry_path.clone(),
                is_dir,
                depth,
            });

            if is_dir && expanded_paths.contains(&entry_path) {
                entries.extend(Self::scan_directory_static(&entry_path, depth + 1, expanded_paths));
            }
        }

        entries
    }

}

impl Render for FileTree {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_list_state(window, cx);

        let list_state = self.list_state.clone().unwrap();
        let sidebar_color = cx.theme().sidebar;

        div()
            .id("file-tree")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .bg(sidebar_color)
            .pt(px(8.0))
            .child(List::new(&list_state).py_0())
    }
}

impl Focusable for FileTree {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
