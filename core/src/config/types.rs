use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "specta")]
use specta::Type;

use super::{
    HotwordSetRecord, PolishKeywordSetRecord, PolishPresetRecord, SummaryTemplateRecord,
    TextReplacementRuleRecord, TextReplacementSetRecord,
};
use crate::ports::asr::{AsrEngine, AsrMode};
use crate::transcription::speaker::SpeakerProfile;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
pub enum AppLanguagePreference {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "en")]
    English,
    #[serde(rename = "zh")]
    SimplifiedChinese,
    #[serde(rename = "zh-TW")]
    TraditionalChinese,
    #[serde(rename = "ja")]
    Japanese,
    #[serde(rename = "ko")]
    Korean,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "lowercase")]
pub enum AppTheme {
    Auto,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "lowercase")]
pub enum AppFont {
    System,
    Serif,
    Sans,
    Mono,
    Arial,
    Georgia,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "lowercase")]
pub enum AppLogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "lowercase")]
pub enum ProjectsViewMode {
    List,
    Grid,
    Table,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "lowercase")]
pub enum VoiceTypingMode {
    Hold,
    Toggle,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "lowercase")]
pub enum GpuAcceleration {
    Auto,
    Cpu,
    Cuda,
    Coreml,
    Directml,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AppAsrModelSelection {
    pub engine: AsrEngine,
    pub mode: AsrMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    pub model_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AppAsrSelections {
    pub live: AppAsrModelSelection,
    pub caption: AppAsrModelSelection,
    pub voice_typing: AppAsrModelSelection,
    pub batch: AppAsrModelSelection,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AppAsrProviderConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<BTreeMap<String, specta_typescript::Unknown>>)
    )]
    pub online: Option<BTreeMap<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Unknown>)
    )]
    pub volcengine_doubao: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Unknown>)
    )]
    pub groq_whisper: Option<Value>,
    #[serde(flatten)]
    #[cfg_attr(feature = "specta", specta(skip))]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AppAsrConfig {
    pub selections: AppAsrSelections,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub providers: Option<AppAsrProviderConfig>,
    #[serde(flatten)]
    #[cfg_attr(feature = "specta", specta(skip))]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub config_version: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_language: Option<AppLanguagePreference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<AppTheme>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<AppFont>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimize_to_tray_on_exit: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_check_updates: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_level: Option<AppLogLevel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projects_view_mode: Option<ProjectsViewMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_record_shortcut: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub microphone_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub microphone_boost: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_audio_device_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mute_during_recording: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_microphone_active: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asr: Option<AppAsrConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub streaming_model_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_model_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub punctuation_model_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vad_model_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_segmentation_model_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_embedding_model_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_download_mirror: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock_window: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub always_on_top: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_on_launch: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption_window_width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption_font_size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption_font_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption_background_opacity: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_timeline: Option<bool>,
    #[serde(
        default,
        rename = "enableITN",
        alias = "enableItn",
        skip_serializing_if = "Option::is_none"
    )]
    pub enable_itn: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_vad_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vad_buffer_size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub max_concurrent: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_acceleration: Option<GpuAcceleration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Unknown>)
    )]
    pub llm_settings: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary_template_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary_custom_templates: Option<Vec<SummaryTemplateRecord>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translation_language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polish_keywords: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polish_preset_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polish_custom_presets: Option<Vec<PolishPresetRecord>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polish_context: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polish_scenario: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_polish: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub auto_polish_frequency: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_request_timeout_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_replacement_sets: Option<Vec<TextReplacementSetRecord>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hotword_sets: Option<Vec<HotwordSetRecord>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polish_keyword_sets: Option<Vec<PolishKeywordSetRecord>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_profiles: Option<Vec<SpeakerProfile>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hotwords: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_replacements: Option<Vec<TextReplacementRuleRecord>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice_typing_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice_typing_shortcut: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice_typing_mode: Option<VoiceTypingMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_server_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_server_host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub http_server_port: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_server_api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub http_server_max_concurrent: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub http_server_max_queue_size: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub http_server_max_streaming: Option<i64>,
    #[serde(
        default,
        rename = "httpServerMaxUploadSizeMB",
        alias = "httpServerMaxUploadSizeMb",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub http_server_max_upload_size_mb: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub http_server_job_ttl_minutes: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_server_ip_whitelist: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub history_audio_retention_days: Option<i64>,
    #[serde(flatten)]
    #[cfg_attr(feature = "specta", specta(skip))]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MigrationResult {
    pub config: Value,
    pub migrated: bool,
}
