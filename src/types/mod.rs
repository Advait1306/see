//! Shared types used across multiple modules

pub mod github;
mod selection;
mod tab;

pub use selection::SelectionPhase;
pub use tab::{EditorTabConfig, PrDetailTabConfig, Tab, TabConfig, TerminalTabConfig};
