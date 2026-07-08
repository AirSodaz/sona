use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::preset_models::{is_preset_model_installed_at, preset_models};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSummary {
    pub id: String,
    pub name: String,
    pub model_type: String,
    pub language: String,
    pub size: String,
    pub modes: Vec<String>,
    pub installed: bool,
    pub install_path: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModelListFilter {
    pub mode: Option<String>,
    pub model_type: Option<String>,
    pub language: Option<String>,
    pub installed_only: bool,
}

#[derive(Debug, Serialize)]
pub struct ModelListEntry {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub model_type: String,
    pub language: String,
    pub size: String,
    pub modes: Vec<String>,
    pub installed: bool,
    pub install_path: String,
}

pub fn list_models(models_dir: &Path) -> Vec<ModelSummary> {
    preset_models()
        .iter()
        .map(|model| {
            let install_path = model.resolve_install_path(models_dir);
            ModelSummary {
                id: model.id.clone(),
                name: model.name.clone(),
                model_type: model.model_type.clone(),
                language: model.language.clone(),
                size: model.size.clone(),
                modes: model.modes.clone().unwrap_or_default(),
                installed: is_preset_model_installed_at(model, models_dir),
                install_path,
            }
        })
        .collect()
}

pub fn select_models(models: Vec<ModelSummary>, filter: &ModelListFilter) -> Vec<ModelSummary> {
    let language_filter = filter.language.as_deref().map(str::to_lowercase);
    models
        .into_iter()
        .filter(|model| {
            filter
                .mode
                .as_deref()
                .map(|mode| model.modes.iter().any(|item| item == mode))
                .unwrap_or(true)
        })
        .filter(|model| {
            filter
                .model_type
                .as_deref()
                .map(|model_type| model.model_type == model_type)
                .unwrap_or(true)
        })
        .filter(|model| {
            language_filter
                .as_deref()
                .map(|language| {
                    model
                        .language
                        .split(',')
                        .any(|item| item.trim().eq_ignore_ascii_case(language))
                })
                .unwrap_or(true)
        })
        .filter(|model| !filter.installed_only || model.installed)
        .collect()
}

impl From<ModelSummary> for ModelListEntry {
    fn from(model: ModelSummary) -> Self {
        Self {
            id: model.id,
            name: model.name,
            model_type: model.model_type,
            language: model.language,
            size: model.size,
            modes: model.modes,
            installed: model.installed,
            install_path: model.install_path.to_string_lossy().to_string(),
        }
    }
}
