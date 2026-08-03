use serde::{Deserialize, Serialize};

use crate::ports::fs::FileSystemError;

/// ID generator port for automation entities. Placed in Core so that outbound
/// adapters (e.g. sona-sqlite) can implement it without depending on the
/// application layer.
pub trait AutomationIdGenerator: Send + Sync {
    fn generate_id(&self) -> String;
}

/// Filesystem capability port required by the automation validation service.
pub trait AutomationFsPort: Send + Sync {
    fn path_exists(&self, path: &str) -> Result<bool, FileSystemError>;
    fn create_dir_all(&self, path: &str) -> Result<(), FileSystemError>;
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRule {
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_save_history")]
    pub save_history: bool,
    #[serde(default)]
    pub tag_ids: Vec<String>,
    #[serde(default)]
    pub watch_directory: String,
    #[serde(default)]
    pub stage_config: AutomationRuleStageConfig,
    #[serde(default)]
    pub export_config: AutomationRuleExportConfig,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuleStageConfig {
    #[serde(default)]
    pub auto_polish: bool,
    #[serde(default)]
    pub auto_translate: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuleExportConfig {
    #[serde(default)]
    pub directory: String,
    #[serde(default = "default_export_mode")]
    pub mode: String,
}

impl Default for AutomationRuleExportConfig {
    fn default() -> Self {
        Self {
            directory: String::new(),
            mode: default_export_mode(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuleValidationResult {
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AutomationRuleActivationEnvironment {
    pub watch_directory_exists: bool,
    pub export_directory_ready: bool,
    pub batch_model_path_exists: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuntimeRuleConfig {
    pub rule_id: String,
    pub watch_directory: String,
    pub recursive: bool,
    pub exclude_directory: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub debounce_ms: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub stable_window_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuntimeReplaceResult {
    pub rule_id: String,
    pub started: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuntimeCandidatePayload {
    pub rule_id: String,
    pub file_path: String,
    pub source_fingerprint: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub size: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub mtime_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum AutomationRuntimePathCollectionOutcome {
    Candidate,
    Missing,
    Unsupported,
    Excluded,
    NotFile,
    Error,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuntimePathCollectionResult {
    pub file_path: String,
    pub outcome: AutomationRuntimePathCollectionOutcome,
    pub candidate: Option<AutomationRuntimeCandidatePayload>,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AutomationRuntimePathMetadata {
    pub is_file: bool,
    pub size: u64,
    pub mtime_ms: u64,
}

fn default_save_history() -> bool {
    true
}

fn default_export_mode() -> String {
    "original".to_string()
}
