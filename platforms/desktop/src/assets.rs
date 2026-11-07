use gpui_kit::{AssetSource, SharedString};
use rust_embed::RustEmbed;
use std::borrow::Cow;

#[derive(RustEmbed)]
#[folder = "assets/"]
struct BrandAssets;

pub(crate) struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> gpui_kit::Result<Option<Cow<'static, [u8]>>> {
        if let Some(file) = BrandAssets::get(path) {
            return Ok(Some(file.data));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> gpui_kit::Result<Vec<SharedString>> {
        let mut paths = gpui_kit::assets::Assets.list(path)?;
        paths.extend(
            BrandAssets::iter().filter_map(|name| name.starts_with(path).then(|| name.into())),
        );
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_assets_include_react_brand_images_and_kit_icons() {
        let assets = AppAssets;
        assert!(assets.load("models/openai.png").unwrap().is_some());
        assert!(assets.load("providers/azure.svg").unwrap().is_some());
        assert!(assets.load("icons/search.svg").unwrap().is_some());
    }
}
