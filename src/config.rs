use serde::{de::DeserializeOwned, Serialize};
use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};

pub const APP_NAME: &str = if cfg!(debug_assertions) {
    "SEE (Dev)"
} else {
    "SEE"
};

thread_local! {
    static CONFIG_DIR_OVERRIDE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

pub fn config_dir() -> PathBuf {
    CONFIG_DIR_OVERRIDE.with(|cell| {
        if let Some(p) = cell.borrow().as_ref() {
            return p.clone();
        }
        let folder = if cfg!(debug_assertions) {
            "SEE-Dev"
        } else {
            "SEE"
        };
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(folder)
    })
}

#[cfg(test)]
pub fn set_test_config_dir(path: PathBuf) -> ConfigDirGuard {
    CONFIG_DIR_OVERRIDE.with(|cell| {
        *cell.borrow_mut() = Some(path);
    });
    ConfigDirGuard
}

#[cfg(test)]
pub struct ConfigDirGuard;

#[cfg(test)]
impl Drop for ConfigDirGuard {
    fn drop(&mut self) {
        CONFIG_DIR_OVERRIDE.with(|cell| {
            *cell.borrow_mut() = None;
        });
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_json_missing_file_returns_default() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.json");
        let result: Vec<String> = load_json(&path);
        assert!(result.is_empty());
    }

    #[test]
    fn test_save_and_load_json_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.json");
        let data = vec!["hello".to_string(), "world".to_string()];
        save_json(&path, &data);
        let loaded: Vec<String> = load_json(&path);
        assert_eq!(loaded, data);
    }

    #[test]
    fn test_save_json_creates_parent_directories() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("a").join("b").join("c").join("test.json");
        save_json(&path, &42u32);
        let loaded: u32 = load_json(&path);
        assert_eq!(loaded, 42);
    }

    #[test]
    fn test_load_json_invalid_json_returns_default() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("bad.json");
        fs::write(&path, "this is not json!").unwrap();
        let result: Vec<String> = load_json(&path);
        assert!(result.is_empty());
    }

    #[test]
    fn test_config_dir_override() {
        let dir = TempDir::new().unwrap();
        let override_path = dir.path().to_path_buf();
        let _guard = set_test_config_dir(override_path.clone());
        assert_eq!(config_dir(), override_path);
    }

    #[test]
    fn test_path_helpers_use_config_dir() {
        let dir = TempDir::new().unwrap();
        let override_path = dir.path().to_path_buf();
        let _guard = set_test_config_dir(override_path.clone());

        assert_eq!(workspaces_path(), override_path.join("workspaces.json"));
        assert_eq!(ui_state_path(), override_path.join("ui-state.json"));
        assert_eq!(layouts_dir(), override_path.join("layouts"));
        assert_eq!(layout_path("abc"), override_path.join("layouts").join("abc.json"));
        assert_eq!(workspaces_dir(), override_path.join("workspaces"));
    }
}
