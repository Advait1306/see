use crate::config::{self, AppState, WorkspaceConfig, MemberConfig};
use crate::terminal::Terminal;
use crate::ui::pane::{Pane, Axis};
use crate::ui::pane_group::{Member, PaneAxis, PaneGroup, PaneGroupEvent};
use crate::ui::TerminalView;
use crate::workspace::WorkspaceManager;
use gpui::prelude::*;
use gpui::*;
use gpui_component::sidebar::{Sidebar, SidebarMenu, SidebarMenuItem};
use gpui_component::{Collapsible, Side};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

pub const SIDEBAR_WIDTH: f32 = 200.0;

pub struct AppView {
    workspace_manager: Entity<WorkspaceManager>,
    workspace_panes: HashMap<String, Entity<PaneGroup>>,
    focus_handle: FocusHandle,
    _keystroke_subscription: Option<Subscription>,
    sidebar_collapsed: bool,
}

impl AppView {
    pub fn new(workspace_manager: Entity<WorkspaceManager>, cx: &mut Context<Self>) -> Self {
        Self {
            workspace_manager,
            workspace_panes: HashMap::new(),
            focus_handle: cx.focus_handle(),
            _keystroke_subscription: None,
            sidebar_collapsed: false,
        }
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
                    });

                WorkspaceConfig {
                    id: w.id.clone(),
                    name: w.name.clone(),
                    path: w.path.clone(),
                    layout,
                }
            })
            .collect();
        AppState {
            workspaces,
            active_workspace_index: manager.active_workspace_index,
        }
    }

    fn collect_member_config(&self, member: &Member, cx: &App) -> MemberConfig {
        match member {
            Member::Pane(pane) => {
                let pane = pane.read(cx);
                MemberConfig::Pane {
                    terminal_count: pane.terminals.len(),
                    active_index: pane.active_index,
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
            let pane_group = cx.new(|cx| {
                let member = Self::create_member_from_config(&workspace_config.layout, &path, cx);
                let active_pane = member.first_pane();
                let mut group = PaneGroup::with_root(path.clone(), member, cx);
                group.active_pane = active_pane;
                group
            });

            self.subscribe_to_pane_group(&pane_group, cx);
            self.workspace_panes
                .insert(workspace_config.id.clone(), pane_group);
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

        cx.notify();
    }

    fn create_member_from_config(
        config: &MemberConfig,
        path: &PathBuf,
        cx: &mut Context<PaneGroup>,
    ) -> Member {
        match config {
            MemberConfig::Pane {
                terminal_count,
                active_index,
            } => {
                let pane = cx.new(|cx| {
                    let mut pane = Pane::new(path.clone(), cx);
                    let count = (*terminal_count).max(1);
                    for _ in 0..count {
                        if let Ok(terminal) = Terminal::new(path.clone()) {
                            let terminal = Arc::new(parking_lot::Mutex::new(terminal));
                            let terminal_view = cx.new(|cx| TerminalView::new(terminal, cx));
                            pane.terminals.push(terminal_view);
                        }
                    }
                    pane.active_index = (*active_index).min(pane.terminals.len().saturating_sub(1));
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
                    .map(|m| Self::create_member_from_config(m, path, cx))
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
            let mut group = PaneGroup::new(path.clone(), cx);
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

    fn add_terminal_to_active_pane(&mut self, cx: &mut Context<Self>) {
        if let Some(workspace_id) = self.active_workspace_id(cx) {
            if let Some(pane_group) = self.workspace_panes.get(&workspace_id) {
                pane_group.update(cx, |pg, cx| {
                    if let Some(pane) = pg.active_pane.clone() {
                        pane.update(cx, |p, cx| {
                            p.add_terminal(cx);
                        });
                    }
                });
                self.save_state(cx);
            }
        }
    }

    pub fn next_terminal(&mut self, cx: &mut Context<Self>) {
        if let Some(workspace_id) = self.active_workspace_id(cx) {
            if let Some(pane_group) = self.workspace_panes.get(&workspace_id) {
                pane_group.update(cx, |pg, cx| {
                    if let Some(pane) = pg.active_pane.clone() {
                        pane.update(cx, |p, cx| {
                            if p.terminals.len() > 1 {
                                p.active_index = (p.active_index + 1) % p.terminals.len();
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
        self.workspace_manager.update(cx, |m, _| {
            if m.workspaces.len() > 1 {
                if let Some(current) = m.active_workspace_index {
                    m.active_workspace_index = Some((current + 1) % m.workspaces.len());
                }
            }
        });
        cx.notify();
        self.save_state(cx);
    }

    pub fn prev_workspace(&mut self, cx: &mut Context<Self>) {
        self.workspace_manager.update(cx, |m, _| {
            if m.workspaces.len() > 1 {
                if let Some(current) = m.active_workspace_index {
                    m.active_workspace_index = Some(if current == 0 {
                        m.workspaces.len() - 1
                    } else {
                        current - 1
                    });
                }
            }
        });
        cx.notify();
        self.save_state(cx);
    }

    pub fn prev_terminal(&mut self, cx: &mut Context<Self>) {
        if let Some(workspace_id) = self.active_workspace_id(cx) {
            if let Some(pane_group) = self.workspace_panes.get(&workspace_id) {
                pane_group.update(cx, |pg, cx| {
                    if let Some(pane) = pg.active_pane.clone() {
                        pane.update(cx, |p, cx| {
                            if p.terminals.len() > 1 {
                                p.active_index = if p.active_index == 0 {
                                    p.terminals.len() - 1
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
                let should_close_pane = pane_group.update(cx, |pg, cx| {
                    if let Some(pane) = pg.active_pane.clone() {
                        let (terminals_count, should_close) = pane.update(cx, |p, cx| {
                            let count = p.terminals.len();
                            if count > 1 {
                                p.terminals.remove(p.active_index);
                                if p.active_index >= p.terminals.len() {
                                    p.active_index = p.terminals.len() - 1;
                                }
                                cx.notify();
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
        self.workspace_manager.update(cx, |m, _| {
            m.set_active(index);
        });
        cx.notify();
        self.save_state(cx);
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

    pub fn sidebar_width(&self) -> f32 {
        if self.sidebar_collapsed {
            48.0
        } else {
            SIDEBAR_WIDTH
        }
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

        div()
            .id("app-view")
            .key_context("AppView")
            .track_focus(&focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x1e1e2e))
            .text_color(rgb(0xcdd6f4))
            // Custom title bar
            .child(
                div()
                    .id("title-bar")
                    .h(px(TITLE_BAR_HEIGHT))
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(0x181825))
                    .border_b_1()
                    .border_color(rgb(0x313244))
                    // Make the title bar draggable for window movement
                    .on_mouse_move(|_, _, _| {})
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x6c7086))
                            .child(config::APP_NAME)
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
                    ),
            )
    }
}

impl Focusable for AppView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
