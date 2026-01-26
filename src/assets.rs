use gpui::{AssetSource, Result, SharedString};
use rust_embed::RustEmbed;
use std::borrow::Cow;

#[derive(RustEmbed)]
#[folder = "assets"]
#[include = "icons/**/*.svg"]
struct LocalAssets;

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        // Try local assets first
        if let Some(file) = LocalAssets::get(path) {
            return Ok(Some(file.data));
        }

        // Fall back to gpui-component-assets
        gpui_component_assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths: Vec<SharedString> = LocalAssets::iter()
            .filter_map(|p| p.starts_with(path).then(|| p.into()))
            .collect();

        // Add paths from gpui-component-assets
        if let Ok(component_paths) = gpui_component_assets::Assets.list(path) {
            for p in component_paths {
                if !paths.contains(&p) {
                    paths.push(p);
                }
            }
        }

        Ok(paths)
    }
}
