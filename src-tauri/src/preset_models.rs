use crate::sherpa::ModelFileConfig;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tauri::Manager;

const PRESET_MODELS_JSON: &str = include_str!("../../src/shared/preset-models.json");

/// Default model rules used when a preset omits explicit requirements.
pub const DEFAULT_MODEL_RULES: ModelRules = ModelRules {
    requires_vad: true,
    requires_punctuation: false,
    timestamp_support_hint: None,
};

#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TimestampSupportHint {
    Token,
    Segment,
    Unknown,
}

/// Companion-model requirements for a preset model.
#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelRules {
    pub requires_vad: bool,
    pub requires_punctuation: bool,
    pub timestamp_support_hint: Option<TimestampSupportHint>,
}

/// Shared preset metadata consumed by both the GUI and the CLI.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetModel {
    pub id: String,
    pub name: String,
    pub description: String,
    pub url: String,
    #[serde(rename = "type")]
    pub model_type: String,
    pub modes: Option<Vec<String>>,
    pub language: String,
    pub size: String,
    pub is_recommended: Option<bool>,
    pub is_archive: Option<bool>,
    pub filename: Option<String>,
    pub engine: Option<String>,
    pub rules: Option<ModelRules>,
    pub file_config: Option<ModelFileConfig>,
    pub group_id: Option<String>,
    pub version_label: Option<String>,
}

static PRESET_MODELS: OnceLock<Vec<PresetModel>> = OnceLock::new();

/// Returns the parsed shared preset metadata.
pub fn preset_models() -> &'static [PresetModel] {
    PRESET_MODELS
        .get_or_init(|| {
            serde_json::from_str(PRESET_MODELS_JSON)
                .expect("shared preset models JSON should be valid")
        })
        .as_slice()
}

/// Finds a shared preset model by its stable identifier.
pub fn find_preset_model(model_id: &str) -> Option<&'static PresetModel> {
    preset_models().iter().find(|model| model.id == model_id)
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalogSnapshot {
    pub models_dir: String,
    pub models: Vec<ModelCatalogModel>,
    pub sections: Vec<ModelCatalogSection>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalogModel {
    pub id: String,
    pub name: String,
    pub description: String,
    pub url: String,
    #[serde(rename = "type")]
    pub model_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modes: Option<Vec<String>>,
    pub language: String,
    pub size: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_recommended: Option<bool>,
    pub is_archive: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    pub engine: String,
    pub rules: ModelRules,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_label: Option<String>,
    pub install_path: String,
    pub download_path: String,
    pub is_installed: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalogSection {
    #[serde(rename = "type")]
    pub section_type: ModelCatalogSectionType,
    pub groups: Vec<ModelCatalogGroup>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalogGroup {
    pub key: String,
    pub models: Vec<ModelCatalogModel>,
}

#[derive(Debug, Clone, Copy, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ModelCatalogSectionType {
    Asr,
    Punctuation,
    Vad,
    SpeakerSegmentation,
    SpeakerEmbedding,
}

const MODEL_CATALOG_SECTION_TYPES: [ModelCatalogSectionType; 5] = [
    ModelCatalogSectionType::Asr,
    ModelCatalogSectionType::Punctuation,
    ModelCatalogSectionType::Vad,
    ModelCatalogSectionType::SpeakerSegmentation,
    ModelCatalogSectionType::SpeakerEmbedding,
];

/// Returns a settings-page-ready catalog snapshot for the app-local models dir.
#[tauri::command]
pub fn get_model_catalog_snapshot<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<ModelCatalogSnapshot, String> {
    let models_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())?
        .join("models");

    std::fs::create_dir_all(&models_dir).map_err(|error| {
        format!(
            "Failed to create models directory {}: {error}",
            models_dir.display()
        )
    })?;

    Ok(build_model_catalog_snapshot(&models_dir))
}

/// Builds model metadata, install paths, install status, and settings groups.
pub fn build_model_catalog_snapshot(models_dir: &Path) -> ModelCatalogSnapshot {
    let models = preset_models()
        .iter()
        .map(|model| ModelCatalogModel::from_preset(model, models_dir))
        .collect::<Vec<_>>();
    let sections = build_catalog_sections(&models);

    ModelCatalogSnapshot {
        models_dir: path_to_catalog_string(models_dir),
        models,
        sections,
    }
}

fn build_catalog_sections(models: &[ModelCatalogModel]) -> Vec<ModelCatalogSection> {
    MODEL_CATALOG_SECTION_TYPES
        .iter()
        .map(|section_type| {
            let mut group_indexes: HashMap<String, usize> = HashMap::new();
            let mut groups: Vec<ModelCatalogGroup> = Vec::new();

            for model in models
                .iter()
                .filter(|model| model_section_type(model) == *section_type)
            {
                let key = model
                    .group_id
                    .clone()
                    .unwrap_or_else(|| model.id.clone());

                if let Some(index) = group_indexes.get(&key).copied() {
                    groups[index].models.push(model.clone());
                } else {
                    group_indexes.insert(key.clone(), groups.len());
                    groups.push(ModelCatalogGroup {
                        key,
                        models: vec![model.clone()],
                    });
                }
            }

            ModelCatalogSection {
                section_type: *section_type,
                groups,
            }
        })
        .collect()
}

fn model_section_type(model: &ModelCatalogModel) -> ModelCatalogSectionType {
    match model.model_type.as_str() {
        "punctuation" => ModelCatalogSectionType::Punctuation,
        "vad" => ModelCatalogSectionType::Vad,
        "speaker-segmentation" => ModelCatalogSectionType::SpeakerSegmentation,
        "speaker-embedding" => ModelCatalogSectionType::SpeakerEmbedding,
        _ => ModelCatalogSectionType::Asr,
    }
}

fn path_to_catalog_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

impl PresetModel {
    /// Resolves the installed model path under the given models directory.
    pub fn resolve_install_path(&self, models_dir: &Path) -> PathBuf {
        match &self.filename {
            Some(filename) => models_dir.join(filename),
            None => models_dir.join(&self.id),
        }
    }

    /// Returns the expected download filename under the given models directory.
    pub fn resolve_download_path(&self, models_dir: &Path) -> PathBuf {
        if self.is_archive() {
            models_dir.join(format!("{}.tar.bz2", self.id))
        } else {
            self.resolve_install_path(models_dir)
        }
    }

    /// Returns true when the preset ships as an archive.
    pub fn is_archive(&self) -> bool {
        self.is_archive.unwrap_or(true)
    }

    /// Returns the effective rules for this preset model.
    pub fn resolved_rules(&self) -> ModelRules {
        self.rules.unwrap_or(DEFAULT_MODEL_RULES)
    }

    /// Returns true when the preset supports the requested mode.
    pub fn supports_mode(&self, mode: &str) -> bool {
        self.modes
            .as_ref()
            .map(|modes| modes.iter().any(|item| item == mode))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn loads_shared_preset_models() {
        assert!(!preset_models().is_empty());
        assert!(find_preset_model("silero-vad").is_some());
    }

    #[test]
    fn resolves_directory_models_by_id() {
        let model = find_preset_model("sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25").unwrap();
        let path = model.resolve_install_path(Path::new("C:/models"));
        assert_eq!(
            path,
            PathBuf::from("C:/models/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25")
        );
    }

    #[test]
    fn resolves_file_models_by_filename() {
        let model = find_preset_model("silero-vad").unwrap();
        let path = model.resolve_install_path(Path::new("C:/models"));
        assert_eq!(path, PathBuf::from("C:/models/silero_vad.onnx"));
    }

    #[test]
    fn resolves_archive_download_path_by_id() {
        let model = find_preset_model("sherpa-onnx-whisper-turbo").unwrap();
        let path = model.resolve_download_path(Path::new("C:/models"));
        assert_eq!(
            path,
            PathBuf::from("C:/models/sherpa-onnx-whisper-turbo.tar.bz2")
        );
    }

    #[test]
    fn resolves_file_download_path_by_filename() {
        let model = find_preset_model("silero-vad").unwrap();
        let path = model.resolve_download_path(Path::new("C:/models"));
        assert_eq!(path, PathBuf::from("C:/models/silero_vad.onnx"));
    }

    #[test]
    fn falls_back_to_default_rules() {
        let model = find_preset_model("silero-vad").unwrap();
        assert_eq!(model.resolved_rules(), DEFAULT_MODEL_RULES);
    }

    #[test]
    fn builds_settings_ready_catalog_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let models_dir = dir.path().join("models");
        fs::create_dir_all(models_dir.join("sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17")).unwrap();
        fs::write(models_dir.join("silero_vad.onnx"), "").unwrap();

        let snapshot = build_model_catalog_snapshot(&models_dir);

        assert_eq!(snapshot.models_dir, models_dir.to_string_lossy().to_string());

        let silero = snapshot
            .models
            .iter()
            .find(|model| model.id == "silero-vad")
            .unwrap();
        assert!(silero.is_installed);
        assert!(silero.install_path.ends_with("silero_vad.onnx"));

        let asr_section = snapshot
            .sections
            .iter()
            .find(|section| section.section_type == ModelCatalogSectionType::Asr)
            .unwrap();
        let sensevoice_group = asr_section
            .groups
            .iter()
            .find(|group| group.key == "sensevoice")
            .unwrap();
        assert_eq!(
            sensevoice_group
                .models
                .iter()
                .map(|model| model.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17",
                "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17",
            ]
        );
    }
}

impl ModelCatalogModel {
    fn from_preset(model: &PresetModel, models_dir: &Path) -> Self {
        let install_path = model.resolve_install_path(models_dir);
        let download_path = model.resolve_download_path(models_dir);

        Self {
            id: model.id.clone(),
            name: model.name.clone(),
            description: model.description.clone(),
            url: model.url.clone(),
            model_type: model.model_type.clone(),
            modes: model.modes.clone(),
            language: model.language.clone(),
            size: model.size.clone(),
            is_recommended: model.is_recommended,
            is_archive: model.is_archive(),
            filename: model.filename.clone(),
            engine: model
                .engine
                .clone()
                .unwrap_or_else(|| "sherpa-onnx".to_string()),
            rules: model.resolved_rules(),
            group_id: model.group_id.clone(),
            version_label: model.version_label.clone(),
            install_path: path_to_catalog_string(&install_path),
            download_path: path_to_catalog_string(&download_path),
            is_installed: install_path.exists(),
        }
    }
}
