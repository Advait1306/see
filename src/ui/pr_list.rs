use crate::stores::github::{GitHubAccountStore, GitHubStore};
use crate::stores::{WindowStore, WindowStoreEvent, Workspace, WorkspaceEvent};
use crate::types::github::{AuthState, PullRequest, RemoteAuthState};
use gpui::{
    div, px, uniform_list, App, ClipboardItem, Context, Entity, FocusHandle, Focusable,
    FontWeight, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Subscription, Window,
};
use gpui::prelude::FluentBuilder;
use gpui_component::theme::ActiveTheme;
use gpui_component::Icon;
use gpui_component::Sizable;
use std::ops::Range;

const PR_ROW_HEIGHT: f32 = 72.0;

pub struct PrList {
    window_store: Entity<WindowStore>,
    pull_requests: Vec<PullRequest>,
    focus_handle: FocusHandle,
    _account_subscription: Subscription,
    _workspace_subscription: Option<Subscription>,
    _window_store_subscription: Subscription,
}

impl PrList {
    pub fn new(window_store: Entity<WindowStore>, cx: &mut Context<Self>) -> Self {
        let account_store = GitHubAccountStore::global(cx);
        let account_sub = cx.subscribe(&account_store, |_this, _store, _event, cx| {
            cx.notify();
        });

        let window_store_sub = cx.subscribe(&window_store, |this, _store, event, cx| {
            if matches!(event, WindowStoreEvent::ActiveWorkspaceChanged) {
                this.subscribe_to_workspace(cx);
                this.refresh_pull_requests(cx);
            }
        });

        let mut view = Self {
            window_store,
            pull_requests: Vec::new(),
            focus_handle: cx.focus_handle(),
            _account_subscription: account_sub,
            _workspace_subscription: None,
            _window_store_subscription: window_store_sub,
        };

        view.subscribe_to_workspace(cx);
        view.refresh_pull_requests(cx);
        view
    }

    fn subscribe_to_workspace(&mut self, cx: &mut Context<Self>) {
        let workspace = self.window_store.read(cx).active_workspace(cx);
        self._workspace_subscription = if let Some(workspace) = workspace {
            Some(cx.subscribe(&workspace, |this, _workspace, event, cx| {
                if matches!(event, WorkspaceEvent::PullRequestsUpdated) {
                    this.refresh_pull_requests(cx);
                }
            }))
        } else {
            None
        };
    }

    fn active_workspace(&self, cx: &App) -> Option<Entity<Workspace>> {
        self.window_store.read(cx).active_workspace(cx)
    }

    fn active_github_store(&self, cx: &App) -> Option<Entity<GitHubStore>> {
        self.active_workspace(cx)
            .and_then(|ws| ws.read(cx).github_store().cloned())
    }

    fn refresh_pull_requests(&mut self, cx: &mut Context<Self>) {
        self.pull_requests = if let Some(github_store) = self.active_github_store(cx) {
            github_store
                .read(cx)
                .all_pull_requests()
                .into_iter()
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
        cx.notify();
    }

    fn has_uninstalled_remotes(&self, cx: &App) -> bool {
        if let Some(github_store) = self.active_github_store(cx) {
            github_store
                .read(cx)
                .remotes()
                .values()
                .any(|r| r.auth_state == RemoteAuthState::AppNotInstalled)
        } else {
            false
        }
    }
}

impl Render for PrList {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let account_store = GitHubAccountStore::global(cx);
        let auth_state = account_store.read(cx).auth_state().clone();
        let has_github_store = self.active_github_store(cx).is_some();

        div()
            .id("pr-list")
            .debug_selector(|| "pr-list".into())
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.sidebar)
            // Header
            .child(
                div()
                    .h(px(32.0))
                    .w_full()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .px(px(12.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.foreground)
                            .child("Pull Requests"),
                    ),
            )
            // Content
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .map(|el| match &auth_state {
                        AuthState::SignedOut | AuthState::Error(_) => {
                            el.child(self.render_sign_in(cx, &auth_state))
                        }
                        AuthState::Authenticating {
                            user_code,
                            verification_uri: _,
                        } => el.child(self.render_authenticating(cx, user_code)),
                        AuthState::SignedIn { username: _ } => {
                            if !has_github_store {
                                el.child(self.render_no_repo(cx))
                            } else if self.has_uninstalled_remotes(cx) && self.pull_requests.is_empty() {
                                el.child(self.render_install_prompt(cx))
                            } else {
                                el.child(self.render_pull_requests(cx))
                            }
                        }
                    }),
            )
    }
}

impl PrList {
    fn render_sign_in(&self, cx: &Context<Self>, auth_state: &AuthState) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(12.0))
            .p(px(16.0))
            .when_some(
                if let AuthState::Error(msg) = auth_state {
                    Some(msg.clone())
                } else {
                    None
                },
                |el, msg| {
                    el.child(
                        div()
                            .text_xs()
                            .text_color(theme.danger)
                            .text_center()
                            .child(msg),
                    )
                },
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .text_center()
                    .child("Connect to GitHub to see pull requests"),
            )
            .child(
                div()
                    .id("sign-in-button")
                    .debug_selector(|| "sign-in-button".into())
                    .px(px(16.0))
                    .py(px(8.0))
                    .rounded(px(6.0))
                    .bg(theme.foreground)
                    .text_color(theme.background)
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.9))
                    .on_click(cx.listener(|_this, _, _, cx| {
                        let account_store = GitHubAccountStore::global(cx);
                        account_store.update(cx, |store, cx| {
                            store.sign_in(cx);
                        });
                    }))
                    .child("Sign in with GitHub"),
            )
    }

    fn render_authenticating(
        &self,
        cx: &Context<Self>,
        user_code: &str,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let code = user_code.to_string();
        let code_for_click = code.clone();

        div()
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(16.0))
            .p(px(16.0))
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .text_center()
                    .child("Enter this code on GitHub:"),
            )
            .child(
                div()
                    .id("device-code")
                    .debug_selector(|| "device-code".into())
                    .px(px(16.0))
                    .py(px(10.0))
                    .rounded(px(8.0))
                    .bg(theme.background)
                    .border_1()
                    .border_color(theme.border)
                    .cursor_pointer()
                    .on_click(cx.listener(move |_this, _, _window, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(
                            code_for_click.clone(),
                        ));
                    }))
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.foreground)
                            .text_center()
                            .child(code),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .text_center()
                    .child("Click code to copy. Waiting for authorization..."),
            )
    }

    fn render_no_repo(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .w_full()
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("No git repository in workspace"),
            )
    }

    fn render_install_prompt(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(12.0))
            .p(px(16.0))
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .text_center()
                    .child("Install the August app on your GitHub organizations to see pull requests."),
            )
            .child(
                div()
                    .id("install-app-button")
                    .px(px(16.0))
                    .py(px(8.0))
                    .rounded(px(6.0))
                    .bg(theme.foreground)
                    .text_color(theme.background)
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.9))
                    .on_click(cx.listener(|_this, _, _, _cx| {
                        let _ = std::process::Command::new("open")
                            .arg("https://github.com/apps/august-see/installations/new")
                            .spawn();
                    }))
                    .child("Install August App"),
            )
    }

    fn render_pull_requests(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let count = self.pull_requests.len();

        if count == 0 {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("No open pull requests"),
                )
                .into_any_element();
        }

        uniform_list("pr-list-items", count, cx.processor(
            |this, range: Range<usize>, _window, cx| {
                let theme = cx.theme().clone();
                range
                    .filter_map(|ix| this.pull_requests.get(ix).map(|pr| (ix, pr.clone())))
                    .map(|(ix, pr)| render_pr_row(&theme, &pr, ix))
                    .collect()
            },
        ))
        .size_full()
        .into_any_element()
    }

}

fn render_pr_row(
    theme: &gpui_component::theme::Theme,
    pr: &PullRequest,
    index: usize,
) -> impl IntoElement + use<> {
    let url = pr.html_url.clone();

    div()
        .id(("pr-row", index))
        .w_full()
        .h(px(PR_ROW_HEIGHT))
        .px(px(12.0))
        .py(px(8.0))
        .flex()
        .flex_col()
        .gap(px(2.0))
        .cursor_pointer()
        .hover(|s| s.bg(gpui::black().opacity(0.05)))
        .border_b_1()
        .border_color(theme.border.opacity(0.5))
        .on_click(move |_, _, _| {
            let _ = std::process::Command::new("open")
                .arg(&url)
                .spawn();
        })
        // Title row
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(
                    Icon::default()
                        .path("icons/git-pull-request.svg")
                        .xsmall()
                        .text_color(if pr.draft {
                            theme.muted_foreground
                        } else {
                            theme.success
                        }),
                )
                .child(
                    div()
                        .flex_1()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.foreground)
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(pr.title.clone()),
                ),
        )
        // Metadata row
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .gap(px(6.0))
                .pl(px(20.0))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!("#{}", pr.number)),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(pr.author_login.clone()),
                )
                .when(pr.draft, |el| {
                    el.child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .px(px(4.0))
                            .py(px(1.0))
                            .rounded(px(3.0))
                            .bg(theme.border)
                            .child("Draft"),
                    )
                }),
        )
        // Branch row
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .gap(px(4.0))
                .pl(px(20.0))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(format!(
                            "{} \u{2190} {}",
                            pr.base_ref, pr.head_ref
                        )),
                ),
        )
}

impl Focusable for PrList {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
