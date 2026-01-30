mod assets;
mod commands;
mod config;
mod constants;
mod file_watcher;
mod kv_store;
mod stores;
mod syntax;
mod terminal;
mod types;
mod ui;

use assets::Assets;
use commands::Quit;
use gpui::*;
use gpui_component::Root;
use kv_store::GlobalKvStore;
use std::borrow::Cow;
use stores::{EditorStore, TerminalStore, WindowStore, WorkspaceStore};
use syntax::LanguageRegistry;
use ui::{OnboardingView, OnboardingViewEvent, WindowView};

enum AppState {
    Onboarding {
        view: Entity<OnboardingView>,
        _subscription: Subscription,
    },
    TransitionToMain,
    Main {
        view: Entity<WindowView>,
    },
}

struct AppRoot {
    state: AppState,
}

impl AppRoot {
    fn new_onboarding(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let onboarding_view = cx.new(|cx| OnboardingView::new(cx));

        let subscription = cx.subscribe(&onboarding_view, |this, _view, event, cx| match event {
            OnboardingViewEvent::Completed => {
                this.state = AppState::TransitionToMain;
                cx.notify();
            }
        });

        window.focus(&onboarding_view.focus_handle(cx));

        Self {
            state: AppState::Onboarding {
                view: onboarding_view,
                _subscription: subscription,
            },
        }
    }

    fn new_main(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let window_view = Self::create_window_view(window, cx);
        window_view.read(cx).focus_active_content(window, cx);

        Self {
            state: AppState::Main { view: window_view },
        }
    }

    fn create_window_view(window: &mut Window, cx: &mut Context<Self>) -> Entity<WindowView> {
        let workspace_store = WorkspaceStore::global(cx);
        if workspace_store.read(cx).is_empty() {
            let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/"));
            workspace_store.update(cx, |store, cx| {
                store.add_workspace("Home".to_string(), home, cx);
            });
        }

        let window_store = cx.new(|cx| WindowStore::new(cx));
        cx.new(|cx| WindowView::new(window_store, window, cx))
    }
}

impl Render for AppRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Handle deferred transition
        if matches!(self.state, AppState::TransitionToMain) {
            let window_view = Self::create_window_view(window, cx);
            window_view.read(cx).focus_active_content(window, cx);
            self.state = AppState::Main { view: window_view };
        }

        match &self.state {
            AppState::Onboarding { view, .. } => view.clone().into_any_element(),
            AppState::Main { view } => view.clone().into_any_element(),
            AppState::TransitionToMain => unreachable!(),
        }
    }
}

fn main() {
    env_logger::init();

    Application::new().with_assets(Assets).run(|cx: &mut App| {
        // Initialize gpui-component
        gpui_component::init(cx);

        // Register keybindings
        commands::register_keybindings(cx);

        // Handle app-level Quit action
        cx.on_action(|_: &Quit, cx| {
            cx.quit();
        });

        // Register Paper Mono font
        let font_data = include_bytes!("../assets/fonts/PaperMono-Regular.ttf");
        cx.text_system()
            .add_fonts(vec![Cow::Borrowed(font_data.as_slice())])
            .expect("Failed to load Paper Mono font");

        GlobalKvStore::init(cx);
        LanguageRegistry::init(cx);
        EditorStore::init(cx);
        TerminalStore::init(cx);
        WorkspaceStore::init(cx);

        // TODO: Remove this override after testing
        let onboarding_complete = false;
        // let onboarding_complete = GlobalKvStore::global(cx)
        //     .get("onboarding_complete")
        //     .is_some_and(|v| v == "true");

        cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some(config::APP_NAME.into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(12.0), px(12.0))),
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1200.0), px(800.0)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| {
                let app_root = if onboarding_complete {
                    cx.new(|cx| AppRoot::new_main(window, cx))
                } else {
                    cx.new(|cx| AppRoot::new_onboarding(window, cx))
                };
                cx.new(|cx| Root::new(app_root, window, cx))
            },
        )
        .unwrap();
    });
}
