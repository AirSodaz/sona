use serde_json::Value;

use crate::config::migrate_app_config;
use crate::config::service::app_config_stored_state_from_value;
use crate::ports::time::UnixMillisClock;

use super::{
    BackupApplyPreparedImportRequest, BackupApplyResult, BackupArchivePort, BackupDataset,
    BackupError, BackupExportRequest, BackupImportRequest, BackupInspectRequest, BackupManifest,
    BackupPrepareImportRequest, BackupRestoreDataset, BackupStateRepository, PreparedBackupImport,
    PreparedBackupSession, build_backup_manifest, validate_backup_manifest,
};

pub struct BackupService<'a> {
    archive: &'a dyn BackupArchivePort,
    state: &'a dyn BackupStateRepository,
    clock: &'a dyn UnixMillisClock,
}

impl<'a> BackupService<'a> {
    pub fn new(
        archive: &'a dyn BackupArchivePort,
        state: &'a dyn BackupStateRepository,
        clock: &'a dyn UnixMillisClock,
    ) -> Self {
        Self {
            archive,
            state,
            clock,
        }
    }

    pub fn export_archive(
        &self,
        request: BackupExportRequest,
    ) -> Result<BackupManifest, BackupError> {
        require_non_empty(&request.archive_path, "archive_path")?;
        require_non_empty(&request.app_version, "app_version")?;

        let dataset = self.state.snapshot()?;
        validate_dataset(&dataset)?;
        let manifest = manifest_for_dataset(self.now_ms()?, request.app_version, &dataset)?;
        self.archive
            .write_archive(&request.archive_path, &manifest, &dataset)?;
        Ok(manifest)
    }

    pub fn prepare_import(
        &self,
        request: BackupPrepareImportRequest,
    ) -> Result<PreparedBackupImport, BackupError> {
        require_non_empty(&request.archive_path, "archive_path")?;
        let preview = self.archive.prepare_import(&request.archive_path)?;
        let import_id = preview.import_id.clone();
        match validate_preview(&preview) {
            Ok(()) => Ok(preview),
            Err(primary) if import_id.trim().is_empty() => Err(primary),
            Err(primary) => self.finish_prepared(&import_id, Err(primary)),
        }
    }

    pub fn apply_prepared_import(
        &self,
        request: BackupApplyPreparedImportRequest,
    ) -> Result<BackupApplyResult, BackupError> {
        require_non_empty(&request.import_id, "import_id")?;
        require_non_empty(&request.default_rule_set_name, "default_rule_set_name")?;
        let import_id = request.import_id.clone();
        let primary_result = self.apply_prepared(&request);
        self.finish_prepared(&import_id, primary_result)
    }

    pub fn dispose_prepared_import(&self, import_id: &str) -> Result<(), BackupError> {
        require_non_empty(import_id, "import_id")?;
        self.archive.dispose_prepared(import_id)
    }

    pub fn inspect_archive(
        &self,
        request: BackupInspectRequest,
    ) -> Result<PreparedBackupImport, BackupError> {
        require_non_empty(&request.archive_path, "archive_path")?;
        let preview = self.prepare_import(BackupPrepareImportRequest {
            archive_path: request.archive_path,
        })?;
        let import_id = preview.import_id.clone();
        self.finish_prepared(&import_id, Ok(preview))
    }

    pub fn import_archive(
        &self,
        request: BackupImportRequest,
    ) -> Result<BackupApplyResult, BackupError> {
        require_non_empty(&request.archive_path, "archive_path")?;
        require_non_empty(&request.default_rule_set_name, "default_rule_set_name")?;
        if !request.confirm_replace {
            return Err(BackupError::ConfirmationRequired);
        }

        let preview = self.prepare_import(BackupPrepareImportRequest {
            archive_path: request.archive_path,
        })?;
        self.apply_prepared_import(BackupApplyPreparedImportRequest {
            import_id: preview.import_id,
            default_rule_set_name: request.default_rule_set_name,
        })
    }

    fn apply_prepared(
        &self,
        request: &BackupApplyPreparedImportRequest,
    ) -> Result<BackupApplyResult, BackupError> {
        let session = self.archive.load_prepared(&request.import_id)?;
        validate_session(&session, &request.import_id)?;

        let PreparedBackupSession {
            import_id,
            manifest,
            dataset,
        } = session;
        let BackupDataset {
            config,
            tags,
            history,
            automation,
            analytics_content,
        } = dataset;
        let migration =
            migrate_app_config(Some(config), None, request.default_rule_set_name.clone());
        let config_state = app_config_stored_state_from_value(&migration.config, self.now_ms()?)
            .map_err(|error| BackupError::Config(error.to_string()))?;
        self.state.replace_all(BackupRestoreDataset {
            import_id,
            manifest,
            config_state,
            tags,
            history,
            automation,
            analytics_content,
        })
    }

    fn finish_prepared<T>(
        &self,
        import_id: &str,
        primary_result: Result<T, BackupError>,
    ) -> Result<T, BackupError> {
        match (primary_result, self.archive.dispose_prepared(import_id)) {
            (Err(primary), _) => Err(primary),
            (Ok(value), Ok(())) => Ok(value),
            (Ok(_), Err(cleanup)) => Err(cleanup),
        }
    }

    fn now_ms(&self) -> Result<i64, BackupError> {
        let now_ms = self
            .clock
            .now_ms()
            .map_err(|error| BackupError::State(format!("Backup clock: {error}")))?;
        i64::try_from(now_ms).map_err(|error| {
            BackupError::State(format!("Backup clock milliseconds exceed i64: {error}"))
        })
    }
}

fn manifest_for_dataset(
    created_at_ms: i64,
    app_version: String,
    dataset: &BackupDataset,
) -> Result<BackupManifest, BackupError> {
    build_backup_manifest(
        created_at_ms,
        app_version,
        dataset.tags.len(),
        dataset.history.items.len(),
        dataset.history.transcript_files.len(),
        dataset.history.summary_files.len(),
        dataset.automation.rules.len(),
        dataset.automation.processed_entries.len(),
    )
}

fn validate_session(session: &PreparedBackupSession, import_id: &str) -> Result<(), BackupError> {
    if session.import_id.trim().is_empty() {
        return Err(BackupError::InvalidBackup(
            "Prepared backup import ID is required.".to_string(),
        ));
    }
    if session.import_id != import_id {
        return Err(BackupError::InvalidBackup(
            "Prepared backup import ID does not match the requested session.".to_string(),
        ));
    }
    validate_backup_manifest(&session.manifest)?;
    validate_dataset(&session.dataset)?;
    validate_manifest_counts(&session.manifest, &session.dataset)
}

fn validate_preview(preview: &PreparedBackupImport) -> Result<(), BackupError> {
    if preview.import_id.trim().is_empty() {
        return Err(BackupError::InvalidBackup(
            "Prepared backup import ID is required.".to_string(),
        ));
    }
    validate_backup_manifest(&preview.manifest)?;
    ensure_config_object(&preview.config)?;
    ensure_analytics_json(&preview.analytics_content)?;
    ensure_count(preview.manifest.counts.tags, preview.tags.len(), "tag")?;
    ensure_count(
        preview.manifest.counts.automation_rules,
        preview.automation_rules.len(),
        "automation-rule",
    )?;
    ensure_count(
        preview.manifest.counts.automation_processed_entries,
        preview.automation_processed_entries.len(),
        "processed-entry",
    )?;
    ensure_count(preview.manifest.counts.analytics_files, 1, "analytics")
}

fn validate_dataset(dataset: &BackupDataset) -> Result<(), BackupError> {
    ensure_config_object(&dataset.config)?;
    ensure_analytics_json(&dataset.analytics_content)
}

fn validate_manifest_counts(
    manifest: &BackupManifest,
    dataset: &BackupDataset,
) -> Result<(), BackupError> {
    ensure_count(manifest.counts.tags, dataset.tags.len(), "tag")?;
    ensure_count(
        manifest.counts.history_items,
        dataset.history.items.len(),
        "history",
    )?;
    ensure_count(
        manifest.counts.transcript_files,
        dataset.history.transcript_files.len(),
        "transcript",
    )?;
    ensure_count(
        manifest.counts.summary_files,
        dataset.history.summary_files.len(),
        "summary",
    )?;
    ensure_count(
        manifest.counts.automation_rules,
        dataset.automation.rules.len(),
        "automation-rule",
    )?;
    ensure_count(
        manifest.counts.automation_processed_entries,
        dataset.automation.processed_entries.len(),
        "processed-entry",
    )?;
    ensure_count(manifest.counts.analytics_files, 1, "analytics")
}

fn ensure_count(expected: u64, actual: usize, label: &str) -> Result<(), BackupError> {
    if expected == actual as u64 {
        return Ok(());
    }
    Err(BackupError::InvalidBackup(format!(
        "Backup {label} count does not match the manifest."
    )))
}

fn ensure_config_object(config: &Value) -> Result<(), BackupError> {
    if config.is_object() {
        return Ok(());
    }
    Err(BackupError::InvalidBackup(
        "Backup config must be an object.".to_string(),
    ))
}

fn ensure_analytics_json(content: &str) -> Result<(), BackupError> {
    let value: Value = serde_json::from_str(content).map_err(|error| {
        BackupError::InvalidBackup(format!("Backup analytics is not valid JSON: {error}"))
    })?;
    if value.is_object() || value.is_array() {
        return Ok(());
    }
    Err(BackupError::InvalidBackup(
        "Backup analytics must be an object or an array.".to_string(),
    ))
}

fn require_non_empty(value: &str, field: &str) -> Result<(), BackupError> {
    if !value.trim().is_empty() {
        return Ok(());
    }
    Err(BackupError::InvalidRequest(format!(
        "{field} must not be empty."
    )))
}
