use crate::terminal::Terminal;
use crate::ui::TerminalView;
use crate::workspace::WorkspaceManager;
use gpui::prelude::*;
use gpui::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;


struct WorkspaceTerminals {
    terminals: Vec<Entity<TerminalView>>,
    active_index: usize,
    path: PathBuf,
}

pub struct AppView {
    workspace_manager: Entity<WorkspaceManager>,
    workspace_terminals: HashMap<String, WorkspaceTerminals>,
    focus_handle: FocusHandle,
    _keystroke_subscription: Option<Subscription>,
}

impl AppView {
    pub fn new(workspace_manager: Entity<WorkspaceManager>, cx: &mut Context<Self>) -> Self {
        let mut app = Self {
            workspace_manager,
            workspace_terminals: HashMap::new(),
            focus_handle: cx.focus_handle(),
            _keystroke_subscription: None,
        };

        // Add default workspace at home directory
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        app.add_workspace("Home".to_string(), home, cx);

        app
    }

    pub fn set_keystroke_subscription(&mut self, subscription: Subscription) {
        self._keystroke_subscription = Some(subscription);
    }

    pub fn add_workspace(&mut self, name: String, path: PathBuf, cx: &mut Context<Self>) {
        let (workspace_id, new_index) = self.workspace_manager.update(cx, |m, _| {
            m.add_workspace(name, path.clone());
            let idx = m.workspaces.len() - 1;
            (m.workspaces.last().unwrap().id.clone(), idx)
        });

        if let Ok(terminal) = Terminal::new(path.clone()) {
            let terminal = Arc::new(parking_lot::Mutex::new(terminal));
            let terminal_view = cx.new(|cx| TerminalView::new(terminal, cx));
            self.workspace_terminals.insert(
                workspace_id,
                WorkspaceTerminals {
                    terminals: vec![terminal_view],
                    active_index: 0,
                    path,
                },
            );
        }

        // Switch to the new workspace
        self.select_workspace(new_index, cx);
    }

    fn add_terminal_to_workspace(&mut self, workspace_id: &str, cx: &mut Context<Self>) {
        if let Some(workspace_terms) = self.workspace_terminals.get_mut(workspace_id) {
            if let Ok(terminal) = Terminal::new(workspace_terms.path.clone()) {
                let terminal = Arc::new(parking_lot::Mutex::new(terminal));
                let terminal_view = cx.new(|cx| TerminalView::new(terminal, cx));
                workspace_terms.terminals.push(terminal_view);
                workspace_terms.active_index = workspace_terms.terminals.len() - 1;
                cx.notify();
            }
        }
    }

    fn select_terminal(&mut self, workspace_id: &str, index: usize, cx: &mut Context<Self>) {
        if let Some(workspace_terms) = self.workspace_terminals.get_mut(workspace_id) {
            if index < workspace_terms.terminals.len() {
                workspace_terms.active_index = index;
                cx.notify();
            }
        }
    }

    pub fn next_terminal(&mut self, cx: &mut Context<Self>) {
        if let Some(workspace_id) = self.active_workspace_id(cx) {
            if let Some(workspace_terms) = self.workspace_terminals.get_mut(&workspace_id) {
                if workspace_terms.terminals.len() > 1 {
                    workspace_terms.active_index =
                        (workspace_terms.active_index + 1) % workspace_terms.terminals.len();
                    cx.notify();
                }
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
    }

    pub fn prev_terminal(&mut self, cx: &mut Context<Self>) {
        if let Some(workspace_id) = self.active_workspace_id(cx) {
            if let Some(workspace_terms) = self.workspace_terminals.get_mut(&workspace_id) {
                if workspace_terms.terminals.len() > 1 {
                    workspace_terms.active_index = if workspace_terms.active_index == 0 {
                        workspace_terms.terminals.len() - 1
                    } else {
                        workspace_terms.active_index - 1
                    };
                    cx.notify();
                }
            }
        }
    }

    pub fn select_workspace(&mut self, index: usize, cx: &mut Context<Self>) {
        self.workspace_manager.update(cx, |m, _| {
            m.set_active(index);
        });
        cx.notify();
    }

    fn active_terminal(&self, cx: &App) -> Option<Entity<TerminalView>> {
        let manager = self.workspace_manager.read(cx);
        manager.active_workspace().and_then(|w| {
            self.workspace_terminals
                .get(&w.id)
                .and_then(|wt| wt.terminals.get(wt.active_index).cloned())
        })
    }

    fn active_workspace_id(&self, cx: &App) -> Option<String> {
        let manager = self.workspace_manager.read(cx);
        manager.active_workspace().map(|w| w.id.clone())
    }

    fn render_tabs(&self, cx: &Context<Self>) -> impl IntoElement {
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

        div()
            .flex()
            .h(px(40.0))
            .bg(rgb(0x181825))
            .border_b_1()
            .border_color(rgb(0x313244))
            .items_center()
            .px_2()
            .gap_1()
            .children(workspaces.into_iter().map(|(idx, name, is_active)| {
                div()
                    .id(ElementId::Name(format!("tab-{}", idx).into()))
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .when(is_active, |el| el.bg(rgb(0x313244)))
                    .when(!is_active, |el| el.hover(|el| el.bg(rgb(0x45475a))))
                    .text_color(if is_active {
                        rgb(0xcdd6f4)
                    } else {
                        rgb(0xa6adc8)
                    })
                    .text_sm()
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        this.select_workspace(idx, cx);
                    }))
                    .child(name)
            }))
            .child(
                div()
                    .id("add-workspace")
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|el| el.bg(rgb(0x45475a)))
                    .text_color(rgb(0x6c7086))
                    .text_sm()
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
                    }))
                    .child("+"),
            )
    }

    fn render_terminal_tabs(&self, cx: &Context<Self>) -> impl IntoElement {
        let workspace_id = self.active_workspace_id(cx);

        let terminal_info: Vec<(usize, bool)> = workspace_id
            .as_ref()
            .and_then(|id| self.workspace_terminals.get(id))
            .map(|wt| {
                wt.terminals
                    .iter()
                    .enumerate()
                    .map(|(i, _)| (i, i == wt.active_index))
                    .collect()
            })
            .unwrap_or_default();

        let workspace_id_for_tabs = workspace_id.clone();
        let workspace_id_for_add = workspace_id.clone();

        div()
            .flex()
            .h(px(32.0))
            .bg(rgb(0x11111b))
            .border_b_1()
            .border_color(rgb(0x313244))
            .items_center()
            .px_2()
            .gap_1()
            .children(terminal_info.into_iter().map(|(idx, is_active)| {
                let ws_id = workspace_id_for_tabs.clone();
                div()
                    .id(ElementId::Name(format!("term-tab-{}", idx).into()))
                    .px_3()
                    .py_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .when(is_active, |el| el.bg(rgb(0x1e1e2e)))
                    .when(!is_active, |el| el.hover(|el| el.bg(rgb(0x313244))))
                    .text_color(if is_active {
                        rgb(0xcdd6f4)
                    } else {
                        rgb(0x6c7086)
                    })
                    .text_xs()
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        if let Some(ref id) = ws_id {
                            this.select_terminal(id, idx, cx);
                        }
                    }))
                    .child(format!("Terminal {}", idx + 1))
            }))
            .child(
                div()
                    .id("add-terminal")
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(|el| el.bg(rgb(0x313244)))
                    .text_color(rgb(0x6c7086))
                    .text_xs()
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        if let Some(ref id) = workspace_id_for_add {
                            this.add_terminal_to_workspace(id, cx);
                        }
                    }))
                    .child("+"),
            )
    }
}

impl Render for AppView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_terminal = self.active_terminal(cx);

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
            .child(self.render_tabs(cx))
            .child(self.render_terminal_tabs(cx))
            .child(
                div()
                    .id("terminal-container")
                    .flex_1()
                    .w_full()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .map(|el| {
                        if let Some(terminal) = active_terminal {
                            el.child(terminal)
                        } else {
                            el
                        }
                    }),
            )
    }
}

impl Focusable for AppView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
