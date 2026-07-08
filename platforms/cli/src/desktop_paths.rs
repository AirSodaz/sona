use std::path::{Path, PathBuf};

use sona_core::models::paths::ModelsDirStatus;

pub fn default_models_dir() -> Option<PathBuf> {
    sona_runtime_fs::default_desktop_models_dir()
}

pub fn models_dir_status(path: &Path) -> ModelsDirStatus {
    sona_runtime_fs::models_dir_status(path)
}
