use crate::stores::{EditorStore, FileEntry, WindowStore};
use crate::ui::EditorView;
use crate::ui::pane::TabItem;
use crate::workspace::{Workspace, WorkspaceEvent};
use gpui::prelude::*;
use gpui::*;
use gpui_component::list::{List, ListDelegate, ListEvent, ListItem, ListState};
use gpui_component::theme::ActiveTheme;
use gpui_component::{Icon, IconName, IndexPath, Selectable, Sizable};
use std::path::PathBuf;

pub struct NonSelectableItem(ListItem);

impl Selectable for NonSelectableItem {
    fn selected(self, _selected: bool) -> Self {
        self
    }

    fn is_selected(&self) -> bool {
        false
    }

    fn secondary_selected(self, _selected: bool) -> Self {
        self
    }
}

impl IntoElement for NonSelectableItem {
    type Element = <ListItem as IntoElement>::Element;

    fn into_element(self) -> Self::Element {
        self.0.into_element()
    }
}

pub struct FileTreeDelegate {
    entries: Vec<FileEntry>,
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
        let is_expanded = entry.is_expanded;
        let depth = entry.depth;
        let is_dir = entry.is_dir;
        let name = entry.name.clone();

        let theme = cx.theme();
        let muted_color = theme.muted_foreground;
        let blue_color = theme.primary;
        let foreground_color = theme.foreground;

        Some(NonSelectableItem(
            ListItem::new(ix).py_0().px_0().child(
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
                                el.child(Icon::new(chevron_icon).xsmall().text_color(muted_color))
                            }),
                    )
                    .child(div().flex().items_center().child(if is_dir {
                        let folder_icon = if is_expanded {
                            IconName::FolderOpen
                        } else {
                            IconName::Folder
                        };
                        Icon::new(folder_icon).small().text_color(blue_color)
                    } else {
                        Icon::new(IconName::File).small().text_color(muted_color)
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

pub struct FileTree {
    window_store: Entity<WindowStore>,
    list_state: Option<Entity<ListState<FileTreeDelegate>>>,
    focus_handle: FocusHandle,
    _window_store_subscription: Subscription,
    _workspace_subscription: Option<Subscription>,
}

impl FileTree {
    pub fn new(window_store: Entity<WindowStore>, cx: &mut Context<Self>) -> Self {
        let window_store_sub = cx.subscribe(&window_store, |this, _store, event, cx| {
            use crate::stores::WindowStoreEvent;
            match event {
                WindowStoreEvent::ActiveWorkspaceChanged => {
                    this.refresh_entries(cx);
                    this.subscribe_to_active_workspace(cx);
                }
                WindowStoreEvent::UiStateChanged => {
                    // UI state changes (sidebar collapsed, file tree visibility) don't affect file tree content
                }
            }
        });

        let mut file_tree = Self {
            window_store,
            list_state: None,
            focus_handle: cx.focus_handle(),
            _window_store_subscription: window_store_sub,
            _workspace_subscription: None,
        };

        file_tree.subscribe_to_active_workspace(cx);
        file_tree
    }

    fn subscribe_to_active_workspace(&mut self, cx: &mut Context<Self>) {
        self._workspace_subscription = self.active_workspace(cx).map(|workspace| {
            cx.subscribe(&workspace, |this, _workspace, event, cx| {
                if matches!(event, WorkspaceEvent::FileTreeChanged) {
                    this.refresh_entries(cx);
                }
            })
        });
    }

    fn active_workspace(&self, cx: &App) -> Option<Entity<Workspace>> {
        let window_store = self.window_store.read(cx);
        window_store.active_workspace(cx).cloned()
    }

    fn entries(&self, cx: &App) -> Vec<FileEntry> {
        if let Some(workspace) = self.active_workspace(cx) {
            let ws = workspace.read(cx);
            ws.file_tree_store().read(cx).entries().clone()
        } else {
            Vec::new()
        }
    }

    fn toggle_expanded(&self, path: &PathBuf, cx: &mut Context<Self>) {
        if let Some(workspace) = self.active_workspace(cx) {
            let file_tree_store = workspace.read(cx).file_tree_store().clone();
            file_tree_store.update(cx, |store, cx| {
                store.toggle_expanded(path, cx);
            });
        }
    }

    fn open_file(&self, path: PathBuf, cx: &mut Context<Self>) {
        let Some(workspace) = self.active_workspace(cx) else {
            return;
        };

        let buffer_store = EditorStore::global(cx);
        let buffer = buffer_store.update(cx, |store, cx| store.open_buffer(path.clone(), cx));

        let Some(buffer) = buffer else {
            log::error!("Failed to open buffer for {:?}", path);
            return;
        };

        let pane_store = workspace.read(cx).pane_store().clone();
        pane_store.update(cx, |ps, cx| {
            if let Some(pane) = ps.active_pane.clone() {
                // TODO: (fix) add a speacial function to add editors. Currently views are being added as Entities, that shouldn't be the case.
                pane.update(cx, |p, cx| {
                    let editor_view = cx.new(|cx| EditorView::new(buffer, path, cx));
                    p.tabs.push(TabItem::Editor(editor_view));
                    p.active_index = p.tabs.len() - 1;
                    cx.notify();
                });
            }
        });
    }

    fn ensure_list_state(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.list_state.is_none() {
            let entries = self.entries(cx);
            let delegate = FileTreeDelegate {
                entries,
                selected_index: None,
            };
            let list_state = cx.new(|cx| ListState::new(delegate, window, cx));

            cx.subscribe(&list_state, |this, list_entity, event: &ListEvent, cx| {
                if let ListEvent::Confirm(ix) = event {
                    let entry = list_entity.read(cx).delegate().entries.get(ix.row).cloned();
                    if let Some(entry) = entry {
                        if entry.is_dir {
                            this.toggle_expanded(&entry.path, cx);
                        } else {
                            this.open_file(entry.path, cx);
                        }
                    }
                }
            })
            .detach();

            self.list_state = Some(list_state);
        }
    }

    fn refresh_entries(&mut self, cx: &mut Context<Self>) {
        let entries = self.entries(cx);
        if let Some(list_state) = &self.list_state {
            list_state.update(cx, |state, _cx| {
                state.delegate_mut().entries = entries;
            });
        }
        cx.notify();
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
