use serde::{Deserialize, Serialize};

use super::AutomationError;

#[cfg(feature = "specta")]
use specta::Type;

fn default_preset_id() -> String {
    "custom".to_string()
}

fn default_polish_preset_id() -> String {
    "general".to_string()
}

fn default_translation_language() -> String {
    "en".to_string()
}

fn default_summary_template_id() -> String {
    "general".to_string()
}

fn default_export_format() -> String {
    "txt".to_string()
}

fn default_export_mode() -> String {
    "original".to_string()
}

fn default_processed_status() -> String {
    "complete".to_string()
}

fn default_processed_kind() -> String {
    "file".to_string()
}

fn default_processed_attempt() -> i64 {
    1
}

fn default_save_history() -> bool {
    true
}

fn default_rule_kind() -> String {
    "file".to_string()
}

fn default_profile_source() -> String {
    "tag_match".to_string()
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationProfileInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_translation_language")]
    pub translation_language: String,
    #[serde(default = "default_polish_preset_id")]
    pub polish_preset_id: String,
    #[serde(default = "default_summary_template_id")]
    pub summary_template_id: String,
    #[serde(default)]
    pub enabled_text_replacement_set_ids: Vec<String>,
    #[serde(default)]
    pub enabled_hotword_set_ids: Vec<String>,
    #[serde(default)]
    pub enabled_polish_keyword_set_ids: Vec<String>,
    #[serde(default)]
    pub enabled_speaker_profile_ids: Vec<String>,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: i64,
}

impl Default for AutomationProfileInput {
    fn default() -> Self {
        Self {
            id: None,
            name: String::new(),
            translation_language: default_translation_language(),
            polish_preset_id: default_polish_preset_id(),
            summary_template_id: default_summary_template_id(),
            enabled_text_replacement_set_ids: Vec::new(),
            enabled_hotword_set_ids: Vec::new(),
            enabled_polish_keyword_set_ids: Vec::new(),
            enabled_speaker_profile_ids: Vec::new(),
            created_at: 0,
            updated_at: 0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationProfileRecord {
    pub id: String,
    pub name: String,
    pub translation_language: String,
    pub polish_preset_id: String,
    pub summary_template_id: String,
    pub enabled_text_replacement_set_ids: Vec<String>,
    pub enabled_hotword_set_ids: Vec<String>,
    pub enabled_polish_keyword_set_ids: Vec<String>,
    pub enabled_speaker_profile_ids: Vec<String>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuleInputActions {
    #[serde(default)]
    pub auto_polish: bool,
    #[serde(default)]
    pub auto_translate: bool,
    #[serde(default)]
    pub auto_summary: bool,
}

impl Default for AutomationRuleInputActions {
    fn default() -> Self {
        Self {
            auto_polish: false,
            auto_translate: false,
            auto_summary: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuleInputStageConfig {
    #[serde(default)]
    pub auto_polish: bool,
    #[serde(default = "default_polish_preset_id")]
    pub polish_preset_id: String,
    #[serde(default)]
    pub auto_translate: bool,
    #[serde(default = "default_translation_language")]
    pub translation_language: String,
    #[serde(default)]
    pub export_enabled: bool,
}

impl Default for AutomationRuleInputStageConfig {
    fn default() -> Self {
        Self {
            auto_polish: false,
            polish_preset_id: default_polish_preset_id(),
            auto_translate: false,
            translation_language: default_translation_language(),
            export_enabled: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuleInputExportConfig {
    #[serde(default)]
    pub directory: String,
    #[serde(default = "default_export_format")]
    pub format: String,
    #[serde(default = "default_export_mode")]
    pub mode: String,
    #[serde(default)]
    pub prefix: String,
}

impl Default for AutomationRuleInputExportConfig {
    fn default() -> Self {
        Self {
            directory: String::new(),
            format: default_export_format(),
            mode: default_export_mode(),
            prefix: String::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuleInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_rule_kind")]
    pub kind: String,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub priority: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    #[serde(default = "default_profile_source")]
    pub profile_source: String,
    #[serde(default = "default_save_history")]
    pub save_history: bool,
    #[serde(default)]
    pub tag_ids: Vec<String>,
    #[serde(default = "default_preset_id")]
    pub preset_id: String,
    #[serde(default)]
    pub watch_directory: String,
    #[serde(default)]
    pub recursive: bool,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub actions: AutomationRuleInputActions,
    #[serde(default)]
    pub stage_config: AutomationRuleInputStageConfig,
    #[serde(default)]
    pub export_config: AutomationRuleInputExportConfig,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration_notice: Option<String>,
}

impl Default for AutomationRuleInput {
    fn default() -> Self {
        Self {
            id: None,
            name: String::new(),
            kind: default_rule_kind(),
            priority: 0,
            profile_id: None,
            profile_source: default_profile_source(),
            save_history: true,
            tag_ids: Vec::new(),
            preset_id: default_preset_id(),
            watch_directory: String::new(),
            recursive: false,
            enabled: false,
            actions: AutomationRuleInputActions::default(),
            stage_config: AutomationRuleInputStageConfig::default(),
            export_config: AutomationRuleInputExportConfig::default(),
            created_at: 0,
            updated_at: 0,
            migration_notice: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationProcessedInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default)]
    pub rule_id: String,
    #[serde(default = "default_processed_kind")]
    pub kind: String,
    #[serde(default)]
    pub input_version: String,
    #[serde(default = "default_processed_attempt")]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub attempt: i64,
    #[serde(default)]
    pub file_path: String,
    #[serde(default)]
    pub source_fingerprint: String,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub size: i64,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub mtime_ms: i64,
    #[serde(default = "default_processed_status")]
    pub status: String,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub processed_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

impl Default for AutomationProcessedInput {
    fn default() -> Self {
        Self {
            id: None,
            rule_id: String::new(),
            kind: default_processed_kind(),
            input_version: String::new(),
            attempt: default_processed_attempt(),
            file_path: String::new(),
            source_fingerprint: String::new(),
            size: 0,
            mtime_ms: 0,
            status: default_processed_status(),
            processed_at: 0,
            history_id: None,
            export_path: None,
            error_message: None,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRepositoryInput {
    #[serde(default)]
    pub profiles: Vec<AutomationProfileInput>,
    #[serde(default)]
    pub rules: Vec<AutomationRuleInput>,
    #[serde(default)]
    pub processed_entries: Vec<AutomationProcessedInput>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuleRecordStageConfig {
    pub auto_polish: bool,
    pub polish_preset_id: String,
    pub auto_translate: bool,
    pub translation_language: String,
    pub export_enabled: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuleRecordExportConfig {
    pub directory: String,
    pub format: String,
    pub mode: String,
    pub prefix: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuleRecord {
    pub id: String,
    pub name: String,
    pub kind: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub priority: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    pub profile_source: String,
    pub save_history: bool,
    pub tag_ids: Vec<String>,
    pub preset_id: String,
    pub watch_directory: String,
    pub recursive: bool,
    pub enabled: bool,
    pub actions: AutomationRuleInputActions,
    pub stage_config: AutomationRuleRecordStageConfig,
    pub export_config: AutomationRuleRecordExportConfig,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub migration_notice: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationProcessedRecord {
    pub id: String,
    pub rule_id: String,
    #[serde(default = "default_processed_kind")]
    pub kind: String,
    #[serde(default)]
    pub input_version: String,
    #[serde(default = "default_processed_attempt")]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub attempt: i64,
    pub file_path: String,
    pub source_fingerprint: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub size: i64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub mtime_ms: i64,
    pub status: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub processed_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub export_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRepositoryState {
    pub profiles: Vec<AutomationProfileRecord>,
    pub rules: Vec<AutomationRuleRecord>,
    pub processed_entries: Vec<AutomationProcessedRecord>,
}

pub trait AutomationStore: Send + Sync {
    fn load_state(&self) -> Result<AutomationRepositoryState, AutomationError>;
    fn replace_profiles(&self, profiles: &[AutomationProfileRecord])
    -> Result<(), AutomationError>;
    fn replace_rules(&self, rules: &[AutomationRuleRecord]) -> Result<(), AutomationError>;
    fn replace_processed_entries(
        &self,
        entries: &[AutomationProcessedRecord],
    ) -> Result<(), AutomationError>;
    fn replace_state(&self, state: &AutomationRepositoryState) -> Result<(), AutomationError>;
}
