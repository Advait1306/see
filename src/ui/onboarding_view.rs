use crate::commands::{OnboardingBack, OnboardingNext};
use crate::kv_store::GlobalKvStore;
use crate::stores::{RightSidebarPanel, WindowStore, WorkspaceStore};
use crate::ui::WindowView;
use gpui::prelude::*;
use gpui::*;
use gpui_component::theme::ActiveTheme;

const TOTAL_STEPS: usize = 4;

#[derive(Clone, Copy, PartialEq)]
enum OnboardingStep {
    Welcome,
    Features,
    WorkspaceSetup,
    Complete,
}

impl OnboardingStep {
    fn from_index(index: usize) -> Self {
        match index {
            0 => Self::Welcome,
            1 => Self::WorkspaceSetup,
            2 => Self::Features,
            _ => Self::Complete,
        }
    }
}

pub enum OnboardingViewEvent {
    Completed,
}

const FEATURE_COUNT: usize = 3;

pub struct OnboardingView {
    current_step: usize,
    feature_index: usize,
    focus_handle: FocusHandle,
    window_view: Option<Entity<WindowView>>,
    window_store: Option<Entity<WindowStore>>,
}

impl OnboardingView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            current_step: 0,
            feature_index: 0,
            focus_handle: cx.focus_handle(),
            window_view: None,
            window_store: None,
        }
    }

    fn current_step_enum(&self) -> OnboardingStep {
        OnboardingStep::from_index(self.current_step)
    }

    fn next_step(&mut self, cx: &mut Context<Self>) {
        // If on features step, cycle through features first
        if self.current_step_enum() == OnboardingStep::Features {
            if self.feature_index < FEATURE_COUNT - 1 {
                self.feature_index += 1;
                self.update_sidebar_for_feature(cx);
                cx.notify();
                return;
            }
        }

        if self.current_step < TOTAL_STEPS - 1 {
            self.current_step += 1;
            self.feature_index = 0;
            // Update sidebar when entering features step
            if self.current_step_enum() == OnboardingStep::Features {
                self.update_sidebar_for_feature(cx);
            }
            cx.notify();
        } else {
            self.complete_onboarding(cx);
        }
    }

    fn prev_step(&mut self, cx: &mut Context<Self>) {
        // If on features step and not on first feature, go back a feature
        if self.current_step_enum() == OnboardingStep::Features && self.feature_index > 0 {
            self.feature_index -= 1;
            self.update_sidebar_for_feature(cx);
            cx.notify();
            return;
        }

        if self.current_step > 0 {
            self.current_step -= 1;
            // When going back to features, start at last feature
            if self.current_step_enum() == OnboardingStep::Features {
                self.feature_index = FEATURE_COUNT - 1;
                self.update_sidebar_for_feature(cx);
            }
            cx.notify();
        }
    }

    fn add_workspace(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
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

                let _ = this.update(cx, |this, cx| {
                    this.next_step(cx);
                });
            }
        })
        .detach();
    }

    fn ensure_window_view(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.window_view.is_none() {
            let window_store = cx.new(|cx| WindowStore::new(cx));
            let window_view = cx.new(|cx| WindowView::new(window_store.clone(), window, cx));
            self.window_store = Some(window_store);
            self.window_view = Some(window_view);
        }
    }

    fn update_sidebar_for_feature(&self, cx: &mut Context<Self>) {
        if let Some(window_store) = &self.window_store {
            let panel = match self.feature_index {
                1 => RightSidebarPanel::FileTree,
                2 => RightSidebarPanel::DiffList,
                _ => RightSidebarPanel::Hidden,
            };
            window_store.update(cx, |store, cx| {
                store.set_right_sidebar(panel, cx);
            });
        }
    }

    fn complete_onboarding(&mut self, cx: &mut Context<Self>) {
        GlobalKvStore::global(cx).set("onboarding_complete", "true");
        cx.emit(OnboardingViewEvent::Completed);
    }

    fn render_progress_indicator(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .absolute()
            .top_4()
            .left_0()
            .right_0()
            .flex()
            .justify_center()
            .gap_2()
            .children((0..TOTAL_STEPS).map(|i| {
                let is_current = i == self.current_step;
                let is_completed = i < self.current_step;

                div()
                    .w_2()
                    .h_2()
                    .rounded_full()
                    .when(is_current, |el| el.bg(theme.accent))
                    .when(is_completed, |el| el.bg(theme.accent.opacity(0.5)))
                    .when(!is_current && !is_completed, |el| {
                        el.bg(theme.foreground.opacity(0.2))
                    })
            }))
    }

    fn render_welcome(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_6()
                    .child(
                        div()
                            .text_3xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.foreground)
                            .child("Welcome to August"),
                    )
                    .child(
                        div()
                            .text_lg()
                            .text_color(theme.foreground.opacity(0.7))
                            .max_w(px(400.0))
                            .text_center()
                            .child("A modern workspace for developers. Let's get you set up."),
                    ),
            )
    }

    fn render_feature_callout(
        &self,
        theme: &gpui_component::theme::Theme,
        title: &str,
        description: &str,
    ) -> Div {
        div()
            .p_4()
            .rounded_lg()
            .bg(theme.background)
            .border_1()
            .border_color(theme.accent)
            .shadow_lg()
            .max_w(px(220.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.foreground)
                            .child(title.to_string()),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.foreground.opacity(0.7))
                            .child(description.to_string()),
                    ),
            )
    }

    fn render_features(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        self.ensure_window_view(window, cx);
        self.update_sidebar_for_feature(cx);

        let theme = cx.theme();
        let window_view = self.window_view.clone();
        let feature_index = self.feature_index;

        div()
            .size_full()
            .relative()
            // Render the actual app UI in the background
            .when_some(window_view, |el, view| el.child(view))
            // Semi-transparent overlay
            .child(
                div()
                    .id("features-overlay")
                    .absolute()
                    .inset_0()
                    .bg(theme.background.opacity(0.3)),
            )
            // Feature progress indicator
            .child(
                div()
                    .absolute()
                    .top(px(50.0))
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .gap_2()
                    .children((0..FEATURE_COUNT).map(|i| {
                        let is_current = i == feature_index;
                        let is_completed = i < feature_index;

                        div()
                            .w(px(6.0))
                            .h(px(6.0))
                            .rounded_full()
                            .when(is_current, |el| el.bg(theme.accent))
                            .when(is_completed, |el| el.bg(theme.accent.opacity(0.5)))
                            .when(!is_current && !is_completed, |el| {
                                el.bg(theme.foreground.opacity(0.2))
                            })
                    })),
            )
            // Workspace sidebar callout (left side) - index 0
            .when(feature_index == 0, |el| {
                el.child(
                    div()
                        .absolute()
                        .left(px(60.0))
                        .top(px(100.0))
                        .child(self.render_feature_callout(
                            theme,
                            "Workspaces",
                            "Switch between projects quickly. Add multiple workspaces to organize your work.",
                        )),
                )
            })
            // File tree callout (right side) - index 1
            .when(feature_index == 1, |el| {
                el.child(
                    div()
                        .absolute()
                        .right(px(280.0))
                        .top(px(100.0))
                        .child(self.render_feature_callout(
                            theme,
                            "File Tree",
                            "Browse and open files in your project. Click to open in the editor.",
                        )),
                )
            })
            // Diff view callout (right side) - index 2
            .when(feature_index == 2, |el| {
                el.child(
                    div()
                        .absolute()
                        .right(px(280.0))
                        .top(px(100.0))
                        .child(self.render_feature_callout(
                            theme,
                            "Diff View",
                            "See your uncommitted changes at a glance. Review modifications before committing.",
                        )),
                )
            })
    }

    fn render_workspace_setup(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_6()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.foreground)
                            .child("Add Your First Workspace"),
                    )
                    .child(
                        div()
                            .text_base()
                            .text_color(theme.foreground.opacity(0.7))
                            .max_w(px(400.0))
                            .text_center()
                            .child(
                                "Select a folder to start working with. You can add more workspaces later.",
                            ),
                    )
                    .child(
                        div()
                            .id("add-workspace-button")
                            .px_6()
                            .py_3()
                            .mt_4()
                            .rounded_lg()
                            .bg(theme.accent)
                            .text_color(theme.accent_foreground)
                            .font_weight(FontWeight::SEMIBOLD)
                            .cursor_pointer()
                            .hover(|el| el.bg(theme.accent.opacity(0.9)))
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.add_workspace(cx);
                            }))
                            .child("Choose Folder"),
                    ),
            )
    }

    fn render_complete(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_6()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.foreground)
                            .child("You're All Set!"),
                    )
                    .child(
                        div()
                            .text_base()
                            .text_color(theme.foreground.opacity(0.7))
                            .max_w(px(400.0))
                            .text_center()
                            .child("Press Enter or click below to start using August."),
                    ),
            )
    }

    fn render_navigation(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let can_go_back = self.current_step > 0;
        let is_workspace_step = self.current_step_enum() == OnboardingStep::WorkspaceSetup;
        let is_final = self.current_step == TOTAL_STEPS - 1;

        div()
            .absolute()
            .bottom_8()
            .left_0()
            .right_0()
            .flex()
            .justify_center()
            .gap_4()
            .when(can_go_back, |el| {
                el.child(
                    div()
                        .id("back-button")
                        .px_4()
                        .py_2()
                        .rounded_lg()
                        .border_1()
                        .border_color(theme.foreground.opacity(0.2))
                        .bg(theme.background)
                        .text_color(theme.foreground.opacity(0.7))
                        .cursor_pointer()
                        .hover(|el| el.bg(theme.foreground.opacity(0.05)))
                        .on_click(cx.listener(|this, _, _window, cx| {
                            this.prev_step(cx);
                        }))
                        .child("Back"),
                )
            })
            .when(!is_workspace_step, |el| {
                let label = if is_final { "Get Started" } else { "Next" };
                el.child(
                    div()
                        .id("next-button")
                        .px_4()
                        .py_2()
                        .rounded_lg()
                        .bg(theme.accent)
                        .text_color(theme.accent_foreground)
                        .cursor_pointer()
                        .hover(|el| el.bg(theme.accent.opacity(0.9)))
                        .on_click(cx.listener(|this, _, _window, cx| {
                            this.next_step(cx);
                        }))
                        .child(label),
                )
            })
    }
}

impl EventEmitter<OnboardingViewEvent> for OnboardingView {}

impl Render for OnboardingView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .id("onboarding-view")
            .key_context("Onboarding")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &OnboardingNext, _window, cx| {
                if this.current_step_enum() != OnboardingStep::WorkspaceSetup {
                    this.next_step(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &OnboardingBack, _window, cx| {
                this.prev_step(cx);
            }))
            .size_full()
            .relative()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(self.render_progress_indicator(cx))
            .child(match self.current_step_enum() {
                OnboardingStep::Welcome => self.render_welcome(cx).into_any_element(),
                OnboardingStep::Features => self.render_features(window, cx).into_any_element(),
                OnboardingStep::WorkspaceSetup => self.render_workspace_setup(cx).into_any_element(),
                OnboardingStep::Complete => self.render_complete(cx).into_any_element(),
            })
            .child(self.render_navigation(cx))
    }
}

impl Focusable for OnboardingView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
