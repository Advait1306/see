//! Shared types used across multiple modules

mod selection;
mod tab;

pub use selection::SelectionPhase;
pub use tab::{EditorTabConfig, Tab, TabConfig, TerminalTabConfig};
