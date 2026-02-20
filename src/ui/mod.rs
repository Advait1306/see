pub mod window_view;
pub mod command_menu;
pub mod diff_list;
mod editor;
pub mod file_tree;
pub mod pane;
pub mod pane_group;
pub mod pr_detail_view;
pub mod pr_list;
pub mod settings_view;
mod terminal;
pub mod workspace_sidebar;

pub use window_view::WindowView;
pub use editor::EditorView;
pub use pr_detail_view::PrDetailView;
pub use terminal::TerminalView;
