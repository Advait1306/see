use crate::stores::{WindowStore, WindowStoreEvent, WorkspaceStore, WorkspaceStoreEvent};
use gpui::prelude::*;
use gpui::{FontWeight, SharedString, *};
use gpui_component::button::{Button, ButtonVariant, ButtonVariants};
use gpui_component::dialog::DialogButtonProps;
use gpui_component::menu::{PopupMenu, PopupMenuItem};
use gpui_component::sidebar::SidebarMenuItem;
use gpui_component::{ActiveTheme, IconName, Sizable, WindowExt};

pub struct WorkspaceSidebar {
    window_store: Entity<WindowStore>,
    focus_handle: FocusHandle,
    context_menu: Option<(Point<Pixels>, Entity<PopupMenu>)>,
    _context_menu_subscription: Option<Subscription>,
    _workspace_store_subscription: Subscription,
    _window_store_subscription: Subscription,
}

impl WorkspaceSidebar {
    pub fn new(window_store: Entity<WindowStore>, cx: &mut Context<Self>) -> Self {
        let workspace_store = WorkspaceStore::global(cx);
        let workspace_store_sub = cx.subscribe(&workspace_store, |_this, _store, event, cx| {
            match event {
                WorkspaceStoreEvent::WorkspacesChanged
                | WorkspaceStoreEvent::WorkspaceRemoved { .. } => {
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
            context_menu: None,
            _context_menu_subscription: None,
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

    fn show_context_menu(
        &mut self,
        workspace_id: String,
        workspace_name: String,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let menu = PopupMenu::build(window, cx, move |menu, _window, _cx| {
            let id = workspace_id.clone();
            let name = workspace_name.clone();
            menu.item(
                PopupMenuItem::new("Delete")
                    .icon(IconName::Delete)
                    .on_click(move |_, window, cx| {
                        Self::confirm_delete_workspace(id.clone(), name.clone(), window, cx);
                    }),
            )
        });

        let subscription = cx.subscribe(&menu, |this, _menu, _event: &DismissEvent, cx| {
            this.context_menu = None;
            this._context_menu_subscription = None;
            cx.notify();
        });

        self.context_menu = Some((position, menu));
        self._context_menu_subscription = Some(subscription);
        cx.notify();
    }

    fn confirm_delete_workspace(
        workspace_id: String,
        workspace_name: String,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.open_dialog(cx, move |dialog, _window, _cx| {
            let id = workspace_id.clone();
            dialog
                .title(format!("Delete \"{}\"?", workspace_name))
                .child(
                    "This workspace will be removed from the sidebar. Your files will not be affected.",
                )
                .confirm()
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("Delete")
                        .ok_variant(ButtonVariant::Danger),
                )
                .on_ok(move |_, _, cx| {
                    WorkspaceStore::global(cx).update(cx, |store, cx| {
                        store.remove_workspace(&id, cx);
                    });
                    true
                })
        });
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

        let context_menu = self.context_menu.clone();

        div()
            .id("workspace-sidebar")
            .size_full()
            .overflow_hidden()
            .track_focus(&self.focus_handle)
            .bg(cx.theme().sidebar)
            .text_color(cx.theme().sidebar_foreground)
            .border_r_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(workspaces.into_iter().enumerate().map(
                            |(idx, (id, name, is_active))| {
                                let id_for_click = id.clone();
                                let id_for_menu = id.clone();
                                let name_for_menu = name.clone();
                                let group_name: SharedString = format!("ws-item-{}", idx).into();

                                div()
                                    .id(("workspace-item", idx))
                                    .group(group_name.clone())
                                    .w_full()
                                    .h_7()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_2()
                                    .px_2()
                                    .rounded(cx.theme().radius)
                                    .text_sm()
                                    .cursor_pointer()
                                    .when(!is_active, |this| {
                                        this.hover(|s| {
                                            s.bg(cx.theme().sidebar_accent.opacity(0.8))
                                                .text_color(cx.theme().sidebar_accent_foreground)
                                        })
                                    })
                                    .when(is_active, |this| {
                                        this.font_weight(FontWeight::MEDIUM)
                                            .bg(cx.theme().sidebar_accent)
                                            .text_color(cx.theme().sidebar_accent_foreground)
                                    })
                                    .on_click(cx.listener(move |this, _, _window, cx| {
                                        this.select_workspace(id_for_click.clone(), cx);
                                    }))
                                    .child(div().flex_1().overflow_x_hidden().child(name))
                                    .child(
                                        Button::new(("ws-menu", idx))
                                            .xsmall()
                                            .ghost()
                                            .icon(IconName::Ellipsis)
                                            .invisible()
                                            .group_hover(group_name, |s| s.visible())
                                            .on_click(cx.listener({
                                                let id = id_for_menu.clone();
                                                let name = name_for_menu.clone();
                                                move |this, _event: &ClickEvent, window, cx| {
                                                    cx.stop_propagation();
                                                    let pos = window.mouse_position();
                                                    this.show_context_menu(
                                                        id.clone(),
                                                        name.clone(),
                                                        pos,
                                                        window,
                                                        cx,
                                                    );
                                                }
                                            })),
                                    )
                            },
                        ))
                        .child(
                            SidebarMenuItem::new("+ Add Workspace").on_click(cx.listener(
                                |this, _, _window, cx| {
                                    this.add_workspace(cx);
                                },
                            )),
                        ),
            )
            .when_some(context_menu, |el, (position, menu)| {
                el.child(
                    deferred(
                        anchored()
                            .position(position)
                            .anchor(Corner::TopLeft)
                            .child(menu),
                    )
                    .with_priority(1),
                )
            })
    }
}

impl Focusable for WorkspaceSidebar {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
