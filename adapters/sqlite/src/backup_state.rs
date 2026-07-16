use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use sona_core::backup::{
    BackupApplyResult, BackupDataset, BackupError, BackupRestoreDataset, BackupStateRepository,
};
use sona_core::config::app_config_value_from_stored_state;
use sona_core::tag::ACTIVE_TAG_SETTINGS_KEY;

use crate::automation::{
    delete_automation_in_transaction, insert_automation_in_transaction,
    load_automation_in_transaction,
};
use crate::config_store::{
    clear_setting_in_transaction, load_state_in_transaction, replace_state_in_transaction,
};
use crate::history_store::{
    PreparedHistoryRestore, acquire_history_file_lock, delete_history_in_transaction,
    insert_history_in_transaction, load_history_backup_in_transaction, prepare_history_restore,
};
use crate::llm_usage::{
    PreparedLlmUsageRows, parse_raw, read_raw_in_transaction, replace_raw_in_transaction,
};
use crate::ports::Database as DatabasePort;
use crate::tag::{
    delete_tags_in_transaction, insert_tags_in_transaction, load_tags_in_transaction,
};
use crate::{Database, DatabaseError};

#[derive(Clone)]
pub struct SqliteBackupStateRepository<D = Database>
where
    D: DatabasePort,
{
    app_local_data_dir: PathBuf,
    db: Arc<D>,
}

impl<D> SqliteBackupStateRepository<D>
where
    D: DatabasePort,
{
    pub fn new(app_local_data_dir: PathBuf, db: Arc<D>) -> Self {
        Self {
            app_local_data_dir,
            db,
        }
    }
}

impl<D> BackupStateRepository for SqliteBackupStateRepository<D>
where
    D: DatabasePort,
{
    fn snapshot(&self) -> Result<BackupDataset, BackupError> {
        let _history_lock =
            acquire_history_file_lock(&self.app_local_data_dir).map_err(backup_state_error)?;
        self.db
            .with_rw_transaction(|tx| {
                let config_state = load_state_in_transaction(tx)?.ok_or_else(|| {
                    DatabaseError::NotFoundError("Application config is missing.".to_string())
                })?;
                let config = app_config_value_from_stored_state(config_state)
                    .map_err(DatabaseError::Internal)?;
                let tags = load_tags_in_transaction(tx)?;
                let history = load_history_backup_in_transaction(tx)?;
                let automation = load_automation_in_transaction(tx)?;
                let analytics_content = read_raw_in_transaction(tx)?;
                Ok(BackupDataset {
                    config,
                    tags,
                    history,
                    automation,
                    analytics_content,
                })
            })
            .map_err(backup_state_error)
    }

    fn replace_all(&self, dataset: BackupRestoreDataset) -> Result<BackupApplyResult, BackupError> {
        let prepared = preflight_backup_restore_dataset(&dataset)?;

        let _history_lock =
            acquire_history_file_lock(&self.app_local_data_dir).map_err(backup_state_error)?;
        self.db
            .with_rw_transaction(|tx| apply_backup_restore(tx, &dataset, &prepared))
            .map_err(backup_state_error)?;

        Ok(BackupApplyResult {
            import_id: dataset.import_id,
            manifest: dataset.manifest,
        })
    }
}

struct PreparedBackupRestore {
    history: PreparedHistoryRestore,
    analytics: PreparedLlmUsageRows,
}

/// Validates a complete restore against the production SQLite schema without
/// opening the target database or touching its application-data directory.
pub fn validate_backup_restore_dataset(dataset: &BackupRestoreDataset) -> Result<(), BackupError> {
    preflight_backup_restore_dataset(dataset).map(drop)
}

fn preflight_backup_restore_dataset(
    dataset: &BackupRestoreDataset,
) -> Result<PreparedBackupRestore, BackupError> {
    let prepared = prepare_backup_restore_dataset(dataset)?;
    let database = Database::open_in_memory().map_err(|error| {
        BackupError::State(format!("Backup restore preflight database: {error}"))
    })?;
    database
        .with_rw_transaction(|tx| apply_backup_restore(tx, dataset, &prepared))
        .map_err(|error| {
            BackupError::InvalidBackup(format!("Backup restore cannot be persisted: {error}"))
        })?;
    Ok(prepared)
}

fn prepare_backup_restore_dataset(
    dataset: &BackupRestoreDataset,
) -> Result<PreparedBackupRestore, BackupError> {
    validate_restore_relationships(dataset)?;
    let history = prepare_history_restore(&dataset.history).map_err(BackupError::InvalidBackup)?;
    let analytics = parse_raw(&dataset.analytics_content).map_err(BackupError::InvalidBackup)?;
    Ok(PreparedBackupRestore { history, analytics })
}

fn apply_backup_restore(
    tx: &rusqlite::Transaction<'_>,
    dataset: &BackupRestoreDataset,
    prepared: &PreparedBackupRestore,
) -> Result<(), DatabaseError> {
    delete_automation_in_transaction(tx)?;
    delete_history_in_transaction(tx)?;
    delete_tags_in_transaction(tx)?;

    replace_state_in_transaction(tx, &dataset.config_state)?;
    clear_setting_in_transaction(tx, ACTIVE_TAG_SETTINGS_KEY)?;
    insert_tags_in_transaction(tx, &dataset.tags)?;
    insert_history_in_transaction(tx, &prepared.history)?;
    insert_automation_in_transaction(tx, &dataset.automation)?;
    replace_raw_in_transaction(tx, &prepared.analytics)
}

fn validate_restore_relationships(dataset: &BackupRestoreDataset) -> Result<(), BackupError> {
    let tag_ids = dataset
        .tags
        .iter()
        .map(|tag| {
            i64::try_from(tag.created_at).map_err(|_| {
                BackupError::InvalidBackup(format!(
                    "Tag created timestamp exceeds SQLite range: {}",
                    tag.id
                ))
            })?;
            i64::try_from(tag.updated_at).map_err(|_| {
                BackupError::InvalidBackup(format!(
                    "Tag updated timestamp exceeds SQLite range: {}",
                    tag.id
                ))
            })?;
            Ok(tag.id.as_str())
        })
        .collect::<Result<HashSet<_>, BackupError>>()?;
    if tag_ids.len() != dataset.tags.len() {
        return Err(BackupError::InvalidBackup(
            "Backup contains duplicate tag IDs.".to_string(),
        ));
    }
    for item in &dataset.history.items {
        for tag_id in &item.tag_ids {
            if !tag_ids.contains(tag_id.as_str()) {
                return Err(BackupError::InvalidBackup(format!(
                    "History item references an unknown tag: {}",
                    item.id
                )));
            }
        }
    }
    Ok(())
}

fn backup_state_error(error: DatabaseError) -> BackupError {
    BackupError::State(error.to_string())
}
