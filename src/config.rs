use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const APP_NAME: &str = if cfg!(debug_assertions) {
    "August (Dev)"
} else {
    "August"
};

pub const APP_ID: &str = if cfg!(debug_assertions) {
    "com.august.app.dev"
} else {
    "com.august.app"
};

#[derive(Serialize, Deserialize, Default)]
pub struct AppState {
    pub workspaces: Vec<WorkspaceConfig>,
    pub active_workspace_index: Option<usize>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct WorkspaceConfig {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub terminal_count: usize,
    pub active_terminal_index: usize,
}

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

pub fn config_path() -> PathBuf {
    config_dir().join("state.json")
}

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
