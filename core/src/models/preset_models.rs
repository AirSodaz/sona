use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::models::config::ModelFileConfig;

const PRESET_MODELS_JSON: &str = include_str!("preset-models.json");
const DEFAULT_SENSEVOICE_INT8_MODEL_ID: &str =
    "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17";
const DEFAULT_SENSEVOICE_FP32_MODEL_ID: &str = "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17";

pub const DEFAULT_SILERO_VAD_MODEL_ID: &str = "silero-vad";
pub const DEFAULT_PUNCTUATION_MODEL_ID: &str =
    "sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8";

/// Default model rules used when a preset omits explicit requirements.
pub const DEFAULT_MODEL_RULES: ModelRules = ModelRules {
    requires_vad: true,
    requires_punctuation: false,
    timestamp_support_hint: None,
};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TimestampSupportHint {
    Token,
    Segment,
    Unknown,
}

/// Companion-model requirements for a preset model.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelRules {
    pub requires_vad: bool,
    pub requires_punctuation: bool,
    pub timestamp_support_hint: Option<TimestampSupportHint>,
}

/// Shared preset metadata consumed by the GUI, CLI, and future bindings.
#[derive(Debug, Clone, Deserialize)]
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
    pub sha256: Option<String>,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalogSnapshot {
    pub models_dir: String,
    pub models: Vec<ModelCatalogModel>,
    pub sections: Vec<ModelCatalogSection>,
    pub selection_options: ModelCatalogSelectionOptions,
    pub model_path_by_id: HashMap<String, String>,
    pub model_id_by_normalized_path: HashMap<String, String>,
    pub path_match_tokens: Vec<ModelCatalogPathMatchToken>,
    pub dependency_requests_by_model_id: HashMap<String, Vec<ModelDependencyRequest>>,
    pub restore_defaults: ModelCatalogRestoreDefaults,
}

#[derive(Debug, Clone, Serialize)]
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
    pub sha256: Option<String>,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalogSection {
    #[serde(rename = "type")]
    pub section_type: ModelCatalogSectionType,
    pub groups: Vec<ModelCatalogGroup>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalogGroup {
    pub key: String,
    pub models: Vec<ModelCatalogModel>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalogSelectionOptions {
    pub streaming: Vec<ModelSelectionOption>,
    pub batch: Vec<ModelSelectionOption>,
    pub speaker_segmentation: Vec<ModelSelectionOption>,
    pub speaker_embedding: Vec<ModelSelectionOption>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelSelectionOption {
    pub id: String,
    pub label: String,
    pub install_path: String,
    pub is_installed: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalogPathMatchToken {
    pub id: String,
    pub token: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ModelDependencyConfigKey {
    VadModelPath,
    PunctuationModelPath,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelDependencyRequest {
    pub model_id: String,
    pub config_key: ModelDependencyConfigKey,
    pub install_path: String,
    pub is_installed: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalogRestoreDefaults {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streaming_model_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_model_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vad_model_path: Option<String>,
    pub punctuation_model_path: Option<String>,
    pub speaker_segmentation_model_path: Option<String>,
    pub speaker_embedding_model_path: Option<String>,
    pub enable_itn: bool,
    pub batch_vad_enabled: bool,
    pub vad_buffer_size: f64,
    pub max_concurrent: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSelectionPaths {
    pub streaming_model_path: String,
    pub batch_model_path: String,
    pub speaker_segmentation_model_path: String,
    pub speaker_embedding_model_path: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalogSelectedIds {
    pub streaming: Option<String>,
    pub batch: Option<String>,
    pub speaker_segmentation: Option<String>,
    pub speaker_embedding: Option<String>,
}

/// Builds model metadata and settings-page grouping from an injected install-status set.
pub fn build_model_catalog_snapshot_with_installed_ids(
    models_dir: &Path,
    installed_model_ids: &HashSet<String>,
) -> ModelCatalogSnapshot {
    let models = preset_models()
        .iter()
        .map(|model| {
            ModelCatalogModel::from_preset(
                model,
                models_dir,
                installed_model_ids.contains(&model.id),
            )
        })
        .collect::<Vec<_>>();
    let sections = build_catalog_sections(&models);
    let selection_options = build_selection_options(&models);
    let model_path_by_id = build_model_path_by_id(&models);
    let model_id_by_normalized_path = build_model_id_by_normalized_path(&models);
    let path_match_tokens = build_path_match_tokens(&models);
    let dependency_requests_by_model_id = build_dependency_requests_by_model_id(&models);
    let restore_defaults = build_restore_defaults(&models);

    ModelCatalogSnapshot {
        models_dir: path_to_catalog_string(models_dir),
        models,
        sections,
        selection_options,
        model_path_by_id,
        model_id_by_normalized_path,
        path_match_tokens,
        dependency_requests_by_model_id,
        restore_defaults,
    }
}

pub fn resolve_model_catalog_selected_ids(
    snapshot: &ModelCatalogSnapshot,
    paths: &ModelSelectionPaths,
) -> ModelCatalogSelectedIds {
    ModelCatalogSelectedIds {
        streaming: resolve_selected_model_id(
            snapshot,
            &paths.streaming_model_path,
            &snapshot.selection_options.streaming,
        ),
        batch: resolve_selected_model_id(
            snapshot,
            &paths.batch_model_path,
            &snapshot.selection_options.batch,
        ),
        speaker_segmentation: resolve_selected_model_id(
            snapshot,
            &paths.speaker_segmentation_model_path,
            &snapshot.selection_options.speaker_segmentation,
        ),
        speaker_embedding: resolve_selected_model_id(
            snapshot,
            &paths.speaker_embedding_model_path,
            &snapshot.selection_options.speaker_embedding,
        ),
    }
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
                let key = model.group_id.clone().unwrap_or_else(|| model.id.clone());

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

fn build_selection_options(models: &[ModelCatalogModel]) -> ModelCatalogSelectionOptions {
    ModelCatalogSelectionOptions {
        streaming: models
            .iter()
            .filter(|model| model.supports_mode("streaming"))
            .map(ModelSelectionOption::from_catalog_model)
            .collect(),
        batch: models
            .iter()
            .filter(|model| model.supports_mode("batch"))
            .map(ModelSelectionOption::from_catalog_model)
            .collect(),
        speaker_segmentation: models
            .iter()
            .filter(|model| model.model_type == "speaker-segmentation")
            .map(ModelSelectionOption::from_catalog_model)
            .collect(),
        speaker_embedding: models
            .iter()
            .filter(|model| model.model_type == "speaker-embedding")
            .map(ModelSelectionOption::from_catalog_model)
            .collect(),
    }
}

fn build_model_path_by_id(models: &[ModelCatalogModel]) -> HashMap<String, String> {
    models
        .iter()
        .map(|model| (model.id.clone(), model.install_path.clone()))
        .collect()
}

fn build_model_id_by_normalized_path(models: &[ModelCatalogModel]) -> HashMap<String, String> {
    models
        .iter()
        .map(|model| {
            (
                normalize_catalog_path(&model.install_path),
                model.id.clone(),
            )
        })
        .collect()
}

fn build_path_match_tokens(models: &[ModelCatalogModel]) -> Vec<ModelCatalogPathMatchToken> {
    models
        .iter()
        .map(|model| ModelCatalogPathMatchToken {
            id: model.id.clone(),
            token: normalize_catalog_path(model.filename.as_deref().unwrap_or(&model.id)),
        })
        .collect()
}

fn build_dependency_requests_by_model_id(
    models: &[ModelCatalogModel],
) -> HashMap<String, Vec<ModelDependencyRequest>> {
    let models_by_id = models
        .iter()
        .map(|model| (model.id.as_str(), model))
        .collect::<HashMap<_, _>>();
    let mut requests_by_model_id = HashMap::new();

    for model in models.iter().filter(|model| model.has_recognition_mode()) {
        let mut requests = Vec::new();
        if model.rules.requires_vad
            && let Some(request) = build_dependency_request(
                &models_by_id,
                DEFAULT_SILERO_VAD_MODEL_ID,
                ModelDependencyConfigKey::VadModelPath,
            )
        {
            requests.push(request);
        }
        if model.rules.requires_punctuation
            && let Some(request) = build_dependency_request(
                &models_by_id,
                DEFAULT_PUNCTUATION_MODEL_ID,
                ModelDependencyConfigKey::PunctuationModelPath,
            )
        {
            requests.push(request);
        }

        if !requests.is_empty() {
            requests_by_model_id.insert(model.id.clone(), requests);
        }
    }

    requests_by_model_id
}

fn build_dependency_request(
    models_by_id: &HashMap<&str, &ModelCatalogModel>,
    model_id: &str,
    config_key: ModelDependencyConfigKey,
) -> Option<ModelDependencyRequest> {
    let model = models_by_id.get(model_id)?;
    Some(ModelDependencyRequest {
        model_id: model.id.clone(),
        config_key,
        install_path: model.install_path.clone(),
        is_installed: model.is_installed,
    })
}

fn build_restore_defaults(models: &[ModelCatalogModel]) -> ModelCatalogRestoreDefaults {
    let models_by_id = models
        .iter()
        .map(|model| (model.id.as_str(), model))
        .collect::<HashMap<_, _>>();

    let fallback_asr_path = models_by_id
        .get(DEFAULT_SENSEVOICE_INT8_MODEL_ID)
        .filter(|model| model.is_installed)
        .or_else(|| {
            models_by_id
                .get(DEFAULT_SENSEVOICE_FP32_MODEL_ID)
                .filter(|model| model.is_installed)
        })
        .map(|model| model.install_path.clone());
    let vad_model_path = models_by_id
        .get(DEFAULT_SILERO_VAD_MODEL_ID)
        .filter(|model| model.is_installed)
        .map(|model| model.install_path.clone());

    ModelCatalogRestoreDefaults {
        streaming_model_path: fallback_asr_path.clone(),
        batch_model_path: fallback_asr_path,
        vad_model_path,
        punctuation_model_path: Some(String::new()),
        speaker_segmentation_model_path: Some(String::new()),
        speaker_embedding_model_path: Some(String::new()),
        enable_itn: true,
        batch_vad_enabled: true,
        vad_buffer_size: 5.0,
        max_concurrent: 2,
    }
}

fn resolve_selected_model_id(
    snapshot: &ModelCatalogSnapshot,
    model_path: &str,
    options: &[ModelSelectionOption],
) -> Option<String> {
    if model_path.trim().is_empty() {
        return None;
    }

    let normalized_path = normalize_catalog_path(model_path);
    if let Some(model_id) = snapshot.model_id_by_normalized_path.get(&normalized_path)
        && options.iter().any(|option| option.id == *model_id)
    {
        return Some(model_id.clone());
    }

    for option in options {
        if let Some(token) = snapshot
            .path_match_tokens
            .iter()
            .find(|token| token.id == option.id)
            && !token.token.is_empty()
            && normalized_path.contains(&token.token)
        {
            return Some(option.id.clone());
        }
    }

    None
}

fn normalize_catalog_path(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

impl ModelCatalogModel {
    fn from_preset(model: &PresetModel, models_dir: &Path, is_installed: bool) -> Self {
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
            sha256: model.sha256.clone(),
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
            is_installed,
        }
    }

    fn supports_mode(&self, mode: &str) -> bool {
        self.modes
            .as_ref()
            .map(|modes| modes.iter().any(|item| item == mode))
            .unwrap_or(false)
    }

    fn has_recognition_mode(&self) -> bool {
        self.modes
            .as_ref()
            .map(|modes| !modes.is_empty())
            .unwrap_or(false)
    }
}

impl ModelSelectionOption {
    fn from_catalog_model(model: &ModelCatalogModel) -> Self {
        Self {
            id: model.id.clone(),
            label: model_selection_label(model),
            install_path: model.install_path.clone(),
            is_installed: model.is_installed,
        }
    }
}

fn model_selection_label(model: &ModelCatalogModel) -> String {
    match &model.version_label {
        Some(version_label) => format!("{} ({})", model.name, version_label),
        None => model.name.clone(),
    }
}
