use std::path::{Path, PathBuf};

use crate::runtime::error::RuntimeValidationError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelsDirStatus {
    Missing,
    Directory,
    NotDirectory,
}

pub fn status_of(exists: bool, is_dir: bool) -> ModelsDirStatus {
    if !exists {
        ModelsDirStatus::Missing
    } else if is_dir {
        ModelsDirStatus::Directory
    } else {
        ModelsDirStatus::NotDirectory
    }
}

pub fn resolve_models_dir<F>(
    configured: Option<PathBuf>,
    default_models_dir: Option<PathBuf>,
    models_dir_status: F,
) -> Result<PathBuf, RuntimeValidationError>
where
    F: FnOnce(&Path) -> ModelsDirStatus,
{
    let path = configured.or(default_models_dir).ok_or_else(|| {
        RuntimeValidationError::new(
            "models_dir",
            "Unable to infer the models directory. Pass --models-dir explicitly.",
        )
    })?;

    if matches!(models_dir_status(&path), ModelsDirStatus::NotDirectory) {
        return Err(RuntimeValidationError::new(
            "models_dir",
            format!(
                "Models directory '{}' exists but is not a directory.",
                path.display()
            ),
        ));
    }

    Ok(path)
}
