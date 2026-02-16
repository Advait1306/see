use crate::stores::{WindowStore, WindowStoreEvent, WorkspaceStore, WorkspaceStoreEvent};
use gpui::{
    div, App, Context, Entity, Focusable, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, Render, Styled, Subscription, Window,
};
use gpui_component::Side;
use gpui_component::sidebar::{Sidebar, SidebarMenu, SidebarMenuItem};

pub struct WorkspaceSidebar {
    window_store: Entity<WindowStore>,
    focus_handle: FocusHandle,
    _workspace_store_subscription: Subscription,
    _window_store_subscription: Subscription,
}

impl WorkspaceSidebar {
    pub fn new(
        window_store: Entity<WindowStore>,
        cx: &mut Context<Self>,
    ) -> Self {
        let workspace_store = WorkspaceStore::global(cx);
        let workspace_store_sub = cx.subscribe(&workspace_store, |_this, _store, event, cx| {
            match event {
                WorkspaceStoreEvent::WorkspacesChanged => {
                    cx.notify();
                }
                WorkspaceStoreEvent::WorkspaceUpdated(_) => {
                    cx.notify();
                }
                WorkspaceStoreEvent::PaneLayoutChanged(_) => {}
            }
        });

        let window_store_sub = cx.subscribe(&window_store, |_this, _store, event, cx| {
            match event {
                WindowStoreEvent::ActiveWorkspaceChanged => {
                    cx.notify();
                }
                WindowStoreEvent::UiStateChanged => {}
            }
        });

        Self {
            window_store,
            focus_handle: cx.focus_handle(),
            _workspace_store_subscription: workspace_store_sub,
            _window_store_subscription: window_store_sub,
        }
    }

    fn select_workspace(&self, id: String, cx: &mut Context<Self>) {
        self.window_store.update(cx, |store, cx| {
            store.set_active_workspace(id, cx);
        });
    }

    fn add_workspace(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |_this, cx| {
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
                    WorkspaceStore::global(cx).update(cx, |store, cx| {
                        store.add_workspace(name, path, cx);
                    });
                });
            }
        })
        .detach();
    }
}

impl Render for WorkspaceSidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ws_store = WorkspaceStore::global(cx);
        let ws_store = ws_store.read(cx);
        let active_id = self.window_store.read(cx).active_workspace_id().cloned();
        let workspaces: Vec<(String, String, bool)> = ws_store
            .workspaces()
            .map(|ws_ref| {
                let ws = ws_ref.entity.read(cx);
                let is_active = active_id.as_ref() == Some(&ws.id);
                (ws.id.clone(), ws.name.clone(), is_active)
            })
            .collect();

        div()
            .id("workspace-sidebar")
            .size_full()
            .overflow_hidden()
            .track_focus(&self.focus_handle)
            .child(
                Sidebar::new(Side::Left).w_full().child(
                    SidebarMenu::new()
                        .children(workspaces.into_iter().map(|(id, name, is_active)| {
                            SidebarMenuItem::new(name)
                                .active(is_active)
                                .on_click(cx.listener(move |this, _, _window, cx| {
                                    this.select_workspace(id.clone(), cx);
                                }))
                        }))
                        .child(
                            SidebarMenuItem::new("+ Add Workspace").on_click(cx.listener(
                                |this, _, _window, cx| {
                                    this.add_workspace(cx);
                                },
                            )),
                        ),
                ),
            )
    }
}

impl Focusable for WorkspaceSidebar {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
