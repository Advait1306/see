use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const APP_NAME: &str = if cfg!(debug_assertions) {
    "August (Dev)"
} else {
    "August"
};

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

pub fn workspaces_path() -> PathBuf {
    config_dir().join("workspaces.json")
}

pub fn ui_state_path() -> PathBuf {
    config_dir().join("ui-state.json")
}

pub fn layouts_dir() -> PathBuf {
    config_dir().join("layouts")
}

pub fn layout_path(workspace_id: &str) -> PathBuf {
    layouts_dir().join(format!("{}.json", workspace_id))
}

pub fn workspaces_dir() -> PathBuf {
    config_dir().join("workspaces")
}

pub fn workspace_file_tree_path(workspace_id: &str) -> PathBuf {
    workspaces_dir().join(workspace_id).join("file-tree.json")
}

pub fn load_json<T: DeserializeOwned + Default>(path: &Path) -> T {
    if !path.exists() {
        return T::default();
    }

    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

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
