mod terminal;
mod ui;
mod workspace;

use gpui::*;
use std::cell::RefCell;
use std::rc::Rc;
use ui::AppView;
use workspace::WorkspaceManager;

fn main() {
    env_logger::init();

    Application::new().run(|cx: &mut App| {
        let workspace_manager = cx.new(|_| WorkspaceManager::new());

        let app_view_holder: Rc<RefCell<Option<Entity<AppView>>>> = Rc::new(RefCell::new(None));
        let app_view_for_interceptor = app_view_holder.clone();

        cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("August".into()),
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1200.0), px(800.0)),
                    cx,
                ))),
                ..Default::default()
            },
            |_, cx| {
                let app_view = cx.new(|cx| AppView::new(workspace_manager, cx));
                *app_view_holder.borrow_mut() = Some(app_view.clone());
                app_view
            },
        )
        .unwrap();

        // Global keystroke interceptor for terminal/workspace switching
        let keystroke_subscription = cx.intercept_keystrokes(move |event, _window, cx| {
            let key = &event.keystroke;

            // Debug: log keystrokes with platform modifier
            if key.modifiers.platform {
                log::info!(
                    "Key: '{}', alt={}, shift={}",
                    key.key,
                    key.modifiers.alt,
                    key.modifiers.shift
                );
            }

            if key.modifiers.platform {
                if let Some(app_view) = app_view_for_interceptor.borrow().as_ref() {
                    match (key.key.as_str(), key.modifiers.alt) {
                        // Cmd+Shift+[ and ] (shown as { and }) - switch terminals
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
