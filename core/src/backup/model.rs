use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::automation::repository::AutomationRepositoryState;
use crate::config::AppConfigStoredState;
use crate::history::HistoryBackupSnapshot;
use crate::tag::TagRecord;

use super::BackupError;

pub const BACKUP_SCHEMA_VERSION: u64 = 3;
pub const BACKUP_HISTORY_MODE: &str = "light";

#[derive(Clone, Debug)]
pub struct BackupDataset {
    pub config: Value,
    pub tags: Vec<TagRecord>,
    pub history: HistoryBackupSnapshot,
    pub automation: AutomationRepositoryState,
    pub analytics_content: String,
}

#[derive(Clone, Debug)]
pub struct PreparedBackupSession {
    pub import_id: String,
    pub manifest: BackupManifest,
    pub dataset: BackupDataset,
}

#[derive(Clone, Debug)]
pub struct BackupRestoreDataset {
    pub import_id: String,
    pub manifest: BackupManifest,
    pub config_state: AppConfigStoredState,
    pub tags: Vec<TagRecord>,
    pub history: HistoryBackupSnapshot,
    pub automation: AutomationRepositoryState,
    pub analytics_content: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BackupExportRequest {
    pub archive_path: String,
    pub app_version: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BackupPrepareImportRequest {
    pub archive_path: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BackupApplyPreparedImportRequest {
    pub import_id: String,
    pub default_rule_set_name: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BackupInspectRequest {
    pub archive_path: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BackupImportRequest {
    pub archive_path: String,
    pub default_rule_set_name: String,
    pub confirm_replace: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BackupApplyResult {
    pub import_id: String,
    pub manifest: BackupManifest,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BackupManifest {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub schema_version: u64,
    pub created_at: String,
    pub app_version: String,
    pub history_mode: String,
    pub scopes: BackupManifestScopes,
    pub counts: BackupManifestCounts,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BackupManifestScopes {
    pub config: bool,
    pub workspace: bool,
    pub history: bool,
    pub automation: bool,
    pub analytics: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BackupManifestCounts {
    #[serde(alias = "projects")]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub tags: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub history_items: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub transcript_files: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub summary_files: u64,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub automation_profiles: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub automation_rules: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub automation_processed_entries: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub analytics_files: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PreparedBackupImport {
    pub import_id: String,
    pub archive_path: String,
    pub manifest: BackupManifest,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Unknown))]
    pub config: Value,
    #[serde(alias = "projects")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Vec<specta_typescript::Unknown>)
    )]
    pub tags: Vec<Value>,
    #[serde(default)]
    #[cfg_attr(
        feature = "specta",
        specta(type = Vec<specta_typescript::Unknown>)
    )]
    pub automation_profiles: Vec<Value>,
    #[cfg_attr(
        feature = "specta",
        specta(type = Vec<specta_typescript::Unknown>)
    )]
    pub automation_rules: Vec<Value>,
    #[cfg_attr(
        feature = "specta",
        specta(type = Vec<specta_typescript::Unknown>)
    )]
    pub automation_processed_entries: Vec<Value>,
    pub analytics_content: String,
}

#[allow(clippy::too_many_arguments)]
pub fn build_backup_manifest(
    created_at_ms: i64,
    app_version: String,
    tag_count: usize,
    history_item_count: usize,
    transcript_file_count: usize,
    summary_file_count: usize,
    automation_profile_count: usize,
    automation_rule_count: usize,
    automation_processed_entry_count: usize,
) -> Result<BackupManifest, BackupError> {
    let created_at = DateTime::<Utc>::from_timestamp_millis(created_at_ms)
        .ok_or_else(|| BackupError::State("Backup clock timestamp is out of range.".to_string()))?
        .to_rfc3339_opts(SecondsFormat::Millis, true);
    Ok(BackupManifest {
        schema_version: BACKUP_SCHEMA_VERSION,
        created_at,
        app_version,
        history_mode: BACKUP_HISTORY_MODE.to_string(),
        scopes: BackupManifestScopes {
            config: true,
            workspace: true,
            history: true,
            automation: true,
            analytics: true,
        },
        counts: BackupManifestCounts {
            tags: tag_count as u64,
            history_items: history_item_count as u64,
            transcript_files: transcript_file_count as u64,
            summary_files: summary_file_count as u64,
            automation_profiles: automation_profile_count as u64,
            automation_rules: automation_rule_count as u64,
            automation_processed_entries: automation_processed_entry_count as u64,
            analytics_files: 1,
        },
    })
}

pub fn validate_backup_manifest(manifest: &BackupManifest) -> Result<(), BackupError> {
    if !matches!(manifest.schema_version, 1 | 2 | BACKUP_SCHEMA_VERSION) {
        return Err(BackupError::InvalidBackup(format!(
            "Unsupported backup schema version: {}",
            manifest.schema_version
        )));
    }
    if manifest.history_mode != BACKUP_HISTORY_MODE {
        return Err(BackupError::InvalidBackup(format!(
            "Unsupported backup history mode: {}",
            manifest.history_mode
        )));
    }
    if !manifest.scopes.config
        || !manifest.scopes.workspace
        || !manifest.scopes.history
        || !manifest.scopes.automation
        || !manifest.scopes.analytics
    {
        return Err(BackupError::InvalidBackup(
            "Backup manifest is missing one or more required scopes.".to_string(),
        ));
    }
    Ok(())
}
