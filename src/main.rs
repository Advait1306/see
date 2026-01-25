mod config;
mod constants;
mod editor;
mod file_watcher;
mod terminal;
mod types;
mod ui;
mod workspace;

use gpui::*;
use gpui_component::Root;
use gpui_component_assets::Assets;
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;
use ui::AppView;
use workspace::WorkspaceManager;

fn main() {
    env_logger::init();

    Application::new().with_assets(Assets).run(|cx: &mut App| {
        // Initialize gpui-component
        gpui_component::init(cx);

        // Register Paper Mono font
        let font_data = include_bytes!("../assets/fonts/PaperMono-Regular.ttf");
        cx.text_system()
            .add_fonts(vec![Cow::Borrowed(font_data.as_slice())])
            .expect("Failed to load Paper Mono font");

        // Using gpui-component's default dark theme
        // No custom color overrides needed - the theme provides all colors via cx.theme()

        // Load saved state
        let saved_state = config::load_state();

        let workspace_manager = cx.new(|_| WorkspaceManager::new());

        let app_view_holder: Rc<RefCell<Option<Entity<AppView>>>> = Rc::new(RefCell::new(None));
        let app_view_for_interceptor = app_view_holder.clone();

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
                    let mut app = AppView::new(workspace_manager, cx);
                    app.restore_state(saved_state, cx);
                    app
                });
                *app_view_holder.borrow_mut() = Some(app_view.clone());
                cx.new(|cx| Root::new(app_view, window, cx))
            },
        )
        .unwrap();

        // Global keystroke interceptor for terminal/workspace switching
        let keystroke_subscription = cx.intercept_keystrokes(move |event, _window, cx| {
            let key = &event.keystroke;

            // Handle tab key - GPUI uses it for focus navigation, but we want it to go to terminal
            if key.key == "tab" && !key.modifiers.platform {
                cx.stop_propagation();
                if let Some(app_view) = app_view_for_interceptor.borrow().as_ref() {
                    app_view.update(cx, |app, cx| {
                        let input = if key.modifiers.shift { "\x1b[Z" } else { "\t" };
                        app.send_to_terminal(input, cx);
                    });
                }
                return;
            }

            if key.modifiers.platform {
                // Cmd+Q - quit application (doesn't need app_view)
                if key.key.as_str() == "q" && !key.modifiers.alt {
                    cx.stop_propagation();
                    cx.quit();
                    return;
                }

                if let Some(app_view) = app_view_for_interceptor.borrow().as_ref() {
                    match (key.key.as_str(), key.modifiers.alt) {
                        // Cmd+W - close current terminal tab
                        ("w", false) => {
                            cx.stop_propagation();
                            app_view.update(cx, |app, cx| {
                                app.close_current_terminal(cx);
                            });
                        }
                        // Cmd+Shift+[ and ] (shown as { and }) - switch tabs
                        ("{", false) => {
                            cx.stop_propagation();
                            app_view.update(cx, |app, cx| {
                                app.prev_terminal(cx);
                            });
                        }
                        ("}", false) => {
                            cx.stop_propagation();
                            app_view.update(cx, |app, cx| {
                                app.next_terminal(cx);
                            });
                        }
                        // Cmd+Option+[ and ] - switch workspaces
                        ("[", true) | ("\u{201c}", _) => {
                            cx.stop_propagation();
                            app_view.update(cx, |app, cx| {
                                app.prev_workspace(cx);
                            });
                        }
                        ("]", true) | ("\u{2019}", _) => {
                            cx.stop_propagation();
                            app_view.update(cx, |app, cx| {
                                app.next_workspace(cx);
                            });
                        }
                        _ => {}
                    }

                    // Cmd+B - toggle file tree
                    if key.key.as_str() == "b" && !key.modifiers.shift && !key.modifiers.alt {
                        cx.stop_propagation();
                        app_view.update(cx, |app, cx| {
                            app.toggle_file_tree(cx);
                        });
                    }
                }
            }
        });

        // Store the subscription in AppView to keep it alive
        if let Some(app_view) = app_view_holder.borrow().as_ref() {
            app_view.update(cx, |app, _cx| {
                app.set_keystroke_subscription(keystroke_subscription);
            });
        }
    });
}
