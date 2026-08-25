use std::path::{Path, PathBuf};

use crate::models::preset_models::{
    DEFAULT_PUNCTUATION_MODEL_ID, DEFAULT_SILERO_VAD_MODEL_ID, PresetModel, find_preset_model,
};
use crate::runtime::error::RuntimeValidationError;

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

pub fn resolve_model_download(
    model_id: &str,
    models_dir: &Path,
) -> Result<ResolvedModelDownload, RuntimeValidationError> {
    let model = find_preset_model(model_id)
        .ok_or_else(|| {
            RuntimeValidationError::new("model_id", format!("Unknown model id: {model_id}"))
        })?
        .clone();
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
                install_path: install_path.join(&artifact.filename),
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
    let mut components = path.components();
    let is_single_normal_component = matches!(
        (components.next(), components.next()),
        (Some(std::path::Component::Normal(_)), None)
    );
    if filename.trim().is_empty() || !is_single_normal_component {
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
