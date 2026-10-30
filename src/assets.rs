use std::borrow::Cow;

use gpui_kit::{AssetSource, SharedString};
use rust_embed::RustEmbed;

/// Application assets (file-type icons and the bundled web frontend).
///
/// GPUI Kit owns the icon set used by the component library. This source keeps
/// the application's own assets and delegates every other lookup to the kit, so
/// the application never has to know about the individual icon bundles.
#[derive(RustEmbed)]
#[folder = "assets"]
#[include = "icons/mimes/**/*"]
#[include = "web/**/*"]
struct ApplicationAssets;

pub struct Assets;

impl Assets {
    /// Load an asset embedded by the application itself.
    pub fn load_embedded(path: &str) -> Option<Cow<'static, [u8]>> {
        ApplicationAssets::get(path).map(|file| file.data)
    }
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if let Some(data) = Self::load_embedded(path) {
            return Ok(Some(data));
        }

        // GPUI Kit reports a missing asset as an error. Asset sources are
        // allowed to report "not found" with `None`, which keeps a missing
        // icon from failing the whole load.
        Ok(gpui_kit::assets::Assets.load(path).ok().flatten())
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        let mut paths: Vec<SharedString> = ApplicationAssets::iter()
            .filter(|name| name.starts_with(path))
            .map(Into::into)
            .collect();

        if let Ok(mut kit_paths) = gpui_kit::assets::Assets.list(path) {
            paths.append(&mut kit_paths);
        }

        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}
