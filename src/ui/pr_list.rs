use crate::stores::{
    AuthState, GitHubStore, WindowStore, WindowStoreEvent, Workspace,
};
use crate::ui::pane::TabItem;
use crate::ui::pr_review::PrReviewView;
use gpui::prelude::*;
use gpui::*;
use gpui_component::theme::ActiveTheme;

pub struct PrList {
    window_store: Entity<WindowStore>,
    focus_handle: FocusHandle,
    _window_store_subscription: Subscription,
    _github_store_subscription: Option<Subscription>,
}

impl PrList {
    pub fn new(window_store: Entity<WindowStore>, cx: &mut Context<Self>) -> Self {
        let window_store_sub =
            cx.subscribe(&window_store, |this, _store, event, cx| match event {
                WindowStoreEvent::ActiveWorkspaceChanged => {
                    this.subscribe_to_github_store(cx);
                    cx.notify();
                }
                WindowStoreEvent::UiStateChanged => {}
            });

        let mut pr_list = Self {
            window_store,
            focus_handle: cx.focus_handle(),
            _window_store_subscription: window_store_sub,
            _github_store_subscription: None,
        };

        pr_list.subscribe_to_github_store(cx);
        pr_list
    }

    fn active_workspace(&self, cx: &App) -> Option<Entity<Workspace>> {
        self.window_store.read(cx).active_workspace(cx)
    }

    fn github_store(&self, cx: &App) -> Option<Entity<GitHubStore>> {
        let ws = self.active_workspace(cx)?;
        ws.read(cx).github_store().cloned()
    }

    fn subscribe_to_github_store(&mut self, cx: &mut Context<Self>) {
        if let Some(gh_store) = self.github_store(cx) {
            self._github_store_subscription =
                Some(cx.subscribe(&gh_store, |_this, _store, _event, cx| {
                    cx.notify();
                }));
        } else {
            self._github_store_subscription = None;
        }
    }

    fn start_sign_in(&self, cx: &mut Context<Self>) {
        if let Some(gh_store) = self.github_store(cx) {
            gh_store.update(cx, |store, cx| {
                store.start_device_flow(cx);
            });
        }
    }

    fn sign_out(&self, cx: &mut Context<Self>) {
        if let Some(gh_store) = self.github_store(cx) {
            gh_store.update(cx, |store, cx| {
                store.sign_out(cx);
            });
        }
    }

    fn retry_installation_check(&self, cx: &mut Context<Self>) {
        if let Some(gh_store) = self.github_store(cx) {
            gh_store.update(cx, |store, cx| {
                store.retry_installation_check(cx);
            });
        }
    }

    fn open_pr_review(&self, pr_number: u64, pr_title: String, cx: &mut Context<Self>) {
        let gh_store = match self.github_store(cx) {
            Some(s) => s,
            None => return,
        };
        let ws = match self.active_workspace(cx) {
            Some(ws) => ws,
            None => return,
        };

        let pr_review = cx.new(|cx| PrReviewView::new(gh_store, pr_number, pr_title, cx));
        let pane_store = ws.read(cx).pane_store().clone();

        if let Some(active_pane) = pane_store.read(cx).active_pane.clone() {
            active_pane.update(cx, |pane, cx| {
                pane.add_tab(TabItem::PrReview(pr_review), cx);
            });
        }
    }

    fn copy_to_clipboard(text: &str, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
    }

    fn render_no_github_remote(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("Not a GitHub repository"),
            )
    }

    fn render_unauthenticated(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("Sign in to view pull requests"),
            )
            .child(
                div()
                    .id("sign-in-button")
                    .debug_selector(|| "sign-in-button".into())
                    .px_4()
                    .py_2()
                    .rounded(px(6.0))
                    .bg(theme.primary)
                    .text_color(theme.primary_foreground)
                    .text_sm()
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.9))
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.start_sign_in(cx);
                    }))
                    .child("Sign in with GitHub"),
            )
    }

    fn render_waiting_for_user(
        &self,
        user_code: &str,
        _verification_uri: &str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let code = user_code.to_string();
        let code_for_copy = user_code.to_string();

        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("Enter this code on GitHub:"),
            )
            .child(
                div()
                    .id("user-code-display")
                    .debug_selector(|| "user-code-display".into())
                    .px_4()
                    .py_2()
                    .rounded(px(6.0))
                    .bg(theme.background)
                    .border_1()
                    .border_color(theme.border)
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.foreground)
                    .child(code),
            )
            .child(
                div()
                    .id("copy-code-button")
                    .debug_selector(|| "copy-code-button".into())
                    .px_3()
                    .py_1()
                    .rounded(px(4.0))
                    .bg(theme.border)
                    .text_xs()
                    .text_color(theme.foreground)
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.8))
                    .on_click(cx.listener(move |_this, _, _window, cx| {
                        Self::copy_to_clipboard(&code_for_copy, cx);
                    }))
                    .child("Copy code"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("Waiting for authorization..."),
            )
    }

    fn render_checking_installation(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("Checking repository access..."),
            )
    }

    fn render_needs_installation(
        &self,
        install_url: &str,
        owner: &str,
        repo: &str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let url = install_url.to_string();

        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .px_4()
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .text_center()
                    .child(format!(
                        "August needs to be installed on {}/{}",
                        owner, repo
                    )),
            )
            .child(
                div()
                    .id("install-app-button")
                    .px_4()
                    .py_2()
                    .rounded(px(6.0))
                    .bg(theme.primary)
                    .text_color(theme.primary_foreground)
                    .text_sm()
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.9))
                    .on_click(cx.listener(move |_this, _, _window, _cx| {
                        let _ = open::that(&url);
                    }))
                    .child("Install GitHub App"),
            )
            .child(
                div()
                    .id("retry-install-button")
                    .px_3()
                    .py_1()
                    .rounded(px(4.0))
                    .bg(theme.border)
                    .text_xs()
                    .text_color(theme.foreground)
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.8))
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.retry_installation_check(cx);
                    }))
                    .child("Retry"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .text_center()
                    .child("Install the app, then click Retry"),
            )
    }

    fn render_no_access(&self, owner: &str, repo: &str, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(format!("No access to {}/{}", owner, repo)),
            )
            .child(
                div()
                    .id("sign-out-button")
                    .px_3()
                    .py_1()
                    .rounded(px(4.0))
                    .bg(theme.border)
                    .text_xs()
                    .text_color(theme.foreground)
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.8))
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.sign_out(cx);
                    }))
                    .child("Sign out"),
            )
    }

    fn render_error(&self, error: &str, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .child(
                div()
                    .text_sm()
                    .text_color(theme.danger)
                    .child(error.to_string()),
            )
            .child(
                div()
                    .id("retry-button")
                    .px_3()
                    .py_1()
                    .rounded(px(4.0))
                    .bg(theme.border)
                    .text_xs()
                    .text_color(theme.foreground)
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.8))
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.sign_out(cx);
                    }))
                    .child("Try again"),
            )
    }

    fn render_pr_list(&self, gh_store: &GitHubStore, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let prs = gh_store.pull_requests();

        if prs.is_empty() {
            return div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("No open pull requests"),
                );
        }

        div()
            .debug_selector(|| "pr-list-items".into())
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .children(prs.iter().enumerate().map(|(idx, pr)| {
                let theme = cx.theme();
                let pr_number = pr.number;
                let pr_title = pr.title.clone();
                div()
                    .id(ElementId::Name(format!("pr-item-{}", idx).into()))
                    .debug_selector(|| "pr-item".into())
                    .w_full()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.secondary))
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        this.open_pr_review(pr_number, pr_title.clone(), cx);
                    }))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .flex_1()
                                    .overflow_hidden()
                                    .child(pr.title.clone()),
                            )
                            .when(pr.draft, |el| {
                                el.child(
                                    div()
                                        .px_1()
                                        .py(px(1.0))
                                        .rounded(px(3.0))
                                        .bg(theme.border)
                                        .text_color(theme.muted_foreground)
                                        .text_xs()
                                        .flex_shrink_0()
                                        .child("Draft"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!("#{}", pr.number))
                            .child(format!("by {}", pr.user.login)),
                    )
            }))
    }
}

impl Render for PrList {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        let gh_store = self.github_store(cx);

        let content = if let Some(gh_store_entity) = gh_store {
            let gh_store = gh_store_entity.read(cx);
            match gh_store.auth_state() {
                AuthState::Unauthenticated => self.render_unauthenticated(cx).into_any_element(),
                AuthState::WaitingForUser {
                    user_code,
                    verification_uri,
                } => self
                    .render_waiting_for_user(user_code, verification_uri, cx)
                    .into_any_element(),
                AuthState::CheckingInstallation => {
                    self.render_checking_installation(cx).into_any_element()
                }
                AuthState::NeedsInstallation { install_url } => {
                    let url = install_url.clone();
                    let owner = gh_store.owner().to_string();
                    let repo = gh_store.repo().to_string();
                    self.render_needs_installation(&url, &owner, &repo, cx)
                        .into_any_element()
                }
                AuthState::Authenticated => {
                    self.render_pr_list(gh_store, cx).into_any_element()
                }
                AuthState::NoAccess => {
                    let owner = gh_store.owner().to_string();
                    let repo = gh_store.repo().to_string();
                    self.render_no_access(&owner, &repo, cx).into_any_element()
                }
                AuthState::Error(e) => {
                    let error = e.clone();
                    self.render_error(&error, cx).into_any_element()
                }
            }
        } else {
            self.render_no_github_remote(cx).into_any_element()
        };

        div()
            .id("pr-list")
            .debug_selector(|| "pr-list".into())
            .key_context("PrList")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.sidebar)
            .child(
                div()
                    .h(px(32.0))
                    .w_full()
                    .flex()
                    .items_center()
                    .px_3()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.muted_foreground)
                            .child("PULL REQUESTS"),
                    ),
            )
            .child(content)
    }
}

impl Focusable for PrList {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stores::{RightSidebarPanel, WorkspaceStore};

    fn init_test_stores(cx: &mut gpui::TestAppContext) -> crate::test_helpers::TestFixture {
        let fixture = crate::test_helpers::TestFixture::new(cx);
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::stores::TerminalStore::init(cx);
            crate::stores::WorkspaceStore::init(cx);

            let workspace_store = crate::stores::WorkspaceStore::global(cx);
            workspace_store.update(cx, |store, cx| {
                store.add_workspace("Test".to_string(), fixture.workspace_path(), cx);
            });
        });
        fixture
    }

    #[core::prelude::v1::test]
    fn test_pr_list_toggle() {
        crate::test_helpers::run_gpui_test(|cx| {
            let _fixture = init_test_stores(cx);
            let window_store = cx.new(|cx| WindowStore::new(cx));

            cx.read(|cx| {
                assert_eq!(
                    window_store.read(cx).right_sidebar(),
                    RightSidebarPanel::Hidden
                );
            });

            window_store.update(cx, |store, cx| {
                store.toggle_pr_list(cx);
            });

            cx.read(|cx| {
                assert_eq!(
                    window_store.read(cx).right_sidebar(),
                    RightSidebarPanel::PrList
                );
            });

            window_store.update(cx, |store, cx| {
                store.toggle_pr_list(cx);
            });

            cx.read(|cx| {
                assert_eq!(
                    window_store.read(cx).right_sidebar(),
                    RightSidebarPanel::Hidden
                );
            });
        });
    }

    #[core::prelude::v1::test]
    fn test_pr_list_exclusive_with_other_panels() {
        crate::test_helpers::run_gpui_test(|cx| {
            let _fixture = init_test_stores(cx);
            let window_store = cx.new(|cx| WindowStore::new(cx));

            // Open PrList
            window_store.update(cx, |store, cx| store.toggle_pr_list(cx));
            cx.read(|cx| {
                assert_eq!(
                    window_store.read(cx).right_sidebar(),
                    RightSidebarPanel::PrList
                );
            });

            // Open FileTree replaces PrList
            window_store.update(cx, |store, cx| store.toggle_file_tree(cx));
            cx.read(|cx| {
                assert_eq!(
                    window_store.read(cx).right_sidebar(),
                    RightSidebarPanel::FileTree
                );
            });

            // Open PrList replaces FileTree
            window_store.update(cx, |store, cx| store.toggle_pr_list(cx));
            cx.read(|cx| {
                assert_eq!(
                    window_store.read(cx).right_sidebar(),
                    RightSidebarPanel::PrList
                );
            });

            // Open DiffList replaces PrList
            window_store.update(cx, |store, cx| store.toggle_diff_list(cx));
            cx.read(|cx| {
                assert_eq!(
                    window_store.read(cx).right_sidebar(),
                    RightSidebarPanel::DiffList
                );
            });
        });
    }
}
