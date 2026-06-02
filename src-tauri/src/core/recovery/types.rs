use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const RECOVERY_VERSION: u32 = 1;
pub const RECOVERY_DIR_NAME: &str = "recovery";
pub const QUEUE_RECOVERY_FILE_NAME: &str = "queue-recovery.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoverySnapshot {
    pub version: u32,
    pub updated_at: Option<u64>,
    pub items: Vec<RecoveredQueueItem>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveredQueueItem {
    pub id: String,
    pub filename: String,
    pub file_path: String,
    pub source: String,
    pub resolution: String,
    pub progress: f64,
    pub segments: Vec<RecoveredTranscriptSegment>,
    pub project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_title: Option<String>,
    pub last_known_stage: String,
    pub updated_at: u64,
    pub has_source_file: bool,
    pub can_resume: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automation_rule_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automation_rule_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_config_snapshot: Option<Value>,
    pub export_config: Value,
    pub stage_config: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_stat: Option<RecoveryFileStat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub export_file_name_prefix: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryFileStat {
    pub size: u64,
    pub mtime_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveredTranscriptSegment {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub start: f64,
    #[serde(default)]
    pub end: f64,
    #[serde(default = "default_true")]
    pub is_final: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timing: Option<RecoveredTranscriptTiming>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamps: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub durations: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker_attribution: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveredTranscriptTiming {
    pub level: String,
    pub source: String,
    pub units: Vec<RecoveredTranscriptTimingUnit>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveredTranscriptTimingUnit {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub start: f64,
    #[serde(default)]
    pub end: f64,
}

pub fn default_true() -> bool {
    true
}
