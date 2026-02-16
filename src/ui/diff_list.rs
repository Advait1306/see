//! Diff carousel showing one file diff at a time with navigation

use crate::commands::{NextDiff, PrevDiff};
use crate::stores::{ChangedFile, FileStatus, GitStore, GitStoreEvent, WindowStore, WindowStoreEvent, Workspace, WorkspaceEvent};
use crate::ui::editor::{EditorView, EditorViewOptions};
use gpui::{
    div, px, App, AppContext as _, Context, Entity, FocusHandle, Focusable, FontWeight,
    InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled,
    Subscription, Window,
};
use gpui::prelude::FluentBuilder;
use gpui_component::theme::ActiveTheme;
use gpui_component::{Icon, IconName, Sizable};
use std::path::PathBuf;

const HEADER_HEIGHT: f32 = 32.0;

struct FileDiffData {
    path: PathBuf,
    status: FileStatus,
    display_name: String,
}

pub struct DiffList {
    window_store: Entity<WindowStore>,
    focus_handle: FocusHandle,
    file_diffs: Vec<FileDiffData>,
    /// Current file index in the carousel
    current_index: usize,
    /// Editor for the current file
    current_editor: Option<Entity<EditorView>>,
    /// Path of the file currently being displayed (for scroll preservation)
    current_editor_path: Option<PathBuf>,
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
            file_diffs: Vec::new(),
            current_index: 0,
            current_editor: None,
            current_editor_path: None,
            _window_store_subscription: window_store_sub,
            _git_store_subscription: None,
            _workspace_subscription: None,
        };

        diff_list.subscribe_to_active_workspace(cx);
        diff_list.refresh_diffs(cx);
        diff_list
    }

    fn subscribe_to_active_workspace(&mut self, cx: &mut Context<Self>) {
        let Some(workspace) = self.active_workspace(cx) else {
            self._workspace_subscription = None;
            self._git_store_subscription = None;
            return;
        };

        self._workspace_subscription = Some(cx.subscribe(&workspace, |this, _workspace, event, cx| {
            if matches!(event, WorkspaceEvent::FileTreeChanged) {
                this.refresh_diffs(cx);
            }
        }));

        let git_store = workspace.read(cx).git_store().cloned();
        self._git_store_subscription = if let Some(git_store) = git_store {
            Some(cx.subscribe(&git_store, |this, _store, event, cx| {
                if matches!(event, GitStoreEvent::ChangedFilesUpdated) {
                    this.refresh_diffs(cx);
                }
            }))
        } else {
            None
        };
    }

    fn active_workspace(&self, cx: &App) -> Option<Entity<Workspace>> {
        self.window_store.read(cx).active_workspace(cx)
    }

    fn active_git_store(&self, cx: &App) -> Option<Entity<GitStore>> {
        let Some(workspace) = self.active_workspace(cx) else {
            return None;
        };
        workspace.read(cx).git_store().cloned()
    }

    fn has_git_store(&self, cx: &App) -> bool {
        self.active_git_store(cx).is_some()
    }

    fn refresh_diffs(&mut self, cx: &mut Context<Self>) {
        let Some(git_store) = self.active_git_store(cx) else {
            self.file_diffs.clear();
            self.current_index = 0;
            self.current_editor = None;
            self.current_editor_path = None;
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

            self.file_diffs.push(FileDiffData {
                path: file.path,
                status: file.status,
                display_name,
            });
        }

        // Reset index if out of bounds
        if self.current_index >= self.file_diffs.len() {
            self.current_index = self.file_diffs.len().saturating_sub(1);
        }

        self.rebuild_current_editor(cx);
        cx.notify();
    }

    fn rebuild_current_editor(&mut self, cx: &mut Context<Self>) {
        if let Some(file_diff) = self.file_diffs.get(self.current_index) {
            let new_path = file_diff.path.clone();

            // Preserve scroll position if showing the same file
            let preserved_scroll = if self.current_editor_path.as_ref() == Some(&new_path) {
                self.current_editor
                    .as_ref()
                    .map(|e| e.read(cx).scroll_offset)
            } else {
                None
            };

            let editor = cx.new(|cx| {
                let mut editor =
                    EditorView::new(new_path.clone(), EditorViewOptions { diff_mode: true }, cx);
                if let Some(scroll_offset) = preserved_scroll {
                    editor.scroll_offset = scroll_offset;
                }
                editor
            });

            self.current_editor = Some(editor);
            self.current_editor_path = Some(new_path);
        } else {
            self.current_editor = None;
            self.current_editor_path = None;
        }
    }

    fn go_to_previous(&mut self, cx: &mut Context<Self>) {
        if self.current_index > 0 {
            self.current_index -= 1;
            self.rebuild_current_editor(cx);
            cx.notify();
        }
    }

    fn go_to_next(&mut self, cx: &mut Context<Self>) {
        if self.current_index + 1 < self.file_diffs.len() {
            self.current_index += 1;
            self.rebuild_current_editor(cx);
            cx.notify();
        }
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
        let success_color = theme.success;
        let warning_color = theme.warning;
        let danger_color = theme.danger;

        let has_git_store = self.has_git_store(cx);
        let total = self.total_changes();
        let current = self.current_index;
        let has_prev = current > 0;
        let has_next = current + 1 < total;

        // Get current file info
        let current_file = self.file_diffs.get(current);
        let display_name = current_file
            .map(|f| f.display_name.clone())
            .unwrap_or_default();
        let status_color = current_file.map(|f| match f.status {
            FileStatus::Added => success_color,
            FileStatus::Modified => warning_color,
            FileStatus::Deleted => danger_color,
        });

        div()
            .id("diff-carousel")
            .debug_selector(|| "diff-carousel".into())
            .key_context("DiffCarousel")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &PrevDiff, window, cx| {
                this.go_to_previous(cx);
                this.focus_handle.focus(window);
            }))
            .on_action(cx.listener(|this, _: &NextDiff, window, cx| {
                this.go_to_next(cx);
                this.focus_handle.focus(window);
            }))
            .size_full()
            .flex()
            .flex_col()
            .bg(sidebar_color)
            // Header with navigation
            .child(
                div()
                    .h(px(HEADER_HEIGHT))
                    .w_full()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(8.0))
                    .border_b_1()
                    .border_color(border_color)
                    .child(
                        // Left: prev button
                        div()
                            .id("diff-prev-btn")
                            .w(px(24.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(has_prev, |el| {
                                el.cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.go_to_previous(cx);
                                    }))
                            })
                            .child(
                                Icon::new(IconName::ChevronLeft)
                                    .xsmall()
                                    .text_color(if has_prev {
                                        foreground_color
                                    } else {
                                        muted_color.opacity(0.3)
                                    }),
                            ),
                    )
                    .child(
                        // Center: file name and status
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(px(6.0))
                            .overflow_hidden()
                            .when_some(status_color, |el, color| {
                                el.child(
                                    div()
                                        .w(px(8.0))
                                        .h(px(8.0))
                                        .rounded_full()
                                        .bg(color),
                                )
                            })
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(foreground_color)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(display_name),
                            )
                            .when(total > 1, |el| {
                                el.child(
                                    div()
                                        .text_sm()
                                        .text_color(muted_color)
                                        .child(format!("({}/{})", current + 1, total)),
                                )
                            }),
                    )
                    .child(
                        // Right: next button
                        div()
                            .id("diff-next-btn")
                            .w(px(24.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(has_next, |el| {
                                el.cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.go_to_next(cx);
                                    }))
                            })
                            .child(
                                Icon::new(IconName::ChevronRight)
                                    .xsmall()
                                    .text_color(if has_next {
                                        foreground_color
                                    } else {
                                        muted_color.opacity(0.3)
                                    }),
                            ),
                    ),
            )
            // Editor area - fills remaining space
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .when_some(self.current_editor.clone(), |el, editor| {
                        el.child(editor)
                    })
                    .when(self.current_editor.is_none(), |el| {
                        let message = if has_git_store {
                            "No changes"
                        } else {
                            "No git repository in workspace"
                        };
                        el.flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(muted_color)
                                    .child(message),
                            )
                    }),
            )
    }
}

impl Focusable for DiffList {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
