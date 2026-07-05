use std::path::PathBuf;

pub fn default_models_dir() -> Option<PathBuf> {
    sona_core::paths::default_desktop_models_dir().map(PathBuf::from)
}
