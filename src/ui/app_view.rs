use crate::config::{self, AppState, WorkspaceConfig, MemberConfig};
use crate::editor::BufferStore;
use crate::ui::EditorView;
use crate::ui::file_tree::{FileTree, FileTreeEvent};
use crate::ui::pane::{Pane, Axis, TabItem};
use crate::ui::pane_group::{Member, PaneAxis, PaneGroup, PaneGroupEvent};
use crate::workspace::{WorkspaceManager, WorkspaceEvent};
use gpui::prelude::*;
use gpui::*;
use gpui_component::sidebar::{Sidebar, SidebarMenu, SidebarMenuItem};
use gpui_component::theme::ActiveTheme;
use gpui_component::{Collapsible, Icon, IconName, Sizable, Side};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

pub struct AppView {
    workspace_manager: Entity<WorkspaceManager>,
    workspace_panes: HashMap<String, Entity<PaneGroup>>,
    workspace_expanded_paths: HashMap<String, HashSet<PathBuf>>,
    focus_handle: FocusHandle,
    _keystroke_subscription: Option<Subscription>,
    sidebar_collapsed: bool,
    file_tree: Option<Entity<FileTree>>,
    file_tree_visible: bool,
    buffer_store: Entity<BufferStore>,
}

impl AppView {
    pub fn new(workspace_manager: Entity<WorkspaceManager>, cx: &mut Context<Self>) -> Self {
        // Subscribe to workspace manager events
        cx.subscribe(&workspace_manager, |this, _manager, event, cx| {
            match event {
                WorkspaceEvent::ActiveWorkspaceChanged => {
                    this.on_active_workspace_changed(cx);
                }
            }
        })
        .detach();

        let buffer_store = cx.new(|cx| BufferStore::new(cx));

        Self {
            workspace_manager,
            workspace_panes: HashMap::new(),
            workspace_expanded_paths: HashMap::new(),
            focus_handle: cx.focus_handle(),
            _keystroke_subscription: None,
            sidebar_collapsed: false,
            file_tree: None,
            file_tree_visible: false,
            buffer_store,
        }
    }

    fn on_active_workspace_changed(&mut self, cx: &mut Context<Self>) {
        self.update_file_tree_path(cx);
        self.save_state(cx);
        cx.notify();
    }

    pub fn set_keystroke_subscription(&mut self, subscription: Subscription) {
        self._keystroke_subscription = Some(subscription);
    }

    pub fn collect_state(&self, cx: &App) -> AppState {
        let manager = self.workspace_manager.read(cx);
        let workspaces = manager
            .workspaces
            .iter()
            .map(|w| {
                let layout = self
                    .workspace_panes
                    .get(&w.id)
                    .map(|pg| self.collect_member_config(&pg.read(cx).root, cx))
                    .unwrap_or(MemberConfig::Pane {
                        terminal_count: 1,
                        active_index: 0,
                        open_files: Vec::new(),
                    });

                let expanded_paths = self
                    .workspace_expanded_paths
                    .get(&w.id)
                    .cloned()
                    .unwrap_or_default();

                WorkspaceConfig {
                    id: w.id.clone(),
                    name: w.name.clone(),
                    path: w.path.clone(),
                    layout,
                    expanded_paths,
                }
            })
            .collect();
        AppState {
            workspaces,
            active_workspace_index: manager.active_workspace_index,
            file_tree_visible: self.file_tree_visible,
        }
    }

    fn collect_member_config(&self, member: &Member, cx: &App) -> MemberConfig {
        match member {
            Member::Pane(pane) => {
                let pane = pane.read(cx);
                MemberConfig::Pane {
                    terminal_count: pane.terminal_count(),
                    active_index: pane.active_index,
                    open_files: pane.open_file_paths(cx),
                }
            }
            Member::Axis(axis) => {
                let axis_type = match axis.axis {
                    Axis::Horizontal => config::Axis::Horizontal,
                    Axis::Vertical => config::Axis::Vertical,
                };
                MemberConfig::Axis {
                    axis: axis_type,
                    ratios: axis.ratios.clone(),
                    members: axis
                        .members
                        .iter()
                        .map(|m| self.collect_member_config(m, cx))
                        .collect(),
                }
            }
        }
    }

    pub fn save_state(&self, cx: &App) {
        let state = self.collect_state(cx);
        config::save_state(&state);
    }

    pub fn restore_state(&mut self, state: AppState, cx: &mut Context<Self>) {
        if state.workspaces.is_empty() {
            // No saved state, create default workspace
            let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
            self.add_workspace("Home".to_string(), home, cx);
            return;
        }

        for workspace_config in &state.workspaces {
            // Add workspace to manager with the saved ID
            self.workspace_manager.update(cx, |m, _| {
                let workspace = crate::workspace::Workspace {
                    id: workspace_config.id.clone(),
                    name: workspace_config.name.clone(),
                    path: workspace_config.path.clone(),
                };
                m.workspaces.push(workspace);
            });

            // Create pane group from layout
            let path = workspace_config.path.clone();
            let buffer_store = self.buffer_store.clone();
            let layout = workspace_config.layout.clone();
            let pane_group = cx.new(|cx| {
                let member = Self::create_member_from_config(&layout, &path, &buffer_store, cx);
                let active_pane = member.first_pane();
                let mut group = PaneGroup::with_root(path.clone(), member, cx);
                group.active_pane = active_pane;
                group
            });

            self.subscribe_to_pane_group(&pane_group, cx);
            self.workspace_panes
                .insert(workspace_config.id.clone(), pane_group);
            self.workspace_expanded_paths
                .insert(workspace_config.id.clone(), workspace_config.expanded_paths.clone());
        }

        // Set the active workspace
        if let Some(active_index) = state.active_workspace_index {
            self.workspace_manager.update(cx, |m, _| {
                if active_index < m.workspaces.len() {
                    m.active_workspace_index = Some(active_index);
                } else if !m.workspaces.is_empty() {
                    m.active_workspace_index = Some(0);
                }
            });
        } else if !state.workspaces.is_empty() {
            self.workspace_manager.update(cx, |m, _| {
                m.active_workspace_index = Some(0);
            });
        }

        // Restore file tree visibility and create file tree if needed
        self.file_tree_visible = state.file_tree_visible;
        if self.file_tree_visible {
            self.create_file_tree(cx);
        }

        cx.notify();
    }

    fn create_file_tree(&mut self, cx: &mut Context<Self>) {
        if let Some(workspace) = self.workspace_manager.read(cx).active_workspace() {
            let workspace_id = workspace.id.clone();
            let path = workspace.path.clone();
            let expanded_paths = self
                .workspace_expanded_paths
                .get(&workspace_id)
                .cloned()
                .unwrap_or_default();

            let file_tree = cx.new(|cx| FileTree::new(path, expanded_paths, cx));

            // Subscribe to file tree events
            cx.subscribe(&file_tree, |this, file_tree, event, cx| {
                match event {
                    FileTreeEvent::ToggleDirectory(path) => {
                        this.handle_toggle_directory(path.clone(), &file_tree, cx);
                    }
                    FileTreeEvent::OpenFile(path) => {
                        this.open_file_in_active_pane(path.clone(), cx);
                    }
                }
            })
            .detach();

            self.file_tree = Some(file_tree);
        }
    }

    fn open_file_in_active_pane(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        // Get or create buffer from central store
        let buffer = self.buffer_store.update(cx, |store, cx| {
            store.open_buffer(path.clone(), cx)
        });

        let Some(buffer) = buffer else {
            log::error!("Failed to open buffer for {:?}", path);
            return;
        };

        if let Some(workspace_id) = self.active_workspace_id(cx) {
            if let Some(pane_group) = self.workspace_panes.get(&workspace_id) {
                pane_group.update(cx, |pg, cx| {
                    if let Some(pane) = pg.active_pane.clone() {
                        pane.update(cx, |p, cx| {
                            p.add_editor(buffer, path, cx);
                        });
                    }
                });
            }
        }

        // Save state when a file is opened
        self.save_state(cx);
    }

    fn handle_toggle_directory(
        &mut self,
        path: PathBuf,
        file_tree: &Entity<FileTree>,
        cx: &mut Context<Self>,
    ) {
        if let Some(workspace) = self.workspace_manager.read(cx).active_workspace() {
            let workspace_id = workspace.id.clone();
            let expanded_paths = self
                .workspace_expanded_paths
                .entry(workspace_id)
                .or_default();

            // Toggle the path
            if expanded_paths.contains(&path) {
                expanded_paths.remove(&path);
            } else {
                expanded_paths.insert(path);
            }

            // Update file tree with new expanded paths
            let new_expanded = expanded_paths.clone();
            file_tree.update(cx, |tree, cx| {
                tree.set_expanded_paths(new_expanded, cx);
            });

            self.save_state(cx);
        }
    }

    fn create_member_from_config(
        config: &MemberConfig,
        path: &PathBuf,
        buffer_store: &Entity<BufferStore>,
        cx: &mut Context<PaneGroup>,
    ) -> Member {
        match config {
            MemberConfig::Pane {
                terminal_count,
                active_index,
                open_files,
            } => {
                let buffer_store = buffer_store.clone();
                let pane = cx.new(|cx| {
                    let mut pane = Pane::new(path.clone(), cx);

                    // Add terminals first
                    let count = if open_files.is_empty() { (*terminal_count).max(1) } else { *terminal_count };
                    for _ in 0..count {
                        pane.add_terminal(cx);
                    }

                    // Add editor tabs for open files
                    for file_path in open_files {
                        if file_path.exists() {
                            if let Some(buffer) = buffer_store.update(cx, |store, cx| {
                                store.open_buffer(file_path.clone(), cx)
                            }) {
                                let editor = cx.new(|cx| EditorView::new(buffer, file_path.clone(), cx));
                                pane.tabs.push(TabItem::Editor(editor));
                            }
                        }
                    }

                    // Ensure at least one tab exists
                    if pane.tabs.is_empty() {
                        pane.add_terminal(cx);
                    }

                    pane.active_index = (*active_index).min(pane.tabs.len().saturating_sub(1));
                    pane
                });
                // Note: Don't subscribe here - with_root will subscribe to all panes
                Member::Pane(pane)
            }
            MemberConfig::Axis {
                axis,
                ratios,
                members,
            } => {
                let axis = match axis {
                    config::Axis::Horizontal => Axis::Horizontal,
                    config::Axis::Vertical => Axis::Vertical,
                };
                let members: Vec<Member> = members
                    .iter()
                    .map(|m| Self::create_member_from_config(m, path, buffer_store, cx))
                    .collect();
                Member::Axis(PaneAxis {
                    axis,
                    members,
                    ratios: ratios.clone(),
                })
            }
        }
    }

    fn subscribe_to_pane_group(&self, pane_group: &Entity<PaneGroup>, cx: &mut Context<Self>) {
        cx.subscribe(pane_group, |this, _pane_group, event, cx| {
            match event {
                PaneGroupEvent::StateChanged
                | PaneGroupEvent::PaneAdded(_)
                | PaneGroupEvent::PaneRemoved(_)
                | PaneGroupEvent::PaneFocused(_) => {
                    this.save_state(cx);
                    cx.notify();
                }
            }
        })
        .detach();
    }

    pub fn add_workspace(&mut self, name: String, path: PathBuf, cx: &mut Context<Self>) {
        let (workspace_id, new_index) = self.workspace_manager.update(cx, |m, _| {
            m.add_workspace(name, path.clone());
            let idx = m.workspaces.len() - 1;
            (m.workspaces.last().unwrap().id.clone(), idx)
        });

        let pane_group = cx.new(|cx| {
            let group = PaneGroup::new(path.clone(), cx);
            // Add initial terminal to the pane
            if let Some(pane) = group.active_pane.clone() {
                pane.update(cx, |p, cx| {
                    p.add_terminal(cx);
                });
            }
            group
        });

        self.subscribe_to_pane_group(&pane_group, cx);
        self.workspace_panes.insert(workspace_id, pane_group);

        // Switch to the new workspace
        self.select_workspace(new_index, cx);
        self.save_state(cx);
    }

    pub fn send_to_terminal(&self, input: &str, cx: &mut Context<Self>) {
        if let Some(workspace_id) = self.active_workspace_id(cx) {
            if let Some(pane_group) = self.workspace_panes.get(&workspace_id) {
                pane_group.read(cx).active_pane.as_ref().map(|pane| {
                    pane.read(cx).active_terminal().map(|terminal_view| {
                        terminal_view.read(cx).write(input);
                    });
                });
                cx.notify();
            }
        }
    }

    pub fn next_terminal(&mut self, cx: &mut Context<Self>) {
        if let Some(workspace_id) = self.active_workspace_id(cx) {
            if let Some(pane_group) = self.workspace_panes.get(&workspace_id) {
                pane_group.update(cx, |pg, cx| {
                    if let Some(pane) = pg.active_pane.clone() {
                        pane.update(cx, |p, cx| {
                            if p.tabs.len() > 1 {
                                p.active_index = (p.active_index + 1) % p.tabs.len();
                                cx.notify();
                            }
                        });
                    }
                });
                self.save_state(cx);
            }
        }
    }

    pub fn next_workspace(&mut self, cx: &mut Context<Self>) {
        self.workspace_manager.update(cx, |m, cx| {
            m.next_workspace(cx);
        });
    }

    pub fn prev_workspace(&mut self, cx: &mut Context<Self>) {
        self.workspace_manager.update(cx, |m, cx| {
            m.prev_workspace(cx);
        });
        self.save_state(cx);
    }

    pub fn prev_terminal(&mut self, cx: &mut Context<Self>) {
        if let Some(workspace_id) = self.active_workspace_id(cx) {
            if let Some(pane_group) = self.workspace_panes.get(&workspace_id) {
                pane_group.update(cx, |pg, cx| {
                    if let Some(pane) = pg.active_pane.clone() {
                        pane.update(cx, |p, cx| {
                            if p.tabs.len() > 1 {
                                p.active_index = if p.active_index == 0 {
                                    p.tabs.len() - 1
                                } else {
                                    p.active_index - 1
                                };
                                cx.notify();
                            }
                        });
                    }
                });
                self.save_state(cx);
            }
        }
    }

    pub fn close_current_terminal(&mut self, cx: &mut Context<Self>) {
        if let Some(workspace_id) = self.active_workspace_id(cx) {
            if let Some(pane_group) = self.workspace_panes.get(&workspace_id) {
                let _should_close_pane = pane_group.update(cx, |pg, cx| {
                    if let Some(pane) = pg.active_pane.clone() {
                        let (_tabs_count, should_close) = pane.update(cx, |p, cx| {
                            let count = p.tabs.len();
                            if count > 1 {
                                p.remove_tab(p.active_index, cx);
                                (count - 1, false)
                            } else {
                                (count, true)
                            }
                        });

                        // If pane has only one terminal and there are multiple panes, remove the pane
                        if should_close && pg.pane_count() > 1 {
                            pg.remove_pane(&pane, cx);
                        }
                    }
                    false
                });
                self.save_state(cx);
            }
        }
    }

    pub fn select_workspace(&mut self, index: usize, cx: &mut Context<Self>) {
        self.workspace_manager.update(cx, |m, cx| {
            m.set_active(index, cx);
        });
    }

    fn active_workspace_id(&self, cx: &App) -> Option<String> {
        let manager = self.workspace_manager.read(cx);
        manager.active_workspace().map(|w| w.id.clone())
    }

    fn render_sidebar(&self, cx: &Context<Self>) -> impl IntoElement {
        let manager = self.workspace_manager.read(cx);
        let workspaces: Vec<(usize, String, bool)> = manager
            .workspaces
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let is_active = manager.active_workspace_index == Some(i);
                (i, w.name.clone(), is_active)
            })
            .collect();

        let sidebar_collapsed = self.sidebar_collapsed;

        Sidebar::new(Side::Left)
            .collapsed(sidebar_collapsed)
            .child(
                SidebarMenu::new()
                    .collapsed(sidebar_collapsed)
                    .children(workspaces.into_iter().map(|(idx, name, is_active)| {
                        SidebarMenuItem::new(name)
                            .active(is_active)
                            .on_click(cx.listener(move |this, _, _window, cx| {
                                this.select_workspace(idx, cx);
                            }))
                    }))
                    .child(
                        SidebarMenuItem::new("+ Add Workspace")
                            .on_click(cx.listener(|_this, _, _window, cx| {
                                let entity = cx.entity().downgrade();

                                cx.spawn(async move |_this_weak, cx| {
                                    let path = rfd::AsyncFileDialog::new()
                                        .set_title("Select Workspace Folder")
                                        .pick_folder()
                                        .await;

                                    if let Some(handle) = path {
                                        let path = handle.path().to_path_buf();
                                        let name = path
                                            .file_name()
                                            .map(|n| n.to_string_lossy().to_string())
                                            .unwrap_or_else(|| "Workspace".to_string());

                                        let _ = cx.update(|cx| {
                                            let _ = entity.update(cx, |this, cx| {
                                                this.add_workspace(name, path, cx);
                                            });
                                        });
                                    }
                                }).detach();
                            })),
                    ),
            )
    }

    pub fn toggle_file_tree(&mut self, cx: &mut Context<Self>) {
        self.file_tree_visible = !self.file_tree_visible;

        if self.file_tree_visible && self.file_tree.is_none() {
            self.create_file_tree(cx);
        }

        self.save_state(cx);
        cx.notify();
    }

    fn update_file_tree_path(&mut self, cx: &mut Context<Self>) {
        if self.file_tree_visible {
            // Recreate file tree for the new workspace (with correct expanded paths and subscription)
            self.file_tree = None;
            self.create_file_tree(cx);
        }
    }

    fn render_file_tree_sidebar(&self, cx: &Context<Self>) -> impl IntoElement {
        let file_tree = self.file_tree.clone();
        let theme = cx.theme();

        div()
            .id("file-tree-sidebar")
            .w(px(250.0))
            .h_full()
            .flex_shrink_0()
            .border_l_1()
            .border_color(theme.border)
            .bg(theme.sidebar)
            .map(|el| {
                if let Some(ft) = file_tree {
                    el.child(ft)
                } else {
                    el
                }
            })
    }
}

const TITLE_BAR_HEIGHT: f32 = 38.0;

impl Render for AppView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let workspace_id = self.active_workspace_id(cx);
        let pane_group = workspace_id
            .as_ref()
            .and_then(|id| self.workspace_panes.get(id))
            .cloned();

        let focus_handle = self.focus_handle.clone();
        let theme = cx.theme();
        let icon_color = if self.file_tree_visible {
            theme.foreground
        } else {
            theme.muted_foreground
        };

        div()
            .id("app-view")
            .key_context("AppView")
            .track_focus(&focus_handle)
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
                    // Make the title bar draggable for window movement
                    .on_mouse_move(|_, _, _| {})
                    // Left spacer for traffic lights
                    .child(div().w(px(80.0)))
                    // Center title
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(config::APP_NAME)
                    )
                    // Right side with toggle button
                    .child(
                        div()
                            .w(px(80.0))
                            .flex()
                            .justify_end()
                            .pr(px(12.0))
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
                                            .text_color(icon_color)
                                    )
                            )
                    )
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
                    .child(self.render_sidebar(cx))
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
                                        if let Some(pg) = pane_group {
                                            el.child(pg)
                                        } else {
                                            el
                                        }
                                    }),
                            ),
                    )
                    .when(self.file_tree_visible, |el| {
                        el.child(self.render_file_tree_sidebar(cx))
                    }),
            )
    }
}

impl Focusable for AppView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
