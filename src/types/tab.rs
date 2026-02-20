use gpui::{App, Focusable, Render};
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

/// Serializable config for a PR detail tab
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PrDetailTabConfig {
    pub owner: String,
    pub repo: String,
    pub number: u64,
    pub title: String,
    pub author_login: String,
    pub head_ref: String,
    pub base_ref: String,
    pub draft: bool,
    pub html_url: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Serializable state for a single tab (tagged union for JSON)
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TabConfig {
    Terminal(TerminalTabConfig),
    Editor(EditorTabConfig),
    #[serde(rename = "pull_request")]
    PullRequest(PrDetailTabConfig),
}

/// Trait for tab-like views that can be serialized/deserialized
pub trait Tab: Render + Focusable {
    /// Get the display label for this tab
    fn label(&self, cx: &App) -> String;

    /// Serialize this tab's state to a config
    fn to_config(&self, cx: &App) -> TabConfig;
}
