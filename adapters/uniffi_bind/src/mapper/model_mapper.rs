use sona_core::models::downloads::{RequiredCompanionModels, ResolvedModelDownload};
use sona_core::models::preset_models::{
    ModelCatalogGroup, ModelCatalogModel, ModelCatalogPathMatchToken, ModelCatalogRestoreDefaults,
    ModelCatalogSection, ModelCatalogSectionType, ModelCatalogSelectedIds,
    ModelCatalogSelectionOptions, ModelCatalogSnapshot, ModelDependencyConfigKey,
    ModelDependencyRequest, ModelRules, ModelSelectionOption, ModelSelectionPaths, PresetModel,
    TimestampSupportHint,
};

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiTimestampSupportHint {
    Token,
    Segment,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiModelRules {
    pub requires_vad: bool,
    pub requires_punctuation: bool,
    pub timestamp_support_hint: Option<FfiTimestampSupportHint>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiPresetModel {
    pub id: String,
    pub name: String,
    pub description: String,
    pub url: String,
    pub model_type: String,
    pub modes: Vec<String>,
    pub language: String,
    pub size: String,
    pub sha256: Option<String>,
    pub is_recommended: bool,
    pub is_archive: bool,
    pub filename: Option<String>,
    pub engine: String,
    pub rules: FfiModelRules,
    pub group_id: Option<String>,
    pub version_label: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiModelCatalogModel {
    pub id: String,
    pub name: String,
    pub description: String,
    pub url: String,
    pub model_type: String,
    pub modes: Vec<String>,
    pub language: String,
    pub size: String,
    pub sha256: Option<String>,
    pub is_recommended: bool,
    pub is_archive: bool,
    pub filename: Option<String>,
    pub engine: String,
    pub rules: FfiModelRules,
    pub group_id: Option<String>,
    pub version_label: Option<String>,
    pub install_path: String,
    pub download_path: String,
    pub is_installed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiModelSelectionOption {
    pub id: String,
    pub label: String,
    pub install_path: String,
    pub is_installed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiModelCatalogSelectionOptions {
    pub streaming: Vec<FfiModelSelectionOption>,
    pub batch: Vec<FfiModelSelectionOption>,
    pub speaker_segmentation: Vec<FfiModelSelectionOption>,
    pub speaker_embedding: Vec<FfiModelSelectionOption>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiModelPathByIdEntry {
    pub id: String,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiModelIdByNormalizedPathEntry {
    pub normalized_path: String,
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiModelCatalogPathMatchToken {
    pub id: String,
    pub token: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiModelCatalogSectionType {
    Asr,
    Punctuation,
    Vad,
    SpeakerSegmentation,
    SpeakerEmbedding,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiModelCatalogGroup {
    pub key: String,
    pub models: Vec<FfiModelCatalogModel>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiModelCatalogSection {
    pub section_type: FfiModelCatalogSectionType,
    pub groups: Vec<FfiModelCatalogGroup>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiModelDependencyConfigKey {
    VadModelPath,
    PunctuationModelPath,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiModelDependencyRequest {
    pub model_id: String,
    pub config_key: FfiModelDependencyConfigKey,
    pub install_path: String,
    pub is_installed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiModelDependencyRequestsForModel {
    pub model_id: String,
    pub requests: Vec<FfiModelDependencyRequest>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiModelCatalogRestoreDefaults {
    pub streaming_model_path: Option<String>,
    pub batch_model_path: Option<String>,
    pub vad_model_path: Option<String>,
    pub punctuation_model_path: Option<String>,
    pub speaker_segmentation_model_path: Option<String>,
    pub speaker_embedding_model_path: Option<String>,
    pub enable_itn: bool,
    pub batch_vad_enabled: bool,
    pub vad_buffer_size: f64,
    pub max_concurrent: u64,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiModelCatalogSnapshot {
    pub models_dir: String,
    pub models: Vec<FfiModelCatalogModel>,
    pub sections: Vec<FfiModelCatalogSection>,
    pub selection_options: FfiModelCatalogSelectionOptions,
    pub model_path_by_id: Vec<FfiModelPathByIdEntry>,
    pub model_id_by_normalized_path: Vec<FfiModelIdByNormalizedPathEntry>,
    pub path_match_tokens: Vec<FfiModelCatalogPathMatchToken>,
    pub dependency_requests_by_model_id: Vec<FfiModelDependencyRequestsForModel>,
    pub restore_defaults: FfiModelCatalogRestoreDefaults,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiModelSelectionPaths {
    pub streaming_model_path: String,
    pub batch_model_path: String,
    pub speaker_segmentation_model_path: String,
    pub speaker_embedding_model_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiModelCatalogSelectedIds {
    pub streaming: Option<String>,
    pub batch: Option<String>,
    pub speaker_segmentation: Option<String>,
    pub speaker_embedding: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiRequiredCompanionModels {
    pub vad_model_id: Option<String>,
    pub punctuation_model_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiResolvedModelDownload {
    pub model: FfiPresetModel,
    pub models_dir: String,
    pub download_path: String,
    pub install_path: String,
    pub required_companions: FfiRequiredCompanionModels,
}

pub fn timestamp_support_hint_to_ffi(hint: TimestampSupportHint) -> FfiTimestampSupportHint {
    match hint {
        TimestampSupportHint::Token => FfiTimestampSupportHint::Token,
        TimestampSupportHint::Segment => FfiTimestampSupportHint::Segment,
        TimestampSupportHint::Unknown => FfiTimestampSupportHint::Unknown,
    }
}

pub fn model_rules_to_ffi(rules: ModelRules) -> FfiModelRules {
    FfiModelRules {
        requires_vad: rules.requires_vad,
        requires_punctuation: rules.requires_punctuation,
        timestamp_support_hint: rules
            .timestamp_support_hint
            .map(timestamp_support_hint_to_ffi),
    }
}

pub fn preset_model_to_ffi(model: &PresetModel) -> FfiPresetModel {
    FfiPresetModel {
        id: model.id.clone(),
        name: model.name.clone(),
        description: model.description.clone(),
        url: model.url.clone(),
        model_type: model.model_type.clone(),
        modes: model.modes.clone().unwrap_or_default(),
        language: model.language.clone(),
        size: model.size.clone(),
        sha256: model.sha256.clone(),
        is_recommended: model.is_recommended.unwrap_or(false),
        is_archive: model.is_archive(),
        filename: model.filename.clone(),
        engine: model
            .engine
            .clone()
            .unwrap_or_else(|| "sherpa-onnx".to_string()),
        rules: model_rules_to_ffi(model.resolved_rules()),
        group_id: model.group_id.clone(),
        version_label: model.version_label.clone(),
    }
}

fn model_selection_option_to_ffi(option: ModelSelectionOption) -> FfiModelSelectionOption {
    FfiModelSelectionOption {
        id: option.id,
        label: option.label,
        install_path: option.install_path,
        is_installed: option.is_installed,
    }
}

fn model_catalog_selection_options_to_ffi(
    options: ModelCatalogSelectionOptions,
) -> FfiModelCatalogSelectionOptions {
    FfiModelCatalogSelectionOptions {
        streaming: options
            .streaming
            .into_iter()
            .map(model_selection_option_to_ffi)
            .collect(),
        batch: options
            .batch
            .into_iter()
            .map(model_selection_option_to_ffi)
            .collect(),
        speaker_segmentation: options
            .speaker_segmentation
            .into_iter()
            .map(model_selection_option_to_ffi)
            .collect(),
        speaker_embedding: options
            .speaker_embedding
            .into_iter()
            .map(model_selection_option_to_ffi)
            .collect(),
    }
}

pub fn model_path_by_id_to_ffi(
    model_path_by_id: std::collections::HashMap<String, String>,
) -> Vec<FfiModelPathByIdEntry> {
    let mut entries = model_path_by_id.into_iter().collect::<Vec<_>>();
    entries.sort_by(|(left, _), (right, _)| left.cmp(right));

    entries
        .into_iter()
        .map(|(id, path)| FfiModelPathByIdEntry { id, path })
        .collect()
}

pub fn model_id_by_normalized_path_to_ffi(
    model_id_by_normalized_path: std::collections::HashMap<String, String>,
) -> Vec<FfiModelIdByNormalizedPathEntry> {
    let mut entries = model_id_by_normalized_path.into_iter().collect::<Vec<_>>();
    entries.sort_by(|(left, _), (right, _)| left.cmp(right));

    entries
        .into_iter()
        .map(|(normalized_path, id)| FfiModelIdByNormalizedPathEntry {
            normalized_path,
            id,
        })
        .collect()
}

pub fn model_catalog_path_match_token_to_ffi(
    token: ModelCatalogPathMatchToken,
) -> FfiModelCatalogPathMatchToken {
    FfiModelCatalogPathMatchToken {
        id: token.id,
        token: token.token,
    }
}

fn model_catalog_section_type_to_ffi(
    section_type: ModelCatalogSectionType,
) -> FfiModelCatalogSectionType {
    match section_type {
        ModelCatalogSectionType::Asr => FfiModelCatalogSectionType::Asr,
        ModelCatalogSectionType::Punctuation => FfiModelCatalogSectionType::Punctuation,
        ModelCatalogSectionType::Vad => FfiModelCatalogSectionType::Vad,
        ModelCatalogSectionType::SpeakerSegmentation => {
            FfiModelCatalogSectionType::SpeakerSegmentation
        }
        ModelCatalogSectionType::SpeakerEmbedding => FfiModelCatalogSectionType::SpeakerEmbedding,
    }
}

fn model_catalog_group_to_ffi(group: ModelCatalogGroup) -> FfiModelCatalogGroup {
    FfiModelCatalogGroup {
        key: group.key,
        models: group
            .models
            .into_iter()
            .map(model_catalog_model_to_ffi)
            .collect(),
    }
}

pub fn model_catalog_section_to_ffi(section: ModelCatalogSection) -> FfiModelCatalogSection {
    FfiModelCatalogSection {
        section_type: model_catalog_section_type_to_ffi(section.section_type),
        groups: section
            .groups
            .into_iter()
            .map(model_catalog_group_to_ffi)
            .collect(),
    }
}

fn model_dependency_config_key_to_ffi(
    config_key: ModelDependencyConfigKey,
) -> FfiModelDependencyConfigKey {
    match config_key {
        ModelDependencyConfigKey::VadModelPath => FfiModelDependencyConfigKey::VadModelPath,
        ModelDependencyConfigKey::PunctuationModelPath => {
            FfiModelDependencyConfigKey::PunctuationModelPath
        }
    }
}

fn model_dependency_request_to_ffi(request: ModelDependencyRequest) -> FfiModelDependencyRequest {
    FfiModelDependencyRequest {
        model_id: request.model_id,
        config_key: model_dependency_config_key_to_ffi(request.config_key),
        install_path: request.install_path,
        is_installed: request.is_installed,
    }
}

pub fn model_dependency_requests_by_model_id_to_ffi(
    requests_by_model_id: std::collections::HashMap<String, Vec<ModelDependencyRequest>>,
) -> Vec<FfiModelDependencyRequestsForModel> {
    let mut entries = requests_by_model_id.into_iter().collect::<Vec<_>>();
    entries.sort_by(|(left, _), (right, _)| left.cmp(right));

    entries
        .into_iter()
        .map(|(model_id, requests)| FfiModelDependencyRequestsForModel {
            model_id,
            requests: requests
                .into_iter()
                .map(model_dependency_request_to_ffi)
                .collect(),
        })
        .collect()
}

fn model_catalog_restore_defaults_to_ffi(
    defaults: ModelCatalogRestoreDefaults,
) -> FfiModelCatalogRestoreDefaults {
    FfiModelCatalogRestoreDefaults {
        streaming_model_path: defaults.streaming_model_path,
        batch_model_path: defaults.batch_model_path,
        vad_model_path: defaults.vad_model_path,
        punctuation_model_path: defaults.punctuation_model_path,
        speaker_segmentation_model_path: defaults.speaker_segmentation_model_path,
        speaker_embedding_model_path: defaults.speaker_embedding_model_path,
        enable_itn: defaults.enable_itn,
        batch_vad_enabled: defaults.batch_vad_enabled,
        vad_buffer_size: defaults.vad_buffer_size,
        max_concurrent: defaults.max_concurrent as u64,
    }
}

pub fn model_catalog_model_to_ffi(model: ModelCatalogModel) -> FfiModelCatalogModel {
    FfiModelCatalogModel {
        id: model.id,
        name: model.name,
        description: model.description,
        url: model.url,
        model_type: model.model_type,
        modes: model.modes.unwrap_or_default(),
        language: model.language,
        size: model.size,
        sha256: model.sha256,
        is_recommended: model.is_recommended.unwrap_or(false),
        is_archive: model.is_archive,
        filename: model.filename,
        engine: model.engine,
        rules: model_rules_to_ffi(model.rules),
        group_id: model.group_id,
        version_label: model.version_label,
        install_path: model.install_path,
        download_path: model.download_path,
        is_installed: model.is_installed,
    }
}

pub fn model_catalog_snapshot_to_ffi(snapshot: ModelCatalogSnapshot) -> FfiModelCatalogSnapshot {
    FfiModelCatalogSnapshot {
        models_dir: snapshot.models_dir,
        models: snapshot
            .models
            .into_iter()
            .map(model_catalog_model_to_ffi)
            .collect(),
        sections: snapshot
            .sections
            .into_iter()
            .map(model_catalog_section_to_ffi)
            .collect(),
        selection_options: model_catalog_selection_options_to_ffi(snapshot.selection_options),
        model_path_by_id: model_path_by_id_to_ffi(snapshot.model_path_by_id),
        model_id_by_normalized_path: model_id_by_normalized_path_to_ffi(
            snapshot.model_id_by_normalized_path,
        ),
        path_match_tokens: snapshot
            .path_match_tokens
            .into_iter()
            .map(model_catalog_path_match_token_to_ffi)
            .collect(),
        dependency_requests_by_model_id: model_dependency_requests_by_model_id_to_ffi(
            snapshot.dependency_requests_by_model_id,
        ),
        restore_defaults: model_catalog_restore_defaults_to_ffi(snapshot.restore_defaults),
    }
}

pub fn model_selection_paths_from_ffi(paths: FfiModelSelectionPaths) -> ModelSelectionPaths {
    ModelSelectionPaths {
        streaming_model_path: paths.streaming_model_path,
        batch_model_path: paths.batch_model_path,
        speaker_segmentation_model_path: paths.speaker_segmentation_model_path,
        speaker_embedding_model_path: paths.speaker_embedding_model_path,
    }
}

pub fn model_catalog_selected_ids_to_ffi(
    selected: ModelCatalogSelectedIds,
) -> FfiModelCatalogSelectedIds {
    FfiModelCatalogSelectedIds {
        streaming: selected.streaming,
        batch: selected.batch,
        speaker_segmentation: selected.speaker_segmentation,
        speaker_embedding: selected.speaker_embedding,
    }
}

pub fn required_companion_models_to_ffi(
    companions: RequiredCompanionModels,
) -> FfiRequiredCompanionModels {
    FfiRequiredCompanionModels {
        vad_model_id: companions.vad_model_id,
        punctuation_model_id: companions.punctuation_model_id,
    }
}

pub fn resolved_model_download_to_ffi(
    resolved: ResolvedModelDownload,
    required_companions: RequiredCompanionModels,
) -> FfiResolvedModelDownload {
    FfiResolvedModelDownload {
        model: preset_model_to_ffi(&resolved.model),
        models_dir: resolved.models_dir.to_string_lossy().into_owned(),
        download_path: resolved.download_path.to_string_lossy().into_owned(),
        install_path: resolved.install_path.to_string_lossy().into_owned(),
        required_companions: required_companion_models_to_ffi(required_companions),
    }
}
