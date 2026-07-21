use crate::DatabaseError;
use crate::history_fs_utils::{
    ensure_safe_file_name, optional_history_child_path,
    remove_path_if_exists,
};
use crate::ports::Database as DatabasePort;
use crate::sync_repository::{
    record_local_delete_in_transaction, record_local_field_change_in_transaction,
};
use serde_json::Value;
use sona_core::dashboard::error::DashboardServiceError;
use sona_core::history::item_factory::HistoryItemGeneratedValues;
use sona_core::history::mutation_repository::{
    HistoryCompleteLiveDraftRequest, HistoryCreateTranscriptSnapshotRequest, HistoryItemMetaPatch,
    HistoryMutationError, HistoryMutationRepository, HistoryPurgeItemsRequest,
    HistoryReplaceTagAssignmentsRequest, HistoryRestoreItemsRequest, HistoryTrashItemsRequest,
    HistoryUpdateItemMetaRequest, HistoryUpdateTagAssignmentsRequest,
    HistoryUpdateTranscriptRequest,
};
use sona_core::history::query_repository::HistoryQueryRepository;
use sona_core::history::transcript_payload::{
    canonicalize_history_transcript_segments, normalize_history_transcript_segments,
};
use sona_core::history::workspace_query::{
    normalize_workspace_search_text, validate_workspace_query_request, workspace_item_search_match,
};
use sona_core::history::{
    HistoryAudioCleanupReport, HistoryAudioCleanupRequest, HistoryAudioStatus,
    HistoryCreateLiveDraftRequest, HistoryDraftSource, HistoryIdGenerator, HistoryItemKind,
    HistoryItemRecord, HistoryItemStatus, HistoryListOptions, HistorySaveImportedFileRequest,
    HistorySaveRecordingRequest, HistorySummaryPayload, HistoryWorkspaceDateFilter,
    HistoryWorkspaceFilterType, HistoryWorkspaceItemCounts, HistoryWorkspaceQueryRequest,
    HistoryWorkspaceQueryResult, HistoryWorkspaceScope, HistoryWorkspaceSortOrder,
    HistoryWorkspaceSummary, LiveRecordingDraftResult, TranscriptSnapshotMetadata,
    TranscriptSnapshotReason, TranscriptSnapshotRecord,
};
use sona_core::history_store::{HistoryStore, HistoryStoreError};
use sona_core::ports::fs::{FileSystemError, FileSystemOperation};
use sona_core::ports::time::{ClockError, UnixMillisClock};
use sona_core::sync::SyncEntityKind;
use sona_core::transcription::transcript::TranscriptSegment;
use rusqlite::types::ToSql;
use std::cell::Cell;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use super::lock::acquire_history_file_lock;
use super::row_map::{
    apply_history_item_updates, history_store_mutation_error, map_row_to_item, validate_id,
};
pub(super) use super::sql::{HISTORY_ITEM_ROW_COLUMNS, history_insert_sql};
use super::sql::{
    history_select_columns, insert_history_item_row,
    load_tag_ids, new_history_item_generated_values, replace_history_item_tags,
    require_history_source_file,
};
use super::util::{
    HISTORY_DIR_NAME, MILLIS_PER_DAY, STAGED_AUDIO_MARKER, TRANSCRIPT_SNAPSHOT_RETENTION_LIMIT,
    db_file_system_error,
};
use super::workspace::{
    add_workspace_scope_condition, current_workspace_date_filter_thresholds,
    workspace_match_query_parts, workspace_order_by,
};

pub struct SqliteHistoryStore<D = crate::Database>
where
    D: DatabasePort,
{
    app_local_data_dir: PathBuf,
    db: Arc<D>,
    clock: Option<Arc<dyn UnixMillisClock>>,
    ids: Option<Arc<dyn HistoryIdGenerator>>,
}

impl<D> SqliteHistoryStore<D>
where
    D: DatabasePort,
{
    pub fn new(app_local_data_dir: PathBuf, db: Arc<D>) -> Self {
        Self {
            app_local_data_dir,
            db,
            clock: None,
            ids: None,
        }
    }

    pub fn with_environment(
        app_local_data_dir: PathBuf,
        db: Arc<D>,
        clock: Arc<dyn UnixMillisClock>,
        ids: Arc<dyn HistoryIdGenerator>,
    ) -> Self {
        Self {
            app_local_data_dir,
            db,
            clock: Some(clock),
            ids: Some(ids),
        }
    }

    fn get_db(&self) -> Result<&D, DatabaseError> {
        Ok(self.db.as_ref())
    }

    fn clock(&self) -> Result<&dyn UnixMillisClock, ClockError> {
        self.clock
            .as_deref()
            .ok_or_else(|| ClockError::Unavailable("History clock is not configured".to_string()))
    }

    fn ids(&self) -> Result<&dyn HistoryIdGenerator, DatabaseError> {
        self.ids.as_deref().ok_or_else(|| {
            DatabaseError::Internal("History ID generator is not configured".to_string())
        })
    }

    fn mutation_now_ms(&self) -> Result<u64, HistoryMutationError> {
        Ok(self.clock()?.now_ms()?)
    }

    fn query_now_ms(&self) -> Result<u64, HistoryStoreError> {
        Ok(self.clock()?.now_ms()?)
    }

    fn generated_values(&self) -> Result<HistoryItemGeneratedValues, HistoryMutationError> {
        let clock = self.clock()?;
        let ids = self.ids()?;
        Ok(new_history_item_generated_values(clock, ids)?)
    }
}

impl SqliteHistoryStore<crate::Database> {
    #[cfg(test)]
    pub(crate) fn with_db(app_local_data_dir: PathBuf, db: crate::Database) -> Self {
        Self::with_environment(
            app_local_data_dir,
            Arc::new(db),
            Arc::new(TestHistoryClock),
            Arc::new(TestHistoryIds),
        )
    }
}

#[cfg(test)]
struct TestHistoryClock;

#[cfg(test)]
impl UnixMillisClock for TestHistoryClock {
    fn now_ms(&self) -> Result<u64, ClockError> {
        std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .map_err(|error| ClockError::BeforeUnixEpoch(error.to_string()))
    }
}

#[cfg(test)]
struct TestHistoryIds;

#[cfg(test)]
impl HistoryIdGenerator for TestHistoryIds {
    fn generate_id(&self) -> String {
        uuid::Uuid::new_v4().to_string()
    }
}

struct StagedHistoryAudio {
    staging_path: PathBuf,
    target_path: PathBuf,
}

#[derive(Clone, Debug)]
struct AudioCleanupCandidate {
    id: String,
    audio_path: String,
}

impl StagedHistoryAudio {
    fn promote(&self) -> Result<(), DatabaseError> {
        match fs::metadata(&self.target_path) {
            Ok(_) => {
                return Err(DatabaseError::Internal(format!(
                    "History audio target already exists: {}",
                    self.target_path.to_string_lossy()
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(db_file_system_error(
                    FileSystemOperation::Metadata,
                    &self.target_path,
                    error,
                ));
            }
        }
        fs::rename(&self.staging_path, &self.target_path).map_err(|error| {
            DatabaseError::FileSystem(FileSystemError::with_target(
                FileSystemOperation::Rename,
                &self.staging_path,
                &self.target_path,
                error.to_string(),
            ))
        })
    }

    fn cleanup_staging(&self) {
        let _ = remove_path_if_exists(&self.staging_path);
    }

    fn cleanup_final(&self) {
        let _ = remove_path_if_exists(&self.target_path);
    }
}

impl<D> SqliteHistoryStore<D>
where
    D: DatabasePort,
{
    fn with_history_file_lock<T>(
        &self,
        operation: impl FnOnce() -> Result<T, HistoryMutationError>,
    ) -> Result<T, HistoryMutationError> {
        let _guard = acquire_history_file_lock(&self.app_local_data_dir)
            .map_err(HistoryMutationError::from)?;
        operation()
    }

    fn history_dir(&self) -> PathBuf {
        self.app_local_data_dir.join(HISTORY_DIR_NAME)
    }

    fn audio_path(&self, file_name: &str) -> Result<PathBuf, DatabaseError> {
        Ok(self.history_dir().join(
            ensure_safe_file_name(file_name, "History audio path")
                .map_err(|error| DatabaseError::Internal(error.to_string()))?,
        ))
    }

    fn staging_audio_path(&self, target_path: &Path) -> Result<PathBuf, DatabaseError> {
        let parent = target_path.parent().ok_or_else(|| {
            DatabaseError::Internal("History audio target has no parent directory.".to_string())
        })?;
        let file_name = target_path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| {
                DatabaseError::Internal("History audio target has no file name.".to_string())
            })?;
        Ok(parent.join(format!(
            "{file_name}{STAGED_AUDIO_MARKER}{}",
            self.ids()?.generate_id()
        )))
    }

    fn stage_audio_bytes(
        &self,
        target_path: PathBuf,
        bytes: &[u8],
    ) -> Result<StagedHistoryAudio, DatabaseError> {
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                db_file_system_error(FileSystemOperation::CreateDirectory, parent, error)
            })?;
        }
        let staging_path = self.staging_audio_path(&target_path)?;
        let write_result = (|| -> Result<(), DatabaseError> {
            let file = fs::File::create(&staging_path).map_err(|error| {
                db_file_system_error(FileSystemOperation::WriteFile, &staging_path, error)
            })?;
            let mut writer = BufWriter::new(file);
            writer.write_all(bytes).map_err(|error| {
                db_file_system_error(FileSystemOperation::WriteFile, &staging_path, error)
            })?;
            writer.flush().map_err(|error| {
                db_file_system_error(FileSystemOperation::WriteFile, &staging_path, error)
            })?;
            Ok(())
        })();
        if write_result.is_err() {
            let _ = remove_path_if_exists(&staging_path);
        }
        write_result?;
        Ok(StagedHistoryAudio {
            staging_path,
            target_path,
        })
    }

    fn stage_audio_copy(
        &self,
        source_path: &Path,
        target_path: PathBuf,
    ) -> Result<StagedHistoryAudio, DatabaseError> {
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                db_file_system_error(FileSystemOperation::CreateDirectory, parent, error)
            })?;
        }
        let staging_path = self.staging_audio_path(&target_path)?;
        let copy_result = fs::copy(source_path, &staging_path)
            .map(|_| ())
            .map_err(|error| {
                DatabaseError::FileSystem(FileSystemError::with_target(
                    FileSystemOperation::Copy,
                    source_path,
                    &staging_path,
                    error.to_string(),
                ))
            });
        if copy_result.is_err() {
            let _ = remove_path_if_exists(&staging_path);
        }
        copy_result?;
        Ok(StagedHistoryAudio {
            staging_path,
            target_path,
        })
    }

    fn cleanup_stale_staged_audio_files(&self) -> Result<(), DatabaseError> {
        let history_dir = self.history_dir();
        match fs::metadata(&history_dir) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                return Err(db_file_system_error(
                    FileSystemOperation::Metadata,
                    &history_dir,
                    error,
                ));
            }
        }

        for entry in fs::read_dir(&history_dir).map_err(|error| {
            db_file_system_error(FileSystemOperation::ReadDirectory, &history_dir, error)
        })? {
            let entry = entry.map_err(|error| {
                db_file_system_error(FileSystemOperation::ReadDirectory, &history_dir, error)
            })?;
            let file_name = entry.file_name();
            if file_name.to_string_lossy().contains(STAGED_AUDIO_MARKER) {
                remove_path_if_exists(&entry.path()).map_err(DatabaseError::FileSystem)?;
            }
        }

        Ok(())
    }

    fn update_audio_status(
        &self,
        history_id: &str,
        status: HistoryAudioStatus,
    ) -> Result<(), DatabaseError> {
        let status_str = status.to_string();
        self.get_db()?.with_write_connection(|conn| {
            conn.execute(
                "UPDATE history_items SET audio_status = ?1 WHERE id = ?2",
                rusqlite::params![status_str, history_id],
            )?;
            Ok(())
        })
    }

    fn audio_cleanup_cutoff(now_millis: u64, retention_days: Option<u64>) -> Option<i64> {
        let retention_days = retention_days?;
        let retention_millis = retention_days
            .saturating_mul(MILLIS_PER_DAY)
            .min(i64::MAX as u64) as i64;
        Some((now_millis.min(i64::MAX as u64) as i64).saturating_sub(retention_millis))
    }

    fn audio_cleanup_candidates(
        &self,
        cutoff_millis: i64,
    ) -> Result<Vec<AudioCleanupCandidate>, DatabaseError> {
        self.get_db()?.with_connection(|conn| {
            let mut stmt = conn.prepare_cached(
                "SELECT id, audio_path
                 FROM history_items
                 WHERE status = 'complete'
                   AND audio_status = 'available'
                   AND timestamp <= ?1",
            )?;
            let rows = stmt.query_map([cutoff_millis], |row| {
                Ok(AudioCleanupCandidate {
                    id: row.get(0)?,
                    audio_path: row.get(1)?,
                })
            })?;

            let mut candidates = Vec::new();
            for row in rows {
                candidates.push(row?);
            }
            Ok(candidates)
        })
    }

    fn run_audio_cleanup(
        &self,
        request: HistoryAudioCleanupRequest,
        apply: bool,
    ) -> Result<HistoryAudioCleanupReport, HistoryStoreError> {
        self.ensure_ready()?;

        let now_millis = self.query_now_ms()?;
        let Some(cutoff_millis) = Self::audio_cleanup_cutoff(now_millis, request.retention_days)
        else {
            return Ok(HistoryAudioCleanupReport::default());
        };

        if let Some(history_id) = request.exclude_history_id.as_deref() {
            validate_id(history_id, "Excluded history ID").map_err(DatabaseError::Internal)?;
        }

        let history_dir = self.history_dir();
        let candidates = self.audio_cleanup_candidates(cutoff_millis)?;
        let mut report = HistoryAudioCleanupReport::default();

        for candidate in candidates {
            if request.exclude_history_id.as_deref() == Some(candidate.id.as_str()) {
                report.skipped_active_count += 1;
                continue;
            }

            report.eligible_count += 1;

            let Some(audio_path) = optional_history_child_path(&history_dir, &candidate.audio_path)
            else {
                report.missing_marked_count += 1;
                if apply {
                    self.update_audio_status(&candidate.id, HistoryAudioStatus::Missing)?;
                }
                continue;
            };

            match fs::metadata(&audio_path) {
                Ok(metadata) if metadata.is_file() && metadata.len() > 0 => {
                    report.removed_count += 1;
                    report.removed_bytes = report.removed_bytes.saturating_add(metadata.len());
                    if apply {
                        if let Err(error) =
                            self.update_audio_status(&candidate.id, HistoryAudioStatus::Removed)
                        {
                            log::warn!(
                                "Failed to mark history audio removed before cleanup {}: {}",
                                audio_path.display(),
                                error
                            );
                            report.removed_count -= 1;
                            report.removed_bytes =
                                report.removed_bytes.saturating_sub(metadata.len());
                            report.failed_count += 1;
                            continue;
                        }

                        if let Err(error) = remove_path_if_exists(&audio_path) {
                            log::warn!(
                                "Failed to clean history audio {}: {}",
                                audio_path.display(),
                                error
                            );
                            if let Err(reset_error) = self
                                .update_audio_status(&candidate.id, HistoryAudioStatus::Available)
                            {
                                log::warn!(
                                    "Failed to restore history audio status after cleanup failure {}: {}",
                                    candidate.id,
                                    reset_error
                                );
                            }
                            report.removed_count -= 1;
                            report.removed_bytes =
                                report.removed_bytes.saturating_sub(metadata.len());
                            report.failed_count += 1;
                        }
                    }
                }
                Ok(_) => {
                    report.missing_marked_count += 1;
                    if apply {
                        self.update_audio_status(&candidate.id, HistoryAudioStatus::Missing)?;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    report.missing_marked_count += 1;
                    if apply {
                        self.update_audio_status(&candidate.id, HistoryAudioStatus::Missing)?;
                    }
                }
                Err(error) => {
                    log::warn!(
                        "Failed to inspect history audio {}: {}",
                        audio_path.display(),
                        error
                    );
                    report.failed_count += 1;
                }
            }
        }

        Ok(report)
    }

    fn insert_history_item_and_transcript(
        tx: &rusqlite::Transaction,
        item: &HistoryItemRecord,
        segments_str: &str,
    ) -> Result<(), DatabaseError> {
        insert_history_item_row(tx, item, &item.transcript_path)?;

        tx.execute(
            "INSERT INTO history_transcripts (history_id, segments) VALUES (?1, ?2)",
            rusqlite::params![item.id, segments_str],
        )?;

        Ok(())
    }

    fn record_history_item_sync(
        tx: &rusqlite::Transaction<'_>,
        item: &HistoryItemRecord,
        now_ms: u64,
    ) -> Result<(), DatabaseError> {
        if item.status != HistoryItemStatus::Complete {
            return Ok(());
        }
        for (field, value) in [
            ("timestamp", serde_json::json!(item.timestamp)),
            ("duration", serde_json::json!(item.duration)),
            ("title", serde_json::json!(item.title)),
            ("previewText", serde_json::json!(item.preview_text)),
            ("icon", serde_json::json!(item.icon)),
            ("kind", serde_json::json!(item.kind.to_string())),
            ("tagIds", serde_json::json!(item.tag_ids)),
            ("deletedAt", serde_json::json!(item.deleted_at)),
        ] {
            record_local_field_change_in_transaction(
                tx,
                SyncEntityKind::HistoryItem,
                &item.id,
                field,
                value,
                now_ms,
            )?;
        }
        Ok(())
    }

    fn record_transcript_sync(
        tx: &rusqlite::Transaction<'_>,
        history_id: &str,
        segments: &Value,
        now_ms: u64,
    ) -> Result<(), DatabaseError> {
        record_local_field_change_in_transaction(
            tx,
            SyncEntityKind::HistoryTranscript,
            history_id,
            "document",
            segments.clone(),
            now_ms,
        )
    }

    fn reconcile_live_drafts(&self) -> Result<(), DatabaseError> {
        let history_dir = self.history_dir();
        #[allow(clippy::type_complexity)]
        let draft_items: Vec<(HistoryItemRecord, Option<Value>)> =
            self.get_db()?.with_connection(|conn| {
                let history_columns =
                    history_select_columns(Some("h"), &[("search_content", "''")]);
                let mut stmt = conn.prepare_cached(&format!(
                    "SELECT {history_columns}, t.segments AS segments
                 FROM history_items h
                 LEFT JOIN history_transcripts t ON h.id = t.history_id
                 WHERE h.status = 'draft' AND h.draft_source = 'live_record'"
                ))?;
                let rows = stmt.query_map([], |row| {
                    let item = map_row_to_item(row)?;
                    let segments: Option<String> = row.get("segments")?;
                    let segments_val = segments.and_then(|s| serde_json::from_str(&s).ok());
                    Ok((item, segments_val))
                })?;
                let mut items = Vec::new();
                for row in rows {
                    items.push(row?);
                }
                Ok(items)
            })?;

        let mut verified_updates = Vec::new();

        for (mut item, segments_val) in draft_items {
            let Some(audio_path) = optional_history_child_path(&history_dir, &item.audio_path)
            else {
                continue;
            };
            let _metadata = match std::fs::metadata(&audio_path) {
                Ok(m) if m.is_file() && m.len() > 0 => m,
                Ok(_) => continue,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(db_file_system_error(
                        FileSystemOperation::Metadata,
                        &audio_path,
                        error,
                    ));
                }
            };

            let Some(segments_val) = segments_val else {
                continue;
            };

            let normalized_transcript = match normalize_history_transcript_segments(segments_val) {
                Ok(t) if !t.segments.is_empty() => t,
                _ => continue,
            };

            item.preview_text = normalized_transcript.preview_text.clone();
            item.search_content = normalized_transcript.search_content.clone();
            let transcript_duration = normalized_transcript
                .segments
                .iter()
                .filter_map(|s| s.end.is_finite().then_some(s.end))
                .fold(0.0, f64::max);
            item.duration = item.duration.max(transcript_duration).max(0.0);
            item.status = HistoryItemStatus::Complete;
            item.draft_source = None;

            let segments_str = serde_json::to_string(&normalized_transcript.segments)?;

            verified_updates.push((item, segments_str));
        }

        if !verified_updates.is_empty() {
            self.get_db()?.with_rw_transaction(|tx| {
                for (item, segments_str) in &verified_updates {
                    let rows_affected = tx.execute(
                        "UPDATE history_items SET preview_text = ?1, search_content = ?2, duration = ?3, status = 'complete', draft_source = NULL WHERE id = ?4",
                        rusqlite::params![item.preview_text, item.search_content, item.duration, item.id]
                    )?;
                    if rows_affected == 0 {
                        continue;
                    }

                    tx.execute(
                        "INSERT OR REPLACE INTO history_transcripts (history_id, segments) VALUES (?1, ?2)",
                        rusqlite::params![item.id, segments_str],
                    )?;
                }
                Ok(())
            })?;
        }

        Ok(())
    }
}


impl<D> SqliteHistoryStore<D>
where
    D: DatabasePort,
{
    fn ensure_ready_inner(&self) -> Result<(), DatabaseError> {
        fs::create_dir_all(self.history_dir()).map_err(|error| {
            db_file_system_error(
                FileSystemOperation::CreateDirectory,
                &self.history_dir(),
                error,
            )
        })?;
        self.cleanup_stale_staged_audio_files()
    }

    fn ensure_ready(&self) -> Result<(), HistoryStoreError> {
        self.with_history_file_lock(|| {
            self.ensure_ready_inner()
                .map_err(HistoryMutationError::from)
        })
        .map_err(history_store_mutation_error)
    }

    fn list_items(&self) -> Result<Vec<HistoryItemRecord>, HistoryStoreError> {
        Ok(self.get_db()?.with_connection(|conn| {
            let columns = history_select_columns(None, &[]);
            let mut stmt = conn.prepare_cached(&format!(
                "SELECT {columns} FROM history_items WHERE deleted_at IS NULL ORDER BY timestamp DESC"
            ))?;
            let rows = stmt.query_map([], map_row_to_item)?;
            let mut items = Vec::new();
            for row in rows {
                items.push(row?);
            }
            Ok(items)
        })?)
    }

    fn list_items_with_reconciled_live_drafts(
        &self,
    ) -> Result<Vec<HistoryItemRecord>, HistoryStoreError> {
        self.reconcile_live_drafts()?;
        Ok(self.get_db()?.with_connection(|conn| {
            let columns = history_select_columns(None, &[]);
            let mut stmt = conn.prepare_cached(&format!(
                "SELECT {columns} FROM history_items WHERE deleted_at IS NULL ORDER BY timestamp DESC"
            ))?;
            let rows = stmt.query_map([], map_row_to_item)?;
            let mut items = Vec::new();
            for row in rows {
                items.push(row?);
            }
            Ok(items)
        })?)
    }

    fn list_items_paginated(
        &self,
        opts: HistoryListOptions,
    ) -> Result<Vec<HistoryItemRecord>, HistoryStoreError> {
        Ok(self.get_db()?.with_connection(|conn| {
            let limit = opts.limit.map(|l| l as i64).unwrap_or(-1);
            let offset = opts.offset.map(|o| o as i64).unwrap_or(0);
            let columns = history_select_columns(None, &[]);
            let mut stmt = conn.prepare_cached(&format!(
                "SELECT {columns} FROM history_items WHERE deleted_at IS NULL ORDER BY timestamp DESC LIMIT ?1 OFFSET ?2"
            ))?;
            let rows = stmt.query_map(rusqlite::params![limit, offset], map_row_to_item)?;
            let mut items = Vec::new();
            for row in rows {
                items.push(row?);
            }
            Ok(items)
        })?)
    }

    fn list_items_with_reconciled_live_drafts_paginated(
        &self,
        opts: HistoryListOptions,
    ) -> Result<Vec<HistoryItemRecord>, HistoryStoreError> {
        self.reconcile_live_drafts()?;
        self.list_items_paginated(opts)
    }

    fn query_workspace(
        &self,
        request: HistoryWorkspaceQueryRequest,
    ) -> Result<HistoryWorkspaceQueryResult, HistoryStoreError> {
        validate_workspace_query_request(&request)?;
        let limit = i64::try_from(request.limit).map_err(|_| {
            HistoryStoreError::InvalidRequest("limit exceeds SQLite range".to_string())
        })?;
        let offset = i64::try_from(request.offset).map_err(|_| {
            HistoryStoreError::InvalidRequest("offset exceeds SQLite range".to_string())
        })?;

        self.ensure_ready()?;
        self.reconcile_live_drafts()?;
        let date_filter_thresholds =
            current_workspace_date_filter_thresholds(self.query_now_ms()?)?;

        Ok(self.get_db()?.with_connection(|conn| {
            let tx = conn.unchecked_transaction()?;
            let item_counts = {
                let untagged = tx.query_row(
                    "SELECT COUNT(*) FROM history_items h
                     WHERE h.deleted_at IS NULL
                       AND NOT EXISTS (SELECT 1 FROM history_item_tags hit WHERE hit.history_id = h.id)",
                    [],
                    |row| row.get::<_, i64>(0),
                )? as usize;
                let trash = tx.query_row(
                    "SELECT COUNT(*) FROM history_items WHERE deleted_at IS NOT NULL",
                    [],
                    |row| row.get::<_, i64>(0),
                )? as usize;
                let mut stmt = tx.prepare_cached(
                    "SELECT hit.tag_id, COUNT(*)
                     FROM history_item_tags hit
                     JOIN history_items h ON h.id = hit.history_id
                     WHERE h.deleted_at IS NULL
                     GROUP BY hit.tag_id",
                )?;
                let mut by_tag_id = BTreeMap::new();
                let rows = stmt.query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
                })?;
                for row in rows {
                    let (tag_id, count) = row?;
                    by_tag_id.insert(tag_id, count);
                }
                HistoryWorkspaceItemCounts {
                    untagged,
                    trash,
                    by_tag_id,
                }
            };

            let summary = {
                let mut conditions = Vec::new();
                let mut params: Vec<Box<dyn ToSql>> = Vec::new();
                add_workspace_scope_condition(&request.scope, &mut conditions, &mut params);
                let where_clause = if conditions.is_empty() {
                    String::new()
                } else {
                    format!("WHERE {}", conditions.join(" AND "))
                };
                let sql = format!(
                    "SELECT COUNT(*), COALESCE(SUM(h.duration), 0.0), MAX(h.timestamp), \
                     SUM(CASE WHEN h.kind = 'recording' THEN 1 ELSE 0 END), \
                     SUM(CASE WHEN h.kind = 'batch' THEN 1 ELSE 0 END) \
                     FROM history_items h {where_clause}"
                );
                let mut stmt = tx.prepare_cached(&sql)?;
                let param_refs = params
                    .iter()
                    .map(|value| value.as_ref())
                    .collect::<Vec<_>>();
                stmt.query_row(param_refs.as_slice(), |row| {
                    let total_items: i64 = row.get(0)?;
                    let total_duration: f64 = row.get(1)?;
                    let latest_timestamp: Option<i64> = row.get(2)?;
                    let recording_count: Option<i64> = row.get(3)?;
                    let batch_count: Option<i64> = row.get(4)?;
                    Ok(HistoryWorkspaceSummary {
                        total_items: total_items as usize,
                        total_duration,
                        latest_timestamp: latest_timestamp.map(|value| value as u64),
                        recording_count: recording_count.unwrap_or_default() as usize,
                        batch_count: batch_count.unwrap_or_default() as usize,
                    })
                })?
            };

            let normalized_query = normalize_workspace_search_text(&request.query).text;

            let filtered_item_count = {
                let (where_clause, params) = workspace_match_query_parts(
                    &request,
                    date_filter_thresholds,
                    &normalized_query,
                );
                let sql = format!("SELECT COUNT(*) FROM history_items h {where_clause}");
                let mut stmt = tx.prepare_cached(&sql)?;
                let param_refs = params
                    .iter()
                    .map(|value| value.as_ref())
                    .collect::<Vec<_>>();
                stmt.query_row(param_refs.as_slice(), |row| row.get::<_, i64>(0))? as usize
            };

            let items = {
                let (where_clause, mut params) = workspace_match_query_parts(
                    &request,
                    date_filter_thresholds,
                    &normalized_query,
                );
                params.push(Box::new(limit));
                params.push(Box::new(offset));
                let sql = format!(
                    "SELECT {} FROM history_items h {where_clause} \
                     ORDER BY {} LIMIT ? OFFSET ?",
                    history_select_columns(Some("h"), &[]),
                    workspace_order_by(request.sort_order),
                );
                let mut stmt = tx.prepare_cached(&sql)?;
                let param_refs = params
                    .iter()
                    .map(|value| value.as_ref())
                    .collect::<Vec<_>>();
                let rows = stmt.query_map(param_refs.as_slice(), map_row_to_item)?;
                let mut items = Vec::new();
                for row in rows {
                    items.push(row?);
                }
                items
            };

            let search_match_by_item_id = items
                .iter()
                .map(|item| {
                    let search_match = if normalized_query.is_empty() {
                        None
                    } else {
                        workspace_item_search_match(item, &normalized_query)
                    };
                    (item.id.clone(), search_match)
                })
                .collect::<BTreeMap<_, _>>();
            let has_more = request.offset.saturating_add(items.len()) < filtered_item_count;

            let result = HistoryWorkspaceQueryResult {
                filtered_items: items,
                search_match_by_item_id,
                filtered_item_count,
                has_more,
                summary,
                item_counts,
            };
            tx.commit()?;
            Ok(result)
        })?)
    }

    fn create_live_draft(
        &self,
        request: HistoryCreateLiveDraftRequest,
    ) -> Result<LiveRecordingDraftResult, HistoryMutationError> {
        self.ensure_ready_inner()?;
        let generated = self.generated_values()?;
        let item = sona_core::history::item_factory::create_live_draft_item(request, generated);
        let audio_absolute_path = self
            .audio_path(&item.audio_path)?
            .to_string_lossy()
            .into_owned();

        self.get_db()?.with_transaction(|tx| {
            Self::insert_history_item_and_transcript(tx, &item, "[]")?;
            Ok(())
        })?;

        Ok(LiveRecordingDraftResult {
            item,
            audio_absolute_path,
        })
    }

    fn complete_live_draft(
        &self,
        history_id: &str,
        segments: Vec<TranscriptSegment>,
        duration: f64,
    ) -> Result<HistoryItemRecord, HistoryMutationError> {
        let normalized_transcript = canonicalize_history_transcript_segments(segments);
        let segments_str = serde_json::to_string(&normalized_transcript.segments)?;

        let now_ms = self.mutation_now_ms()?;
        Ok(self.get_db()?.with_rw_transaction(|tx| {
            let rows_affected = tx.execute(
                "UPDATE history_items
                 SET preview_text = ?1, search_content = ?2, duration = ?3, status = 'complete', draft_source = NULL
                 WHERE id = ?4",
                rusqlite::params![
                    normalized_transcript.preview_text,
                    normalized_transcript.search_content,
                    duration.max(0.0),
                    history_id,
                ],
            )?;
            if rows_affected == 0 {
                return Err(DatabaseError::NotFoundError(format!(
                    "History item not found: {history_id}"
                )));
            }

            tx.execute(
                "INSERT OR REPLACE INTO history_transcripts (history_id, segments) VALUES (?1, ?2)",
                rusqlite::params![history_id, segments_str],
            )?;

            let columns = history_select_columns(None, &[]);
            let mut stmt = tx.prepare_cached(&format!(
                "SELECT {columns} FROM history_items WHERE id = ?1"
            ))?;
            let item = stmt.query_row([history_id], map_row_to_item)?;
            Self::record_history_item_sync(tx, &item, now_ms)?;
            Self::record_transcript_sync(
                tx,
                history_id,
                &serde_json::to_value(&normalized_transcript.segments)?,
                now_ms,
            )?;
            Ok(item)
        })?)
    }

    fn save_recording(
        &self,
        request: HistorySaveRecordingRequest,
    ) -> Result<HistoryItemRecord, HistoryMutationError> {
        self.ensure_ready_inner()?;
        let HistorySaveRecordingRequest {
            segments,
            duration,
            tag_ids,
            audio_bytes,
            native_audio_path,
            audio_extension,
        } = request;

        let normalized_transcript = canonicalize_history_transcript_segments(segments);
        let generated = self.generated_values()?;
        let mut item = sona_core::history::item_factory::create_recording_item(
            generated,
            duration,
            tag_ids,
            audio_extension.as_deref(),
            native_audio_path.as_deref(),
        );
        item.preview_text = normalized_transcript.preview_text;
        item.search_content = normalized_transcript.search_content;

        let target_path = self.audio_path(&item.audio_path)?;
        let staged_audio = match (audio_bytes, native_audio_path) {
            (Some(bytes), _) => self.stage_audio_bytes(target_path, &bytes)?,
            (None, Some(native_path)) => {
                let source_path = PathBuf::from(&native_path);
                require_history_source_file(&source_path, || {
                    format!(
                        "Native recording source file does not exist: {}",
                        source_path.to_string_lossy()
                    )
                })?;
                self.stage_audio_copy(&source_path, target_path)?
            }
            (None, None) => {
                return Err(HistoryMutationError::Internal(
                    "History recording save requires audio bytes or a native audio path."
                        .to_string(),
                ));
            }
        };

        let segments_str = serde_json::to_string(&normalized_transcript.segments)?;

        let promoted = Cell::new(false);
        // The large write/copy into a staging file has already completed.
        // Keep only SQL plus the same-directory rename in this transaction so
        // the final audio path is visible before the DB row commits.
        let save_result: Result<(), DatabaseError> = self.get_db()?.with_transaction(|tx| {
            Self::insert_history_item_and_transcript(tx, &item, &segments_str)?;
            Self::record_history_item_sync(tx, &item, item.timestamp)?;
            Self::record_transcript_sync(
                tx,
                &item.id,
                &serde_json::to_value(&normalized_transcript.segments)?,
                item.timestamp,
            )?;
            staged_audio.promote()?;
            promoted.set(true);
            Ok(())
        });

        if let Err(error) = save_result {
            if promoted.get() {
                staged_audio.cleanup_final();
            } else {
                staged_audio.cleanup_staging();
            }
            return Err(error.into());
        }

        Ok(item)
    }

    fn save_imported_file(
        &self,
        request: HistorySaveImportedFileRequest,
    ) -> Result<HistoryItemRecord, HistoryMutationError> {
        self.ensure_ready_inner()?;
        let HistorySaveImportedFileRequest {
            id,
            source_path,
            segments,
            duration,
            tag_ids,
            converted_source_path,
        } = request;

        let normalized_transcript = canonicalize_history_transcript_segments(segments);
        let generated = self.generated_values()?;
        let imported = sona_core::history::item_factory::create_imported_file_item(
            id,
            source_path,
            converted_source_path,
            duration,
            tag_ids,
            generated,
        );
        let mut item = imported.item;
        item.preview_text = normalized_transcript.preview_text;
        item.search_content = normalized_transcript.search_content;

        let source = PathBuf::from(imported.copy_source_path);
        require_history_source_file(&source, || {
            format!(
                "Imported source file does not exist: {}",
                source.to_string_lossy()
            )
        })?;

        let target_path = self.audio_path(&item.audio_path)?;
        let staged_audio = self.stage_audio_copy(&source, target_path)?;

        let segments_str = serde_json::to_string(&normalized_transcript.segments)?;

        let promoted = Cell::new(false);
        // The large write/copy into a staging file has already completed.
        // Keep only SQL plus the same-directory rename in this transaction so
        // the final audio path is visible before the DB row commits.
        let save_result: Result<(), DatabaseError> = self.get_db()?.with_transaction(|tx| {
            Self::insert_history_item_and_transcript(tx, &item, &segments_str)?;
            Self::record_history_item_sync(tx, &item, item.timestamp)?;
            Self::record_transcript_sync(
                tx,
                &item.id,
                &serde_json::to_value(&normalized_transcript.segments)?,
                item.timestamp,
            )?;
            staged_audio.promote()?;
            promoted.set(true);
            Ok(())
        });

        if let Err(error) = save_result {
            if promoted.get() {
                staged_audio.cleanup_final();
            } else {
                staged_audio.cleanup_staging();
            }
            return Err(error.into());
        }

        Ok(item)
    }

    fn trash_items(&self, ids: &[String], deleted_at: u64) -> Result<(), HistoryMutationError> {
        if ids.is_empty() {
            return Ok(());
        }
        let deleted_at = i64::try_from(deleted_at).map_err(|_| {
            HistoryMutationError::InvalidRequest("deletedAt exceeds SQLite range".to_string())
        })?;
        let now_ms = self.mutation_now_ms()?;
        Ok(self.get_db()?.with_transaction(|tx| {
            let mut statement = tx.prepare_cached(
                "UPDATE history_items SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
            )?;
            for id in ids {
                if statement.execute(rusqlite::params![deleted_at, id])? > 0 {
                    record_local_field_change_in_transaction(
                        tx,
                        SyncEntityKind::HistoryItem,
                        id,
                        "deletedAt",
                        serde_json::json!(deleted_at),
                        now_ms,
                    )?;
                }
            }
            Ok(())
        })?)
    }

    fn restore_items(&self, ids: &[String]) -> Result<(), HistoryMutationError> {
        if ids.is_empty() {
            return Ok(());
        }
        let now_ms = self.mutation_now_ms()?;
        Ok(self.get_db()?.with_transaction(|tx| {
            let mut statement = tx.prepare_cached(
                "UPDATE history_items SET deleted_at = NULL WHERE id = ?1 AND deleted_at IS NOT NULL",
            )?;
            for id in ids {
                if statement.execute([id])? > 0 {
                    record_local_field_change_in_transaction(
                        tx,
                        SyncEntityKind::HistoryItem,
                        id,
                        "deletedAt",
                        serde_json::Value::Null,
                        now_ms,
                    )?;
                }
            }
            Ok(())
        })?)
    }

    fn purge_items(&self, ids: &[String]) -> Result<(), HistoryMutationError> {
        if ids.is_empty() {
            return Ok(());
        }

        let now_ms = self.mutation_now_ms()?;
        let audio_paths = self.get_db()?.with_rw_transaction(|tx| {
            let mut audio_paths = Vec::new();
            let mut existing_ids = Vec::new();
            let mut snapshot_entity_ids = Vec::new();
            {
                let mut stmt = tx.prepare_cached(
                    "SELECT audio_path FROM history_items
                     WHERE id = ?1
                       AND (deleted_at IS NOT NULL OR (status = 'draft' AND draft_source = 'live_record'))",
                )?;
                for id in ids {
                    let mut rows = stmt.query([id])?;
                    if let Some(row) = rows.next()? {
                        let audio_path: String = row.get(0)?;
                        audio_paths.push(audio_path);
                        existing_ids.push(id.clone());
                    }
                }
            }
            {
                let mut stmt = tx.prepare_cached(
                    "SELECT id FROM transcript_snapshots WHERE history_id = ?1 ORDER BY id",
                )?;
                for history_id in &existing_ids {
                    let rows = stmt.query_map([history_id], |row| row.get::<_, String>(0))?;
                    for snapshot_id in rows {
                        snapshot_entity_ids.push(format!("{history_id}::{}", snapshot_id?));
                    }
                }
            }
            {
                let mut stmt = tx.prepare_cached("DELETE FROM history_items WHERE id = ?1")?;
                for id in &existing_ids {
                    stmt.execute([id])?;
                }
            }
            for id in &existing_ids {
                for kind in [
                    SyncEntityKind::HistoryItem,
                    SyncEntityKind::HistoryTranscript,
                    SyncEntityKind::HistorySummary,
                ] {
                    record_local_delete_in_transaction(tx, kind, id, now_ms)?;
                }
            }
            for entity_id in snapshot_entity_ids {
                record_local_delete_in_transaction(
                    tx,
                    SyncEntityKind::TranscriptSnapshot,
                    &entity_id,
                    now_ms,
                )?;
            }
            Ok(audio_paths)
        })?;

        for audio_path_str in audio_paths {
            if let Some(path) = optional_history_child_path(&self.history_dir(), &audio_path_str)
                && let Err(error) = remove_path_if_exists(&path)
            {
                log::warn!(
                    "Failed to remove history audio after permanently deleting DB row {}: {}",
                    path.display(),
                    error
                );
            }
        }

        Ok(())
    }

    fn load_transcript(
        &self,
        history_id: &str,
    ) -> Result<Option<Vec<TranscriptSegment>>, HistoryStoreError> {
        Ok(self.get_db()?.with_connection(|conn| {
            let mut stmt = conn.prepare_cached(
                "SELECT t.segments
                 FROM history_items i
                 JOIN history_transcripts t ON i.id = t.history_id
                 WHERE i.id = ?1",
            )?;
            let mut rows = stmt.query([history_id])?;
            if let Some(row) = rows.next()? {
                let segments_str: String = row.get(0)?;
                let parsed_val: Value = serde_json::from_str(&segments_str)?;
                let normalized = normalize_history_transcript_segments(parsed_val)
                    .map_err(|error| DatabaseError::Internal(error.to_string()))?;
                Ok(Some(normalized.segments))
            } else {
                let exists: bool = conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM history_items WHERE id = ?1)",
                    [history_id],
                    |row| row.get(0),
                )?;
                if exists {
                    Ok(None)
                } else {
                    Err(DatabaseError::NotFoundError(format!(
                        "History item not found: {history_id}"
                    )))
                }
            }
        })?)
    }

    fn update_transcript(
        &self,
        history_id: &str,
        segments: Vec<TranscriptSegment>,
    ) -> Result<HistoryItemRecord, HistoryMutationError> {
        let normalized_transcript = canonicalize_history_transcript_segments(segments);
        let segments_str = serde_json::to_string(&normalized_transcript.segments)?;
        let now_ms = self.mutation_now_ms()?;

        Ok(self.get_db()?.with_rw_transaction(|tx| {
            let rows_affected = tx.execute(
                "UPDATE history_items
                 SET preview_text = ?1, search_content = ?2
                 WHERE id = ?3",
                rusqlite::params![
                    normalized_transcript.preview_text,
                    normalized_transcript.search_content,
                    history_id,
                ],
            )?;
            if rows_affected == 0 {
                return Err(DatabaseError::NotFoundError(format!(
                    "History item not found: {history_id}"
                )));
            }

            tx.execute(
                "INSERT OR REPLACE INTO history_transcripts (history_id, segments) VALUES (?1, ?2)",
                rusqlite::params![history_id, segments_str],
            )?;

            let columns = history_select_columns(None, &[]);
            let mut stmt = tx.prepare_cached(&format!(
                "SELECT {columns} FROM history_items WHERE id = ?1"
            ))?;
            let item = stmt.query_row([history_id], map_row_to_item)?;
            Self::record_history_item_sync(tx, &item, now_ms)?;
            Self::record_transcript_sync(
                tx,
                history_id,
                &serde_json::to_value(&normalized_transcript.segments)?,
                now_ms,
            )?;
            Ok(item)
        })?)
    }

    fn create_transcript_snapshot(
        &self,
        history_id: &str,
        reason: TranscriptSnapshotReason,
        segments: Vec<TranscriptSegment>,
    ) -> Result<TranscriptSnapshotMetadata, HistoryMutationError> {
        validate_id(history_id, "History ID").map_err(DatabaseError::Internal)?;
        let normalized_transcript = canonicalize_history_transcript_segments(segments);
        let parsed_segments = normalized_transcript.segments;

        let created_at = self.mutation_now_ms()?;
        let metadata = TranscriptSnapshotMetadata {
            id: format!("{created_at}-{}", self.ids()?.generate_id()),
            history_id: history_id.to_string(),
            reason,
            created_at,
            segment_count: parsed_segments.len() as u64,
        };

        let reason_str = match reason {
            TranscriptSnapshotReason::Polish => "polish",
            TranscriptSnapshotReason::Translate => "translate",
            TranscriptSnapshotReason::Retranscribe => "retranscribe",
            TranscriptSnapshotReason::Restore => "restore",
        };

        let segments_str = serde_json::to_string(&parsed_segments)?;

        Ok(self.get_db()?.with_rw_transaction(|tx| {
            let previous_ids = {
                let mut statement = tx.prepare_cached(
                    "SELECT id FROM transcript_snapshots WHERE history_id = ?1 ORDER BY id",
                )?;
                statement
                    .query_map([history_id], |row| row.get::<_, String>(0))?
                    .collect::<Result<HashSet<_>, _>>()?
            };
            let rows_affected = tx.execute(
                "INSERT INTO transcript_snapshots (id, history_id, reason, created_at, segment_count, segments)
                 SELECT ?1, ?2, ?3, ?4, ?5, ?6
                 WHERE EXISTS (SELECT 1 FROM history_items WHERE id = ?2)",
                rusqlite::params![
                    metadata.id,
                    metadata.history_id,
                    reason_str,
                    metadata.created_at as i64,
                    metadata.segment_count as i64,
                    segments_str,
                ],
            )?;
            if rows_affected == 0 {
                return Err(DatabaseError::NotFoundError(format!(
                    "History item not found: {history_id}"
                )));
            }

            tx.execute(
                "DELETE FROM transcript_snapshots
                 WHERE history_id = ?1 AND id NOT IN (
                     SELECT id FROM transcript_snapshots
                     WHERE history_id = ?1
                     ORDER BY created_at DESC, id DESC
                     LIMIT ?2
                 )",
                rusqlite::params![
                    history_id,
                    TRANSCRIPT_SNAPSHOT_RETENTION_LIMIT as i64
                ],
            )?;

            let current_ids = {
                let mut statement = tx.prepare_cached(
                    "SELECT id FROM transcript_snapshots WHERE history_id = ?1 ORDER BY id",
                )?;
                statement
                    .query_map([history_id], |row| row.get::<_, String>(0))?
                    .collect::<Result<HashSet<_>, _>>()?
            };
            for removed_id in previous_ids.difference(&current_ids) {
                record_local_delete_in_transaction(
                    tx,
                    SyncEntityKind::TranscriptSnapshot,
                    &format!("{history_id}::{removed_id}"),
                    created_at,
                )?;
            }
            let entity_id = format!("{history_id}::{}", metadata.id);
            for (field, value) in [
                ("document", serde_json::to_value(&parsed_segments)?),
                ("reason", serde_json::json!(reason_str)),
                ("createdAt", serde_json::json!(metadata.created_at)),
                ("segmentCount", serde_json::json!(metadata.segment_count)),
            ] {
                record_local_field_change_in_transaction(
                    tx,
                    SyncEntityKind::TranscriptSnapshot,
                    &entity_id,
                    field,
                    value,
                    created_at,
                )?;
            }
            Ok(metadata.clone())
        })?)
    }

    fn list_transcript_snapshots(
        &self,
        history_id: &str,
    ) -> Result<Vec<TranscriptSnapshotMetadata>, HistoryStoreError> {
        validate_id(history_id, "History ID").map_err(DatabaseError::Internal)?;
        Ok(self.get_db()?.with_connection(|conn| {
            let mut stmt = conn.prepare_cached(
                "SELECT id, reason, created_at, segment_count
                 FROM transcript_snapshots
                 WHERE history_id = ?1
                 ORDER BY created_at DESC, id DESC",
            )?;
            let rows = stmt.query_map([history_id], |row| {
                let id: String = row.get(0)?;
                let reason_str: String = row.get(1)?;
                let created_at: i64 = row.get(2)?;
                let segment_count: i64 = row.get(3)?;

                let reason = match reason_str.as_str() {
                    "polish" => TranscriptSnapshotReason::Polish,
                    "translate" => TranscriptSnapshotReason::Translate,
                    "retranscribe" => TranscriptSnapshotReason::Retranscribe,
                    "restore" => TranscriptSnapshotReason::Restore,
                    _ => TranscriptSnapshotReason::Polish,
                };

                Ok(TranscriptSnapshotMetadata {
                    id,
                    history_id: history_id.to_string(),
                    reason,
                    created_at: created_at as u64,
                    segment_count: segment_count as u64,
                })
            })?;

            let mut list = Vec::new();
            for r in rows {
                list.push(r?);
            }
            Ok(list)
        })?)
    }

    fn load_transcript_snapshot(
        &self,
        history_id: &str,
        snapshot_id: &str,
    ) -> Result<Option<TranscriptSnapshotRecord>, HistoryStoreError> {
        validate_id(history_id, "History ID").map_err(DatabaseError::Internal)?;
        validate_id(snapshot_id, "Snapshot ID").map_err(DatabaseError::Internal)?;

        Ok(self.get_db()?.with_connection(|conn| {
            let mut stmt = conn.prepare_cached(
                "SELECT reason, created_at, segment_count, segments
                 FROM transcript_snapshots
                 WHERE history_id = ?1 AND id = ?2",
            )?;
            let mut rows = stmt.query([history_id, snapshot_id])?;
            if let Some(row) = rows.next()? {
                let reason_str: String = row.get(0)?;
                let created_at: i64 = row.get(1)?;
                let segment_count: i64 = row.get(2)?;
                let segments_str: String = row.get(3)?;

                let reason = match reason_str.as_str() {
                    "polish" => TranscriptSnapshotReason::Polish,
                    "translate" => TranscriptSnapshotReason::Translate,
                    "retranscribe" => TranscriptSnapshotReason::Retranscribe,
                    "restore" => TranscriptSnapshotReason::Restore,
                    _ => TranscriptSnapshotReason::Polish,
                };

                let parsed_val: Value = serde_json::from_str(&segments_str)?;

                let normalized = normalize_history_transcript_segments(parsed_val)
                    .map_err(|error| DatabaseError::Internal(error.to_string()))?;

                let metadata = TranscriptSnapshotMetadata {
                    id: snapshot_id.to_string(),
                    history_id: history_id.to_string(),
                    reason,
                    created_at: created_at as u64,
                    segment_count: segment_count as u64,
                };

                Ok(Some(TranscriptSnapshotRecord {
                    metadata,
                    segments: normalized.segments,
                }))
            } else {
                Ok(None)
            }
        })?)
    }

    fn update_item_meta(
        &self,
        history_id: &str,
        updates: HistoryItemMetaPatch,
    ) -> Result<(), HistoryMutationError> {
        let now_ms = self.mutation_now_ms()?;
        Ok(self.get_db()?.with_rw_transaction(|tx| {
            let columns = history_select_columns(None, &[]);
            let mut stmt = tx.prepare_cached(
                &format!("SELECT {columns} FROM history_items WHERE id = ?1")
            )?;
            let mut item = match stmt.query_row([history_id], map_row_to_item) {
                Ok(item) => item,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    return Err(DatabaseError::NotFoundError(format!(
                        "History item not found: {history_id}"
                    )));
                }
                Err(error) => return Err(DatabaseError::QueryError(error)),
            };

            apply_history_item_updates(&mut item, &updates);

            let kind_str = item.kind.to_string();
            let status_str = item.status.to_string();
            let audio_status_str = item.audio_status.to_string();
            let draft_source_str = item.draft_source.map(|s| s.to_string());
            let deleted_at = item.deleted_at.map(i64::try_from).transpose().map_err(|_| {
                DatabaseError::Internal("History deleted timestamp exceeds SQLite range.".into())
            })?;

            let rows_affected = tx.execute(
                "UPDATE history_items
                 SET id = ?1, timestamp = ?2, duration = ?3, audio_path = ?4, audio_status = ?5, transcript_path = ?6, title = ?7, preview_text = ?8, icon = ?9, kind = ?10, search_content = ?11, deleted_at = ?12, status = ?13, draft_source = ?14
                 WHERE id = ?15",
                rusqlite::params![
                    item.id,
                    item.timestamp as i64,
                    item.duration,
                    item.audio_path,
                    audio_status_str,
                    item.transcript_path,
                    item.title,
                    item.preview_text,
                    item.icon,
                    kind_str,
                    item.search_content,
                    deleted_at,
                    status_str,
                    draft_source_str,
                    history_id,
                ],
            )?;
            if rows_affected == 0 {
                return Err(DatabaseError::NotFoundError(format!(
                    "History item not found: {history_id}"
                )));
            }

            Self::record_history_item_sync(tx, &item, now_ms)?;
            Ok(())
        })?)
    }

    fn update_tag_assignments(
        &self,
        ids: &[String],
        add_tag_ids: &[String],
        remove_tag_ids: &[String],
    ) -> Result<(), HistoryMutationError> {
        if ids.is_empty() {
            return Ok(());
        }

        let now_ms = self.mutation_now_ms()?;
        Ok(self.get_db()?.with_transaction(|tx| {
            let mut add = tx.prepare_cached(
                "INSERT OR IGNORE INTO history_item_tags (history_id, tag_id) VALUES (?1, ?2)",
            )?;
            let mut remove = tx.prepare_cached(
                "DELETE FROM history_item_tags WHERE history_id = ?1 AND tag_id = ?2",
            )?;
            for id in ids {
                for tag_id in add_tag_ids {
                    add.execute(rusqlite::params![id, tag_id])?;
                }
                for tag_id in remove_tag_ids {
                    remove.execute(rusqlite::params![id, tag_id])?;
                }
            }
            drop(add);
            drop(remove);
            for id in ids {
                let tag_ids = load_tag_ids(tx, id)?;
                record_local_field_change_in_transaction(
                    tx,
                    SyncEntityKind::HistoryItem,
                    id,
                    "tagIds",
                    serde_json::json!(tag_ids),
                    now_ms,
                )?;
            }
            Ok(())
        })?)
    }

    fn replace_tag_assignments(
        &self,
        ids: &[String],
        tag_ids: &[String],
    ) -> Result<(), HistoryMutationError> {
        let now_ms = self.mutation_now_ms()?;
        Ok(self.get_db()?.with_transaction(|tx| {
            for id in ids {
                replace_history_item_tags(tx, id, tag_ids)?;
                record_local_field_change_in_transaction(
                    tx,
                    SyncEntityKind::HistoryItem,
                    id,
                    "tagIds",
                    serde_json::json!(tag_ids),
                    now_ms,
                )?;
            }
            Ok(())
        })?)
    }

    fn load_summary(
        &self,
        history_id: &str,
    ) -> Result<Option<HistorySummaryPayload>, HistoryStoreError> {
        validate_id(history_id, "History ID").map_err(DatabaseError::Internal)?;

        Ok(self.get_db()?.with_connection(|conn| {
            let mut stmt =
                conn.prepare_cached("SELECT payload FROM history_summaries WHERE history_id = ?1")?;
            let mut rows = stmt.query([history_id])?;
            if let Some(row) = rows.next()? {
                let payload_str: String = row.get(0)?;
                let payload = serde_json::from_str(&payload_str)?;
                Ok(Some(payload))
            } else {
                Ok(None)
            }
        })?)
    }

    fn save_summary(
        &self,
        history_id: &str,
        summary_payload: HistorySummaryPayload,
    ) -> Result<(), HistoryStoreError> {
        validate_id(history_id, "History ID").map_err(DatabaseError::Internal)?;
        let payload_str = serde_json::to_string(&summary_payload)?;
        let now_ms = self.query_now_ms()?;

        Ok(self.get_db()?.with_rw_transaction(|tx| {
            tx.execute(
                "INSERT OR REPLACE INTO history_summaries (history_id, payload) VALUES (?1, ?2)",
                rusqlite::params![history_id, payload_str],
            )?;
            record_local_field_change_in_transaction(
                tx,
                SyncEntityKind::HistorySummary,
                history_id,
                "document",
                serde_json::to_value(summary_payload)?,
                now_ms,
            )?;
            Ok(())
        })?)
    }

    fn delete_summary(&self, history_id: &str) -> Result<(), HistoryStoreError> {
        validate_id(history_id, "History ID").map_err(DatabaseError::Internal)?;
        let now_ms = self.query_now_ms()?;

        Ok(self.get_db()?.with_rw_transaction(|tx| {
            tx.execute(
                "DELETE FROM history_summaries WHERE history_id = ?1",
                [history_id],
            )?;
            record_local_delete_in_transaction(
                tx,
                SyncEntityKind::HistorySummary,
                history_id,
                now_ms,
            )?;
            Ok(())
        })?)
    }

    fn resolve_audio_path(&self, history_id: &str) -> Result<Option<String>, HistoryStoreError> {
        validate_id(history_id, "History ID").map_err(DatabaseError::Internal)?;

        let audio_record: Option<(String, HistoryAudioStatus)> =
            self.get_db()?.with_connection(|conn| {
                let mut stmt = conn.prepare_cached(
                    "SELECT audio_path, audio_status FROM history_items WHERE id = ?1",
                )?;
                let mut rows = stmt.query([history_id])?;
                if let Some(row) = rows.next()? {
                    let p: String = row.get(0)?;
                    let status_str: String = row.get(1)?;
                    let status = HistoryAudioStatus::from_str(&status_str)
                        .unwrap_or(HistoryAudioStatus::Available);
                    Ok(Some((p, status)))
                } else {
                    Ok(None)
                }
            })?;

        let Some((audio_path_str, audio_status)) = audio_record else {
            return Err(DatabaseError::NotFoundError(format!(
                "History item not found: {history_id}"
            ))
            .into());
        };

        if audio_status == HistoryAudioStatus::Removed {
            return Ok(None);
        }

        let Some(audio_path) = optional_history_child_path(&self.history_dir(), &audio_path_str)
        else {
            if audio_status == HistoryAudioStatus::Available {
                self.update_audio_status(history_id, HistoryAudioStatus::Missing)?;
            }
            return Ok(None);
        };

        match fs::metadata(&audio_path) {
            Ok(metadata) if metadata.is_file() && metadata.len() > 0 => {
                if audio_status == HistoryAudioStatus::Missing {
                    self.update_audio_status(history_id, HistoryAudioStatus::Available)?;
                }
                Ok(Some(audio_path.to_string_lossy().into_owned()))
            }
            Ok(_) => {
                if audio_status == HistoryAudioStatus::Available {
                    self.update_audio_status(history_id, HistoryAudioStatus::Missing)?;
                }
                Ok(None)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if audio_status == HistoryAudioStatus::Available {
                    self.update_audio_status(history_id, HistoryAudioStatus::Missing)?;
                }
                Ok(None)
            }
            Err(error) => Err(HistoryStoreError::FileSystem(FileSystemError::new(
                FileSystemOperation::Metadata,
                &audio_path,
                error.to_string(),
            ))),
        }
    }

    fn preview_audio_cleanup(
        &self,
        request: HistoryAudioCleanupRequest,
    ) -> Result<HistoryAudioCleanupReport, HistoryStoreError> {
        Ok(self.run_audio_cleanup(request, false)?)
    }

    fn cleanup_audio(
        &self,
        request: HistoryAudioCleanupRequest,
    ) -> Result<HistoryAudioCleanupReport, HistoryStoreError> {
        Ok(self.run_audio_cleanup(request, true)?)
    }
}

impl<D> HistoryQueryRepository for SqliteHistoryStore<D>
where
    D: DatabasePort,
{
    fn list_items(&self) -> Result<Vec<HistoryItemRecord>, HistoryStoreError> {
        SqliteHistoryStore::list_items(self)
    }

    fn list_items_with_reconciled_live_drafts(
        &self,
    ) -> Result<Vec<HistoryItemRecord>, HistoryStoreError> {
        SqliteHistoryStore::list_items_with_reconciled_live_drafts(self)
    }

    fn list_items_paginated(
        &self,
        opts: HistoryListOptions,
    ) -> Result<Vec<HistoryItemRecord>, HistoryStoreError> {
        SqliteHistoryStore::list_items_paginated(self, opts)
    }

    fn list_items_with_reconciled_live_drafts_paginated(
        &self,
        opts: HistoryListOptions,
    ) -> Result<Vec<HistoryItemRecord>, HistoryStoreError> {
        SqliteHistoryStore::list_items_with_reconciled_live_drafts_paginated(self, opts)
    }

    fn query_workspace(
        &self,
        request: HistoryWorkspaceQueryRequest,
    ) -> Result<HistoryWorkspaceQueryResult, HistoryStoreError> {
        SqliteHistoryStore::query_workspace(self, request)
    }

    fn load_transcript(
        &self,
        history_id: &str,
    ) -> Result<Option<Vec<TranscriptSegment>>, HistoryStoreError> {
        SqliteHistoryStore::load_transcript(self, history_id)
    }

    fn list_transcript_snapshots(
        &self,
        history_id: &str,
    ) -> Result<Vec<TranscriptSnapshotMetadata>, HistoryStoreError> {
        SqliteHistoryStore::list_transcript_snapshots(self, history_id)
    }

    fn load_transcript_snapshot(
        &self,
        history_id: &str,
        snapshot_id: &str,
    ) -> Result<Option<TranscriptSnapshotRecord>, HistoryStoreError> {
        SqliteHistoryStore::load_transcript_snapshot(self, history_id, snapshot_id)
    }
}

impl<D> HistoryMutationRepository for SqliteHistoryStore<D>
where
    D: DatabasePort,
{
    fn create_live_draft(
        &self,
        request: HistoryCreateLiveDraftRequest,
    ) -> Result<LiveRecordingDraftResult, HistoryMutationError> {
        self.with_history_file_lock(|| SqliteHistoryStore::create_live_draft(self, request))
    }

    fn complete_live_draft(
        &self,
        request: HistoryCompleteLiveDraftRequest,
    ) -> Result<HistoryItemRecord, HistoryMutationError> {
        SqliteHistoryStore::complete_live_draft(
            self,
            &request.history_id,
            request.segments,
            request.duration,
        )
    }

    fn save_recording(
        &self,
        request: HistorySaveRecordingRequest,
    ) -> Result<HistoryItemRecord, HistoryMutationError> {
        self.with_history_file_lock(|| SqliteHistoryStore::save_recording(self, request))
    }

    fn save_imported_file(
        &self,
        request: HistorySaveImportedFileRequest,
    ) -> Result<HistoryItemRecord, HistoryMutationError> {
        self.with_history_file_lock(|| SqliteHistoryStore::save_imported_file(self, request))
    }

    fn trash_items(&self, request: HistoryTrashItemsRequest) -> Result<(), HistoryMutationError> {
        SqliteHistoryStore::trash_items(self, &request.ids, request.deleted_at)
    }

    fn restore_items(
        &self,
        request: HistoryRestoreItemsRequest,
    ) -> Result<(), HistoryMutationError> {
        SqliteHistoryStore::restore_items(self, &request.ids)
    }

    fn purge_items(&self, request: HistoryPurgeItemsRequest) -> Result<(), HistoryMutationError> {
        self.with_history_file_lock(|| SqliteHistoryStore::purge_items(self, &request.ids))
    }

    fn update_transcript(
        &self,
        request: HistoryUpdateTranscriptRequest,
    ) -> Result<HistoryItemRecord, HistoryMutationError> {
        SqliteHistoryStore::update_transcript(self, &request.history_id, request.segments)
    }

    fn create_transcript_snapshot(
        &self,
        request: HistoryCreateTranscriptSnapshotRequest,
    ) -> Result<TranscriptSnapshotMetadata, HistoryMutationError> {
        SqliteHistoryStore::create_transcript_snapshot(
            self,
            &request.history_id,
            request.reason,
            request.segments,
        )
    }

    fn update_item_meta(
        &self,
        request: HistoryUpdateItemMetaRequest,
    ) -> Result<(), HistoryMutationError> {
        SqliteHistoryStore::update_item_meta(self, &request.history_id, request.updates)
    }

    fn update_tag_assignments(
        &self,
        request: HistoryUpdateTagAssignmentsRequest,
    ) -> Result<(), HistoryMutationError> {
        SqliteHistoryStore::update_tag_assignments(
            self,
            &request.ids,
            &request.add_tag_ids,
            &request.remove_tag_ids,
        )
    }

    fn replace_tag_assignments(
        &self,
        request: HistoryReplaceTagAssignmentsRequest,
    ) -> Result<(), HistoryMutationError> {
        SqliteHistoryStore::replace_tag_assignments(self, &request.ids, &request.tag_ids)
    }
}

impl<D> HistoryStore for SqliteHistoryStore<D>
where
    D: DatabasePort,
{
    fn ensure_ready(&self) -> Result<(), HistoryStoreError> {
        SqliteHistoryStore::ensure_ready(self)
    }

    fn load_summary(
        &self,
        history_id: &str,
    ) -> Result<Option<HistorySummaryPayload>, HistoryStoreError> {
        SqliteHistoryStore::load_summary(self, history_id)
    }

    fn save_summary(
        &self,
        history_id: &str,
        summary_payload: HistorySummaryPayload,
    ) -> Result<(), HistoryStoreError> {
        SqliteHistoryStore::save_summary(self, history_id, summary_payload)
    }

    fn delete_summary(&self, history_id: &str) -> Result<(), HistoryStoreError> {
        SqliteHistoryStore::delete_summary(self, history_id)
    }

    fn resolve_audio_path(&self, history_id: &str) -> Result<Option<String>, HistoryStoreError> {
        SqliteHistoryStore::resolve_audio_path(self, history_id)
    }

    fn preview_audio_cleanup(
        &self,
        request: HistoryAudioCleanupRequest,
    ) -> Result<HistoryAudioCleanupReport, HistoryStoreError> {
        SqliteHistoryStore::preview_audio_cleanup(self, request)
    }

    fn cleanup_audio(
        &self,
        request: HistoryAudioCleanupRequest,
    ) -> Result<HistoryAudioCleanupReport, HistoryStoreError> {
        SqliteHistoryStore::cleanup_audio(self, request)
    }
}

#[async_trait::async_trait]
impl<D> sona_core::dashboard::ports::HistoryRepository for SqliteHistoryStore<D>
where
    D: DatabasePort,
{
    async fn list_items(&self) -> Result<Vec<HistoryItemRecord>, DashboardServiceError> {
        HistoryQueryRepository::list_items(self)
            .map_err(|error| DashboardServiceError::HistoryRepository(error.to_string()))
    }

    async fn load_transcript(
        &self,
        history_id: &str,
    ) -> Result<Option<Vec<TranscriptSegment>>, DashboardServiceError> {
        HistoryQueryRepository::load_transcript(self, history_id)
            .map_err(|error| DashboardServiceError::HistoryRepository(error.to_string()))
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
