use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const APP_NAME: &str = if cfg!(debug_assertions) {
    "August (Dev)"
} else {
    "August"
};

/// Get the config directory for the app
pub fn config_dir() -> PathBuf {
    let folder = if cfg!(debug_assertions) {
        "August-Dev"
    } else {
        "August"
    };
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(folder)
}

/// Path to workspaces.json
pub fn workspaces_path() -> PathBuf {
    config_dir().join("workspaces.json")
}

/// Path to file-tree-state.json
pub fn file_tree_state_path() -> PathBuf {
    config_dir().join("file-tree-state.json")
}

/// Path to ui-state.json
pub fn ui_state_path() -> PathBuf {
    config_dir().join("ui-state.json")
}

/// Path to layouts directory
pub fn layouts_dir() -> PathBuf {
    config_dir().join("layouts")
}

/// Path to a specific workspace layout file
pub fn layout_path(workspace_id: &str) -> PathBuf {
    layouts_dir().join(format!("{}.json", workspace_id))
}

/// Generic JSON load helper
pub fn load_json<T: DeserializeOwned + Default>(path: &Path) -> T {
    if !path.exists() {
        return T::default();
    }

    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Generic JSON save helper
pub fn save_json<T: Serialize>(path: &Path, data: &T) {
    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            log::error!("Failed to create config directory: {}", e);
            return;
        }
    }

    match serde_json::to_string_pretty(data) {
        Ok(json) => {
            if let Err(e) = fs::write(path, json) {
                log::error!("Failed to save state to {:?}: {}", path, e);
            }
        }
        Err(e) => {
            log::error!("Failed to serialize state: {}", e);
        }
    }
}

// =============================================================================
// Legacy types for backwards compatibility during migration
// These will be removed after full migration to new config structure
// =============================================================================

#[derive(Serialize, Deserialize, Default)]
pub struct AppState {
    pub workspaces: Vec<WorkspaceConfig>,
    pub active_workspace_index: Option<usize>,
    #[serde(default)]
    pub file_tree_visible: bool,
}

use std::collections::HashSet;

#[derive(Serialize, Deserialize, Clone)]
pub struct WorkspaceConfig {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    #[serde(default = "default_layout")]
    pub layout: MemberConfig,
    #[serde(default)]
    pub expanded_paths: HashSet<PathBuf>,
}

fn default_layout() -> MemberConfig {
    MemberConfig::Pane {
        terminal_count: 1,
        active_index: 0,
        open_files: Vec::new(),
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum MemberConfig {
    Pane {
        terminal_count: usize,
        active_index: usize,
        #[serde(default)]
        open_files: Vec<PathBuf>,
    },
    Axis {
        axis: Axis,
        ratios: Vec<f32>,
        members: Vec<MemberConfig>,
    },
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

/// Legacy config path (used during migration)
pub fn config_path() -> PathBuf {
    config_dir().join("state.json")
}

/// Load legacy state (for migration)
pub fn load_state() -> AppState {
    let path = config_path();
    if path.exists() {
        match fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => AppState::default(),
        }
    } else {
        AppState::default()
    }
}

/// Save legacy state (for backwards compatibility during migration)
pub fn save_state(state: &AppState) {
    let dir = config_dir();
    if let Err(e) = fs::create_dir_all(&dir) {
        log::error!("Failed to create config directory: {}", e);
        return;
    }

    let path = config_path();
    match serde_json::to_string_pretty(state) {
        Ok(json) => {
            if let Err(e) = fs::write(&path, json) {
                log::error!("Failed to save state: {}", e);
            }
        }
        Err(e) => {
            log::error!("Failed to serialize state: {}", e);
        }
    }
}
