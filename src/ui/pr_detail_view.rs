use crate::stores::github::http;
use crate::stores::github::GitHubAccountStore;
use crate::types::github::{
    PrComment, PrCommit, PrDetail, PrReview, PullRequest, PullRequestState, ReviewState,
};
use crate::types::{PrDetailTabConfig, Tab, TabConfig};
use gpui::{
    div, list, px, AnyElement, App, Context, FocusHandle, Focusable, FontWeight,
    InteractiveElement, IntoElement, ListAlignment, ListState, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window,
};
use gpui::prelude::FluentBuilder;
use gpui_component::theme::ActiveTheme;

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

pub struct PrDetailView {
    pull_request: PullRequest,
    active_sub_tab: PrSubTab,
    load_state: LoadState,
    conversation_rows: Vec<ConversationRow>,
    list_state: ListState,
    commit_rows: Vec<CommitRow>,
    commits_list_state: ListState,
    focus_handle: FocusHandle,
}

impl PrDetailView {
    pub fn new(pull_request: PullRequest, cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            pull_request,
            active_sub_tab: PrSubTab::Conversation,
            load_state: LoadState::Loading,
            conversation_rows: Vec::new(),
            list_state: ListState::new(0, ListAlignment::Top, px(1024.0)),
            commit_rows: Vec::new(),
            commits_list_state: ListState::new(0, ListAlignment::Top, px(1024.0)),
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
            let client_body = client.clone();
            let client_comments = client.clone();
            let client_reviews = client.clone();
            let client_commits = client.clone();
            let owner_b = owner.clone();
            let owner_c = owner.clone();
            let owner_r = owner.clone();
            let owner_k = owner.clone();
            let repo_b = repo.clone();
            let repo_c = repo.clone();
            let repo_r = repo.clone();
            let repo_k = repo.clone();

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

            let body = body_fut.await.unwrap_or(None);
            let comments = comments_fut.await.unwrap_or_default();
            let reviews = reviews_fut.await.unwrap_or_default();
            let commits = commits_fut.await.unwrap_or_default();

            let detail = PrDetail {
                body,
                comments,
                reviews,
                commits,
            };

            this.update(cx, |this, cx| {
                this.conversation_rows = build_conversation_rows(&this.pull_request, &detail);
                this.list_state.reset(this.conversation_rows.len());
                this.commit_rows = build_commit_rows(&detail.commits);
                this.commits_list_state.reset(this.commit_rows.len());
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

    fn render_placeholder(&self, label: &str, cx: &Context<Self>) -> impl IntoElement + use<'_> {
        let theme = cx.theme();
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(format!("{} — coming soon", label)),
            )
    }
}

impl Render for PrDetailView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .id("pr-detail-view")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .child(self.render_sub_tab_bar(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .map(|el| match self.active_sub_tab {
                        PrSubTab::Conversation => el.child(self.render_conversation(cx)),
                        PrSubTab::Commits => el.child(self.render_commits(cx)),
                        PrSubTab::FilesChanged => {
                            el.child(self.render_placeholder("Files Changed", cx))
                        }
                    }),
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

fn format_timestamp(ts: &str) -> String {
    if let Some(date_part) = ts.split('T').next() {
        date_part.to_string()
    } else {
        ts.to_string()
    }
}
