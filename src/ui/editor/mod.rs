//! Editor view module
//!
//! Provides text editing UI with syntax highlighting support.

mod diff_mode;
mod element;
mod input;
mod selection;
mod view;

pub use diff_mode::DiffDisplayLine;
pub use view::{EditorView, EditorViewEvent, EditorViewOptions};
