//! Terminal view module
//!
//! Provides terminal emulation UI using Alacritty as backend.

mod colors;
mod cursor;
mod element;
mod input;
mod text_batch;
mod view;

pub use view::TerminalView;
