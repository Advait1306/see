use gpui::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Serializable config for a terminal tab
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TerminalTabConfig {
    pub cwd: PathBuf,
}

/// Serializable config for an editor tab
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EditorTabConfig {
    pub path: PathBuf,
}

pub use crate::ui::pr_review::PrReviewTabConfig;

/// Serializable state for a single tab (tagged union for JSON)
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TabConfig {
    Terminal(TerminalTabConfig),
    Editor(EditorTabConfig),
    #[serde(rename = "pr_review")]
    PrReview(PrReviewTabConfig),
}

/// Trait for tab-like views that can be serialized/deserialized
pub trait Tab: Render + Focusable {
    /// Get the display label for this tab
    fn label(&self, cx: &App) -> String;

    /// Serialize this tab's state to a config
    fn to_config(&self, cx: &App) -> TabConfig;
}
