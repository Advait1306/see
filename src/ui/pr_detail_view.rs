use crate::stores::github::http;
use crate::stores::github::GitHubAccountStore;
use crate::stores::{DiffLine, DiffLineTag};
use crate::types::github::{
    PrComment, PrCommit, PrDetail, PrFile, PrFileStatus, PrReview, PullRequest, PullRequestState,
    ReviewState,
};
use crate::types::{PrDetailTabConfig, Tab, TabConfig};
use crate::ui::editor::EditorView;
use crate::ui::file_tree::NonSelectableItem;
use gpui::{
    div, list, px, AnyElement, App, AppContext as _, Context, Entity, FocusHandle, Focusable,
    FontWeight, InteractiveElement, IntoElement, ListAlignment,
    ParentElement, Render,
    StatefulInteractiveElement, Styled, Window,
};
use gpui::prelude::FluentBuilder;
use gpui_component::list::{List, ListDelegate, ListEvent, ListItem, ListState};
use gpui_component::theme::ActiveTheme;
use gpui_component::{Icon, IconName, IndexPath, Sizable};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrSubTab {
    Conversation,
    Commits,
    FilesChanged,
}

enum LoadState {
    Loading,
    Loaded,
    Error(String),
}

#[derive(Clone)]
enum ConversationRow {
    Header {
        title: String,
        number: u64,
        state: PullRequestState,
        draft: bool,
        author_login: String,
        base_ref: String,
        head_ref: String,
    },
    Body {
        author_login: String,
        body: String,
    },
    Comment {
        author: String,
        body: String,
        created_at: String,
        review_state: Option<ReviewState>,
    },
}

#[derive(Clone)]
enum CommitRow {
    Commit {
        short_sha: String,
        title: String,
        author_name: String,
        author: Option<String>,
        date: String,
    },
}

#[derive(Clone)]
struct PrFileEntry {
    name: String,
    path: String,
    is_dir: bool,
    is_expanded: bool,
    depth: usize,
    file_index: Option<usize>,
    status: Option<PrFileStatus>,
    additions: u64,
    deletions: u64,
}

struct PrFileTreeDelegate {
    entries: Vec<PrFileEntry>,
    selected_index: Option<usize>,
}

impl ListDelegate for PrFileTreeDelegate {
    type Item = NonSelectableItem;

    fn items_count(&self, _section: usize, _cx: &App) -> usize {
        self.entries.len()
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let entry = self.entries.get(ix.row)?;
        let depth = entry.depth;
        let is_dir = entry.is_dir;
        let is_expanded = entry.is_expanded;
        let name = entry.name.clone();
        let is_selected = self.selected_index == Some(ix.row);

        let theme = cx.theme();
        let muted_color = theme.muted_foreground;
        let blue_color = theme.primary;
        let foreground_color = theme.foreground;
        let selection_color = theme.selection;

        let row = div()
            .h(px(24.0))
            .w_full()
            .flex()
            .items_center()
            .gap(px(4.0))
            .pl(px(8.0 + (depth as f32 * 16.0)))
            .pr(px(8.0))
            .when(is_selected && !is_dir, |el| el.bg(selection_color));

        if is_dir {
            let chevron_icon = if is_expanded {
                IconName::ChevronDown
            } else {
                IconName::ChevronRight
            };
            let folder_icon = if is_expanded {
                IconName::FolderOpen
            } else {
                IconName::Folder
            };

            Some(NonSelectableItem(
                ListItem::new(ix).py_0().px_0().child(
                    row.child(
                        div()
                            .w(px(16.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Icon::new(chevron_icon).xsmall().text_color(muted_color)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .child(Icon::new(folder_icon).small().text_color(blue_color)),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(foreground_color)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(name),
                    ),
                ),
            ))
        } else {
            let (badge_letter, badge_color) = match entry.status.as_ref() {
                Some(PrFileStatus::Added) => ("A", theme.success),
                Some(PrFileStatus::Modified) => ("M", theme.warning),
                Some(PrFileStatus::Removed) => ("D", theme.danger),
                Some(PrFileStatus::Renamed) => ("R", theme.link),
                Some(PrFileStatus::Copied) | Some(PrFileStatus::Changed) => {
                    ("C", muted_color)
                }
                None => ("?", muted_color),
            };
            let additions = entry.additions;
            let deletions = entry.deletions;

            Some(NonSelectableItem(
                ListItem::new(ix).py_0().px_0().child(
                    row
                        // Extra indent to align past chevron column
                        .pl(px(8.0 + (depth as f32 * 16.0) + 20.0))
                        .child(
                            div()
                                .w(px(14.0))
                                .h(px(14.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(2.0))
                                .bg(badge_color.opacity(0.15))
                                .text_color(badge_color)
                                .child(
                                    div()
                                        .text_size(px(9.0))
                                        .font_weight(FontWeight::BOLD)
                                        .child(badge_letter),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_sm()
                                .text_color(foreground_color)
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(name),
                        )
                        .child(
                            div()
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .when(additions > 0, |el| {
                                    el.child(
                                        div()
                                            .text_size(px(9.0))
                                            .text_color(theme.success)
                                            .child(format!("+{}", additions)),
                                    )
                                })
                                .when(deletions > 0, |el| {
                                    el.child(
                                        div()
                                            .text_size(px(9.0))
                                            .text_color(theme.danger)
                                            .child(format!("-{}", deletions)),
                                    )
                                }),
                        ),
                ),
            ))
        }
    }

    fn render_empty(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        let muted_color = cx.theme().muted_foreground;
        div()
            .p(px(12.0))
            .text_sm()
            .text_color(muted_color)
            .child("No files changed")
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) {
        self.selected_index = ix.map(|i| i.row);
    }
}

pub struct PrDetailView {
    pull_request: PullRequest,
    active_sub_tab: PrSubTab,
    load_state: LoadState,
    conversation_rows: Vec<ConversationRow>,
    list_state: gpui::ListState,
    commit_rows: Vec<CommitRow>,
    commits_list_state: gpui::ListState,
    pr_files: Vec<PrFile>,
    expanded_dirs: HashSet<String>,
    file_tree_list: Option<Entity<ListState<PrFileTreeDelegate>>>,
    selected_file_editor: Option<Entity<EditorView>>,
    focus_handle: FocusHandle,
}

impl PrDetailView {
    pub fn new(pull_request: PullRequest, cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            pull_request,
            active_sub_tab: PrSubTab::Conversation,
            load_state: LoadState::Loading,
            conversation_rows: Vec::new(),
            list_state: gpui::ListState::new(0, ListAlignment::Top, px(1024.0)),
            commit_rows: Vec::new(),
            commits_list_state: gpui::ListState::new(0, ListAlignment::Top, px(1024.0)),
            pr_files: Vec::new(),
            expanded_dirs: HashSet::new(),
            file_tree_list: None,
            selected_file_editor: None,
            focus_handle: cx.focus_handle(),
        };
        view.fetch_detail(cx);
        view
    }

    fn fetch_detail(&mut self, cx: &mut Context<Self>) {
        let account_store = GitHubAccountStore::global(cx);
        let token = match account_store.read(cx).access_token() {
            Some(t) => t.to_string(),
            None => {
                self.load_state = LoadState::Error("Not signed in".into());
                return;
            }
        };

        let owner = self.pull_request.repo.owner.clone();
        let repo = self.pull_request.repo.repo.clone();
        let number = self.pull_request.number;

        cx.spawn(async move |this, cx| {
            let client = http::http_client();
            let handle = http::http_runtime().handle().clone();

            let token_body = token.clone();
            let token_comments = token.clone();
            let token_reviews = token.clone();
            let token_commits = token.clone();
            let token_files = token.clone();
            let client_body = client.clone();
            let client_comments = client.clone();
            let client_reviews = client.clone();
            let client_commits = client.clone();
            let client_files = client.clone();
            let owner_b = owner.clone();
            let owner_c = owner.clone();
            let owner_r = owner.clone();
            let owner_k = owner.clone();
            let owner_f = owner.clone();
            let repo_b = repo.clone();
            let repo_c = repo.clone();
            let repo_r = repo.clone();
            let repo_k = repo.clone();
            let repo_f = repo.clone();

            let body_fut = handle.spawn(async move {
                let url = format!(
                    "https://api.github.com/repos/{}/{}/pulls/{}",
                    owner_b, repo_b, number
                );
                let resp = client_body
                    .get(&url)
                    .header("Authorization", format!("Bearer {}", token_body))
                    .header("User-Agent", "august-app")
                    .header("Accept", "application/vnd.github+json")
                    .send()
                    .await;
                match resp {
                    Ok(r) if r.status().is_success() => {
                        let json: serde_json::Value = r.json().await.unwrap_or_default();
                        json["body"].as_str().map(|s| s.to_string())
                    }
                    _ => None,
                }
            });

            let comments_fut = handle.spawn(async move {
                let url = format!(
                    "https://api.github.com/repos/{}/{}/issues/{}/comments?per_page=100",
                    owner_c, repo_c, number
                );
                let resp = client_comments
                    .get(&url)
                    .header("Authorization", format!("Bearer {}", token_comments))
                    .header("User-Agent", "august-app")
                    .header("Accept", "application/vnd.github+json")
                    .send()
                    .await;
                match resp {
                    Ok(r) if r.status().is_success() => {
                        let json: serde_json::Value = r.json().await.unwrap_or_default();
                        parse_comments(&json)
                    }
                    _ => Vec::new(),
                }
            });

            let reviews_fut = handle.spawn(async move {
                let url = format!(
                    "https://api.github.com/repos/{}/{}/pulls/{}/reviews?per_page=100",
                    owner_r, repo_r, number
                );
                let resp = client_reviews
                    .get(&url)
                    .header("Authorization", format!("Bearer {}", token_reviews))
                    .header("User-Agent", "august-app")
                    .header("Accept", "application/vnd.github+json")
                    .send()
                    .await;
                match resp {
                    Ok(r) if r.status().is_success() => {
                        let json: serde_json::Value = r.json().await.unwrap_or_default();
                        parse_reviews(&json)
                    }
                    _ => Vec::new(),
                }
            });

            let commits_fut = handle.spawn(async move {
                let url = format!(
                    "https://api.github.com/repos/{}/{}/pulls/{}/commits?per_page=250",
                    owner_k, repo_k, number
                );
                let resp = client_commits
                    .get(&url)
                    .header("Authorization", format!("Bearer {}", token_commits))
                    .header("User-Agent", "august-app")
                    .header("Accept", "application/vnd.github+json")
                    .send()
                    .await;
                match resp {
                    Ok(r) if r.status().is_success() => {
                        let json: serde_json::Value = r.json().await.unwrap_or_default();
                        parse_commits(&json)
                    }
                    _ => Vec::new(),
                }
            });

            let files_fut = handle.spawn(async move {
                let url = format!(
                    "https://api.github.com/repos/{}/{}/pulls/{}/files?per_page=100",
                    owner_f, repo_f, number
                );
                let resp = client_files
                    .get(&url)
                    .header("Authorization", format!("Bearer {}", token_files))
                    .header("User-Agent", "august-app")
                    .header("Accept", "application/vnd.github+json")
                    .send()
                    .await;
                match resp {
                    Ok(r) if r.status().is_success() => {
                        let json: serde_json::Value = r.json().await.unwrap_or_default();
                        parse_files(&json)
                    }
                    _ => Vec::new(),
                }
            });

            let body = body_fut.await.unwrap_or(None);
            let comments = comments_fut.await.unwrap_or_default();
            let reviews = reviews_fut.await.unwrap_or_default();
            let commits = commits_fut.await.unwrap_or_default();
            let files = files_fut.await.unwrap_or_default();

            let detail = PrDetail {
                body,
                comments,
                reviews,
                commits,
                files,
            };

            this.update(cx, |this, cx| {
                this.conversation_rows = build_conversation_rows(&this.pull_request, &detail);
                this.list_state.reset(this.conversation_rows.len());
                this.commit_rows = build_commit_rows(&detail.commits);
                this.commits_list_state.reset(this.commit_rows.len());
                this.pr_files = detail.files.clone();
                this.expanded_dirs = collect_all_dirs(&detail.files);
                // file_tree_list will be lazily created in render
                this.load_state = LoadState::Loaded;
                cx.notify();
            })
        })
        .detach();
    }

    fn render_sub_tab_bar(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let tabs = [
            (PrSubTab::Conversation, "Conversation"),
            (PrSubTab::Commits, "Commits"),
            (PrSubTab::FilesChanged, "Files Changed"),
        ];

        div()
            .flex()
            .h(px(36.0))
            .border_b_1()
            .border_color(theme.border)
            .items_end()
            .px_2()
            .gap_1()
            .children(tabs.into_iter().enumerate().map(|(i, (tab, label))| {
                let is_active = self.active_sub_tab == tab;
                div()
                    .id(("pr-sub-tab", i))
                    .px_3()
                    .py(px(6.0))
                    .cursor_pointer()
                    .text_sm()
                    .when(is_active, |el| {
                        el.border_b_2()
                            .border_color(theme.foreground)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.foreground)
                    })
                    .when(!is_active, |el| {
                        el.text_color(theme.muted_foreground)
                            .hover(|el| el.text_color(theme.foreground))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.active_sub_tab = tab;
                        cx.notify();
                    }))
                    .child(label)
            }))
    }

    fn render_conversation(&self, cx: &Context<Self>) -> gpui::AnyElement {
        let theme = cx.theme();

        match &self.load_state {
            LoadState::Loading => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("Loading..."),
                )
                .into_any_element(),
            LoadState::Error(msg) => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.danger)
                        .child(msg.clone()),
                )
                .into_any_element(),
            LoadState::Loaded => {
                list(
                    self.list_state.clone(),
                    cx.processor(|this, ix: usize, window, cx| -> AnyElement {
                        if let Some(row) = this.conversation_rows.get(ix) {
                            render_conversation_row(row, ix, window, cx).into_any_element()
                        } else {
                            div().into_any_element()
                        }
                    }),
                )
                .size_full()
                .into_any_element()
            }
        }
    }

    fn render_commits(&self, cx: &Context<Self>) -> gpui::AnyElement {
        let theme = cx.theme();

        match &self.load_state {
            LoadState::Loading => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("Loading..."),
                )
                .into_any_element(),
            LoadState::Error(msg) => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.danger)
                        .child(msg.clone()),
                )
                .into_any_element(),
            LoadState::Loaded if self.commit_rows.is_empty() => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("No commits"),
                )
                .into_any_element(),
            LoadState::Loaded => {
                list(
                    self.commits_list_state.clone(),
                    cx.processor(|this, ix: usize, _window, cx| -> AnyElement {
                        if let Some(row) = this.commit_rows.get(ix) {
                            render_commit_row(row, ix, cx).into_any_element()
                        } else {
                            div().into_any_element()
                        }
                    }),
                )
                .size_full()
                .into_any_element()
            }
        }
    }

    fn select_file(&mut self, file_index: usize, cx: &mut Context<Self>) {
        let Some(file) = self.pr_files.get(file_index) else {
            return;
        };
        let Some(ref patch) = file.patch else {
            return; // binary file, no patch
        };

        let diff_lines = parse_patch_to_diff_lines(patch);
        let filename = file.filename.clone();

        let editor = cx.new(|cx| {
            EditorView::new_from_diff_lines(filename.clone(), diff_lines, cx)
        });

        self.selected_file_editor = Some(editor);
        cx.notify();
    }

    fn toggle_dir(&mut self, path: &str, cx: &mut Context<Self>) {
        if self.expanded_dirs.contains(path) {
            self.expanded_dirs.remove(path);
        } else {
            self.expanded_dirs.insert(path.to_string());
        }
        self.refresh_file_tree(cx);
    }

    fn refresh_file_tree(&mut self, cx: &mut Context<Self>) {
        let entries = build_file_tree_entries(&self.pr_files, &self.expanded_dirs);
        if let Some(ref list_state) = self.file_tree_list {
            list_state.update(cx, |state, _cx| {
                state.delegate_mut().entries = entries;
            });
        }
        cx.notify();
    }

    fn ensure_file_tree_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.file_tree_list.is_some() {
            return;
        }
        if !matches!(self.load_state, LoadState::Loaded) {
            return;
        }

        let entries = build_file_tree_entries(&self.pr_files, &self.expanded_dirs);
        let delegate = PrFileTreeDelegate {
            entries,
            selected_index: None,
        };
        let list_state = cx.new(|cx| ListState::new(delegate, window, cx));

        cx.subscribe(&list_state, |this, list_entity, event: &ListEvent, cx| {
            if let ListEvent::Confirm(ix) = event {
                let entry = list_entity
                    .read(cx)
                    .delegate()
                    .entries
                    .get(ix.row)
                    .cloned();
                if let Some(entry) = entry {
                    if entry.is_dir {
                        this.toggle_dir(&entry.path, cx);
                    } else if let Some(file_index) = entry.file_index {
                        // Update selected index in the delegate for highlight
                        list_entity.update(cx, |state, _cx| {
                            state.delegate_mut().selected_index = Some(ix.row);
                        });
                        this.select_file(file_index, cx);
                    }
                }
            }
        })
        .detach();

        self.file_tree_list = Some(list_state);
    }

    fn render_files_changed(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let theme = cx.theme();

        match &self.load_state {
            LoadState::Loading => {
                return div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child("Loading..."),
                    )
                    .into_any_element();
            }
            LoadState::Error(msg) => {
                let msg = msg.clone();
                return div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.danger)
                            .child(msg),
                    )
                    .into_any_element();
            }
            LoadState::Loaded => {}
        }

        self.ensure_file_tree_list(window, cx);

        let theme = cx.theme();

        if self.pr_files.is_empty() {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("No files changed"),
                )
                .into_any_element();
        }

        let list_state = self.file_tree_list.clone().unwrap();

        // Summary header
        let total_files = self.pr_files.len();
        let total_additions: u64 = self.pr_files.iter().map(|f| f.additions).sum();
        let total_deletions: u64 = self.pr_files.iter().map(|f| f.deletions).sum();

        let tree_panel = div()
            .w(px(280.0))
            .flex_shrink_0()
            .h_full()
            .border_r_1()
            .border_color(theme.border)
            .bg(theme.sidebar)
            .flex()
            .flex_col()
            .child(
                div()
                    .px(px(12.0))
                    .py(px(8.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.foreground)
                            .child(format!(
                                "{} file{}",
                                total_files,
                                if total_files == 1 { "" } else { "s" }
                            )),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.success)
                            .child(format!("+{}", total_additions)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.danger)
                            .child(format!("-{}", total_deletions)),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(List::new(&list_state).py_0()),
            );

        let editor_panel = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .map(|el| {
                if let Some(ref editor) = self.selected_file_editor {
                    el.child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .child(editor.clone()),
                    )
                } else {
                    el.child(
                        div()
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child("Select a file to view diff"),
                            ),
                    )
                }
            });

        div()
            .size_full()
            .flex()
            .flex_row()
            .child(tree_panel)
            .child(editor_panel)
            .into_any_element()
    }
}

impl Render for PrDetailView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.active_sub_tab;

        let tab_content = match active_tab {
            PrSubTab::Conversation => self.render_conversation(cx),
            PrSubTab::Commits => self.render_commits(cx),
            PrSubTab::FilesChanged => self.render_files_changed(window, cx),
        };

        let bg = cx.theme().background;

        div()
            .id("pr-detail-view")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .bg(bg)
            .child(self.render_sub_tab_bar(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(tab_content),
            )
    }
}

impl Focusable for PrDetailView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Tab for PrDetailView {
    fn label(&self, _cx: &App) -> String {
        format!("PR #{}", self.pull_request.number)
    }

    fn to_config(&self, _cx: &App) -> TabConfig {
        let pr = &self.pull_request;
        TabConfig::PullRequest(PrDetailTabConfig {
            owner: pr.repo.owner.clone(),
            repo: pr.repo.repo.clone(),
            number: pr.number,
            title: pr.title.clone(),
            author_login: pr.author_login.clone(),
            head_ref: pr.head_ref.clone(),
            base_ref: pr.base_ref.clone(),
            draft: pr.draft,
            html_url: pr.html_url.clone(),
            created_at: pr.created_at.clone(),
            updated_at: pr.updated_at.clone(),
        })
    }
}

fn build_conversation_rows(pr: &PullRequest, detail: &PrDetail) -> Vec<ConversationRow> {
    let mut rows = Vec::new();

    rows.push(ConversationRow::Header {
        title: pr.title.clone(),
        number: pr.number,
        state: pr.state.clone(),
        draft: pr.draft,
        author_login: pr.author_login.clone(),
        base_ref: pr.base_ref.clone(),
        head_ref: pr.head_ref.clone(),
    });

    if let Some(body) = &detail.body {
        if !body.trim().is_empty() {
            rows.push(ConversationRow::Body {
                author_login: pr.author_login.clone(),
                body: body.clone(),
            });
        }
    }

    let mut comments: Vec<ConversationRow> = Vec::new();
    for c in &detail.comments {
        comments.push(ConversationRow::Comment {
            author: c.author.clone(),
            body: c.body.clone(),
            created_at: c.created_at.clone(),
            review_state: None,
        });
    }
    for r in &detail.reviews {
        if r.body.trim().is_empty() && r.state == ReviewState::Commented {
            continue;
        }
        comments.push(ConversationRow::Comment {
            author: r.author.clone(),
            body: r.body.clone(),
            created_at: r.created_at.clone(),
            review_state: Some(r.state.clone()),
        });
    }
    comments.sort_by(|a, b| {
        let a_ts = if let ConversationRow::Comment { created_at, .. } = a { created_at.as_str() } else { "" };
        let b_ts = if let ConversationRow::Comment { created_at, .. } = b { created_at.as_str() } else { "" };
        a_ts.cmp(b_ts)
    });

    rows.extend(comments);
    rows
}

fn render_conversation_row(
    row: &ConversationRow,
    index: usize,
    window: &Window,
    cx: &App,
) -> impl IntoElement + use<> {
    let theme = cx.theme().clone();

    div()
        .id(("conversation-row", index))
        .w_full()
        .px(px(20.0))
        .py(px(8.0))
        .child(match row {
            ConversationRow::Header {
                title,
                number,
                state,
                draft,
                author_login,
                base_ref,
                head_ref,
            } => {
                let state_label = match state {
                    PullRequestState::Open => "Open",
                    PullRequestState::Closed => "Closed",
                    PullRequestState::Merged => "Merged",
                };
                let state_color = match state {
                    PullRequestState::Open => theme.success,
                    PullRequestState::Closed => theme.danger,
                    PullRequestState::Merged => theme.primary,
                };

                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.foreground)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(title.clone()),
                            )
                            .child(
                                div()
                                    .text_lg()
                                    .flex_shrink_0()
                                    .text_color(theme.muted_foreground)
                                    .child(format!("#{}", number)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .flex_wrap()
                            .child(
                                div()
                                    .px(px(8.0))
                                    .py(px(2.0))
                                    .rounded(px(12.0))
                                    .bg(state_color.opacity(0.15))
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(state_color)
                                    .child(state_label),
                            )
                            .when(*draft, |el| {
                                el.child(
                                    div()
                                        .px(px(8.0))
                                        .py(px(2.0))
                                        .rounded(px(12.0))
                                        .bg(theme.border)
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child("Draft"),
                                )
                            })
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(format!(
                                        "{} wants to merge into {} from {}",
                                        author_login, base_ref, head_ref
                                    )),
                            ),
                    )
                    .into_any_element()
            }
            ConversationRow::Body { author_login, body } => div()
                .w_full()
                .p(px(16.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(theme.border)
                .bg(theme.sidebar)
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!("{} opened this pull request", author_login)),
                )
                .child(
                    crate::ui::markdown::render_markdown(
                        body,
                        &format!("pr-body-{}", index),
                        window,
                        cx,
                    ),
                )
                .into_any_element(),
            ConversationRow::Comment {
                author,
                body,
                created_at,
                review_state,
            } => {
                let review_label = review_state.as_ref().map(|s| match s {
                    ReviewState::Approved => ("approved", theme.success),
                    ReviewState::ChangesRequested => ("requested changes", theme.danger),
                    ReviewState::Commented => ("reviewed", theme.muted_foreground),
                    ReviewState::Dismissed => ("dismissed review", theme.muted_foreground),
                    ReviewState::Pending => ("pending review", theme.muted_foreground),
                });

                div()
                    .w_full()
                    .p(px(16.0))
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.sidebar)
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.foreground)
                                    .child(author.clone()),
                            )
                            .when_some(review_label, |el, (label, color)| {
                                el.child(
                                    div()
                                        .px(px(6.0))
                                        .py(px(1.0))
                                        .rounded(px(8.0))
                                        .bg(color.opacity(0.15))
                                        .text_xs()
                                        .text_color(color)
                                        .child(label),
                                )
                            })
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(format_timestamp(created_at)),
                            ),
                    )
                    .when(!body.trim().is_empty(), |el| {
                        el.child(
                            crate::ui::markdown::render_markdown(
                                body,
                                &format!("pr-comment-{}", index),
                                window,
                                cx,
                            ),
                        )
                    })
                    .into_any_element()
            }
        })
}

fn parse_comments(json: &serde_json::Value) -> Vec<PrComment> {
    let Some(items) = json.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let author = item["user"]["login"].as_str()?.to_string();
            let body = item["body"].as_str().unwrap_or("").to_string();
            let created_at = item["created_at"].as_str().unwrap_or("").to_string();
            Some(PrComment {
                author,
                body,
                created_at,
            })
        })
        .collect()
}

fn parse_reviews(json: &serde_json::Value) -> Vec<PrReview> {
    let Some(items) = json.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let author = item["user"]["login"].as_str()?.to_string();
            let body = item["body"].as_str().unwrap_or("").to_string();
            let created_at = item["submitted_at"].as_str().unwrap_or("").to_string();
            let state = match item["state"].as_str().unwrap_or("") {
                "APPROVED" => ReviewState::Approved,
                "CHANGES_REQUESTED" => ReviewState::ChangesRequested,
                "COMMENTED" => ReviewState::Commented,
                "DISMISSED" => ReviewState::Dismissed,
                _ => ReviewState::Pending,
            };
            Some(PrReview {
                author,
                body,
                state,
                created_at,
            })
        })
        .collect()
}

fn parse_commits(json: &serde_json::Value) -> Vec<PrCommit> {
    let Some(items) = json.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let sha = item["sha"].as_str()?.to_string();
            let message = item["commit"]["message"].as_str().unwrap_or("").to_string();
            let author_name = item["commit"]["author"]["name"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let author = item["author"]["login"].as_str().map(|s| s.to_string());
            let date = item["commit"]["author"]["date"]
                .as_str()
                .unwrap_or("")
                .to_string();
            Some(PrCommit {
                sha,
                message,
                author_name,
                author,
                date,
            })
        })
        .collect()
}

fn build_commit_rows(commits: &[PrCommit]) -> Vec<CommitRow> {
    commits
        .iter()
        .map(|c| {
            let title = c.message.lines().next().unwrap_or("").to_string();
            let short_sha = c.sha.get(..7).unwrap_or(&c.sha).to_string();
            CommitRow::Commit {
                short_sha,
                title,
                author_name: c.author_name.clone(),
                author: c.author.clone(),
                date: c.date.clone(),
            }
        })
        .collect()
}

fn render_commit_row(row: &CommitRow, index: usize, cx: &App) -> impl IntoElement + use<> {
    let theme = cx.theme().clone();

    let CommitRow::Commit {
        short_sha,
        title,
        author_name,
        author,
        date,
    } = row;

    let display_author = if let Some(login) = author {
        login.clone()
    } else {
        author_name.clone()
    };

    div()
        .id(("commit-row", index))
        .w_full()
        .px(px(20.0))
        .py(px(10.0))
        .border_b_1()
        .border_color(theme.border)
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .text_sm()
                .text_color(theme.foreground)
                .overflow_hidden()
                .text_ellipsis()
                .child(title.clone()),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(
                    div()
                        .font_family("monospace")
                        .text_color(theme.link)
                        .child(short_sha.clone()),
                )
                .child(display_author)
                .child(format_timestamp(date)),
        )
}

fn parse_files(json: &serde_json::Value) -> Vec<PrFile> {
    let Some(items) = json.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let filename = item["filename"].as_str()?.to_string();
            let status = match item["status"].as_str().unwrap_or("") {
                "added" => PrFileStatus::Added,
                "removed" => PrFileStatus::Removed,
                "renamed" => PrFileStatus::Renamed,
                "copied" => PrFileStatus::Copied,
                "changed" => PrFileStatus::Changed,
                _ => PrFileStatus::Modified,
            };
            let additions = item["additions"].as_u64().unwrap_or(0);
            let deletions = item["deletions"].as_u64().unwrap_or(0);
            let patch = item["patch"].as_str().map(|s| s.to_string());
            let previous_filename = item["previous_filename"].as_str().map(|s| s.to_string());
            Some(PrFile {
                filename,
                status,
                additions,
                deletions,
                patch,
                previous_filename,
            })
        })
        .collect()
}

fn collect_all_dirs(files: &[PrFile]) -> HashSet<String> {
    let mut dirs = HashSet::new();
    for file in files {
        let parts: Vec<&str> = file.filename.split('/').collect();
        let mut path = String::new();
        for part in &parts[..parts.len().saturating_sub(1)] {
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str(part);
            dirs.insert(path.clone());
        }
    }
    dirs
}

fn build_file_tree_entries(
    files: &[PrFile],
    expanded_dirs: &HashSet<String>,
) -> Vec<PrFileEntry> {
    let mut sorted: Vec<(Vec<&str>, usize)> = files
        .iter()
        .enumerate()
        .map(|(i, f)| (f.filename.split('/').collect::<Vec<_>>(), i))
        .collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    let mut entries = Vec::new();
    let mut emitted_dirs: HashSet<String> = HashSet::new();

    for (parts, file_index) in &sorted {
        let mut dir_path = String::new();
        let mut parent_collapsed = false;

        for (depth, dir_name) in parts[..parts.len().saturating_sub(1)].iter().enumerate() {
            if !dir_path.is_empty() {
                dir_path.push('/');
            }
            dir_path.push_str(dir_name);

            if parent_collapsed {
                break;
            }

            if !emitted_dirs.contains(&dir_path) {
                emitted_dirs.insert(dir_path.clone());
                let is_expanded = expanded_dirs.contains(&dir_path);
                entries.push(PrFileEntry {
                    name: dir_name.to_string(),
                    path: dir_path.clone(),
                    is_dir: true,
                    is_expanded,
                    depth,
                    file_index: None,
                    status: None,
                    additions: 0,
                    deletions: 0,
                });
                if !is_expanded {
                    parent_collapsed = true;
                }
            } else if !expanded_dirs.contains(&dir_path) {
                parent_collapsed = true;
            }
        }

        if parent_collapsed {
            continue;
        }

        let file = &files[*file_index];
        let depth = parts.len() - 1;
        let name = parts.last().unwrap_or(&"").to_string();
        entries.push(PrFileEntry {
            name,
            path: file.filename.clone(),
            is_dir: false,
            is_expanded: false,
            depth,
            file_index: Some(*file_index),
            status: Some(file.status.clone()),
            additions: file.additions,
            deletions: file.deletions,
        });
    }

    entries
}

fn parse_hunk_header(line: &str) -> Option<(usize, usize)> {
    // Parse @@ -old_start,old_count +new_start,new_count @@
    let line = line.strip_prefix("@@ ")?;
    let parts: Vec<&str> = line.splitn(3, ' ').collect();
    if parts.len() < 2 {
        return None;
    }

    let old_part = parts[0].strip_prefix('-')?;
    let new_part = parts[1].strip_prefix('+')?;

    let old_start: usize = old_part.split(',').next()?.parse().ok()?;
    let new_start: usize = new_part.split(',').next()?.parse().ok()?;

    Some((old_start, new_start))
}

fn parse_patch_to_diff_lines(patch: &str) -> Vec<DiffLine> {
    let mut lines = Vec::new();
    let mut old_line: usize = 0;
    let mut new_line: usize = 0;

    for raw_line in patch.lines() {
        if raw_line.starts_with("@@") {
            if let Some((old_start, new_start)) = parse_hunk_header(raw_line) {
                old_line = old_start;
                new_line = new_start;
            }
            // Add the hunk header as a context line
            lines.push(DiffLine {
                tag: DiffLineTag::Equal,
                old_line_num: None,
                new_line_num: None,
                content: raw_line.to_string(),
            });
        } else if let Some(content) = raw_line.strip_prefix('+') {
            lines.push(DiffLine {
                tag: DiffLineTag::Insert,
                old_line_num: None,
                new_line_num: Some(new_line),
                content: content.to_string(),
            });
            new_line += 1;
        } else if let Some(content) = raw_line.strip_prefix('-') {
            lines.push(DiffLine {
                tag: DiffLineTag::Delete,
                old_line_num: Some(old_line),
                new_line_num: None,
                content: content.to_string(),
            });
            old_line += 1;
        } else if let Some(content) = raw_line.strip_prefix(' ') {
            lines.push(DiffLine {
                tag: DiffLineTag::Equal,
                old_line_num: Some(old_line),
                new_line_num: Some(new_line),
                content: content.to_string(),
            });
            old_line += 1;
            new_line += 1;
        } else {
            // No-newline-at-end-of-file markers or other content
            lines.push(DiffLine {
                tag: DiffLineTag::Equal,
                old_line_num: None,
                new_line_num: None,
                content: raw_line.to_string(),
            });
        }
    }

    lines
}

fn format_timestamp(ts: &str) -> String {
    if let Some(date_part) = ts.split('T').next() {
        date_part.to_string()
    } else {
        ts.to_string()
    }
}
