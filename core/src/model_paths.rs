use std::path::PathBuf;

use crate::paths::default_desktop_models_dir;

pub fn resolve_models_dir(configured: Option<PathBuf>) -> Result<PathBuf, String> {
    let path = if let Some(path) = configured {
        path
    } else {
        default_desktop_models_dir().ok_or_else(|| {
            "Unable to infer the models directory. Pass --models-dir explicitly.".to_string()
        })?
    };

    if std::fs::metadata(&path)
        .map(|metadata| !metadata.is_dir())
        .unwrap_or(false)
    {
        return Err(format!(
            "Models directory '{}' exists but is not a directory.",
            path.display()
        ));
    }

    Ok(path)
}
