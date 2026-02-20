use crate::commands::*;
use crate::config;
use crate::stores::{
    FileStore, FileStoreEvent, PaneStore, RightSidebarPanel, ScanState, WindowStore,
    WindowStoreEvent, Workspace,
};
use crate::ui::command_menu::CommandMenu;
use crate::ui::diff_list::DiffList;
use crate::ui::file_tree::FileTree;
use crate::ui::pane_group::PaneGroupView;
use crate::ui::pr_list::PrList;
use crate::ui::settings_view::SettingsView;
use crate::ui::workspace_sidebar::WorkspaceSidebar;
use gpui::{
    div, percentage, px, relative, Animation, App, AppContext as _, Context, Entity, FocusHandle,
    Focusable, InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement,
    Styled, Subscription, Transformation, Window,
};
use gpui::prelude::FluentBuilder;
use gpui::AnimationExt;
use gpui_component::theme::ActiveTheme;
use gpui_component::{Icon, IconName, Sizable};

pub struct WindowView {
    window_store: Entity<WindowStore>,
    workspace_sidebar: Entity<WorkspaceSidebar>,
    file_tree: Entity<FileTree>,
    diff_list: Entity<DiffList>,
    pr_list: Entity<PrList>,
    settings_view: Entity<SettingsView>,
    command_menu: Entity<CommandMenu>,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
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
        let pr_list = cx.new(|cx| PrList::new(window_store.clone(), cx));
        let settings_view = cx.new(|cx| SettingsView::new(window_store.clone(), cx));
        let command_menu = cx.new(|cx| CommandMenu::new(window_store.clone(), window, cx));

        let mut subscriptions = Vec::new();

        // Subscribe to window store events to update file store subscription
        subscriptions.push(cx.subscribe(&window_store, |this, _store, event, cx| {
            if let WindowStoreEvent::ActiveWorkspaceChanged = event {
                this.subscribe_to_file_store(cx);
            }
        }));

        let mut view = Self {
            window_store,
            workspace_sidebar,
            file_tree,
            diff_list,
            pr_list,
            settings_view,
            command_menu,
            focus_handle: cx.focus_handle(),
            _subscriptions: subscriptions,
        };

        view.subscribe_to_file_store(cx);
        view
    }

    fn subscribe_to_file_store(&mut self, cx: &mut Context<Self>) {
        if let Some(file_store) = self.active_file_store(cx) {
            self._subscriptions
                .push(cx.subscribe(&file_store, |_this, _store, event, cx| {
                    if matches!(
                        event,
                        FileStoreEvent::ScanStarted | FileStoreEvent::ScanCompleted
                    ) {
                        cx.notify();
                    }
                }));
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

    fn active_file_store(&self, cx: &App) -> Option<Entity<FileStore>> {
        self.active_workspace(cx)
            .map(|ws| ws.read(cx).file_store().clone())
    }

    fn scan_state(&self, cx: &App) -> ScanState {
        self.active_file_store(cx)
            .map(|fs| fs.read(cx).scan_state())
            .unwrap_or_default()
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
        cx.notify();
    }

    pub fn toggle_diff_list(&mut self, cx: &mut Context<Self>) {
        self.window_store.update(cx, |store, cx| {
            store.toggle_diff_list(cx);
        });
        cx.notify();
    }

    pub fn toggle_pr_list(&mut self, cx: &mut Context<Self>) {
        self.window_store.update(cx, |store, cx| {
            store.toggle_pull_requests(cx);
        });
        cx.notify();
    }

    pub fn toggle_workspace_sidebar(&mut self, cx: &mut Context<Self>) {
        self.window_store.update(cx, |store, cx| {
            store.toggle_sidebar(cx);
        });
        cx.notify();
    }

    fn show_settings(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.window_store.update(cx, |store, cx| {
            store.show_settings(cx);
        });
        self.settings_view.read(cx).focus_handle(cx).focus(window);
    }

    fn hide_settings(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.window_store.update(cx, |store, cx| {
            store.hide_settings(cx);
        });
        self.focus_active_content(window, cx);
    }

    fn settings_open(&self, cx: &App) -> bool {
        self.window_store.read(cx).settings_open()
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
        // With sidebar:    DiffList (2:5:5), FileTree (1:4:1), PullRequests (2:5:5)
        // Without sidebar: DiffList (0:5:5), FileTree (0:5:1), PullRequests (0:5:5)
        let width_pct = match (panel, sidebar_collapsed) {
            (RightSidebarPanel::DiffList, false) => 42.0,        // 5/12 ≈ 42%
            (RightSidebarPanel::DiffList, true) => 50.0,         // 5/10 = 50%
            (RightSidebarPanel::PullRequests, false) => 42.0,    // same as DiffList
            (RightSidebarPanel::PullRequests, true) => 50.0,     // same as DiffList
            (RightSidebarPanel::FileTree, false) => 17.0,        // 1/6 ≈ 17%
            (RightSidebarPanel::FileTree, true) => 17.0,         // 1/6 ≈ 17%
            (RightSidebarPanel::Hidden, _) => 0.0,
        };

        div()
            .id("right-sidebar")
            .debug_selector(|| "right-sidebar".into())
            .w(relative(width_pct / 100.0))
            .h_full()
            .flex_shrink_0()
            .border_l_1()
            .border_color(theme.border)
            .bg(theme.sidebar)
            .map(|el| match panel {
                RightSidebarPanel::FileTree => el.child(self.file_tree.clone()),
                RightSidebarPanel::DiffList => el.child(self.diff_list.clone()),
                RightSidebarPanel::PullRequests => el.child(self.pr_list.clone()),
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
        let settings_open = self.settings_open(cx);

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
        let pr_list_icon_color = if right_sidebar == RightSidebarPanel::PullRequests {
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
            .on_action(cx.listener(|this, _: &TogglePrList, window, cx| {
                this.toggle_pr_list(cx);
                if this.right_sidebar(cx) == RightSidebarPanel::PullRequests {
                    this.pr_list.read(cx).focus_handle(cx).focus(window);
                } else {
                    this.focus_active_content(window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &ShowCommandMenu, window, cx| {
                this.command_menu.update(cx, |menu, cx| {
                    menu.toggle(window, cx);
                });
            }))
            .on_action(cx.listener(|this, _: &ShowSettings, window, cx| {
                this.show_settings(window, cx);
            }))
            .on_action(cx.listener(|this, _: &HideSettings, window, cx| {
                this.hide_settings(window, cx);
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
                    .debug_selector(|| "title-bar".into())
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
                            .w(px(130.0))
                            .flex()
                            .justify_end()
                            .gap(px(4.0))
                            .pr(px(12.0))
                            .child(
                                div()
                                    .id("pr-list-toggle")
                                    .debug_selector(|| "pr-list-toggle".into())
                                    .p(px(6.0))
                                    .rounded(px(4.0))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.border))
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.toggle_pr_list(cx);
                                    }))
                                    .child(
                                        Icon::default()
                                            .path("icons/git-pull-request.svg")
                                            .small()
                                            .text_color(pr_list_icon_color),
                                    ),
                            )
                            .child(
                                div()
                                    .id("diff-list-toggle")
                                    .debug_selector(|| "diff-list-toggle".into())
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
                                    .debug_selector(|| "file-tree-toggle".into())
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
            .when(settings_open, |el| {
                el.child(self.settings_view.clone())
            })
            .when(!settings_open, |el| {
                // Ratios - DiffList: 2:5:5, FileTree: 1:4:1, Hidden: 1:6:0
                // Width percentages based on ratios:
                // With sidebar:    DiffList (2:5:5), FileTree (1:4:1), PullRequests (2:5:5), Hidden (1:5:0)
                // Without sidebar: DiffList (0:5:5), FileTree (0:5:1), PullRequests (0:5:5), Hidden (0:6:0)
                let (sidebar_pct, main_pct) = match (right_sidebar, sidebar_collapsed) {
                    (RightSidebarPanel::DiffList, false) => (17.0, 41.0),         // 2:5:5 → 2/12, 5/12
                    (RightSidebarPanel::DiffList, true) => (0.0, 50.0),           // 0:5:5 → 5/10
                    (RightSidebarPanel::PullRequests, false) => (17.0, 41.0),     // same as DiffList
                    (RightSidebarPanel::PullRequests, true) => (0.0, 50.0),       // same as DiffList
                    (RightSidebarPanel::FileTree, false) => (17.0, 66.0),         // 1:4:1 → 1/6, 4/6
                    (RightSidebarPanel::FileTree, true) => (0.0, 83.0),           // 0:5:1 → 5/6
                    (RightSidebarPanel::Hidden, false) => (17.0, 83.0),           // 1:5:0 → 1/6, 5/6
                    (RightSidebarPanel::Hidden, true) => (0.0, 100.0),            // 0:6:0 → 6/6
                };

                el.child(
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
                                    .debug_selector(|| "workspace-sidebar-container".into())
                                    .w(relative(sidebar_pct / 100.0))
                                    .h_full()
                                    .flex_shrink_0()
                                    .child(self.workspace_sidebar.clone()),
                            )
                        })
                        .child(
                            div()
                                .id("main-content")
                                .debug_selector(|| "main-content".into())
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
                        }),
                )
            })
            // Footer
            .child(
                div()
                    .id("footer")
                    .h(px(32.0))
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_end()
                    .px(px(12.0))
                    .bg(theme.sidebar)
                    .border_t_1()
                    .border_color(theme.border)
                    .when(
                        matches!(self.scan_state(cx), ScanState::Scanning { .. }),
                        |el| {
                            if let ScanState::Scanning { scanned_files } = self.scan_state(cx) {
                                el.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.0))
                                        .child(
                                            Icon::new(IconName::Loader)
                                                .xsmall()
                                                .text_color(theme.muted_foreground)
                                                .with_animation(
                                                    "rotate",
                                                    Animation::new(std::time::Duration::from_secs(1))
                                                        .repeat(),
                                                    |icon, delta| {
                                                        icon.transform(Transformation::rotate(
                                                            percentage(delta),
                                                        ))
                                                    },
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.muted_foreground)
                                                .child(format!("Scanning files... {}", scanned_files)),
                                        ),
                                )
                            } else {
                                el
                            }
                        },
                    ),
            )
            // Command menu overlay
            .child(self.command_menu.clone())
    }
}

impl Focusable for WindowView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    fn init_test_stores(cx: &mut TestAppContext) -> crate::test_helpers::TestFixture {
        let fixture = crate::test_helpers::TestFixture::new(cx);
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::stores::TerminalStore::init(cx);
            crate::stores::GitHubAccountStore::init(cx);
            crate::stores::WorkspaceStore::init(cx);

            let workspace_store = crate::stores::WorkspaceStore::global(cx);
            workspace_store.update(cx, |store, cx| {
                store.add_workspace(
                    "Test".to_string(),
                    fixture.workspace_path(),
                    cx,
                );
            });
        });
        fixture
    }

    #[test]
    fn test_toggle_file_tree() {
        crate::test_helpers::run_gpui_test(|cx| {
            let _fixture = init_test_stores(cx);

            let window_store = cx.new(|cx| WindowStore::new(cx));

            // Initially hidden
            cx.read(|cx| {
                assert_eq!(window_store.read(cx).right_sidebar(), RightSidebarPanel::Hidden);
            });

            // Toggle on
            window_store.update(cx, |store, cx| {
                store.toggle_file_tree(cx);
            });

            cx.read(|cx| {
                assert_eq!(window_store.read(cx).right_sidebar(), RightSidebarPanel::FileTree);
            });

            // Toggle off
            window_store.update(cx, |store, cx| {
                store.toggle_file_tree(cx);
            });

            cx.read(|cx| {
                assert_eq!(window_store.read(cx).right_sidebar(), RightSidebarPanel::Hidden);
            });
        });
    }

    #[test]
    fn test_toggle_diff_list() {
        crate::test_helpers::run_gpui_test(|cx| {
            let _fixture = init_test_stores(cx);

            let window_store = cx.new(|cx| WindowStore::new(cx));

            window_store.update(cx, |store, cx| {
                store.toggle_diff_list(cx);
            });

            cx.read(|cx| {
                assert_eq!(window_store.read(cx).right_sidebar(), RightSidebarPanel::DiffList);
            });
        });
    }

    #[test]
    fn test_toggle_workspace_sidebar() {
        crate::test_helpers::run_gpui_test(|cx| {
            let _fixture = init_test_stores(cx);

            let window_store = cx.new(|cx| WindowStore::new(cx));

            // Initially not collapsed
            cx.read(|cx| {
                assert!(!window_store.read(cx).sidebar_collapsed());
            });

            window_store.update(cx, |store, cx| {
                store.toggle_sidebar(cx);
            });

            cx.read(|cx| {
                assert!(window_store.read(cx).sidebar_collapsed());
            });

            window_store.update(cx, |store, cx| {
                store.toggle_sidebar(cx);
            });

            cx.read(|cx| {
                assert!(!window_store.read(cx).sidebar_collapsed());
            });
        });
    }

    #[test]
    fn test_title_bar_renders() {
        crate::test_helpers::run_gpui_test(|cx| {
            let _fixture = init_test_stores(cx);
            let window_store = cx.new(|cx| WindowStore::new(cx));

            let (_view, cx) = cx.add_window_view(|window, cx| {
                WindowView::new(window_store.clone(), window, cx)
            });

            assert!(cx.debug_bounds("title-bar").is_some(), "title-bar should be rendered");
            assert!(cx.debug_bounds("diff-list-toggle").is_some(), "diff-list-toggle should be rendered");
            assert!(cx.debug_bounds("file-tree-toggle").is_some(), "file-tree-toggle should be rendered");
        });
    }

    #[test]
    fn test_sidebar_initially_visible() {
        crate::test_helpers::run_gpui_test(|cx| {
            let _fixture = init_test_stores(cx);
            let window_store = cx.new(|cx| WindowStore::new(cx));

            let (_view, cx) = cx.add_window_view(|window, cx| {
                WindowView::new(window_store.clone(), window, cx)
            });

            assert!(
                cx.debug_bounds("workspace-sidebar-container").is_some(),
                "sidebar should be visible initially"
            );
        });
    }

    #[test]
    fn test_sidebar_hidden_when_collapsed() {
        crate::test_helpers::run_gpui_test(|cx| {
            let _fixture = init_test_stores(cx);
            let window_store = cx.new(|cx| WindowStore::new(cx));

            // Collapse before creating the view
            window_store.update(cx, |store, cx| {
                store.toggle_sidebar(cx);
            });

            let (_view, cx) = cx.add_window_view(|window, cx| {
                WindowView::new(window_store.clone(), window, cx)
            });

            assert!(
                cx.debug_bounds("workspace-sidebar-container").is_none(),
                "sidebar should be hidden when collapsed"
            );
        });
    }

    #[test]
    fn test_right_sidebar_file_tree_renders() {
        crate::test_helpers::run_gpui_test(|cx| {
            let _fixture = init_test_stores(cx);
            let window_store = cx.new(|cx| WindowStore::new(cx));

            let (view, cx) = cx.add_window_view(|window, cx| {
                WindowView::new(window_store.clone(), window, cx)
            });

            assert!(
                cx.debug_bounds("right-sidebar").is_none(),
                "right sidebar should be hidden initially"
            );

            view.update_in(cx, |view, _window, cx| {
                view.toggle_file_tree(cx);
            });

            assert!(
                cx.debug_bounds("right-sidebar").is_some(),
                "right sidebar should appear with file tree"
            );
            assert!(
                cx.debug_bounds("file-tree").is_some(),
                "file tree should be rendered inside right sidebar"
            );

            view.update_in(cx, |view, _window, cx| {
                view.toggle_file_tree(cx);
            });

            assert!(
                cx.debug_bounds("right-sidebar").is_none(),
                "right sidebar should disappear after toggle off"
            );
        });
    }

    #[test]
    fn test_right_sidebar_diff_list_renders() {
        crate::test_helpers::run_gpui_test(|cx| {
            let _fixture = init_test_stores(cx);
            let window_store = cx.new(|cx| WindowStore::new(cx));

            let (view, cx) = cx.add_window_view(|window, cx| {
                WindowView::new(window_store.clone(), window, cx)
            });

            assert!(
                cx.debug_bounds("right-sidebar").is_none(),
                "right sidebar should be hidden initially"
            );

            view.update_in(cx, |view, _window, cx| {
                view.toggle_diff_list(cx);
            });

            assert!(
                cx.debug_bounds("right-sidebar").is_some(),
                "right sidebar should appear with diff list"
            );
            assert!(
                cx.debug_bounds("diff-carousel").is_some(),
                "diff carousel should be rendered inside right sidebar"
            );

            view.update_in(cx, |view, _window, cx| {
                view.toggle_diff_list(cx);
            });

            assert!(
                cx.debug_bounds("right-sidebar").is_none(),
                "right sidebar should disappear after toggle off"
            );
        });
    }

    #[test]
    fn test_layout_structure() {
        crate::test_helpers::run_gpui_test(|cx| {
            let _fixture = init_test_stores(cx);
            let window_store = cx.new(|cx| WindowStore::new(cx));

            let (_view, cx) = cx.add_window_view(|window, cx| {
                WindowView::new(window_store.clone(), window, cx)
            });

            assert!(cx.debug_bounds("title-bar").is_some(), "title-bar should be present");
            assert!(cx.debug_bounds("main-content").is_some(), "main-content should be present");
            assert!(
                cx.debug_bounds("workspace-sidebar-container").is_some(),
                "workspace sidebar should be present"
            );
        });
    }

    #[test]
    fn test_file_tree_and_diff_list_exclusive() {
        crate::test_helpers::run_gpui_test(|cx| {
            let _fixture = init_test_stores(cx);

            let window_store = cx.new(|cx| WindowStore::new(cx));

            // Open file tree
            window_store.update(cx, |store, cx| {
                store.toggle_file_tree(cx);
            });

            cx.read(|cx| {
                assert_eq!(window_store.read(cx).right_sidebar(), RightSidebarPanel::FileTree);
            });

            // Open diff list should replace file tree
            window_store.update(cx, |store, cx| {
                store.toggle_diff_list(cx);
            });

            cx.read(|cx| {
                assert_eq!(window_store.read(cx).right_sidebar(), RightSidebarPanel::DiffList);
            });

            // Open file tree should replace diff list
            window_store.update(cx, |store, cx| {
                store.toggle_file_tree(cx);
            });

            cx.read(|cx| {
                assert_eq!(window_store.read(cx).right_sidebar(), RightSidebarPanel::FileTree);
            });
        });
    }

    #[test]
    fn test_toggle_pr_list() {
        crate::test_helpers::run_gpui_test(|cx| {
            let _fixture = init_test_stores(cx);

            let window_store = cx.new(|cx| WindowStore::new(cx));

            // Initially hidden
            cx.read(|cx| {
                assert_eq!(window_store.read(cx).right_sidebar(), RightSidebarPanel::Hidden);
            });

            // Toggle on
            window_store.update(cx, |store, cx| {
                store.toggle_pull_requests(cx);
            });

            cx.read(|cx| {
                assert_eq!(window_store.read(cx).right_sidebar(), RightSidebarPanel::PullRequests);
            });

            // Toggle off
            window_store.update(cx, |store, cx| {
                store.toggle_pull_requests(cx);
            });

            cx.read(|cx| {
                assert_eq!(window_store.read(cx).right_sidebar(), RightSidebarPanel::Hidden);
            });
        });
    }

    #[test]
    fn test_pr_list_toggle_renders() {
        crate::test_helpers::run_gpui_test(|cx| {
            let _fixture = init_test_stores(cx);
            let window_store = cx.new(|cx| WindowStore::new(cx));

            let (_view, cx) = cx.add_window_view(|window, cx| {
                WindowView::new(window_store.clone(), window, cx)
            });

            assert!(
                cx.debug_bounds("pr-list-toggle").is_some(),
                "pr-list-toggle icon should be rendered in title bar"
            );
        });
    }

    #[test]
    fn test_pr_list_exclusive_with_others() {
        crate::test_helpers::run_gpui_test(|cx| {
            let _fixture = init_test_stores(cx);

            let window_store = cx.new(|cx| WindowStore::new(cx));

            // Open file tree
            window_store.update(cx, |store, cx| {
                store.toggle_file_tree(cx);
            });

            cx.read(|cx| {
                assert_eq!(window_store.read(cx).right_sidebar(), RightSidebarPanel::FileTree);
            });

            // Open PR list should replace file tree
            window_store.update(cx, |store, cx| {
                store.toggle_pull_requests(cx);
            });

            cx.read(|cx| {
                assert_eq!(window_store.read(cx).right_sidebar(), RightSidebarPanel::PullRequests);
            });

            // Open diff list should replace PR list
            window_store.update(cx, |store, cx| {
                store.toggle_diff_list(cx);
            });

            cx.read(|cx| {
                assert_eq!(window_store.read(cx).right_sidebar(), RightSidebarPanel::DiffList);
            });
        });
    }
}
