use crate::commands::*;
use crate::config;
use crate::stores::{PaneStore, RightSidebarPanel, WindowStore, Workspace, WorkspaceStore};
use crate::ui::diff_list::DiffList;
use crate::ui::file_tree::FileTree;
use crate::ui::pane_group::PaneGroupView;
use crate::ui::workspace_sidebar::WorkspaceSidebar;
use gpui::prelude::*;
use gpui::*;
use gpui_component::theme::ActiveTheme;
use gpui_component::{Icon, IconName, Sizable};

pub struct WindowView {
    workspace_store: Entity<WorkspaceStore>,
    window_store: Entity<WindowStore>,
    workspace_sidebar: Entity<WorkspaceSidebar>,
    file_tree: Entity<FileTree>,
    diff_list: Entity<DiffList>,
    focus_handle: FocusHandle,
}

impl WindowView {
    pub fn new(
        workspace_store: Entity<WorkspaceStore>,
        window_store: Entity<WindowStore>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let workspace_sidebar =
            cx.new(|cx| WorkspaceSidebar::new(workspace_store.clone(), window_store.clone(), cx));
        let file_tree = cx.new(|cx| FileTree::new(window_store.clone(), cx));
        let diff_list = cx.new(|cx| DiffList::new(window_store.clone(), cx));

        Self {
            workspace_store,
            window_store,
            workspace_sidebar,
            file_tree,
            diff_list,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn workspace_store(&self) -> &Entity<WorkspaceStore> {
        &self.workspace_store
    }

    fn active_workspace(&self, cx: &App) -> Option<Entity<Workspace>> {
        self.window_store.read(cx).active_workspace(cx).cloned()
    }

    fn active_pane_store(&self, cx: &App) -> Option<Entity<PaneStore>> {
        self.active_workspace(cx)
            .map(|ws| ws.read(cx).pane_store().clone())
    }

    fn active_pane_group_view(&self, cx: &App) -> Option<Entity<PaneGroupView>> {
        self.active_workspace(cx)
            .map(|ws| ws.read(cx).pane_group_view().clone())
    }

    pub fn toggle_file_tree(&mut self, cx: &mut Context<Self>) {
        self.window_store.update(cx, |store, cx| {
            store.toggle_file_tree(cx);
        });
    }

    pub fn toggle_diff_list(&mut self, cx: &mut Context<Self>) {
        self.window_store.update(cx, |store, cx| {
            store.toggle_diff_list(cx);
        });
    }

    pub fn toggle_workspace_sidebar(&mut self, cx: &mut Context<Self>) {
        self.window_store.update(cx, |store, cx| {
            store.toggle_sidebar(cx);
        });
    }

    fn right_sidebar(&self, cx: &App) -> RightSidebarPanel {
        self.window_store.read(cx).right_sidebar()
    }

    fn sidebar_collapsed(&self, cx: &App) -> bool {
        self.window_store.read(cx).sidebar_collapsed()
    }

    fn render_right_sidebar(&self, panel: RightSidebarPanel, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        let width = match panel {
            RightSidebarPanel::DiffList => 500.0,
            _ => 250.0,
        };

        div()
            .id("right-sidebar")
            .w(px(width))
            .h_full()
            .flex_shrink_0()
            .border_l_1()
            .border_color(theme.border)
            .bg(theme.sidebar)
            .map(|el| match panel {
                RightSidebarPanel::FileTree => el.child(self.file_tree.clone()),
                RightSidebarPanel::DiffList => el.child(self.diff_list.clone()),
                RightSidebarPanel::Hidden => el,
            })
    }

    // =========================================================================
    // Pane Operations (delegated to PaneStore via Workspace)
    // =========================================================================

    fn next_pane(&self, cx: &mut Context<Self>) {
        if let Some(pane_store) = self.active_pane_store(cx) {
            pane_store.update(cx, |store, cx| {
                store.next_pane(cx);
            });
        }
    }

    fn prev_pane(&self, cx: &mut Context<Self>) {
        if let Some(pane_store) = self.active_pane_store(cx) {
            pane_store.update(cx, |store, cx| {
                store.prev_pane(cx);
            });
        }
    }

    fn close_current_pane(&self, cx: &mut Context<Self>) {
        if let Some(pane_store) = self.active_pane_store(cx) {
            pane_store.update(cx, |store, cx| {
                store.close_current_pane(cx);
            });
        }
    }

    // =========================================================================
    // Workspace Operations (delegated to WorkspaceStore)
    // =========================================================================

    fn next_workspace(&self, cx: &mut Context<Self>) {
        self.window_store.update(cx, |store, cx| {
            store.next_workspace(cx);
        });
    }

    fn prev_workspace(&self, cx: &mut Context<Self>) {
        self.window_store.update(cx, |store, cx| {
            store.prev_workspace(cx);
        });
    }
}

const TITLE_BAR_HEIGHT: f32 = 38.0;

impl Render for WindowView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pane_group_view = self.active_pane_group_view(cx);
        let right_sidebar = self.right_sidebar(cx);
        let sidebar_collapsed = self.sidebar_collapsed(cx);

        let focus_handle = self.focus_handle.clone();
        let theme = cx.theme();
        let file_tree_icon_color = if right_sidebar == RightSidebarPanel::FileTree {
            theme.foreground
        } else {
            theme.muted_foreground
        };
        let diff_list_icon_color = if right_sidebar == RightSidebarPanel::DiffList {
            theme.foreground
        } else {
            theme.muted_foreground
        };

        div()
            .id("app-view")
            .key_context("AppView")
            .track_focus(&focus_handle)
            // Pane commands
            .on_action(cx.listener(|this, _: &ClosePane, _window, cx| {
                this.close_current_pane(cx);
            }))
            .on_action(cx.listener(|this, _: &PrevPane, _window, cx| {
                this.prev_pane(cx);
            }))
            .on_action(cx.listener(|this, _: &NextPane, _window, cx| {
                this.next_pane(cx);
            }))
            // Workspace commands
            .on_action(cx.listener(|this, _: &PrevWorkspace, _window, cx| {
                this.prev_workspace(cx);
            }))
            .on_action(cx.listener(|this, _: &NextWorkspace, _window, cx| {
                this.next_workspace(cx);
            }))
            // UI commands
            .on_action(
                cx.listener(|this, _: &ToggleWorkspaceSidebar, _window, cx| {
                    this.toggle_workspace_sidebar(cx);
                }),
            )
            .on_action(cx.listener(|this, _: &ToggleFileTree, _window, cx| {
                this.toggle_file_tree(cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleDiffList, _window, cx| {
                this.toggle_diff_list(cx);
            }))
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .text_color(theme.foreground)
            // Custom title bar
            .child(
                div()
                    .id("title-bar")
                    .h(px(TITLE_BAR_HEIGHT))
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .bg(theme.sidebar)
                    .border_b_1()
                    .border_color(theme.border)
                    .on_mouse_move(|_, _, _| {})
                    .child(div().w(px(80.0)))
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(config::APP_NAME),
                    )
                    .child(
                        div()
                            .w(px(100.0))
                            .flex()
                            .justify_end()
                            .gap(px(4.0))
                            .pr(px(12.0))
                            .child(
                                div()
                                    .id("diff-list-toggle")
                                    .p(px(6.0))
                                    .rounded(px(4.0))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.border))
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.toggle_diff_list(cx);
                                    }))
                                    .child(
                                        Icon::default()
                                            .path("icons/file-diff.svg")
                                            .small()
                                            .text_color(diff_list_icon_color),
                                    ),
                            )
                            .child(
                                div()
                                    .id("file-tree-toggle")
                                    .p(px(6.0))
                                    .rounded(px(4.0))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.border))
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.toggle_file_tree(cx);
                                    }))
                                    .child(
                                        Icon::new(IconName::FolderOpen)
                                            .small()
                                            .text_color(file_tree_icon_color),
                                    ),
                            ),
                    ),
            )
            // Main content area
            .child(
                div()
                    .id("content-area")
                    .flex_1()
                    .w_full()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .when(!sidebar_collapsed, |el| {
                        el.child(self.workspace_sidebar.clone())
                    })
                    .child(
                        div()
                            .id("main-content")
                            .flex_1()
                            .h_full()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .id("pane-container")
                                    .flex_1()
                                    .w_full()
                                    .min_h_0()
                                    .flex()
                                    .flex_col()
                                    .overflow_hidden()
                                    .map(|el| {
                                        if let Some(pgv) = pane_group_view.clone() {
                                            el.child(pgv)
                                        } else {
                                            el
                                        }
                                    }),
                            ),
                    )
                    .when(right_sidebar != RightSidebarPanel::Hidden, |el| {
                        el.child(self.render_right_sidebar(right_sidebar, cx))
                    }),
            )
    }
}

impl Focusable for WindowView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
