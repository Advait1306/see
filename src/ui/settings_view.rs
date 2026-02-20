use crate::commands::HideSettings;
use crate::stores::github::GitHubAccountStore;
use crate::stores::WindowStore;
use crate::types::github::AuthState;
use gpui::{
    div, px, App, ClipboardItem, Context, Entity, FocusHandle, Focusable, FontWeight,
    InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled,
    Subscription, Window,
};
use gpui::prelude::FluentBuilder;
use gpui_component::theme::ActiveTheme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsSection {
    GitHub,
}

pub struct SettingsView {
    window_store: Entity<WindowStore>,
    github_account_store: Entity<GitHubAccountStore>,
    selected_section: SettingsSection,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl SettingsView {
    pub fn new(window_store: Entity<WindowStore>, cx: &mut Context<Self>) -> Self {
        let github_account_store = GitHubAccountStore::global(cx);

        let account_sub = cx.subscribe(&github_account_store, |_this, _store, _event, cx| {
            cx.notify();
        });

        Self {
            window_store,
            github_account_store,
            selected_section: SettingsSection::GitHub,
            focus_handle: cx.focus_handle(),
            _subscriptions: vec![account_sub],
        }
    }

    fn render_sidebar(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .w(px(200.0))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .bg(theme.sidebar)
            .border_r_1()
            .border_color(theme.border)
            // Back button
            .child(
                div()
                    .id("settings-back")
                    .debug_selector(|| "settings-back".into())
                    .h(px(36.0))
                    .w_full()
                    .flex()
                    .items_center()
                    .px(px(12.0))
                    .gap(px(4.0))
                    .cursor_pointer()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .hover(|s| s.text_color(theme.foreground))
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.window_store.update(cx, |store, cx| {
                            store.hide_settings(cx);
                        });
                    }))
                    .child("\u{2190} Back"),
            )
            // Section list
            .child(
                div()
                    .flex_1()
                    .pt(px(4.0))
                    .child(self.render_section_item(SettingsSection::GitHub, "GitHub", cx)),
            )
    }

    fn render_section_item(
        &self,
        section: SettingsSection,
        label: &str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let is_selected = self.selected_section == section;

        div()
            .id("settings-section-github")
            .h(px(32.0))
            .mx(px(8.0))
            .px(px(8.0))
            .flex()
            .items_center()
            .rounded(px(4.0))
            .text_sm()
            .cursor_pointer()
            .when(is_selected, |el| {
                el.bg(theme.border)
                    .text_color(theme.foreground)
                    .font_weight(FontWeight::MEDIUM)
            })
            .when(!is_selected, |el| {
                el.text_color(theme.muted_foreground)
                    .hover(|s| s.bg(theme.border.opacity(0.5)))
            })
            .child(label.to_string())
    }

    fn render_content(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .p(px(24.0))
            .overflow_y_hidden()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.foreground)
                    .pb(px(16.0))
                    .child("GitHub"),
            )
            .child(self.render_github_content(cx))
    }

    fn render_github_content(&self, cx: &Context<Self>) -> impl IntoElement {
        let auth_state = self.github_account_store.read(cx).auth_state().clone();

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .map(|el| match &auth_state {
                AuthState::SignedIn { username } => {
                    el.child(self.render_signed_in(username, cx))
                }
                AuthState::Authenticating { user_code, .. } => {
                    el.child(self.render_authenticating(user_code, cx))
                }
                AuthState::SignedOut => {
                    el.child(self.render_signed_out(cx, None))
                }
                AuthState::Error(msg) => {
                    el.child(self.render_signed_out(cx, Some(msg.as_str())))
                }
            })
    }

    fn render_signed_in(&self, username: &str, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(
                div()
                    .w_full()
                    .p(px(16.0))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child("Connected as"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.foreground)
                                    .child(username.to_string()),
                            ),
                    )
                    .child(
                        div()
                            .id("disconnect-button")
                            .debug_selector(|| "disconnect-button".into())
                            .px(px(12.0))
                            .py(px(6.0))
                            .rounded(px(6.0))
                            .bg(theme.danger)
                            .text_color(theme.background)
                            .text_sm()
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.9))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.github_account_store.update(cx, |store, cx| {
                                    store.sign_out(cx);
                                });
                            }))
                            .child("Disconnect"),
                    ),
            )
    }

    fn render_authenticating(&self, user_code: &str, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let code = user_code.to_string();
        let code_for_click = code.clone();

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("Enter this code on GitHub:"),
            )
            .child(
                div()
                    .id("settings-device-code")
                    .debug_selector(|| "settings-device-code".into())
                    .w(px(200.0))
                    .px(px(16.0))
                    .py(px(10.0))
                    .rounded(px(8.0))
                    .bg(theme.sidebar)
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
                    .child("Click code to copy. Waiting for authorization..."),
            )
    }

    fn render_signed_out(&self, cx: &Context<Self>, error: Option<&str>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .when_some(error.map(|s| s.to_string()), |el, msg| {
                el.child(
                    div()
                        .text_sm()
                        .text_color(theme.danger)
                        .child(msg),
                )
            })
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("Not connected to GitHub."),
            )
            .child(
                div()
                    .id("connect-button")
                    .debug_selector(|| "connect-button".into())
                    .w_auto()
                    .px(px(16.0))
                    .py(px(8.0))
                    .rounded(px(6.0))
                    .bg(theme.foreground)
                    .text_color(theme.background)
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.9))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.github_account_store.update(cx, |store, cx| {
                            store.sign_in(cx);
                        });
                    }))
                    .child("Connect to GitHub"),
            )
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .id("settings-view")
            .key_context("SettingsView")
            .debug_selector(|| "settings-view".into())
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &HideSettings, _window, cx| {
                this.window_store.update(cx, |store, cx| {
                    store.hide_settings(cx);
                });
            }))
            .size_full()
            .flex()
            .flex_row()
            .bg(theme.background)
            .child(self.render_sidebar(cx))
            .child(self.render_content(cx))
    }
}

impl Focusable for SettingsView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
