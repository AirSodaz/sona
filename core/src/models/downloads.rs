use serde::{Deserialize, Serialize};
#[cfg(feature = "specta")]
use specta::Type;
use std::path::{Path, PathBuf};

use crate::models::preset_models::{
    DEFAULT_PUNCTUATION_MODEL_ID, DEFAULT_SILERO_VAD_MODEL_ID, PresetModel, find_preset_model,
};
use crate::runtime::error::RuntimeValidationError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct InterruptedDownloadInfo {
    pub model_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub downloaded_bytes: u64,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub total_bytes: Option<u64>,
    pub percentage: Option<u8>,
}

pub fn temporary_download_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".download");
    PathBuf::from(s)
}

pub fn staging_install_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".installing");
    PathBuf::from(s)
}

pub fn install_lock_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".install.lock");
    PathBuf::from(s)
}

pub fn is_file_locked(path: &Path) -> bool {
    use fs3::FileExt;
    if !path.exists() {
        return false;
    }
    match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
    {
        Ok(file) => {
            if file.try_lock_exclusive().is_ok() {
                let _ = file.unlock();
                false
            } else {
                true
            }
        }
        Err(_) => true,
    }
}

pub fn inspect_interrupted_download(
    models_dir: &Path,
    model_id: &str,
) -> Option<InterruptedDownloadInfo> {
    let resolved = resolve_model_download(model_id, models_dir).ok()?;

    let lock_path = install_lock_path(&resolved.install_path);
    if is_file_locked(&lock_path) {
        return None;
    }

    let temp_download = temporary_download_path(&resolved.download_path);
    if is_file_locked(&temp_download) {
        return None;
    }

    let staging_path = staging_install_path(&resolved.install_path);
    if staging_path.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&staging_path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().is_some_and(|ext| ext == "download") && is_file_locked(&p) {
                    return None;
                }
            }
        }
    }

    let mut downloaded_bytes = 0_u64;
    let mut total_bytes = None;

    if resolved.model.is_multi_file() {
        let expected_total = resolved
            .artifacts
            .iter()
            .map(|a| a.size_bytes.unwrap_or(0))
            .sum::<u64>();
        if expected_total > 0 {
            total_bytes = Some(expected_total);
        }

        if staging_path.is_dir() {
            for artifact in &resolved.artifacts {
                let staged_path = staging_path.join(&artifact.filename);
                let artifact_size = artifact.size_bytes.unwrap_or(0);
                if staged_path.is_file() {
                    let len = staged_path.metadata().map(|m| m.len()).unwrap_or(0);
                    if artifact_size > 0 && len == artifact_size {
                        downloaded_bytes = downloaded_bytes.saturating_add(len);
                        continue;
                    }
                }
                let temp_path = temporary_download_path(&staged_path);
                if temp_path.is_file() {
                    let len = temp_path.metadata().map(|m| m.len()).unwrap_or(0);
                    downloaded_bytes = downloaded_bytes.saturating_add(len);
                }
            }
        }
    } else {
        if temp_download.is_file() {
            let len = temp_download.metadata().map(|m| m.len()).unwrap_or(0);
            if len > 0 {
                downloaded_bytes = len;
            }
        }
        if staging_path.exists() {
            if staging_path.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&staging_path) {
                    for entry in entries.flatten() {
                        if let Ok(meta) = entry.metadata() {
                            downloaded_bytes = downloaded_bytes.saturating_add(meta.len());
                        }
                    }
                }
            } else if let Ok(meta) = staging_path.metadata() {
                downloaded_bytes = downloaded_bytes.saturating_add(meta.len());
            }
        }
        total_bytes = resolved.artifacts.first().and_then(|a| a.size_bytes);
    }

    let has_interrupted_artifacts =
        downloaded_bytes > 0 || temp_download.is_file() || staging_path.exists();

    if has_interrupted_artifacts {
        let percentage = total_bytes.map(|t| {
            if t > 0 {
                ((downloaded_bytes as f64 / t as f64) * 100.0)
                    .round()
                    .min(99.0) as u8
            } else {
                0
            }
        });
        Some(InterruptedDownloadInfo {
            model_id: model_id.to_string(),
            downloaded_bytes,
            total_bytes,
            percentage,
        })
    } else {
        None
    }
}

pub fn clean_interrupted_download(models_dir: &Path, model_id: &str) -> std::io::Result<bool> {
    let Ok(resolved) = resolve_model_download(model_id, models_dir) else {
        return Ok(false);
    };

    let lock_path = install_lock_path(&resolved.install_path);
    if is_file_locked(&lock_path) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::ResourceBusy,
            format!("Model download is actively in progress: {model_id}"),
        ));
    }

    let temp_download = temporary_download_path(&resolved.download_path);
    if is_file_locked(&temp_download) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::ResourceBusy,
            format!("Model download file is actively locked: {model_id}"),
        ));
    }

    let staging_path = staging_install_path(&resolved.install_path);
    if staging_path.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&staging_path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().is_some_and(|ext| ext == "download") && is_file_locked(&p) {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::ResourceBusy,
                        format!("Staged download file is actively locked: {}", p.display()),
                    ));
                }
            }
        }
    }

    let mut removed = false;

    match std::fs::symlink_metadata(&staging_path) {
        Ok(meta) => {
            if meta.file_type().is_dir() {
                std::fs::remove_dir_all(&staging_path)?;
            } else {
                std::fs::remove_file(&staging_path)?;
            }
            removed = true;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }

    match std::fs::symlink_metadata(&temp_download) {
        Ok(_) => {
            std::fs::remove_file(&temp_download)?;
            removed = true;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }

    if lock_path.exists() && !is_file_locked(&lock_path) {
        let _ = std::fs::remove_file(&lock_path);
    }

    Ok(removed)
}

#[derive(Debug, Clone)]
pub struct ResolvedModelDownload {
    pub model: PresetModel,
    pub models_dir: PathBuf,
    pub download_path: PathBuf,
    pub install_path: PathBuf,
    pub artifacts: Vec<ResolvedModelArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedModelArtifact {
    pub url: String,
    pub filename: String,
    pub sha256: Option<String>,
    pub size_bytes: Option<u64>,
    pub install_path: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RequiredCompanionModels {
    pub vad_model_id: Option<String>,
    pub punctuation_model_id: Option<String>,
}

impl RequiredCompanionModels {
    pub fn companion_model_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        if let Some(vad_id) = &self.vad_model_id {
            ids.push(vad_id.clone());
        }
        if let Some(punct_id) = &self.punctuation_model_id {
            ids.push(punct_id.clone());
        }
        ids
    }
}

pub fn resolve_model_download(
    model_id: &str,
    models_dir: &Path,
) -> Result<ResolvedModelDownload, RuntimeValidationError> {
    let model = find_preset_model(model_id)
        .cloned()
        .or_else(|| {
            crate::llm::local_models::find_local_llm_model(model_id).map(|m| m.to_preset_model())
        })
        .ok_or_else(|| {
            RuntimeValidationError::new(
                "model_id",
                crate::models::preset_models::format_unknown_model_error(model_id),
            )
        })?;
    let download_path = model.resolve_download_path(models_dir);
    let install_path = model.resolve_install_path(models_dir);
    if model.artifacts.is_empty() {
        return Err(RuntimeValidationError::new(
            "model_id",
            format!("Model '{model_id}' has no download artifacts"),
        ));
    }
    let artifacts = model
        .artifacts
        .iter()
        .map(|artifact| {
            validate_artifact_filename(model_id, &artifact.filename)?;
            Ok(ResolvedModelArtifact {
                url: artifact.url.clone(),
                filename: artifact.filename.clone(),
                sha256: artifact.sha256.clone(),
                size_bytes: artifact.size_bytes,
                install_path: if model.filename.is_some()
                    && !model.is_archive()
                    && !model.is_multi_file()
                {
                    install_path.clone()
                } else {
                    install_path.join(&artifact.filename)
                },
            })
        })
        .collect::<Result<Vec<_>, RuntimeValidationError>>()?;

    Ok(ResolvedModelDownload {
        model,
        models_dir: models_dir.to_path_buf(),
        download_path,
        install_path,
        artifacts,
    })
}

fn validate_artifact_filename(
    model_id: &str,
    filename: &str,
) -> Result<(), RuntimeValidationError> {
    let path = Path::new(filename);
    let is_valid_relative_path = !filename.trim().is_empty()
        && path.is_relative()
        && path
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)));
    if !is_valid_relative_path {
        return Err(RuntimeValidationError::new(
            "model_id",
            format!("Model '{model_id}' has an invalid artifact filename: {filename}"),
        ));
    }
    Ok(())
}

pub fn required_companion_models(model: &PresetModel) -> RequiredCompanionModels {
    let rules = model.resolved_rules();
    RequiredCompanionModels {
        vad_model_id: rules
            .requires_vad
            .then(|| DEFAULT_SILERO_VAD_MODEL_ID.to_string()),
        punctuation_model_id: rules
            .requires_punctuation
            .then(|| DEFAULT_PUNCTUATION_MODEL_ID.to_string()),
    }
}
