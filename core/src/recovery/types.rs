use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use serde_with::{DefaultOnError, serde_as};

use crate::transcription::transcript::{
    SpeakerAttribution, SpeakerTag, TranscriptTimingLevel, TranscriptTimingSource,
};

#[cfg(feature = "specta")]
use specta::Type;

pub const RECOVERY_VERSION: u32 = 2;
pub const RECOVERY_DIR_NAME: &str = "recovery";
pub const QUEUE_RECOVERY_FILE_NAME: &str = "queue-recovery.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, strum::Display)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum RecoverySource {
    BatchImport,
    Automation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, strum::Display)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum RecoveryResolution {
    Pending,
    Resumed,
    Discarded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, strum::Display)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum RecoveryItemStage {
    Queued,
    Transcribing,
    Polishing,
    Translating,
    Exporting,
}

#[serde_as]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct RecoverySnapshotInput {
    #[serde_as(as = "DefaultOnError")]
    #[serde(default)]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub updated_at: Option<u64>,
    #[serde(default, deserialize_with = "deserialize_valid_items")]
    #[cfg_attr(feature = "specta", specta(type = Vec<RecoveryItemInput>))]
    pub items: Vec<RecoveryItemInput>,
}

#[serde_as]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct RecoveryItemInput {
    #[serde_as(as = "DefaultOnError")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub id: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub recovery_id: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub filename: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub file_path: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub source: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub origin: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub resolution: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub status: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub progress: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_valid_items")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Vec<RecoveredTranscriptSegment>)
    )]
    pub segments: Vec<RecoveredTranscriptSegment>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "specta", specta(type = Vec<String>))]
    pub tag_ids: Vec<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub project_id: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub history_id: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub history_title: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub last_known_stage: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub updated_at: Option<u64>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(type = Option<bool>))]
    pub has_source_file: Option<bool>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(type = Option<bool>))]
    pub can_resume: Option<bool>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub automation_rule_id: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub automation_rule_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Unknown>)
    )]
    pub resolved_config_snapshot: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Unknown>)
    )]
    pub export_config: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Unknown>)
    )]
    pub stage_config: Option<Value>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub source_fingerprint: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(type = Option<RecoveryFileStat>))]
    pub file_stat: Option<RecoveryFileStat>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub export_file_name_prefix: Option<String>,
}

fn deserialize_valid_items<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let value = Value::deserialize(deserializer)?;
    let values = value.as_array().cloned().unwrap_or_default();
    Ok(values
        .into_iter()
        .filter_map(|value| serde_json::from_value(value).ok())
        .collect())
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct RecoverySnapshot {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub version: u32,
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub updated_at: Option<u64>,
    pub items: Vec<RecoveredQueueItem>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct RecoveredQueueItem {
    pub id: String,
    pub filename: String,
    pub file_path: String,
    pub source: RecoverySource,
    pub resolution: RecoveryResolution,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub progress: f64,
    pub segments: Vec<RecoveredTranscriptSegment>,
    #[serde(default)]
    pub tag_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_title: Option<String>,
    pub last_known_stage: RecoveryItemStage,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: u64,
    pub has_source_file: bool,
    pub can_resume: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automation_rule_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automation_rule_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Unknown>)
    )]
    pub resolved_config_snapshot: Option<Value>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Unknown))]
    pub export_config: Value,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Unknown))]
    pub stage_config: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_stat: Option<RecoveryFileStat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub export_file_name_prefix: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct RecoveryFileStat {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub size: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub mtime_ms: u64,
}

#[serde_as]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct RecoveredTranscriptSegment {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub start: f64,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub end: f64,
    #[serde(default = "default_true")]
    pub is_final: bool,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<RecoveredTranscriptTiming>)
    )]
    pub timing: Option<RecoveredTranscriptTiming>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<Vec<specta_typescript::Number>>)
    )]
    pub timestamps: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<Vec<specta_typescript::Number>>)
    )]
    pub durations: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(type = Option<SpeakerTag>))]
    pub speaker: Option<SpeakerTag>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(type = Option<SpeakerAttribution>))]
    pub speaker_attribution: Option<SpeakerAttribution>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct RecoveredTranscriptTiming {
    pub level: TranscriptTimingLevel,
    pub source: TranscriptTimingSource,
    pub units: Vec<RecoveredTranscriptTimingUnit>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct RecoveredTranscriptTimingUnit {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub start: f64,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub end: f64,
}

pub fn default_true() -> bool {
    true
}
