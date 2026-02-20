use august::assets::Assets;
use august::commands::{self, Quit, ShowSettings};
use august::config;
use august::stores::{EditorStore, GitHubAccountStore, TerminalStore, WindowStore, WorkspaceStore};
use august::syntax::LanguageRegistry;
use august::ui::WindowView;
use gpui::{
    App, AppContext, Application, Bounds, Menu, MenuItem, TitlebarOptions, WindowBounds,
    WindowOptions, point, px, size,
};
use gpui_component::Root;
use std::borrow::Cow;

fn main() {
    env_logger::init();

    Application::new().with_assets(Assets).run(|cx: &mut App| {
        // Initialize gpui-component
        gpui_component::init(cx);

        // Register keybindings
        commands::register_keybindings(cx);
        august::ui::command_menu::register_command_menu_keybindings(cx);

        // Handle app-level Quit action
        cx.on_action(|_: &Quit, cx| {
            cx.quit();
        });

        // macOS menu bar
        cx.set_menus(vec![Menu {
            name: "August".into(),
            items: vec![
                MenuItem::action("Settings...", ShowSettings),
                MenuItem::separator(),
                MenuItem::action("Quit August", Quit),
            ],
        }]);

        // Register Paper Mono font
        let font_data = include_bytes!("../assets/fonts/PaperMono-Regular.ttf");
        cx.text_system()
            .add_fonts(vec![Cow::Borrowed(font_data.as_slice())])
            .expect("Failed to load Paper Mono font");

        LanguageRegistry::init(cx);
        EditorStore::init(cx);
        TerminalStore::init(cx);
        GitHubAccountStore::init(cx);
        WorkspaceStore::init(cx);

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
                let window_store = cx.new(|cx| WindowStore::new(cx));
                let window_view = cx.new(|cx| WindowView::new(window_store.clone(), window, cx));

                // Set initial focus on the active content
                window_view.read(cx).focus_active_content(window, cx);

                cx.new(|cx| Root::new(window_view, window, cx))
            },
        )
        .unwrap();
    });
}
