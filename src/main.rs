mod assets;
mod commands;
mod config;
mod constants;
mod file_watcher;
mod stores;
mod terminal;
mod types;
mod ui;

use assets::Assets;
use commands::Quit;
use gpui::*;
use gpui_component::Root;
use std::borrow::Cow;
use stores::{EditorStore, TerminalStore, WindowStore, WorkspaceStore};
use ui::WindowView;

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

        EditorStore::init(cx);
        TerminalStore::init(cx);
        let workspace_store = WorkspaceStore::init(cx);

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
                // Create WindowStore for this window
                let window_store = cx.new(|cx| WindowStore::new(workspace_store.clone(), cx));

                let window_view = cx.new(|cx| {
                    let window_view =
                        WindowView::new(workspace_store.clone(), window_store.clone(), window, cx);

                    // Ensure at least one workspace exists
                    if window_view.workspace_store().read(cx).is_empty() {
                        let home =
                            dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/"));
                        window_view.workspace_store().update(cx, |store, cx| {
                            store.add_workspace("Home".to_string(), home, cx);
                        });
                    }

                    window_view
                });
                cx.new(|cx| Root::new(window_view, window, cx))
            },
        )
        .unwrap();
    });
}
