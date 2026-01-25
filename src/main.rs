mod commands;
mod config;
mod constants;
mod editor;
mod file_watcher;
mod terminal;
mod types;
mod ui;
mod workspace;

use commands::Quit;
use editor::BufferStore;
use gpui::*;
use gpui_component::Root;
use gpui_component_assets::Assets;
use std::borrow::Cow;
use ui::AppView;
use workspace::WorkspaceStore;

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

        // Using gpui-component's default dark theme
        // No custom color overrides needed - the theme provides all colors via cx.theme()

        // Initialize global stores
        BufferStore::init(cx);

        // Load saved state (legacy format for now, will be migrated)
        let saved_state = config::load_state();

        // Create workspace store as an entity (needed for event emission)
        let workspace_store = cx.new(|_| WorkspaceStore {
            workspaces: Vec::new(),
            active_workspace_index: None,
        });

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
                let app_view = cx.new(|cx| {
                    let mut app = AppView::new(workspace_store, cx);
                    app.restore_state(saved_state, cx);
                    app
                });
                cx.new(|cx| Root::new(app_view, window, cx))
            },
        )
        .unwrap();
    });
}
