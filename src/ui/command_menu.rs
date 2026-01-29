use crate::stores::{WindowStore, WindowStoreEvent, Workspace, WorkspaceEvent};
use crate::ui::pane::TabItem;
use crate::ui::EditorView;
use gpui::prelude::*;
use gpui::*;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::theme::ActiveTheme;
use gpui_component::{Icon, IconName, Sizable};
use std::path::PathBuf;

pub struct FileItem {
    pub path: PathBuf,
    pub display_path: String,
}

pub struct CommandMenu {
    window_store: Entity<WindowStore>,
    visible: bool,
    input_state: Entity<InputState>,
    query: String,
    all_files: Vec<FileItem>,
    filtered_indices: Vec<(usize, i32)>, // (index, score)
    selected_index: usize,
    scroll_handle: ScrollHandle,
    focus_handle: FocusHandle,
    previous_focus: Option<FocusHandle>,
    _input_subscription: Subscription,
    _window_store_subscription: Subscription,
    _workspace_subscription: Option<Subscription>,
}

impl CommandMenu {
    pub fn new(
        window_store: Entity<WindowStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input_state = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Search files...")
        });

        let input_subscription =
            cx.subscribe_in(&input_state, window, |this, state, event, _window, cx| {
                if let InputEvent::Change = event {
                    this.query = state.read(cx).value().to_string();
                    this.filter_files();
                    this.selected_index = 0;
                    cx.notify();
                }
            });

        let window_store_subscription = cx.subscribe(&window_store, |this, _store, event, cx| {
            if let WindowStoreEvent::ActiveWorkspaceChanged = event {
                this.refresh_files(cx);
                this.subscribe_to_active_workspace(cx);
            }
        });

        let mut menu = Self {
            window_store,
            visible: false,
            input_state,
            query: String::new(),
            all_files: Vec::new(),
            filtered_indices: Vec::new(),
            selected_index: 0,
            scroll_handle: ScrollHandle::new(),
            focus_handle: cx.focus_handle(),
            previous_focus: None,
            _input_subscription: input_subscription,
            _window_store_subscription: window_store_subscription,
            _workspace_subscription: None,
        };

        menu.subscribe_to_active_workspace(cx);
        menu.refresh_files(cx);
        menu
    }

    fn subscribe_to_active_workspace(&mut self, cx: &mut Context<Self>) {
        self._workspace_subscription = self.active_workspace(cx).map(|workspace| {
            cx.subscribe(&workspace, |this, _workspace, event, cx| {
                if matches!(event, WorkspaceEvent::FileTreeChanged) {
                    this.refresh_files(cx);
                }
            })
        });
    }

    fn active_workspace(&self, cx: &App) -> Option<Entity<Workspace>> {
        self.window_store.read(cx).active_workspace(cx)
    }

    fn refresh_files(&mut self, cx: &App) {
        self.all_files.clear();

        let Some(workspace) = self.active_workspace(cx) else {
            return;
        };

        let workspace_path = workspace.read(cx).path.clone();
        self.scan_directory_recursive(&workspace_path, &workspace_path);
        self.filter_files();
    }

    fn scan_directory_recursive(&mut self, dir: &PathBuf, workspace_root: &PathBuf) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };

        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden files and common ignored directories
            if name.starts_with('.') || name == "node_modules" || name == "target" {
                continue;
            }

            if path.is_dir() {
                self.scan_directory_recursive(&path, workspace_root);
            } else {
                let display_path = path
                    .strip_prefix(workspace_root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();

                self.all_files.push(FileItem {
                    path,
                    display_path,
                });
            }
        }
    }

    fn filter_files(&mut self) {
        let query = self.query.to_lowercase();

        if query.is_empty() {
            // Show all files sorted alphabetically when no query
            self.filtered_indices = self
                .all_files
                .iter()
                .enumerate()
                .map(|(i, _)| (i, 0))
                .collect();
            self.filtered_indices.sort_by(|a, b| {
                self.all_files[a.0].display_path.cmp(&self.all_files[b.0].display_path)
            });
        } else {
            // Fuzzy match and sort by score
            self.filtered_indices = self
                .all_files
                .iter()
                .enumerate()
                .filter_map(|(i, file)| {
                    let score = fuzzy_match(&query, &file.display_path.to_lowercase());
                    if score > 0 {
                        Some((i, score))
                    } else {
                        None
                    }
                })
                .collect();

            // Sort by score (higher is better)
            self.filtered_indices.sort_by(|a, b| b.1.cmp(&a.1));
        }
    }

    pub fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.visible {
            self.hide(window, cx);
        } else {
            self.show(window, cx);
        }
    }

    pub fn show(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.visible {
            return;
        }
        self.previous_focus = window.focused(cx);
        self.visible = true;
        self.query.clear();
        self.refresh_files(cx);
        self.selected_index = 0;
        self.input_state.update(cx, |state, cx| {
            state.set_value("", window, cx);
            state.focus(window, cx);
        });
        cx.notify();
    }

    pub fn hide(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.visible = false;
        if let Some(focus_handle) = self.previous_focus.take() {
            focus_handle.focus(window);
        }
        cx.notify();
    }

    fn select_next(&mut self, cx: &mut Context<Self>) {
        if !self.filtered_indices.is_empty() && self.selected_index < self.filtered_indices.len() - 1 {
            self.selected_index += 1;
            self.scroll_handle.scroll_to_item(self.selected_index);
            cx.notify();
        }
    }

    fn select_prev(&mut self, cx: &mut Context<Self>) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            self.scroll_handle.scroll_to_item(self.selected_index);
            cx.notify();
        }
    }

    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(&(file_index, _)) = self.filtered_indices.get(self.selected_index) {
            let path = self.all_files[file_index].path.clone();
            self.open_file(path, cx);
        }
        self.hide(window, cx);
    }

    fn open_file(&self, path: PathBuf, cx: &mut Context<Self>) {
        let Some(workspace) = self.active_workspace(cx) else {
            return;
        };

        let pane_store = workspace.read(cx).pane_store().clone();
        pane_store.update(cx, |ps, cx| {
            if let Some(pane) = ps.active_pane.clone() {
                pane.update(cx, |p, cx| {
                    let editor_view =
                        cx.new(|cx| EditorView::new(path.clone(), Default::default(), cx));
                    p.add_tab(TabItem::Editor(editor_view), cx);
                });
            }
        });
    }

    fn select_item(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_index = index;
        self.confirm(window, cx);
    }

    fn render_item(
        &self,
        list_index: usize,
        file: &FileItem,
        is_selected: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let bg_color = if is_selected {
            theme.list_active
        } else {
            theme.transparent
        };

        // Split path into directory and filename for display
        let path = std::path::Path::new(&file.display_path);
        let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let dir = path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();

        div()
            .id(ElementId::Integer(list_index as u64))
            .h(px(32.0))
            .w_full()
            .px(px(12.0))
            .flex()
            .items_center()
            .gap(px(10.0))
            .bg(bg_color)
            .rounded(px(6.0))
            .cursor_pointer()
            .hover(|s| s.bg(theme.list_active))
            .on_mouse_down(MouseButton::Left, |_, _, _| {})
            .on_click(cx.listener(move |this, _, window, cx| {
                this.select_item(list_index, window, cx);
            }))
            .child(
                Icon::new(IconName::File)
                    .small()
                    .text_color(theme.muted_foreground),
            )
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(px(8.0))
                    .overflow_hidden()
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.foreground)
                            .child(filename),
                    )
                    .when(!dir.is_empty(), |el| {
                        el.child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(dir),
                        )
                    }),
            )
    }
}

impl Render for CommandMenu {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.visible {
            return div().into_any_element();
        }

        let theme = cx.theme();
        let filtered_items: Vec<_> = self
            .filtered_indices
            .iter()
            .enumerate()
            .take(100) // Limit displayed items for performance
            .map(|(list_idx, &(file_idx, _))| {
                let is_selected = list_idx == self.selected_index;
                self.render_item(list_idx, &self.all_files[file_idx], is_selected, cx)
            })
            .collect();

        let file_count = self.filtered_indices.len();

        div()
            .id("command-menu-overlay")
            .absolute()
            .inset_0()
            .flex()
            .items_start()
            .justify_center()
            .pt(px(80.0))
            .bg(hsla(0.0, 0.0, 0.0, 0.5))
            .occlude()
            .child(
                div()
                    .id("command-menu")
                    .key_context("CommandMenu")
                    .track_focus(&self.focus_handle)
                    .on_mouse_down_out(cx.listener(|this, _, window, cx| {
                        this.hide(window, cx);
                    }))
                    .on_action(cx.listener(|this, _: &crate::commands::HideCommandMenu, window, cx| {
                        this.hide(window, cx);
                    }))
                    .on_action(cx.listener(|this, _: &SelectNext, _window, cx| {
                        this.select_next(cx);
                    }))
                    .on_action(cx.listener(|this, _: &SelectPrev, _window, cx| {
                        this.select_prev(cx);
                    }))
                    .on_action(cx.listener(|this, _: &Confirm, window, cx| {
                        this.confirm(window, cx);
                    }))
                    .w(px(550.0))
                    .max_h(px(450.0))
                    .bg(theme.background)
                    .border_1()
                    .border_color(theme.border)
                    .rounded(px(12.0))
                    .shadow_lg()
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .p(px(12.0))
                            .border_b_1()
                            .border_color(theme.border)
                            .child(
                                Input::new(&self.input_state)
                                    .appearance(false)
                                    .prefix(Icon::new(IconName::Search).small())
                                    .cleanable(true),
                            ),
                    )
                    .child(
                        div()
                            .id("command-menu-list")
                            .flex_1()
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll_handle)
                            .p(px(8.0))
                            .children(filtered_items)
                            .when(self.filtered_indices.is_empty(), |el| {
                                el.child(
                                    div()
                                        .p(px(12.0))
                                        .text_sm()
                                        .text_color(theme.muted_foreground)
                                        .child("No files found"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .px(px(12.0))
                            .py(px(8.0))
                            .border_t_1()
                            .border_color(theme.border)
                            .flex()
                            .justify_between()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(format!("{} files", file_count)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("↑↓ navigate • ↵ open • esc close"),
                            ),
                    ),
            )
            .into_any_element()
    }
}

impl Focusable for CommandMenu {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

fn fuzzy_match(pattern: &str, text: &str) -> i32 {
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();

    if pattern_chars.is_empty() {
        return 1;
    }

    let mut pattern_idx = 0;
    let mut score = 0;
    let mut last_match_idx: Option<usize> = None;
    let mut consecutive_bonus = 0;

    for (i, &c) in text_chars.iter().enumerate() {
        if pattern_idx < pattern_chars.len() && c == pattern_chars[pattern_idx] {
            score += 1;

            // Bonus for consecutive matches
            if let Some(last) = last_match_idx {
                if i == last + 1 {
                    consecutive_bonus += 2;
                }
            }

            // Bonus for matching at word boundaries
            if i == 0 || text_chars[i - 1] == '/' || text_chars[i - 1] == '_' || text_chars[i - 1] == '-' {
                score += 3;
            }

            last_match_idx = Some(i);
            pattern_idx += 1;
        }
    }

    if pattern_idx == pattern_chars.len() {
        score + consecutive_bonus
    } else {
        0
    }
}

actions!(command_menu, [SelectNext, SelectPrev, Confirm,]);

pub fn register_command_menu_keybindings(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("down", SelectNext, Some("CommandMenu")),
        KeyBinding::new("up", SelectPrev, Some("CommandMenu")),
        KeyBinding::new("enter", Confirm, Some("CommandMenu")),
    ]);
}
