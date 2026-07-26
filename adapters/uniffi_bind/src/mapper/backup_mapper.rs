use serde_json::Value;
use sona_core::backup::{
    BackupApplyResult, BackupManifest, BackupManifestCounts, BackupManifestScopes,
    PreparedBackupImport,
};

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiBackupManifestV1 {
    pub schema_version: u64,
    pub created_at: String,
    pub app_version: String,
    pub history_mode: String,
    pub scopes: FfiBackupManifestScopesV1,
    pub counts: FfiBackupManifestCountsV1,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiBackupManifestScopesV1 {
    pub config: bool,
    pub workspace: bool,
    pub history: bool,
    pub automation: bool,
    pub analytics: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiBackupManifestCountsV1 {
    pub tags: u64,
    pub history_items: u64,
    pub transcript_files: u64,
    pub summary_files: u64,
    pub automation_profiles: u64,
    pub automation_rules: u64,
    pub automation_processed_entries: u64,
    pub analytics_files: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiBackupApplyResultV1 {
    pub import_id: String,
    pub manifest: FfiBackupManifestV1,
}

/// The manifest is fully structured; the restored payload arrays stay canonical
/// JSON leaf strings because their contents are user config and automation
/// documents whose schema the archive — not this binding — owns.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiPreparedBackupImportV1 {
    pub import_id: String,
    pub archive_path: String,
    pub manifest: FfiBackupManifestV1,
    pub config_json: String,
    pub tags_json: Vec<String>,
    pub automation_profiles_json: Vec<String>,
    pub automation_rules_json: Vec<String>,
    pub automation_processed_entries_json: Vec<String>,
    pub analytics_content: String,
}

impl From<BackupManifestScopes> for FfiBackupManifestScopesV1 {
    fn from(value: BackupManifestScopes) -> Self {
        Self {
            config: value.config,
            workspace: value.workspace,
            history: value.history,
            automation: value.automation,
            analytics: value.analytics,
        }
    }
}

impl From<BackupManifestCounts> for FfiBackupManifestCountsV1 {
    fn from(value: BackupManifestCounts) -> Self {
        Self {
            tags: value.tags,
            history_items: value.history_items,
            transcript_files: value.transcript_files,
            summary_files: value.summary_files,
            automation_profiles: value.automation_profiles,
            automation_rules: value.automation_rules,
            automation_processed_entries: value.automation_processed_entries,
            analytics_files: value.analytics_files,
        }
    }
}

impl From<BackupManifest> for FfiBackupManifestV1 {
    fn from(value: BackupManifest) -> Self {
        Self {
            schema_version: value.schema_version,
            created_at: value.created_at,
            app_version: value.app_version,
            history_mode: value.history_mode,
            scopes: value.scopes.into(),
            counts: value.counts.into(),
        }
    }
}

impl From<BackupApplyResult> for FfiBackupApplyResultV1 {
    fn from(value: BackupApplyResult) -> Self {
        Self {
            import_id: value.import_id,
            manifest: value.manifest.into(),
        }
    }
}

pub(crate) fn prepared_backup_import_to_ffi(
    value: PreparedBackupImport,
) -> Result<FfiPreparedBackupImportV1, serde_json::Error> {
    Ok(FfiPreparedBackupImportV1 {
        import_id: value.import_id,
        archive_path: value.archive_path,
        manifest: value.manifest.into(),
        config_json: serde_json::to_string(&value.config)?,
        tags_json: values_to_json(value.tags)?,
        automation_profiles_json: values_to_json(value.automation_profiles)?,
        automation_rules_json: values_to_json(value.automation_rules)?,
        automation_processed_entries_json: values_to_json(value.automation_processed_entries)?,
        analytics_content: value.analytics_content,
    })
}

fn values_to_json(values: Vec<Value>) -> Result<Vec<String>, serde_json::Error> {
    values.iter().map(serde_json::to_string).collect()
}
