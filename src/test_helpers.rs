use crate::config::{self, ConfigDirGuard};
use gpui::TestAppContext;
use std::fs;
use std::panic::RefUnwindSafe;
use std::path::PathBuf;
use tempfile::TempDir;

pub fn run_gpui_test<F>(test_fn: F)
where
    F: Fn(&mut gpui::TestAppContext) + RefUnwindSafe,
{
    gpui::run_test(
        1,
        &[],
        0,
        &mut |dispatcher, _seed| {
            let mut cx = gpui::TestAppContext::build(dispatcher.clone(), None);
            test_fn(&mut cx);
            dispatcher.run_until_parked();
            cx.quit();
            dispatcher.run_until_parked();
        },
        None,
    );
}

pub struct TestFixture {
    pub config_dir: TempDir,
    pub workspace_dir: TempDir,
    _config_guard: ConfigDirGuard,
}

impl TestFixture {
    pub fn new(cx: &mut TestAppContext) -> Self {
        let config_dir = TempDir::new().expect("Failed to create temp config dir");
        let workspace_dir = TempDir::new().expect("Failed to create temp workspace dir");

        let guard = config::set_test_config_dir(config_dir.path().to_path_buf());

        cx.update(|cx| {
            crate::syntax::LanguageRegistry::init(cx);
            crate::stores::EditorStore::init(cx);
        });

        Self {
            config_dir,
            workspace_dir,
            _config_guard: guard,
        }
    }

    pub fn create_file(&self, name: &str, content: &str) -> PathBuf {
        let path = self.workspace_dir.path().join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("Failed to create parent directories");
        }
        fs::write(&path, content).expect("Failed to write test file");
        path
    }

    pub fn create_tree(&self, files: &[(&str, &str)]) {
        for (name, content) in files {
            self.create_file(name, content);
        }
    }

    pub fn workspace_path(&self) -> PathBuf {
        self.workspace_dir.path().to_path_buf()
    }
}
