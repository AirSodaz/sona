use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_opener::OpenerExt;

pub const STORAGE_BOOTSTRAP_FILE_NAME: &str = "storage_location.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StorageBootstrapConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_data_dir: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_models_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StorageDirectoriesInfo {
    pub data_dir: String,
    pub default_data_dir: String,
    pub is_custom_data_dir: bool,
    pub models_dir: String,
    pub default_models_dir: String,
    pub is_custom_models_dir: bool,
}

pub fn load_bootstrap_config(default_app_local_data_dir: &Path) -> StorageBootstrapConfig {
    let path = default_app_local_data_dir.join(STORAGE_BOOTSTRAP_FILE_NAME);
    if !path.exists() {
        return StorageBootstrapConfig::default();
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(error) => {
            log::warn!(
                "Failed to read storage_location.json at {}: {}",
                path.display(),
                error
            );
            StorageBootstrapConfig::default()
        }
    }
}

pub fn save_bootstrap_config(
    default_app_local_data_dir: &Path,
    config: &StorageBootstrapConfig,
) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(default_app_local_data_dir)?;
    let path = default_app_local_data_dir.join(STORAGE_BOOTSTRAP_FILE_NAME);
    let content = serde_json::to_string_pretty(config)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let temp_path = default_app_local_data_dir.join(format!("{}.tmp", STORAGE_BOOTSTRAP_FILE_NAME));
    std::fs::write(&temp_path, content)?;
    std::fs::rename(&temp_path, &path)?;
    Ok(())
}

pub fn resolve_active_data_dir(default_app_local_data_dir: &Path) -> PathBuf {
    let config = load_bootstrap_config(default_app_local_data_dir);
    if let Some(custom) = config.custom_data_dir.filter(|p| !p.as_os_str().is_empty()) {
        if custom.exists() || std::fs::create_dir_all(&custom).is_ok() {
            return custom;
        }
        log::warn!(
            "Custom data directory '{}' is inaccessible; falling back to default '{}'",
            custom.display(),
            default_app_local_data_dir.display()
        );
    }
    default_app_local_data_dir.to_path_buf()
}

pub fn resolve_active_models_dir(
    default_app_local_data_dir: &Path,
    active_data_dir: &Path,
) -> PathBuf {
    let config = load_bootstrap_config(default_app_local_data_dir);
    if let Some(custom) = config
        .custom_models_dir
        .filter(|p| !p.as_os_str().is_empty())
    {
        if custom.exists() || std::fs::create_dir_all(&custom).is_ok() {
            return custom;
        }
        log::warn!(
            "Custom models directory '{}' is inaccessible; falling back to default",
            custom.display()
        );
    }
    active_data_dir.join("models")
}

pub fn default_app_local_data_dir_for_app<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map_err(|e| format!("Failed to resolve default AppLocalData: {e}"))
}

pub fn resolve_active_data_dir_for_app<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let default_dir = default_app_local_data_dir_for_app(app)?;
    Ok(resolve_active_data_dir(&default_dir))
}

pub fn resolve_active_models_dir_for_app<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<PathBuf, String> {
    let default_dir = default_app_local_data_dir_for_app(app)?;
    let active_data_dir = resolve_active_data_dir(&default_dir);
    Ok(resolve_active_models_dir(&default_dir, &active_data_dir))
}

pub fn get_storage_directories_info<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<StorageDirectoriesInfo, String> {
    let default_data_dir = default_app_local_data_dir_for_app(app)?;
    let active_data_dir = resolve_active_data_dir(&default_data_dir);
    let active_models_dir = resolve_active_models_dir(&default_data_dir, &active_data_dir);
    let default_models_dir = default_data_dir.join("models");

    let config = load_bootstrap_config(&default_data_dir);
    let is_custom_data_dir = config.custom_data_dir.is_some()
        && config.custom_data_dir.as_ref() != Some(&default_data_dir);
    let is_custom_models_dir = config.custom_models_dir.is_some()
        && config.custom_models_dir.as_ref() != Some(&default_models_dir);

    Ok(StorageDirectoriesInfo {
        data_dir: active_data_dir.to_string_lossy().into_owned(),
        default_data_dir: default_data_dir.to_string_lossy().into_owned(),
        is_custom_data_dir,
        models_dir: active_models_dir.to_string_lossy().into_owned(),
        default_models_dir: default_models_dir.to_string_lossy().into_owned(),
        is_custom_models_dir,
    })
}

pub fn validate_target_directory(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty() {
        return Err("Directory path cannot be empty".to_string());
    }
    if path.is_file() {
        return Err(format!("'{}' is a file, not a directory", path.display()));
    }
    std::fs::create_dir_all(path)
        .map_err(|e| format!("Failed to create directory '{}': {}", path.display(), e))?;

    let test_file = path.join(".sona_write_test");
    std::fs::write(&test_file, b"test")
        .map_err(|e| format!("Directory '{}' is not writable: {}", path.display(), e))?;
    let _ = std::fs::remove_file(&test_file);
    Ok(())
}

pub fn copy_directory_contents(src: &Path, dst: &Path) -> Result<(), std::io::Error> {
    if !src.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let entry_path = entry.path();
        let target_path = dst.join(entry.file_name());
        if entry_path.is_dir() {
            copy_directory_contents(&entry_path, &target_path)?;
        } else if entry_path.is_file() {
            std::fs::copy(&entry_path, &target_path)?;
        }
    }
    Ok(())
}

pub fn migrate_data_directory<R: Runtime>(
    app: &AppHandle<R>,
    target_dir_str: String,
    copy_existing: bool,
) -> Result<StorageDirectoriesInfo, String> {
    let target_path = PathBuf::from(target_dir_str.trim());
    validate_target_directory(&target_path)?;

    let default_data_dir = default_app_local_data_dir_for_app(app)?;
    let active_data_dir = resolve_active_data_dir(&default_data_dir);

    if active_data_dir == target_path {
        return get_storage_directories_info(app);
    }

    if copy_existing && active_data_dir.exists() {
        std::fs::create_dir_all(&target_path).map_err(|e| e.to_string())?;

        // Database and config files to copy
        let file_names = [
            "sona.db",
            "sona.db-wal",
            "sona.db-shm",
            "sona-analytics.db",
            "sona-analytics.db-wal",
            "sona-analytics.db-shm",
            "sync.json",
        ];
        for file_name in file_names {
            let src_file = active_data_dir.join(file_name);
            if src_file.exists() && src_file.is_file() {
                let dst_file = target_path.join(file_name);
                std::fs::copy(&src_file, &dst_file)
                    .map_err(|e| format!("Failed to copy {}: {}", src_file.display(), e))?;
            }
        }

        // Subdirectories to copy
        let subdirectories = ["history", "speaker-profiles", "recovery"];
        for dir_name in subdirectories {
            let src_sub = active_data_dir.join(dir_name);
            if src_sub.exists() && src_sub.is_dir() {
                let dst_sub = target_path.join(dir_name);
                copy_directory_contents(&src_sub, &dst_sub).map_err(|e| {
                    format!("Failed to copy directory {}: {}", src_sub.display(), e)
                })?;
            }
        }

        // If models folder was inside active_data_dir and no custom models dir is set, copy models too
        let bootstrap = load_bootstrap_config(&default_data_dir);
        if bootstrap.custom_models_dir.is_none() {
            let src_models = active_data_dir.join("models");
            if src_models.exists() && src_models.is_dir() {
                let dst_models = target_path.join("models");
                copy_directory_contents(&src_models, &dst_models)
                    .map_err(|e| format!("Failed to copy models directory: {}", e))?;
            }
        }
    }

    let mut bootstrap = load_bootstrap_config(&default_data_dir);
    if target_path == default_data_dir {
        bootstrap.custom_data_dir = None;
    } else {
        bootstrap.custom_data_dir = Some(target_path);
    }
    save_bootstrap_config(&default_data_dir, &bootstrap).map_err(|e| e.to_string())?;

    get_storage_directories_info(app)
}

pub fn reset_data_directory<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<StorageDirectoriesInfo, String> {
    let default_data_dir = default_app_local_data_dir_for_app(app)?;
    let mut bootstrap = load_bootstrap_config(&default_data_dir);
    bootstrap.custom_data_dir = None;
    save_bootstrap_config(&default_data_dir, &bootstrap).map_err(|e| e.to_string())?;

    get_storage_directories_info(app)
}

pub fn set_models_directory<R: Runtime>(
    app: &AppHandle<R>,
    target_dir_str: String,
    move_existing: bool,
) -> Result<StorageDirectoriesInfo, String> {
    let target_path = PathBuf::from(target_dir_str.trim());
    validate_target_directory(&target_path)?;

    let default_data_dir = default_app_local_data_dir_for_app(app)?;
    let active_data_dir = resolve_active_data_dir(&default_data_dir);
    let active_models_dir = resolve_active_models_dir(&default_data_dir, &active_data_dir);

    if active_models_dir != target_path && move_existing && active_models_dir.exists() {
        copy_directory_contents(&active_models_dir, &target_path)
            .map_err(|e| format!("Failed to copy model files: {}", e))?;
    }

    let default_models_dir = default_data_dir.join("models");
    let mut bootstrap = load_bootstrap_config(&default_data_dir);
    if target_path == default_models_dir {
        bootstrap.custom_models_dir = None;
    } else {
        bootstrap.custom_models_dir = Some(target_path);
    }
    save_bootstrap_config(&default_data_dir, &bootstrap).map_err(|e| e.to_string())?;

    get_storage_directories_info(app)
}

pub fn reset_models_directory<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<StorageDirectoriesInfo, String> {
    let default_data_dir = default_app_local_data_dir_for_app(app)?;
    let mut bootstrap = load_bootstrap_config(&default_data_dir);
    bootstrap.custom_models_dir = None;
    save_bootstrap_config(&default_data_dir, &bootstrap).map_err(|e| e.to_string())?;

    get_storage_directories_info(app)
}

pub fn open_storage_path<R: Runtime>(app: &AppHandle<R>, path_str: String) -> Result<(), String> {
    let path = PathBuf::from(path_str.trim());
    if !path.exists() {
        let _ = std::fs::create_dir_all(&path);
    }
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn bootstrap_config_load_and_save() {
        let temp = TempDir::new().unwrap();
        let default_dir = temp.path();

        let initial = load_bootstrap_config(default_dir);
        assert_eq!(initial, StorageBootstrapConfig::default());

        let custom_data = default_dir.join("custom_data");
        let custom_models = default_dir.join("custom_models");
        let config = StorageBootstrapConfig {
            custom_data_dir: Some(custom_data.clone()),
            custom_models_dir: Some(custom_models.clone()),
        };

        save_bootstrap_config(default_dir, &config).unwrap();

        let loaded = load_bootstrap_config(default_dir);
        assert_eq!(loaded.custom_data_dir, Some(custom_data.clone()));
        assert_eq!(loaded.custom_models_dir, Some(custom_models.clone()));

        assert_eq!(resolve_active_data_dir(default_dir), custom_data);
        assert_eq!(
            resolve_active_models_dir(default_dir, &custom_data),
            custom_models
        );
    }

    #[test]
    fn fallback_to_default_when_custom_dir_empty() {
        let temp = TempDir::new().unwrap();
        let default_dir = temp.path();

        let config = StorageBootstrapConfig {
            custom_data_dir: Some(PathBuf::from("")),
            custom_models_dir: Some(PathBuf::from("")),
        };
        save_bootstrap_config(default_dir, &config).unwrap();

        assert_eq!(resolve_active_data_dir(default_dir), default_dir);
        assert_eq!(
            resolve_active_models_dir(default_dir, default_dir),
            default_dir.join("models")
        );
    }

    #[test]
    fn validate_target_directory_checks_write_permission() {
        let temp = TempDir::new().unwrap();
        let valid_dir = temp.path().join("valid_sub");
        assert!(validate_target_directory(&valid_dir).is_ok());

        assert!(validate_target_directory(Path::new("")).is_err());

        let file_path = temp.path().join("file.txt");
        std::fs::write(&file_path, b"hello").unwrap();
        assert!(validate_target_directory(&file_path).is_err());
    }

    #[test]
    fn copy_directory_contents_recursively() {
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("src");
        let dst = temp.path().join("dst");

        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("a.txt"), b"aaa").unwrap();
        std::fs::write(src.join("sub").join("b.txt"), b"bbb").unwrap();

        copy_directory_contents(&src, &dst).unwrap();

        assert_eq!(std::fs::read(dst.join("a.txt")).unwrap(), b"aaa");
        assert_eq!(
            std::fs::read(dst.join("sub").join("b.txt")).unwrap(),
            b"bbb"
        );
    }
}
