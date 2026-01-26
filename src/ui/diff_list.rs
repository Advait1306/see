//! Diff list sidebar showing changed files with unified diffs

use crate::constants::{CELL_HEIGHT, PADDING};
use crate::git::{ChangedFile, FileStatus, GitStore, GitStoreEvent};
use crate::stores::{WindowStore, WindowStoreEvent};
use crate::ui::editor::{DiffLine, DiffLineTag, EditorView};
use crate::workspace::{Workspace, WorkspaceEvent};
use gpui::prelude::*;
use gpui::*;
use gpui_component::theme::ActiveTheme;
use gpui_component::{v_virtual_list, VirtualListScrollHandle};
use gpui_component::{Icon, IconName, Sizable};
use similar::{ChangeTag, TextDiff};
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;

const FILE_HEADER_HEIGHT: f32 = 28.0;

struct FileDiffData {
    path: PathBuf,
    status: FileStatus,
    display_name: String,
    diff_lines: Vec<DiffLine>,
}

/// Entry in the virtualized list
enum ListEntry {
    FileHeader { file_index: usize },
    DiffEditor { file_index: usize },
}

pub struct DiffList {
    window_store: Entity<WindowStore>,
    focus_handle: FocusHandle,
    collapsed_files: HashSet<PathBuf>,
    file_diffs: Vec<FileDiffData>,
    diff_editors: Vec<(PathBuf, Entity<EditorView>)>,
    /// Flattened list of entries for virtualization
    list_entries: Vec<ListEntry>,
    /// Pre-computed sizes for each entry
    item_sizes: Rc<Vec<Size<Pixels>>>,
    scroll_handle: VirtualListScrollHandle,
    _window_store_subscription: Subscription,
    _git_store_subscription: Option<Subscription>,
    _workspace_subscription: Option<Subscription>,
}

impl DiffList {
    pub fn new(window_store: Entity<WindowStore>, cx: &mut Context<Self>) -> Self {
        let window_store_sub = cx.subscribe(&window_store, |this, _store, event, cx| match event {
            WindowStoreEvent::ActiveWorkspaceChanged => {
                this.subscribe_to_active_workspace(cx);
                this.refresh_diffs(cx);
            }
            WindowStoreEvent::UiStateChanged => {}
        });

        let mut diff_list = Self {
            window_store,
            focus_handle: cx.focus_handle(),
            collapsed_files: HashSet::new(),
            file_diffs: Vec::new(),
            diff_editors: Vec::new(),
            list_entries: Vec::new(),
            item_sizes: Rc::new(Vec::new()),
            scroll_handle: VirtualListScrollHandle::new(),
            _window_store_subscription: window_store_sub,
            _git_store_subscription: None,
            _workspace_subscription: None,
        };

        diff_list.subscribe_to_active_workspace(cx);
        diff_list.refresh_diffs(cx);
        diff_list
    }

    fn subscribe_to_active_workspace(&mut self, cx: &mut Context<Self>) {
        let workspace = self.active_workspace(cx);

        self._workspace_subscription = workspace.as_ref().map(|workspace| {
            cx.subscribe(workspace, |this, _workspace, event, cx| {
                if matches!(event, WorkspaceEvent::FileTreeChanged) {
                    this.refresh_diffs(cx);
                }
            })
        });

        self._git_store_subscription = workspace.map(|workspace| {
            let git_store = workspace.read(cx).git_store().clone();
            cx.subscribe(&git_store, |this, _store, event, cx| match event {
                GitStoreEvent::ChangedFilesUpdated | GitStoreEvent::DiffUpdated(_) => {
                    this.refresh_diffs(cx);
                }
            })
        });
    }

    fn active_workspace(&self, cx: &App) -> Option<Entity<Workspace>> {
        self.window_store.read(cx).active_workspace(cx).cloned()
    }

    fn active_git_store(&self, cx: &App) -> Option<Entity<GitStore>> {
        self.active_workspace(cx)
            .map(|ws| ws.read(cx).git_store().clone())
    }

    fn refresh_diffs(&mut self, cx: &mut Context<Self>) {
        let Some(git_store) = self.active_git_store(cx) else {
            self.file_diffs.clear();
            self.diff_editors.clear();
            self.rebuild_list_entries(cx);
            cx.notify();
            return;
        };

        let store = git_store.read(cx);
        let changed_files: Vec<ChangedFile> = store.changed_files().to_vec();

        self.file_diffs.clear();

        for file in changed_files {
            let display_name = file
                .path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| file.path.to_string_lossy().to_string());

            let diff_lines = self.compute_diff_lines(&git_store, &file.path, cx);

            self.file_diffs.push(FileDiffData {
                path: file.path,
                status: file.status,
                display_name,
                diff_lines,
            });
        }

        self.rebuild_diff_editors(cx);
        self.rebuild_list_entries(cx);
        cx.notify();
    }

    fn compute_diff_lines(
        &self,
        git_store: &Entity<GitStore>,
        file_path: &PathBuf,
        cx: &App,
    ) -> Vec<DiffLine> {
        let store = git_store.read(cx);

        let Some(workspace) = self.active_workspace(cx) else {
            return Vec::new();
        };

        let workdir = workspace.read(cx).path.clone();

        let relative_path = file_path.strip_prefix(&workdir).ok();
        let Some(rel_path) = relative_path else {
            return Vec::new();
        };

        let old_content = store.get_head_content_for_path(rel_path);
        let new_content = std::fs::read_to_string(file_path).unwrap_or_default();

        let diff = TextDiff::from_lines(&old_content, &new_content);
        let mut all_lines: Vec<DiffLine> = Vec::new();
        let mut old_line = 1usize;
        let mut new_line = 1usize;

        for change in diff.iter_all_changes() {
            let content = change.value().trim_end_matches('\n').to_string();
            match change.tag() {
                ChangeTag::Equal => {
                    all_lines.push(DiffLine {
                        tag: DiffLineTag::Equal,
                        old_line_num: Some(old_line),
                        new_line_num: Some(new_line),
                        content,
                    });
                    old_line += 1;
                    new_line += 1;
                }
                ChangeTag::Delete => {
                    all_lines.push(DiffLine {
                        tag: DiffLineTag::Delete,
                        old_line_num: Some(old_line),
                        new_line_num: None,
                        content,
                    });
                    old_line += 1;
                }
                ChangeTag::Insert => {
                    all_lines.push(DiffLine {
                        tag: DiffLineTag::Insert,
                        old_line_num: None,
                        new_line_num: Some(new_line),
                        content,
                    });
                    new_line += 1;
                }
            }
        }

        all_lines
    }

    fn rebuild_diff_editors(&mut self, cx: &mut Context<Self>) {
        self.diff_editors.clear();

        for file_diff in &self.file_diffs {
            if !self.collapsed_files.contains(&file_diff.path) {
                let diff_lines = file_diff.diff_lines.clone();
                let editor = cx.new(|cx| EditorView::new_diff_mode(diff_lines, cx));
                self.diff_editors.push((file_diff.path.clone(), editor));
            }
        }
    }

    fn rebuild_list_entries(&mut self, cx: &mut Context<Self>) {
        self.list_entries.clear();
        let mut sizes = Vec::new();

        for (file_index, file_diff) in self.file_diffs.iter().enumerate() {
            // Always add file header
            self.list_entries.push(ListEntry::FileHeader { file_index });
            sizes.push(Size {
                width: px(0.0), // Width is ignored for vertical lists
                height: px(FILE_HEADER_HEIGHT),
            });

            // Add diff editor if expanded
            if !self.collapsed_files.contains(&file_diff.path) {
                self.list_entries.push(ListEntry::DiffEditor { file_index });

                // Calculate editor height based on line count
                let editor_height = if let Some(editor) = self.get_editor_for_path(&file_diff.path)
                {
                    let line_count = editor.read(cx).diff_line_count();
                    (line_count as f32 * CELL_HEIGHT) + (PADDING * 2.0)
                } else {
                    100.0 // Fallback height
                };

                sizes.push(Size {
                    width: px(0.0),
                    height: px(editor_height),
                });
            }
        }

        self.item_sizes = Rc::new(sizes);
    }

    fn toggle_file(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.collapsed_files.contains(&path) {
            self.collapsed_files.remove(&path);
        } else {
            self.collapsed_files.insert(path);
        }
        self.rebuild_diff_editors(cx);
        self.rebuild_list_entries(cx);
        cx.notify();
    }

    fn get_editor_for_path(&self, path: &PathBuf) -> Option<&Entity<EditorView>> {
        self.diff_editors
            .iter()
            .find(|(p, _)| p == path)
            .map(|(_, e)| e)
    }

    fn total_changes(&self) -> usize {
        self.file_diffs.len()
    }

}

impl Render for DiffList {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let sidebar_color = theme.sidebar;
        let foreground_color = theme.foreground;
        let border_color = theme.border;
        let muted_color = theme.muted_foreground;
        let list_hover = theme.list_hover;
        let success_color = theme.success;
        let warning_color = theme.warning;
        let danger_color = theme.danger;

        let total_changes = self.total_changes();
        let item_sizes = self.item_sizes.clone();
        let scroll_handle = self.scroll_handle.clone();

        div()
            .id("diff-list")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .bg(sidebar_color)
            .child(
                div()
                    .h(px(32.0))
                    .w_full()
                    .flex()
                    .items_center()
                    .px(px(12.0))
                    .border_b_1()
                    .border_color(border_color)
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(foreground_color)
                            .child(format!("Changes {}", total_changes)),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .size_full()
                    .overflow_hidden()
                    .child(
                        v_virtual_list(
                            cx.entity().clone(),
                            "diff-virtual-list",
                            item_sizes,
                            move |this, visible_range, _window, cx| {
                                let mut elements: Vec<AnyElement> = Vec::new();

                                for ix in visible_range {
                                    let Some(entry) = this.list_entries.get(ix) else {
                                        continue;
                                    };

                                    let element = match entry {
                                        ListEntry::FileHeader { file_index } => {
                                            let file_diff = &this.file_diffs[*file_index];
                                            let path = file_diff.path.clone();
                                            let is_expanded = !this.collapsed_files.contains(&path);
                                            let status = file_diff.status;
                                            let display_name = file_diff.display_name.clone();

                                            let status_color = match status {
                                                FileStatus::Added => success_color,
                                                FileStatus::Modified => warning_color,
                                                FileStatus::Deleted => danger_color,
                                            };

                                            let chevron_icon = if is_expanded {
                                                IconName::ChevronDown
                                            } else {
                                                IconName::ChevronRight
                                            };

                                            div()
                                                .id(SharedString::from(format!("file-header-{}", path.display())))
                                                .h(px(FILE_HEADER_HEIGHT))
                                                .w_full()
                                                .flex()
                                                .items_center()
                                                .gap(px(4.0))
                                                .px(px(8.0))
                                                .bg(sidebar_color)
                                                .border_b_1()
                                                .border_color(border_color)
                                                .cursor_pointer()
                                                .hover(move |s| s.bg(list_hover))
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.toggle_file(path.clone(), cx);
                                                }))
                                                .child(
                                                    div()
                                                        .w(px(16.0))
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .child(Icon::new(chevron_icon).xsmall().text_color(muted_color)),
                                                )
                                                .child(
                                                    div()
                                                        .w(px(8.0))
                                                        .h(px(8.0))
                                                        .rounded_full()
                                                        .bg(status_color),
                                                )
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(foreground_color)
                                                        .overflow_hidden()
                                                        .text_ellipsis()
                                                        .child(display_name),
                                                )
                                                .into_any_element()
                                        }
                                        ListEntry::DiffEditor { file_index } => {
                                            let file_diff = &this.file_diffs[*file_index];

                                            if let Some(editor) = this.get_editor_for_path(&file_diff.path) {
                                                div()
                                                    .w_full()
                                                    .border_b_1()
                                                    .border_color(border_color)
                                                    .child(editor.clone())
                                                    .into_any_element()
                                            } else {
                                                div().into_any_element()
                                            }
                                        }
                                    };

                                    elements.push(element);
                                }

                                elements
                            },
                        )
                        .track_scroll(&scroll_handle),
                    ),
            )
    }
}

impl Focusable for DiffList {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
