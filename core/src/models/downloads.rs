use std::path::{Path, PathBuf};

use crate::models::preset_models::{
    DEFAULT_PUNCTUATION_MODEL_ID, DEFAULT_SILERO_VAD_MODEL_ID, PresetModel, find_preset_model,
};

#[derive(Debug, Clone)]
pub struct ResolvedModelDownload {
    pub model: PresetModel,
    pub models_dir: PathBuf,
    pub download_path: PathBuf,
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
) -> Result<ResolvedModelDownload, String> {
    let model = find_preset_model(model_id)
        .ok_or_else(|| format!("Unknown model id: {model_id}"))?
        .clone();
    let download_path = model.resolve_download_path(models_dir);
    let install_path = model.resolve_install_path(models_dir);

    Ok(ResolvedModelDownload {
        model,
        models_dir: models_dir.to_path_buf(),
        download_path,
        install_path,
    })
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
