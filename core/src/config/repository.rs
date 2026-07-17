use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "specta")]
use specta::Type;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct SummaryTemplateRecord {
    pub id: String,
    pub name: String,
    pub instructions: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct PolishPresetRecord {
    pub id: String,
    pub name: String,
    pub context: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct TextReplacementRuleRecord {
    pub id: String,
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct TextReplacementSetRecord {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub ignore_case: bool,
    pub rules: Vec<TextReplacementRuleRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct HotwordRuleRecord {
    pub id: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct HotwordSetRecord {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub rules: Vec<HotwordRuleRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct PolishKeywordSetRecord {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub keywords: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct SpeakerProfileSampleRecord {
    pub id: String,
    pub file_path: String,
    pub source_name: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub duration_seconds: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct SpeakerProfileRecord {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub samples: Vec<SpeakerProfileSampleRecord>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AppConfigLibrary {
    pub summary_templates: Vec<SummaryTemplateRecord>,
    pub polish_presets: Vec<PolishPresetRecord>,
    pub text_replacement_sets: Vec<TextReplacementSetRecord>,
    pub hotword_sets: Vec<HotwordSetRecord>,
    pub polish_keyword_sets: Vec<PolishKeywordSetRecord>,
    pub speaker_profiles: Vec<SpeakerProfileRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppConfigStartupProjection {
    pub http_server_enabled: bool,
    pub host: String,
    pub port: i64,
    pub api_key: String,
    pub max_concurrent: i64,
    pub max_queue_size: i64,
    pub max_upload_size_mb: i64,
    pub job_ttl_minutes: i64,
    pub max_streaming: i64,
    pub ip_whitelist: String,
    pub gpu_acceleration: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AppConfigStoredState {
    pub base_config_json: String,
    pub library: AppConfigLibrary,
    pub config_version: i64,
    pub updated_at: i64,
    pub startup_projection: AppConfigStartupProjection,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfigRepositorySnapshot {
    pub config: Value,
    pub config_version: i64,
    pub updated_at: i64,
    pub summary_template_count: u64,
    pub polish_preset_count: u64,
    pub vocabulary_set_count: u64,
    pub speaker_profile_count: u64,
}

pub trait AppConfigStore: Send + Sync {
    fn load_state(&self) -> Result<Option<AppConfigStoredState>, String>;
    fn load_base_config_json(&self) -> Result<Option<String>, String>;
    fn load_startup_projection(&self) -> Result<Option<AppConfigStartupProjection>, String>;
    fn replace_state(&self, state: AppConfigStoredState) -> Result<(), String>;
    fn load_setting_json(&self, key: &str) -> Result<Option<String>, String>;
    fn set_setting_json(&self, key: &str, value_json: String) -> Result<(), String>;
}
