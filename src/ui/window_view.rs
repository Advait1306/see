use crate::commands::*;
use crate::config;
use crate::stores::{PaneStore, RightSidebarPanel, WindowStore, Workspace};
use crate::ui::command_menu::CommandMenu;
use crate::ui::diff_list::DiffList;
use crate::ui::file_tree::FileTree;
use crate::ui::pane_group::PaneGroupView;
use crate::ui::workspace_sidebar::WorkspaceSidebar;
use gpui::prelude::*;
use gpui::*;
use gpui_component::theme::ActiveTheme;
use gpui_component::{Icon, IconName, Sizable};

pub struct WindowView {
    window_store: Entity<WindowStore>,
    workspace_sidebar: Entity<WorkspaceSidebar>,
    file_tree: Entity<FileTree>,
    diff_list: Entity<DiffList>,
    command_menu: Entity<CommandMenu>,
    focus_handle: FocusHandle,
}

impl WindowView {
    pub fn new(
        window_store: Entity<WindowStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let workspace_sidebar =
            cx.new(|cx| WorkspaceSidebar::new(window_store.clone(), cx));
        let file_tree = cx.new(|cx| FileTree::new(window_store.clone(), cx));
        let diff_list = cx.new(|cx| DiffList::new(window_store.clone(), cx));
        let command_menu = cx.new(|cx| CommandMenu::new(window_store.clone(), window, cx));

        Self {
            window_store,
            workspace_sidebar,
            file_tree,
            diff_list,
            command_menu,
            focus_handle: cx.focus_handle(),
        }
    }

    fn active_workspace(&self, cx: &App) -> Option<Entity<Workspace>> {
        self.window_store.read(cx).active_workspace(cx)
    }

    fn active_pane_store(&self, cx: &App) -> Option<Entity<PaneStore>> {
        self.active_workspace(cx)
            .map(|ws| ws.read(cx).pane_store().clone())
    }

    fn active_pane_group_view(&self, cx: &App) -> Option<Entity<PaneGroupView>> {
        self.active_workspace(cx)
            .map(|ws| ws.read(cx).pane_group_view().clone())
    }

    pub fn focus_active_content(&self, window: &mut Window, cx: &App) {
        if let Some(pane_store) = self.active_pane_store(cx) {
            if let Some(pane) = &pane_store.read(cx).active_pane {
                pane.read(cx).focus_active_tab(window, cx);
            }
        }
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

    fn render_right_sidebar(&self, panel: RightSidebarPanel, sidebar_collapsed: bool, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        // Width percentages based on ratios:
        // With sidebar:    DiffList (2:5:5), FileTree (1:4:1)
        // Without sidebar: DiffList (0:5:5), FileTree (0:5:1)
        let width_pct = match (panel, sidebar_collapsed) {
            (RightSidebarPanel::DiffList, false) => 42.0,  // 5/12 ≈ 42%
            (RightSidebarPanel::DiffList, true) => 50.0,   // 5/10 = 50%
            (RightSidebarPanel::FileTree, false) => 17.0,  // 1/6 ≈ 17%
            (RightSidebarPanel::FileTree, true) => 17.0,   // 1/6 ≈ 17%
            (RightSidebarPanel::Hidden, _) => 0.0,
        };

        div()
            .id("right-sidebar")
            .w(relative(width_pct / 100.0))
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
            .on_action(cx.listener(|this, _: &ClosePane, window, cx| {
                this.close_current_pane(cx);
                this.focus_active_content(window, cx);
            }))
            .on_action(cx.listener(|this, _: &PrevPane, window, cx| {
                this.prev_pane(cx);
                this.focus_active_content(window, cx);
            }))
            .on_action(cx.listener(|this, _: &NextPane, window, cx| {
                this.next_pane(cx);
                this.focus_active_content(window, cx);
            }))
            // Workspace commands
            .on_action(cx.listener(|this, _: &PrevWorkspace, window, cx| {
                this.prev_workspace(cx);
                this.focus_active_content(window, cx);
            }))
            .on_action(cx.listener(|this, _: &NextWorkspace, window, cx| {
                this.next_workspace(cx);
                this.focus_active_content(window, cx);
            }))
            // UI commands
            .on_action(
                cx.listener(|this, _: &ToggleWorkspaceSidebar, _window, cx| {
                    this.toggle_workspace_sidebar(cx);
                }),
            )
            .on_action(cx.listener(|this, _: &ToggleFileTree, window, cx| {
                this.toggle_file_tree(cx);
                if this.right_sidebar(cx) == RightSidebarPanel::FileTree {
                    this.file_tree.read(cx).focus_handle(cx).focus(window);
                } else {
                    this.focus_active_content(window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &ToggleDiffList, window, cx| {
                this.toggle_diff_list(cx);
                if this.right_sidebar(cx) == RightSidebarPanel::DiffList {
                    this.diff_list.read(cx).focus_handle(cx).focus(window);
                } else {
                    this.focus_active_content(window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &ShowCommandMenu, window, cx| {
                this.command_menu.update(cx, |menu, cx| {
                    menu.toggle(window, cx);
                });
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
            // Ratios - DiffList: 2:5:5, FileTree: 1:4:1, Hidden: 1:6:0
            .child({
                // Width percentages based on ratios:
                // With sidebar:    DiffList (2:5:5), FileTree (1:4:1), Hidden (1:5:0)
                // Without sidebar: DiffList (0:5:5), FileTree (0:5:1), Hidden (0:6:0)
                let (sidebar_pct, main_pct) = match (right_sidebar, sidebar_collapsed) {
                    (RightSidebarPanel::DiffList, false) => (17.0, 41.0),   // 2:5:5 → 2/12, 5/12
                    (RightSidebarPanel::DiffList, true) => (0.0, 50.0),     // 0:5:5 → 5/10
                    (RightSidebarPanel::FileTree, false) => (17.0, 66.0),   // 1:4:1 → 1/6, 4/6
                    (RightSidebarPanel::FileTree, true) => (0.0, 83.0),     // 0:5:1 → 5/6
                    (RightSidebarPanel::Hidden, false) => (17.0, 83.0),     // 1:5:0 → 1/6, 5/6
                    (RightSidebarPanel::Hidden, true) => (0.0, 100.0),      // 0:6:0 → 6/6
                };

                div()
                    .id("content-area")
                    .flex_1()
                    .w_full()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .when(!sidebar_collapsed, |el| {
                        el.child(
                            div()
                                .id("workspace-sidebar-container")
                                .w(relative(sidebar_pct / 100.0))
                                .h_full()
                                .flex_shrink_0()
                                .child(self.workspace_sidebar.clone()),
                        )
                    })
                    .child(
                        div()
                            .id("main-content")
                            .w(relative(main_pct / 100.0))
                            .h_full()
                            .flex_shrink_0()
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
                        el.child(self.render_right_sidebar(right_sidebar, sidebar_collapsed, cx))
                    })
            })
            // Command menu overlay
            .child(self.command_menu.clone())
    }
}

impl Focusable for WindowView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
