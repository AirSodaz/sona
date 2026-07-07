use std::path::{Path, PathBuf};

use sona_core::model_paths::{ModelsDirStatus, status_of};

pub fn default_models_dir() -> Option<PathBuf> {
    sona_core::paths::default_desktop_models_dir()
}

pub fn models_dir_status(path: &Path) -> ModelsDirStatus {
    match path.metadata() {
        Ok(metadata) => status_of(true, metadata.is_dir()),
        Err(_) => status_of(false, false),
    }
}
