use crate::DatabaseError;
use crate::history_fs_utils::{
    ensure_json_array_value, ensure_safe_file_name, optional_history_child_path,
    remove_path_if_exists,
};
use crate::ports::Database as DatabasePort;
use serde_json::{Map, Value};
use sona_core::dashboard::error::DashboardServiceError;
use sona_core::history::item_factory::HistoryItemGeneratedValues;
use sona_core::history::mutation_repository::{
    HistoryCompleteLiveDraftRequest, HistoryCreateTranscriptSnapshotRequest,
    HistoryDeleteItemsRequest, HistoryMutationError, HistoryMutationRepository,
    HistoryReassignProjectRequest, HistoryUpdateItemMetaRequest,
    HistoryUpdateProjectAssignmentsRequest, HistoryUpdateTranscriptRequest,
};
use sona_core::history::query_repository::HistoryQueryRepository;
use sona_core::history::transcript_payload::normalize_history_transcript_segments;
use sona_core::history::workspace_query::{
    HistoryWorkspaceDateFilterThresholds, normalize_workspace_search_text,
    validate_workspace_query_request, workspace_item_search_match,
};
use sona_core::history::{
    HistoryAudioCleanupReport, HistoryAudioCleanupRequest, HistoryAudioStatus,
    HistoryBackupSnapshot, HistoryCreateLiveDraftRequest, HistoryDraftSource, HistoryItemKind,
    HistoryItemRecord, HistoryItemStatus, HistoryListOptions, HistorySaveImportedFileRequest,
    HistorySaveRecordingRequest, HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType,
    HistoryWorkspaceItemCounts, HistoryWorkspaceQueryRequest, HistoryWorkspaceQueryResult,
    HistoryWorkspaceScope, HistoryWorkspaceSortOrder, HistoryWorkspaceSummary,
    LiveRecordingDraftResult, TranscriptSnapshotMetadata, TranscriptSnapshotReason,
    TranscriptSnapshotRecord,
};
use sona_core::history_store::{HistoryStore, HistoryStoreError};
use sona_core::transcription::transcript::TranscriptSegment;
use std::cell::Cell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Datelike, Duration, Local, LocalResult, TimeZone};
use rusqlite::types::{ToSql, Type};
use uuid::Uuid;

use fs3::FileExt;

const STAGED_AUDIO_MARKER: &str = ".sona-staging-";
const MILLIS_PER_DAY: u64 = 86_400_000;
const HISTORY_DIR_NAME: &str = "history";
const HISTORY_FILE_LOCK_NAME: &str = ".sona-history.lock";
const TRANSCRIPT_SNAPSHOT_RETENTION_LIMIT: usize = 20;

#[must_use]
pub(crate) struct HistoryFileLockGuard {
    _file: File,
}

pub(crate) fn acquire_history_file_lock(
    app_local_data_dir: &Path,
) -> Result<HistoryFileLockGuard, DatabaseError> {
    fs::create_dir_all(app_local_data_dir)
        .map_err(|error| DatabaseError::Internal(error.to_string()))?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(app_local_data_dir.join(HISTORY_FILE_LOCK_NAME))
        .map_err(|error| DatabaseError::Internal(error.to_string()))?;
    file.lock_exclusive()
        .map_err(|error| DatabaseError::Internal(error.to_string()))?;
    Ok(HistoryFileLockGuard { _file: file })
}

fn current_time_millis() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .map_err(|error| error.to_string())
}

fn new_history_item_generated_values() -> Result<HistoryItemGeneratedValues, String> {
    let timestamp = current_time_millis()?;
    Ok(HistoryItemGeneratedValues {
        fallback_id: Uuid::new_v4().to_string(),
        timestamp,
        recording_title: build_recording_title(timestamp),
    })
}

fn build_recording_title(timestamp: u64) -> String {
    let local_time =
        chrono::DateTime::<Local>::from(UNIX_EPOCH + std::time::Duration::from_millis(timestamp));
    format!("Recording {}", local_time.format("%Y-%m-%d %H-%M-%S"))
}

pub(crate) const HISTORY_ITEM_COLUMNS: [&str; 14] = [
    "id",
    "timestamp",
    "duration",
    "audio_path",
    "audio_status",
    "transcript_path",
    "title",
    "preview_text",
    "icon",
    "kind",
    "search_content",
    "project_id",
    "status",
    "draft_source",
];

fn column_list(columns: &[&str]) -> String {
    columns.join(", ")
}

fn named_param_list(columns: &[&str]) -> String {
    columns
        .iter()
        .map(|column| format!(":{column}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn history_select_columns(alias: Option<&str>, overrides: &[(&str, &str)]) -> String {
    HISTORY_ITEM_COLUMNS
        .iter()
        .map(|column| {
            if let Some((_, expression)) = overrides
                .iter()
                .find(|(override_column, _)| override_column == column)
            {
                return format!("{expression} AS {column}");
            }

            match alias {
                Some(alias) => format!("{alias}.{column} AS {column}"),
                None => (*column).to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn history_insert_sql() -> String {
    format!(
        "INSERT INTO history_items ({}) VALUES ({})",
        column_list(&HISTORY_ITEM_COLUMNS),
        named_param_list(&HISTORY_ITEM_COLUMNS)
    )
}

pub(crate) fn insert_history_item_row(
    tx: &rusqlite::Transaction<'_>,
    item: &HistoryItemRecord,
    transcript_path: &str,
) -> Result<(), DatabaseError> {
    let kind_str = item.kind.to_string();
    let status_str = item.status.to_string();
    let audio_status_str = item.audio_status.to_string();
    let draft_source_str = item.draft_source.map(|s| s.to_string());
    let timestamp = i64::try_from(item.timestamp)
        .map_err(|_| DatabaseError::Internal("History timestamp exceeds SQLite range.".into()))?;

    tx.execute(
        &history_insert_sql(),
        rusqlite::named_params! {
            ":id": &item.id,
            ":timestamp": timestamp,
            ":duration": item.duration,
            ":audio_path": &item.audio_path,
            ":audio_status": audio_status_str,
            ":transcript_path": transcript_path,
            ":title": &item.title,
            ":preview_text": &item.preview_text,
            ":icon": item.icon.as_deref(),
            ":kind": kind_str,
            ":search_content": &item.search_content,
            ":project_id": item.project_id.as_deref(),
            ":status": status_str,
            ":draft_source": draft_source_str.as_deref(),
        },
    )?;

    Ok(())
}

pub(crate) struct PreparedHistoryRestore {
    items: Vec<PreparedHistoryItem>,
    summaries: Vec<(String, String)>,
    snapshots: Vec<PreparedTranscriptSnapshot>,
}

struct PreparedHistoryItem {
    item: HistoryItemRecord,
    transcript_json: String,
}

struct PreparedTranscriptSnapshot {
    id: String,
    history_id: String,
    reason: &'static str,
    created_at: i64,
    segment_count: i64,
    segments_json: String,
}

pub(crate) fn prepare_history_restore(
    snapshot: &HistoryBackupSnapshot,
) -> Result<PreparedHistoryRestore, String> {
    let mut item_ids = HashSet::new();
    for item in &snapshot.items {
        let safe_id = ensure_safe_file_name(&item.id, "History item ID")?;
        if safe_id != item.id || !item_ids.insert(item.id.clone()) {
            return Err(format!(
                "History item ID is invalid or duplicated: {}",
                item.id
            ));
        }
        if item.status != HistoryItemStatus::Complete || item.draft_source.is_some() {
            return Err(format!("Backup history item must be complete: {}", item.id));
        }
        i64::try_from(item.timestamp)
            .map_err(|_| format!("History timestamp exceeds SQLite range: {}", item.id))?;
        if !item.duration.is_finite() || item.duration < 0.0 {
            return Err(format!("History duration is invalid: {}", item.id));
        }
    }

    let mut transcripts = HashMap::new();
    for (file_name, value) in &snapshot.transcript_files {
        let safe_name = ensure_safe_file_name(file_name, "Backup transcript file name")?;
        if safe_name != *file_name {
            return Err("Backup transcript file name is invalid.".to_string());
        }
        let history_id = file_name
            .strip_suffix(".json")
            .filter(|id| item_ids.contains(*id))
            .ok_or_else(|| format!("Backup transcript is orphaned: {file_name}"))?;
        let normalized = normalize_history_transcript_segments(value.clone())?;
        let serialized =
            serde_json::to_string(&normalized.segments).map_err(|error| error.to_string())?;
        if transcripts
            .insert(history_id.to_string(), serialized)
            .is_some()
        {
            return Err(format!("Backup transcript is duplicated: {file_name}"));
        }
    }
    if transcripts.len() != item_ids.len() {
        return Err("Backup must contain exactly one transcript per history item.".to_string());
    }

    let mut prepared_items = Vec::with_capacity(snapshot.items.len());
    for item in &snapshot.items {
        let transcript_json = transcripts
            .remove(&item.id)
            .ok_or_else(|| format!("Backup history item is missing its transcript: {}", item.id))?;
        prepared_items.push(PreparedHistoryItem {
            item: item.clone(),
            transcript_json,
        });
    }

    let mut summaries_by_id = HashMap::new();
    for (history_id, value) in &snapshot.summary_files {
        let safe_id = ensure_safe_file_name(history_id, "Backup summary history ID")?;
        if safe_id != *history_id || !item_ids.contains(history_id) || !value.is_object() {
            return Err(format!("Backup summary is invalid: {history_id}"));
        }
        let payload = serde_json::to_string(value).map_err(|error| error.to_string())?;
        if summaries_by_id
            .insert(history_id.clone(), payload)
            .is_some()
        {
            return Err(format!("Backup summary is duplicated: {history_id}"));
        }
    }
    let summaries = snapshot
        .items
        .iter()
        .filter_map(|item| {
            summaries_by_id
                .remove(&item.id)
                .map(|payload| (item.id.clone(), payload))
        })
        .collect();

    let mut indexes: HashMap<String, Vec<TranscriptSnapshotMetadata>> = HashMap::new();
    let mut records: HashMap<(String, String), TranscriptSnapshotRecord> = HashMap::new();
    for (relative_path, value) in &snapshot.snapshot_files {
        let parts = relative_path.split('/').collect::<Vec<_>>();
        if parts.len() != 3 || parts[0] != "versions" || !item_ids.contains(parts[1]) {
            return Err(format!(
                "Backup transcript snapshot path is invalid: {relative_path}"
            ));
        }
        let history_id = parts[1];
        let safe_history_id = ensure_safe_file_name(history_id, "Snapshot history ID")?;
        if safe_history_id != history_id {
            return Err(format!(
                "Backup transcript snapshot path is invalid: {relative_path}"
            ));
        }
        if parts[2] == "index.json" {
            let metadata: Vec<TranscriptSnapshotMetadata> =
                serde_json::from_value(value.clone()).map_err(|error| error.to_string())?;
            if indexes.insert(history_id.to_string(), metadata).is_some() {
                return Err(format!("Backup snapshot index is invalid: {relative_path}"));
            }
            continue;
        }

        let snapshot_id = parts[2].strip_suffix(".json").ok_or_else(|| {
            format!("Backup transcript snapshot path is invalid: {relative_path}")
        })?;
        let safe_snapshot_id = ensure_safe_file_name(snapshot_id, "Transcript snapshot ID")?;
        if safe_snapshot_id != snapshot_id {
            return Err(format!(
                "Backup transcript snapshot path is invalid: {relative_path}"
            ));
        }
        let record: TranscriptSnapshotRecord =
            serde_json::from_value(value.clone()).map_err(|error| error.to_string())?;
        if records
            .insert((history_id.to_string(), snapshot_id.to_string()), record)
            .is_some()
        {
            return Err(format!(
                "Backup transcript snapshot is duplicated: {relative_path}"
            ));
        }
    }

    let mut indexed_records = HashSet::new();
    let mut prepared_snapshots = Vec::new();
    for item in &snapshot.items {
        let Some(metadata_list) = indexes.remove(&item.id) else {
            continue;
        };
        for metadata in metadata_list {
            let safe_id = ensure_safe_file_name(&metadata.id, "Transcript snapshot ID")?;
            let key = (item.id.clone(), metadata.id.clone());
            if safe_id != metadata.id
                || metadata.history_id != item.id
                || !indexed_records.insert(key.clone())
            {
                return Err(format!(
                    "Backup transcript snapshot metadata is invalid: {}",
                    metadata.id
                ));
            }
            let record = records.remove(&key).ok_or_else(|| {
                format!(
                    "Backup transcript snapshot record is missing: {}",
                    metadata.id
                )
            })?;
            if record.metadata != metadata {
                return Err(format!(
                    "Backup transcript snapshot metadata does not match: {}",
                    metadata.id
                ));
            }
            let normalized = normalize_history_transcript_segments(
                serde_json::to_value(&record.segments).map_err(|error| error.to_string())?,
            )?;
            if metadata.segment_count != normalized.segments.len() as u64 {
                return Err(format!(
                    "Backup transcript snapshot segment count does not match: {}",
                    metadata.id
                ));
            }
            prepared_snapshots.push(PreparedTranscriptSnapshot {
                id: metadata.id,
                history_id: metadata.history_id,
                reason: transcript_snapshot_reason_str(metadata.reason),
                created_at: i64::try_from(metadata.created_at).map_err(|_| {
                    "Transcript snapshot timestamp exceeds SQLite range.".to_string()
                })?,
                segment_count: i64::try_from(metadata.segment_count).map_err(|_| {
                    "Transcript snapshot segment count exceeds SQLite range.".to_string()
                })?,
                segments_json: serde_json::to_string(&normalized.segments)
                    .map_err(|error| error.to_string())?,
            });
        }
    }
    if !indexes.is_empty() || !records.is_empty() {
        return Err("Backup transcript snapshot set contains orphaned entries.".to_string());
    }

    Ok(PreparedHistoryRestore {
        items: prepared_items,
        summaries,
        snapshots: prepared_snapshots,
    })
}

fn transcript_snapshot_reason_str(reason: TranscriptSnapshotReason) -> &'static str {
    match reason {
        TranscriptSnapshotReason::Polish => "polish",
        TranscriptSnapshotReason::Translate => "translate",
        TranscriptSnapshotReason::Retranscribe => "retranscribe",
        TranscriptSnapshotReason::Restore => "restore",
    }
}

pub(crate) fn delete_history_in_transaction(
    tx: &rusqlite::Transaction<'_>,
) -> Result<(), DatabaseError> {
    tx.execute("DELETE FROM transcript_snapshots", [])?;
    tx.execute("DELETE FROM history_summaries", [])?;
    tx.execute("DELETE FROM history_transcripts", [])?;
    tx.execute("DELETE FROM history_items", [])?;
    Ok(())
}

pub(crate) fn insert_history_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    prepared: &PreparedHistoryRestore,
) -> Result<(), DatabaseError> {
    for prepared_item in &prepared.items {
        let transcript_path = format!("{}.json", prepared_item.item.id);
        insert_history_item_row(tx, &prepared_item.item, &transcript_path)?;
        tx.execute(
            "INSERT INTO history_transcripts (history_id, segments) VALUES (?1, ?2)",
            rusqlite::params![prepared_item.item.id, prepared_item.transcript_json],
        )?;
    }
    for (history_id, payload) in &prepared.summaries {
        tx.execute(
            "INSERT INTO history_summaries (history_id, payload) VALUES (?1, ?2)",
            rusqlite::params![history_id, payload],
        )?;
    }
    for snapshot in &prepared.snapshots {
        tx.execute(
            "INSERT INTO transcript_snapshots (
                id, history_id, reason, created_at, segment_count, segments
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                snapshot.id,
                snapshot.history_id,
                snapshot.reason,
                snapshot.created_at,
                snapshot.segment_count,
                snapshot.segments_json,
            ],
        )?;
    }
    Ok(())
}

pub(crate) fn replace_history_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    prepared: &PreparedHistoryRestore,
) -> Result<(), DatabaseError> {
    delete_history_in_transaction(tx)?;
    insert_history_in_transaction(tx, prepared)
}

fn current_workspace_date_filter_thresholds() -> HistoryWorkspaceDateFilterThresholds {
    let now = Local::now();
    let today = local_day_start(now).unwrap_or(now);
    let month = month_threshold(today).unwrap_or_else(|| today - Duration::days(30));

    HistoryWorkspaceDateFilterThresholds {
        today_start_millis: millis_u64(today),
        week_start_millis: millis_u64(today - Duration::days(7)),
        month_start_millis: millis_u64(month),
    }
}

fn local_day_start(value: chrono::DateTime<Local>) -> Option<chrono::DateTime<Local>> {
    match Local.with_ymd_and_hms(value.year(), value.month(), value.day(), 0, 0, 0) {
        LocalResult::Single(value) => Some(value),
        LocalResult::Ambiguous(earliest, _) => Some(earliest),
        LocalResult::None => None,
    }
}

fn month_threshold(today: chrono::DateTime<Local>) -> Option<chrono::DateTime<Local>> {
    let (year, month) = if today.month() == 1 {
        (today.year() - 1, 12)
    } else {
        (today.year(), today.month() - 1)
    };
    match Local.with_ymd_and_hms(
        year,
        month,
        today.day().min(days_in_month(year, month)),
        0,
        0,
        0,
    ) {
        LocalResult::Single(value) => Some(value),
        LocalResult::Ambiguous(earliest, _) => Some(earliest),
        LocalResult::None => None,
    }
}

fn millis_u64(value: chrono::DateTime<Local>) -> u64 {
    value.timestamp_millis().max(0) as u64
}

fn threshold_for_date_filter(
    date_filter: HistoryWorkspaceDateFilter,
    thresholds: HistoryWorkspaceDateFilterThresholds,
) -> Option<i64> {
    let threshold = match date_filter {
        HistoryWorkspaceDateFilter::All => return None,
        HistoryWorkspaceDateFilter::Today => thresholds.today_start_millis,
        HistoryWorkspaceDateFilter::Week => thresholds.week_start_millis,
        HistoryWorkspaceDateFilter::Month => thresholds.month_start_millis,
    };
    Some(threshold.min(i64::MAX as u64) as i64)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let next_month = if month == 12 { 1 } else { month + 1 };
    let next_year = if month == 12 { year + 1 } else { year };
    let Some(first_next_month) = chrono::NaiveDate::from_ymd_opt(next_year, next_month, 1) else {
        return 28;
    };
    (first_next_month - Duration::days(1)).day()
}

fn add_workspace_query_conditions(
    request: &HistoryWorkspaceQueryRequest,
    date_filter_thresholds: HistoryWorkspaceDateFilterThresholds,
    clauses: &mut Vec<String>,
    params: &mut Vec<Box<dyn ToSql>>,
) {
    match &request.scope {
        HistoryWorkspaceScope::All => {}
        HistoryWorkspaceScope::Inbox => {
            clauses.push("h.project_id IS NULL".to_string());
        }
        HistoryWorkspaceScope::Project { project_id } => {
            clauses.push("h.project_id = ?".to_string());
            params.push(Box::new(project_id.clone()));
        }
    }
    match request.filter_type {
        HistoryWorkspaceFilterType::All => {}
        HistoryWorkspaceFilterType::Recording => {
            clauses.push("h.kind = 'recording'".to_string());
        }
        HistoryWorkspaceFilterType::Batch => {
            clauses.push("h.kind = 'batch'".to_string());
        }
    }
    if let Some(threshold) = threshold_for_date_filter(request.date_filter, date_filter_thresholds)
    {
        clauses.push("h.timestamp >= ?".to_string());
        params.push(Box::new(threshold));
    }
}

fn add_workspace_scope_condition(
    scope: &HistoryWorkspaceScope,
    clauses: &mut Vec<String>,
    params: &mut Vec<Box<dyn ToSql>>,
) {
    match scope {
        HistoryWorkspaceScope::All => {}
        HistoryWorkspaceScope::Inbox => clauses.push("h.project_id IS NULL".to_string()),
        HistoryWorkspaceScope::Project { project_id } => {
            clauses.push("h.project_id = ?".to_string());
            params.push(Box::new(project_id.clone()));
        }
    }
}

fn workspace_order_by(sort_order: HistoryWorkspaceSortOrder) -> &'static str {
    match sort_order {
        HistoryWorkspaceSortOrder::Newest => "h.timestamp DESC, h.id ASC",
        HistoryWorkspaceSortOrder::Oldest => "h.timestamp ASC, h.id ASC",
        HistoryWorkspaceSortOrder::DurationDesc => "h.duration DESC, h.timestamp DESC, h.id ASC",
        HistoryWorkspaceSortOrder::DurationAsc => "h.duration ASC, h.timestamp DESC, h.id ASC",
        HistoryWorkspaceSortOrder::TitleAsc => {
            "sona_workspace_title_key(h.title) ASC, h.timestamp DESC, h.id ASC"
        }
    }
}

fn workspace_match_query_parts(
    request: &HistoryWorkspaceQueryRequest,
    date_filter_thresholds: HistoryWorkspaceDateFilterThresholds,
    normalized_query: &str,
) -> (String, Vec<Box<dyn ToSql>>) {
    let mut conditions = Vec::new();
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();

    add_workspace_query_conditions(
        request,
        date_filter_thresholds,
        &mut conditions,
        &mut params,
    );
    if !normalized_query.is_empty() {
        conditions.push(
            "sona_workspace_matches(h.title, h.preview_text, h.search_content, ?) = 1".to_string(),
        );
        params.push(Box::new(normalized_query.to_string()));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };
    (where_clause, params)
}

#[derive(Clone)]
pub struct SqliteHistoryStore<D = crate::Database>
where
    D: DatabasePort,
{
    app_local_data_dir: PathBuf,
    db: Arc<D>,
}

crate::impl_db_repository!(SqliteHistoryStore, app_local_data_dir);

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
        if self.target_path.exists() {
            return Err(DatabaseError::Internal(format!(
                "History audio target already exists: {}",
                self.target_path.to_string_lossy()
            )));
        }
        fs::rename(&self.staging_path, &self.target_path)
            .map_err(|error| DatabaseError::Internal(error.to_string()))
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
            .map_err(|error| HistoryMutationError::Internal(error.to_string()))?;
        operation()
    }

    fn history_dir(&self) -> PathBuf {
        self.app_local_data_dir.join(HISTORY_DIR_NAME)
    }

    fn audio_path(&self, file_name: &str) -> Result<PathBuf, DatabaseError> {
        Ok(self.history_dir().join(
            ensure_safe_file_name(file_name, "History audio path")
                .map_err(DatabaseError::Internal)?,
        ))
    }

    fn staging_audio_path(target_path: &Path) -> Result<PathBuf, DatabaseError> {
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
            Uuid::new_v4()
        )))
    }

    fn stage_audio_bytes(
        &self,
        target_path: PathBuf,
        bytes: &[u8],
    ) -> Result<StagedHistoryAudio, DatabaseError> {
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| DatabaseError::Internal(error.to_string()))?;
        }
        let staging_path = Self::staging_audio_path(&target_path)?;
        let write_result = (|| -> Result<(), DatabaseError> {
            let file = fs::File::create(&staging_path)
                .map_err(|error| DatabaseError::Internal(error.to_string()))?;
            let mut writer = BufWriter::new(file);
            writer
                .write_all(bytes)
                .map_err(|error| DatabaseError::Internal(error.to_string()))?;
            writer
                .flush()
                .map_err(|error| DatabaseError::Internal(error.to_string()))?;
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
            fs::create_dir_all(parent)
                .map_err(|error| DatabaseError::Internal(error.to_string()))?;
        }
        let staging_path = Self::staging_audio_path(&target_path)?;
        let copy_result = fs::copy(source_path, &staging_path)
            .map(|_| ())
            .map_err(|error| DatabaseError::Internal(error.to_string()));
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
        if !history_dir.exists() {
            return Ok(());
        }

        for entry in
            fs::read_dir(&history_dir).map_err(|e| DatabaseError::Internal(e.to_string()))?
        {
            let entry = entry.map_err(|e| DatabaseError::Internal(e.to_string()))?;
            let file_name = entry.file_name();
            if file_name.to_string_lossy().contains(STAGED_AUDIO_MARKER) {
                remove_path_if_exists(&entry.path()).map_err(DatabaseError::Internal)?;
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

    fn audio_cleanup_cutoff(retention_days: Option<u64>) -> Option<i64> {
        let retention_days = retention_days?;
        let retention_millis = retention_days
            .saturating_mul(MILLIS_PER_DAY)
            .min(i64::MAX as u64) as i64;
        Some(
            chrono::Utc::now()
                .timestamp_millis()
                .saturating_sub(retention_millis),
        )
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
    ) -> Result<HistoryAudioCleanupReport, DatabaseError> {
        self.ensure_ready()
            .map_err(|e| DatabaseError::Internal(e.to_string()))?;

        let Some(cutoff_millis) = Self::audio_cleanup_cutoff(request.retention_days) else {
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
                _ => continue,
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

fn validate_id(id: &str, label: &str) -> Result<(), String> {
    ensure_safe_file_name(id, label)?;
    Ok(())
}

fn history_store_mutation_error(error: HistoryMutationError) -> HistoryStoreError {
    match error {
        HistoryMutationError::InvalidRequest(reason) => HistoryStoreError::InvalidRequest(reason),
        HistoryMutationError::Database(reason) => HistoryStoreError::Database(reason),
        HistoryMutationError::Serialization(error) => HistoryStoreError::Serialization(error),
        HistoryMutationError::NotFound(reason) | HistoryMutationError::Internal(reason) => {
            HistoryStoreError::Internal(reason)
        }
    }
}

fn map_row_to_item(row: &rusqlite::Row) -> rusqlite::Result<HistoryItemRecord> {
    let id: String = row.get("id")?;
    let timestamp_val: i64 = row.get("timestamp")?;
    let duration: f64 = row.get("duration")?;
    let audio_path: String = row.get("audio_path")?;
    let audio_status_str: String = row.get("audio_status")?;
    let transcript_path: String = row.get("transcript_path")?;
    let title: String = row.get("title")?;
    let preview_text: String = row.get("preview_text")?;
    let icon: Option<String> = row.get("icon")?;
    let kind_str: String = row.get("kind")?;
    let search_content: String = row.get("search_content")?;
    let project_id: Option<String> = row.get("project_id")?;
    let status_str: String = row.get("status")?;
    let draft_source_str: Option<String> = row.get("draft_source")?;

    let timestamp = checked_history_u64_column(row, "timestamp", timestamp_val)?;
    if !duration.is_finite() || duration < 0.0 {
        return Err(invalid_history_column(
            row,
            "duration",
            Type::Real,
            format!("invalid history duration: {duration}"),
        ));
    }
    let kind = HistoryItemKind::from_str(&kind_str).map_err(|_| {
        invalid_history_column(
            row,
            "kind",
            Type::Text,
            format!("unknown history item kind: {kind_str}"),
        )
    })?;
    let audio_status = HistoryAudioStatus::from_str(&audio_status_str).map_err(|_| {
        invalid_history_column(
            row,
            "audio_status",
            Type::Text,
            format!("unknown history audio status: {audio_status_str}"),
        )
    })?;
    let status = HistoryItemStatus::from_str(&status_str).map_err(|_| {
        invalid_history_column(
            row,
            "status",
            Type::Text,
            format!("unknown history item status: {status_str}"),
        )
    })?;
    let draft_source = draft_source_str
        .map(|value| {
            HistoryDraftSource::from_str(&value).map_err(|_| {
                invalid_history_column(
                    row,
                    "draft_source",
                    Type::Text,
                    format!("unknown history draft source: {value}"),
                )
            })
        })
        .transpose()?;

    Ok(HistoryItemRecord {
        id,
        timestamp,
        duration,
        audio_path,
        audio_status,
        transcript_path,
        title,
        preview_text,
        icon,
        kind,
        search_content,
        project_id,
        status,
        draft_source,
    })
}

fn checked_history_u64_column(
    row: &rusqlite::Row<'_>,
    column: &str,
    value: i64,
) -> rusqlite::Result<u64> {
    let column_index = row.as_ref().column_index(column)?;
    u64::try_from(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(column_index, Type::Integer, Box::new(error))
    })
}

fn invalid_history_column(
    row: &rusqlite::Row<'_>,
    column: &str,
    data_type: Type,
    message: String,
) -> rusqlite::Error {
    let column_index = row.as_ref().column_index(column).unwrap_or_default();
    rusqlite::Error::FromSqlConversionFailure(
        column_index,
        data_type,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message,
        )),
    )
}

fn apply_history_item_updates(item: &mut HistoryItemRecord, updates: &Map<String, Value>) {
    if let Some(id) = updates.get("id").and_then(Value::as_str) {
        item.id = id.to_string();
    }
    if let Some(timestamp) = updates.get("timestamp").and_then(Value::as_u64) {
        item.timestamp = timestamp;
    }
    if let Some(duration) = updates.get("duration").and_then(Value::as_f64) {
        item.duration = duration.max(0.0);
    }
    if let Some(audio_path) = updates.get("audioPath").and_then(Value::as_str) {
        item.audio_path = audio_path.to_string();
    }
    if let Some(audio_status) = updates.get("audioStatus").and_then(Value::as_str) {
        item.audio_status =
            HistoryAudioStatus::from_str(audio_status).unwrap_or(HistoryAudioStatus::Available);
    }
    if let Some(transcript_path) = updates.get("transcriptPath").and_then(Value::as_str) {
        item.transcript_path = transcript_path.to_string();
    }
    if let Some(title) = updates.get("title").and_then(Value::as_str) {
        item.title = title.to_string();
    }
    if let Some(preview_text) = updates.get("previewText").and_then(Value::as_str) {
        item.preview_text = preview_text.to_string();
    }
    if let Some(icon) = updates.get("icon") {
        item.icon = icon.as_str().map(ToString::to_string);
    }
    if let Some(kind) = updates.get("type").and_then(Value::as_str) {
        item.kind = HistoryItemKind::from_str(kind).unwrap_or(HistoryItemKind::Recording);
    }
    if let Some(search_content) = updates.get("searchContent").and_then(Value::as_str) {
        item.search_content = search_content.to_string();
    }
    if let Some(project_id) = updates.get("projectId") {
        item.project_id = project_id.as_str().map(ToString::to_string);
    }
    if let Some(status) = updates.get("status").and_then(Value::as_str) {
        item.status = HistoryItemStatus::from_str(status).unwrap_or(HistoryItemStatus::Complete);
    }
    if let Some(draft_source) = updates.get("draftSource") {
        item.draft_source = draft_source
            .as_str()
            .and_then(|s| HistoryDraftSource::from_str(s).ok());
    }
}

impl<D> SqliteHistoryStore<D>
where
    D: DatabasePort,
{
    fn ensure_ready_inner(&self) -> Result<(), DatabaseError> {
        fs::create_dir_all(self.history_dir())
            .map_err(|e| DatabaseError::Internal(e.to_string()))?;
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
                "SELECT {columns} FROM history_items ORDER BY timestamp DESC"
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
                "SELECT {columns} FROM history_items ORDER BY timestamp DESC"
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
                "SELECT {columns} FROM history_items ORDER BY timestamp DESC LIMIT ?1 OFFSET ?2"
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

        Ok(self.get_db()?.with_connection(|conn| {
            let tx = conn.unchecked_transaction()?;
            let item_counts = {
                let mut stmt = tx.prepare_cached(
                    "SELECT project_id, COUNT(*) FROM history_items GROUP BY project_id",
                )?;
                let mut inbox = 0usize;
                let mut by_project_id = BTreeMap::new();
                let rows = stmt.query_map([], |row| {
                    let pid: Option<String> = row.get(0)?;
                    let cnt: i64 = row.get(1)?;
                    Ok((pid, cnt as usize))
                })?;
                for row in rows {
                    let (pid, cnt) = row?;
                    match pid {
                        None => inbox = cnt,
                        Some(id) => {
                            by_project_id.insert(id, cnt);
                        }
                    }
                }
                HistoryWorkspaceItemCounts {
                    inbox,
                    by_project_id,
                }
            };

            let date_filter_thresholds = current_workspace_date_filter_thresholds();

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
        let generated = new_history_item_generated_values().map_err(DatabaseError::Internal)?;
        let item = sona_core::history::item_factory::create_live_draft_item(request, generated)
            .map_err(DatabaseError::Internal)?;
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
        segments: Value,
        duration: f64,
    ) -> Result<HistoryItemRecord, HistoryMutationError> {
        let normalized_transcript =
            normalize_history_transcript_segments(segments).map_err(DatabaseError::Internal)?;
        let segments_str = serde_json::to_string(&normalized_transcript.segments)?;

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
            project_id,
            audio_bytes,
            native_audio_path,
            audio_extension,
        } = request;

        let normalized_transcript =
            normalize_history_transcript_segments(segments).map_err(DatabaseError::Internal)?;
        let generated = new_history_item_generated_values().map_err(DatabaseError::Internal)?;
        let mut item = sona_core::history::item_factory::create_recording_item(
            generated,
            duration,
            project_id,
            audio_extension.as_deref(),
            native_audio_path.as_deref(),
        )
        .map_err(DatabaseError::Internal)?;
        item.preview_text = normalized_transcript.preview_text;
        item.search_content = normalized_transcript.search_content;

        let target_path = self.audio_path(&item.audio_path)?;
        let staged_audio = match (audio_bytes, native_audio_path) {
            (Some(bytes), _) => self.stage_audio_bytes(target_path, &bytes)?,
            (None, Some(native_path)) => {
                let source_path = PathBuf::from(&native_path);
                if !source_path.is_file() {
                    return Err(HistoryMutationError::Internal(format!(
                        "Native recording source file does not exist: {}",
                        source_path.to_string_lossy()
                    )));
                }
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
            project_id,
            converted_source_path,
        } = request;

        let normalized_transcript =
            normalize_history_transcript_segments(segments).map_err(DatabaseError::Internal)?;
        let generated = new_history_item_generated_values().map_err(DatabaseError::Internal)?;
        let imported = sona_core::history::item_factory::create_imported_file_item(
            id,
            source_path,
            converted_source_path,
            duration,
            project_id,
            generated,
        )
        .map_err(DatabaseError::Internal)?;
        let mut item = imported.item;
        item.preview_text = normalized_transcript.preview_text;
        item.search_content = normalized_transcript.search_content;

        let source = PathBuf::from(imported.copy_source_path);
        if !source.is_file() {
            return Err(HistoryMutationError::Internal(format!(
                "Imported source file does not exist: {}",
                source.to_string_lossy()
            )));
        }

        let target_path = self.audio_path(&item.audio_path)?;
        let staged_audio = self.stage_audio_copy(&source, target_path)?;

        let segments_str = serde_json::to_string(&normalized_transcript.segments)?;

        let promoted = Cell::new(false);
        // The large write/copy into a staging file has already completed.
        // Keep only SQL plus the same-directory rename in this transaction so
        // the final audio path is visible before the DB row commits.
        let save_result: Result<(), DatabaseError> = self.get_db()?.with_transaction(|tx| {
            Self::insert_history_item_and_transcript(tx, &item, &segments_str)?;
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

    fn delete_items(&self, ids: &[String]) -> Result<(), HistoryMutationError> {
        if ids.is_empty() {
            return Ok(());
        }

        let audio_paths = self.get_db()?.with_rw_transaction(|tx| {
            let mut audio_paths = Vec::new();
            let mut stmt =
                tx.prepare_cached("SELECT audio_path FROM history_items WHERE id = ?1")?;
            for id in ids {
                let mut rows = stmt.query([id])?;
                if let Some(row) = rows.next()? {
                    let audio_path: String = row.get(0)?;
                    audio_paths.push(audio_path);
                }
            }

            let mut stmt = tx.prepare_cached("DELETE FROM history_items WHERE id = ?1")?;
            for id in ids {
                stmt.execute([id])?;
            }
            Ok(audio_paths)
        })?;

        for audio_path_str in audio_paths {
            if let Some(path) = optional_history_child_path(&self.history_dir(), &audio_path_str)
                && let Err(error) = remove_path_if_exists(&path)
            {
                log::warn!(
                    "Failed to remove history audio after deleting DB row {}: {}",
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
                    .map_err(DatabaseError::Internal)?;
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
        segments: Value,
    ) -> Result<HistoryItemRecord, HistoryMutationError> {
        let normalized_transcript =
            normalize_history_transcript_segments(segments).map_err(DatabaseError::Internal)?;
        let segments_str = serde_json::to_string(&normalized_transcript.segments)?;

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
            Ok(item)
        })?)
    }

    fn create_transcript_snapshot(
        &self,
        history_id: &str,
        reason: TranscriptSnapshotReason,
        segments: Value,
    ) -> Result<TranscriptSnapshotMetadata, HistoryMutationError> {
        validate_id(history_id, "History ID").map_err(DatabaseError::Internal)?;
        let normalized_transcript =
            normalize_history_transcript_segments(segments).map_err(DatabaseError::Internal)?;
        let parsed_segments = normalized_transcript.segments;

        let created_at = current_time_millis().map_err(DatabaseError::Internal)?;
        let metadata = TranscriptSnapshotMetadata {
            id: format!("{created_at}-{}", Uuid::new_v4()),
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
                    .map_err(DatabaseError::Internal)?;

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
        updates: Value,
    ) -> Result<(), HistoryMutationError> {
        let updates = updates.as_object().ok_or_else(|| {
            DatabaseError::Internal("History item updates must be an object.".to_string())
        })?;

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

            apply_history_item_updates(&mut item, updates);

            let kind_str = item.kind.to_string();
            let status_str = item.status.to_string();
            let audio_status_str = item.audio_status.to_string();
            let draft_source_str = item.draft_source.map(|s| s.to_string());

            let rows_affected = tx.execute(
                "UPDATE history_items
                 SET id = ?1, timestamp = ?2, duration = ?3, audio_path = ?4, audio_status = ?5, transcript_path = ?6, title = ?7, preview_text = ?8, icon = ?9, kind = ?10, search_content = ?11, project_id = ?12, status = ?13, draft_source = ?14
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
                    item.project_id,
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

            Ok(())
        })?)
    }

    fn update_project_assignments(
        &self,
        ids: &[String],
        project_id: Option<String>,
    ) -> Result<(), HistoryMutationError> {
        if ids.is_empty() {
            return Ok(());
        }

        Ok(self.get_db()?.with_transaction(|tx| {
            let mut stmt =
                tx.prepare_cached("UPDATE history_items SET project_id = ?1 WHERE id = ?2")?;
            for id in ids {
                stmt.execute(rusqlite::params![project_id, id])?;
            }
            Ok(())
        })?)
    }

    fn reassign_project(
        &self,
        current_project_id: String,
        next_project_id: Option<String>,
    ) -> Result<(), HistoryMutationError> {
        Ok(self.get_db()?.with_write_connection(|conn| {
            conn.execute(
                "UPDATE history_items SET project_id = ?1 WHERE project_id = ?2",
                rusqlite::params![next_project_id, current_project_id],
            )?;
            Ok(())
        })?)
    }

    fn load_summary(&self, history_id: &str) -> Result<Option<Value>, HistoryStoreError> {
        validate_id(history_id, "History ID").map_err(DatabaseError::Internal)?;

        Ok(self.get_db()?.with_connection(|conn| {
            let mut stmt =
                conn.prepare_cached("SELECT payload FROM history_summaries WHERE history_id = ?1")?;
            let mut rows = stmt.query([history_id])?;
            if let Some(row) = rows.next()? {
                let payload_str: String = row.get(0)?;
                let val: Value = serde_json::from_str(&payload_str)?;
                Ok(Some(val))
            } else {
                Ok(None)
            }
        })?)
    }

    fn save_summary(
        &self,
        history_id: &str,
        summary_payload: Value,
    ) -> Result<(), HistoryStoreError> {
        validate_id(history_id, "History ID").map_err(DatabaseError::Internal)?;

        let summary_payload = crate::history_fs_utils::ensure_json_object_value(
            summary_payload,
            "History summary payload",
        )
        .map_err(DatabaseError::Internal)?;

        let payload_str = serde_json::to_string(&summary_payload)?;

        Ok(self.get_db()?.with_write_connection(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO history_summaries (history_id, payload) VALUES (?1, ?2)",
                rusqlite::params![history_id, payload_str],
            )?;
            Ok(())
        })?)
    }

    fn delete_summary(&self, history_id: &str) -> Result<(), HistoryStoreError> {
        validate_id(history_id, "History ID").map_err(DatabaseError::Internal)?;

        Ok(self.get_db()?.with_write_connection(|conn| {
            conn.execute(
                "DELETE FROM history_summaries WHERE history_id = ?1",
                [history_id],
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
            Err(error) => Err(HistoryStoreError::Internal(error.to_string())),
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

    pub(crate) fn history_snapshot_for_backup(
        &self,
    ) -> Result<HistoryBackupSnapshot, HistoryStoreError> {
        let _history_lock = acquire_history_file_lock(&self.app_local_data_dir)?;
        Ok(self
            .get_db()?
            .with_rw_transaction(load_history_backup_in_transaction)?)
    }
}

pub(crate) fn load_history_backup_in_transaction(
    tx: &rusqlite::Transaction<'_>,
) -> Result<HistoryBackupSnapshot, DatabaseError> {
    let columns = history_select_columns(None, &[]);
    let mut stmt = tx.prepare_cached(&format!(
        "SELECT {columns}
         FROM history_items
         WHERE status != 'draft'
         ORDER BY timestamp DESC, id"
    ))?;
    let rows = stmt.query_map([], map_row_to_item)?;
    let mut items = Vec::new();
    for row in rows {
        let mut item = row?;
        item.status = HistoryItemStatus::Complete;
        item.draft_source = None;
        items.push(item);
    }
    drop(stmt);
    if items.is_empty() {
        return Ok(HistoryBackupSnapshot {
            items,
            transcript_files: Vec::new(),
            summary_files: Vec::new(),
            snapshot_files: Vec::new(),
        });
    }

    let ids = items
        .iter()
        .map(|item| item.id.as_str())
        .collect::<Vec<_>>();
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");

    let transcript_sql = format!(
        "SELECT history_id, segments
         FROM history_transcripts
         WHERE history_id IN ({placeholders})"
    );
    let mut transcript_stmt = tx.prepare_cached(&transcript_sql)?;
    let mut transcript_rows = transcript_stmt.query(rusqlite::params_from_iter(ids.iter()))?;
    let mut transcripts = HashMap::new();
    while let Some(row) = transcript_rows.next()? {
        let history_id: String = row.get(0)?;
        let segments: String = row.get(1)?;
        transcripts.insert(history_id, serde_json::from_str::<Value>(&segments)?);
    }
    drop(transcript_rows);
    drop(transcript_stmt);
    let mut transcript_files = Vec::with_capacity(items.len());
    for item in &items {
        let transcript = transcripts.remove(&item.id).ok_or_else(|| {
            DatabaseError::NotFoundError(format!(
                "History item \"{}\" is missing its transcript file.",
                item.title
            ))
        })?;
        let transcript = ensure_json_array_value(
            transcript,
            &format!("Transcript for history item {}", item.id),
        )
        .map_err(DatabaseError::Internal)?;
        let normalized =
            normalize_history_transcript_segments(transcript).map_err(DatabaseError::Internal)?;
        transcript_files.push((
            format!("{}.json", item.id),
            serde_json::to_value(normalized.segments)?,
        ));
    }

    let summary_sql = format!(
        "SELECT history_id, payload
         FROM history_summaries
         WHERE history_id IN ({placeholders})"
    );
    let mut summary_stmt = tx.prepare_cached(&summary_sql)?;
    let mut summary_rows = summary_stmt.query(rusqlite::params_from_iter(ids.iter()))?;
    let mut summaries = HashMap::new();
    while let Some(row) = summary_rows.next()? {
        let history_id: String = row.get(0)?;
        let payload: String = row.get(1)?;
        summaries.insert(history_id, serde_json::from_str::<Value>(&payload)?);
    }
    drop(summary_rows);
    drop(summary_stmt);
    let summary_files = items
        .iter()
        .filter_map(|item| {
            summaries
                .remove(&item.id)
                .map(|summary| (item.id.clone(), summary))
        })
        .collect();

    let snapshot_sql = format!(
        "SELECT id, history_id, reason, created_at, segment_count, segments
         FROM transcript_snapshots
         WHERE history_id IN ({placeholders})
         ORDER BY created_at DESC, id DESC"
    );
    let mut snapshot_stmt = tx.prepare_cached(&snapshot_sql)?;
    let mut snapshot_rows = snapshot_stmt.query(rusqlite::params_from_iter(ids.iter()))?;
    let mut snapshots_by_item: HashMap<String, Vec<TranscriptSnapshotRecord>> = HashMap::new();
    while let Some(row) = snapshot_rows.next()? {
        let snapshot_id: String = row.get(0)?;
        let history_id: String = row.get(1)?;
        let reason: String = row.get(2)?;
        let created_at: i64 = row.get(3)?;
        let segment_count: i64 = row.get(4)?;
        let segments: String = row.get(5)?;
        let reason = match reason.as_str() {
            "polish" => TranscriptSnapshotReason::Polish,
            "translate" => TranscriptSnapshotReason::Translate,
            "retranscribe" => TranscriptSnapshotReason::Retranscribe,
            "restore" => TranscriptSnapshotReason::Restore,
            value => {
                return Err(DatabaseError::Internal(format!(
                    "Unknown transcript snapshot reason: {value}"
                )));
            }
        };
        let normalized = normalize_history_transcript_segments(serde_json::from_str(&segments)?)
            .map_err(DatabaseError::Internal)?;
        let metadata = TranscriptSnapshotMetadata {
            id: snapshot_id,
            history_id: history_id.clone(),
            reason,
            created_at: u64::try_from(created_at).map_err(|_| {
                DatabaseError::Internal("Transcript snapshot timestamp is negative.".to_string())
            })?,
            segment_count: u64::try_from(segment_count).map_err(|_| {
                DatabaseError::Internal(
                    "Transcript snapshot segment count is negative.".to_string(),
                )
            })?,
        };
        snapshots_by_item
            .entry(history_id)
            .or_default()
            .push(TranscriptSnapshotRecord {
                metadata,
                segments: normalized.segments,
            });
    }
    drop(snapshot_rows);
    drop(snapshot_stmt);

    let mut snapshot_files = Vec::new();
    for item in &items {
        let Some(records) = snapshots_by_item.remove(&item.id) else {
            continue;
        };
        let metadata = records
            .iter()
            .map(|record| record.metadata.clone())
            .collect::<Vec<_>>();
        snapshot_files.push((
            format!("versions/{}/index.json", item.id),
            serde_json::to_value(metadata)?,
        ));
        for record in records {
            snapshot_files.push((
                format!("versions/{}/{}.json", item.id, record.metadata.id),
                serde_json::to_value(record)?,
            ));
        }
    }

    Ok(HistoryBackupSnapshot {
        items,
        transcript_files,
        summary_files,
        snapshot_files,
    })
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

    fn delete_items(&self, request: HistoryDeleteItemsRequest) -> Result<(), HistoryMutationError> {
        self.with_history_file_lock(|| SqliteHistoryStore::delete_items(self, &request.ids))
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

    fn update_project_assignments(
        &self,
        request: HistoryUpdateProjectAssignmentsRequest,
    ) -> Result<(), HistoryMutationError> {
        SqliteHistoryStore::update_project_assignments(self, &request.ids, request.project_id)
    }

    fn reassign_project(
        &self,
        request: HistoryReassignProjectRequest,
    ) -> Result<(), HistoryMutationError> {
        SqliteHistoryStore::reassign_project(
            self,
            request.current_project_id,
            request.next_project_id,
        )
    }
}

impl<D> HistoryStore for SqliteHistoryStore<D>
where
    D: DatabasePort,
{
    fn ensure_ready(&self) -> Result<(), HistoryStoreError> {
        SqliteHistoryStore::ensure_ready(self)
    }

    fn load_summary(&self, history_id: &str) -> Result<Option<Value>, HistoryStoreError> {
        SqliteHistoryStore::load_summary(self, history_id)
    }

    fn save_summary(
        &self,
        history_id: &str,
        summary_payload: Value,
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
mod tests {
    use super::*;
    use crate::Database;
    use serde_json::json;
    use sona_core::history::HistoryWorkspaceSortOrder;
    use std::sync::Arc;
    use tempfile::tempdir;

    fn segment_value(id: &str, text: &str, start: f64, end: f64) -> Value {
        json!({
            "id": id,
            "text": text,
            "start": start,
            "end": end,
            "isFinal": true
        })
    }

    fn set_history_timestamp(store: &SqliteHistoryStore, history_id: &str, timestamp: u64) {
        store
            .get_db()
            .unwrap()
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE history_items SET timestamp = ?1 WHERE id = ?2",
                    rusqlite::params![timestamp as i64, history_id],
                )?;
                Ok(())
            })
            .unwrap();
    }

    fn insert_workspace_item(
        store: &SqliteHistoryStore,
        id: &str,
        timestamp: i64,
        duration: f64,
        title: &str,
        kind: &str,
    ) {
        store
            .get_db()
            .unwrap()
            .with_connection(|conn| {
                conn.execute(
                    "INSERT INTO history_items (
                        id, timestamp, duration, title, preview_text, search_content, kind
                     ) VALUES (?1, ?2, ?3, ?4, ?4, ?4, ?5)",
                    rusqlite::params![id, timestamp, duration, title, kind],
                )?;
                Ok(())
            })
            .unwrap();
    }

    fn saved_audio_path(root: &tempfile::TempDir, item: &HistoryItemRecord) -> PathBuf {
        root.path().join("history").join(&item.audio_path)
    }

    fn table_columns(conn: &rusqlite::Connection, table: &str) -> Vec<String> {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .unwrap();
        let rows = stmt.query_map([], |row| row.get::<_, String>(1)).unwrap();

        rows.collect::<Result<Vec<_>, _>>().unwrap()
    }

    fn assert_query_not_found<T>(result: Result<T, HistoryStoreError>, expected_id: &str) {
        match result {
            Err(HistoryStoreError::Database(message)) => {
                assert!(message.contains(expected_id));
            }
            Ok(_) => panic!("expected NotFoundError for missing history item"),
            Err(error) => panic!("expected NotFoundError, got {error:?}"),
        }
    }

    fn assert_mutation_not_found<T>(result: Result<T, HistoryMutationError>, expected_id: &str) {
        match result {
            Err(HistoryMutationError::NotFound(message)) => {
                assert!(message.contains(expected_id));
            }
            Ok(_) => panic!("expected NotFoundError for missing history item"),
            Err(error) => panic!("expected NotFoundError, got {error:?}"),
        }
    }

    #[test]
    fn history_column_shape_matches_schema() {
        let db = Database::open_in_memory().unwrap();
        db.with_connection(|conn| {
            let mut expected: Vec<String> = HISTORY_ITEM_COLUMNS
                .iter()
                .map(|column| (*column).to_string())
                .collect();
            expected.push("created_at".to_string());

            assert_eq!(table_columns(conn, "history_items"), expected);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn history_row_mapper_reads_columns_by_name() {
        let db = Database::open_in_memory().unwrap();
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO projects (id, name, icon, color, sort_order, created_at, updated_at)
                 VALUES ('project-name-map', 'Mapped Project', 'folder', '', 0, 1000, 1000)",
                [],
            )?;
            conn.execute(
                "INSERT INTO history_items (
                    id, timestamp, duration, audio_path, audio_status, transcript_path,
                    title, preview_text, icon, kind, search_content, project_id, status, draft_source
                )
                VALUES (
                    'history-name-map', 1234, 42.5, 'audio.wav', 'removed', 'history-name-map.json',
                    'Mapped title', 'Mapped preview', 'sparkles', 'batch', 'Mapped search',
                    'project-name-map', 'draft', 'live_record'
                )",
                [],
            )?;

            let mut stmt = conn.prepare(
                "SELECT
                    draft_source AS draft_source,
                    status AS status,
                    project_id AS project_id,
                    search_content AS search_content,
                    kind AS kind,
                    icon AS icon,
                    preview_text AS preview_text,
                    title AS title,
                    transcript_path AS transcript_path,
                    audio_status AS audio_status,
                    audio_path AS audio_path,
                    duration AS duration,
                    timestamp AS timestamp,
                    id AS id
                 FROM history_items
                 WHERE id = 'history-name-map'",
            )?;
            let item = stmt.query_row([], map_row_to_item)?;

            assert_eq!(item.id, "history-name-map");
            assert_eq!(item.timestamp, 1234);
            assert_eq!(item.duration, 42.5);
            assert_eq!(item.audio_path, "audio.wav");
            assert_eq!(item.audio_status, HistoryAudioStatus::Removed);
            assert_eq!(item.transcript_path, "history-name-map.json");
            assert_eq!(item.title, "Mapped title");
            assert_eq!(item.preview_text, "Mapped preview");
            assert_eq!(item.icon.as_deref(), Some("sparkles"));
            assert_eq!(item.kind, HistoryItemKind::Batch);
            assert_eq!(item.search_content, "Mapped search");
            assert_eq!(item.project_id.as_deref(), Some("project-name-map"));
            assert_eq!(item.status, HistoryItemStatus::Draft);
            assert_eq!(item.draft_source, Some(HistoryDraftSource::LiveRecord));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn history_insert_sql_uses_named_params_for_all_columns() {
        let sql = history_insert_sql();

        assert!(!sql.contains('?'));
        for column in HISTORY_ITEM_COLUMNS {
            assert!(
                sql.contains(&format!(":{column}")),
                "missing named param for {column} in {sql}"
            );
        }
        assert_eq!(sql.matches(':').count(), HISTORY_ITEM_COLUMNS.len());
    }

    #[test]
    fn audio_cleanup_removes_only_eligible_audio_and_preserves_text() {
        let root = tempdir().unwrap();
        let db = Arc::new(Database::open_in_memory().unwrap());
        let store = SqliteHistoryStore::new(root.path().to_path_buf(), Arc::clone(&db));
        store.ensure_ready().unwrap();

        let old_item = store
            .save_recording(HistorySaveRecordingRequest {
                segments: json!([segment_value(
                    "seg-old",
                    "Keep the old transcript",
                    0.0,
                    1.0
                )]),
                duration: 1.0,
                project_id: None,
                audio_bytes: Some(vec![1, 2, 3, 4]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();
        let recent_item = store
            .save_recording(HistorySaveRecordingRequest {
                segments: json!([segment_value("seg-new", "Keep recent audio", 0.0, 1.0)]),
                duration: 1.0,
                project_id: None,
                audio_bytes: Some(vec![5, 6]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();

        set_history_timestamp(&store, &old_item.id, 1);

        let preview = store
            .preview_audio_cleanup(HistoryAudioCleanupRequest {
                retention_days: Some(7),
                exclude_history_id: None,
            })
            .unwrap();
        assert_eq!(preview.eligible_count, 1);
        assert_eq!(preview.removed_count, 1);
        assert_eq!(preview.removed_bytes, 4);
        assert_eq!(preview.missing_marked_count, 0);
        assert_eq!(preview.failed_count, 0);
        assert_eq!(preview.skipped_active_count, 0);
        assert!(saved_audio_path(&root, &old_item).exists());

        let report = store
            .cleanup_audio(HistoryAudioCleanupRequest {
                retention_days: Some(7),
                exclude_history_id: None,
            })
            .unwrap();
        assert_eq!(report, preview);
        assert!(!saved_audio_path(&root, &old_item).exists());
        assert!(saved_audio_path(&root, &recent_item).exists());

        let items = store.list_items().unwrap();
        let cleaned = items.iter().find(|item| item.id == old_item.id).unwrap();
        let kept = items.iter().find(|item| item.id == recent_item.id).unwrap();
        assert_eq!(cleaned.audio_status, HistoryAudioStatus::Removed);
        assert_eq!(kept.audio_status, HistoryAudioStatus::Available);

        let transcript = store.load_transcript(&old_item.id).unwrap().unwrap();
        assert_eq!(transcript[0].text, "Keep the old transcript");
    }

    #[test]
    fn audio_cleanup_skips_active_history_and_drafts() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        let active = store
            .save_recording(HistorySaveRecordingRequest {
                segments: json!([segment_value("seg-active", "Active transcript", 0.0, 1.0)]),
                duration: 1.0,
                project_id: None,
                audio_bytes: Some(vec![1]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();
        let draft = store
            .create_live_draft(HistoryCreateLiveDraftRequest {
                id: None,
                audio_extension: "wav".to_string(),
                project_id: None,
                icon: None,
            })
            .unwrap()
            .item;
        std::fs::write(saved_audio_path(&root, &draft), [9, 9, 9]).unwrap();
        set_history_timestamp(&store, &active.id, 1);
        set_history_timestamp(&store, &draft.id, 1);

        let report = store
            .cleanup_audio(HistoryAudioCleanupRequest {
                retention_days: Some(0),
                exclude_history_id: Some(active.id.clone()),
            })
            .unwrap();

        assert_eq!(report.eligible_count, 0);
        assert_eq!(report.removed_count, 0);
        assert_eq!(report.skipped_active_count, 1);
        assert!(saved_audio_path(&root, &active).exists());
        assert!(saved_audio_path(&root, &draft).exists());

        let items = store.list_items().unwrap();
        assert_eq!(
            items
                .iter()
                .find(|item| item.id == active.id)
                .unwrap()
                .audio_status,
            HistoryAudioStatus::Available
        );
        assert_eq!(
            items
                .iter()
                .find(|item| item.id == draft.id)
                .unwrap()
                .audio_status,
            HistoryAudioStatus::Available
        );
    }

    #[test]
    fn audio_cleanup_marks_missing_audio_without_deleting_history() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        let missing = store
            .save_recording(HistorySaveRecordingRequest {
                segments: json!([segment_value(
                    "seg-missing",
                    "Text survives missing audio",
                    0.0,
                    1.0
                )]),
                duration: 1.0,
                project_id: None,
                audio_bytes: Some(vec![1]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();
        set_history_timestamp(&store, &missing.id, 1);
        std::fs::remove_file(saved_audio_path(&root, &missing)).unwrap();

        let report = store
            .cleanup_audio(HistoryAudioCleanupRequest {
                retention_days: Some(0),
                exclude_history_id: None,
            })
            .unwrap();

        assert_eq!(report.eligible_count, 1);
        assert_eq!(report.removed_count, 0);
        assert_eq!(report.removed_bytes, 0);
        assert_eq!(report.missing_marked_count, 1);
        assert_eq!(report.failed_count, 0);

        let item = store
            .list_items()
            .unwrap()
            .into_iter()
            .find(|item| item.id == missing.id)
            .unwrap();
        assert_eq!(item.audio_status, HistoryAudioStatus::Missing);
        let transcript = store.load_transcript(&missing.id).unwrap().unwrap();
        assert_eq!(transcript[0].text, "Text survives missing audio");
    }

    #[cfg(windows)]
    #[test]
    fn audio_cleanup_keeps_available_when_file_deletion_fails() {
        use std::os::windows::fs::OpenOptionsExt;

        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        let item = store
            .save_recording(HistorySaveRecordingRequest {
                segments: json!([segment_value(
                    "seg-fail",
                    "Text survives delete failure",
                    0.0,
                    1.0
                )]),
                duration: 1.0,
                project_id: None,
                audio_bytes: Some(vec![1, 2, 3]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();
        set_history_timestamp(&store, &item.id, 1);

        let audio_path = saved_audio_path(&root, &item);
        let locked_file = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&audio_path)
            .unwrap();

        let report = store
            .cleanup_audio(HistoryAudioCleanupRequest {
                retention_days: Some(0),
                exclude_history_id: None,
            })
            .unwrap();

        assert_eq!(report.eligible_count, 1);
        assert_eq!(report.removed_count, 0);
        assert_eq!(report.removed_bytes, 0);
        assert_eq!(report.missing_marked_count, 0);
        assert_eq!(report.failed_count, 1);
        assert!(audio_path.exists());

        let item = store
            .list_items()
            .unwrap()
            .into_iter()
            .find(|candidate| candidate.id == item.id)
            .unwrap();
        assert_eq!(item.audio_status, HistoryAudioStatus::Available);

        drop(locked_file);
    }

    #[test]
    fn audio_cleanup_disabled_when_retention_is_none() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        let item = store
            .save_recording(HistorySaveRecordingRequest {
                segments: json!([segment_value("seg-keep", "Keep forever", 0.0, 1.0)]),
                duration: 1.0,
                project_id: None,
                audio_bytes: Some(vec![1, 2]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();
        set_history_timestamp(&store, &item.id, 1);

        let report = store
            .cleanup_audio(HistoryAudioCleanupRequest {
                retention_days: None,
                exclude_history_id: None,
            })
            .unwrap();

        assert_eq!(report, HistoryAudioCleanupReport::default());
        assert!(saved_audio_path(&root, &item).exists());
        assert_eq!(
            store.list_items().unwrap()[0].audio_status,
            HistoryAudioStatus::Available
        );
    }

    #[test]
    fn save_imported_file_rolls_back_db_and_cleans_staging_when_promote_fails() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        let source_path = root.path().join("source.wav");
        std::fs::write(&source_path, [1, 2, 3]).unwrap();
        let target_path = root.path().join("history").join("promote-fail.wav");
        std::fs::write(&target_path, [9, 9, 9]).unwrap();

        let result = store.save_imported_file(HistorySaveImportedFileRequest {
            id: Some("promote-fail".to_string()),
            source_path: source_path.to_string_lossy().to_string(),
            converted_source_path: None,
            segments: json!([segment_value("seg-promote", "Promote failure", 0.0, 1.0)]),
            duration: 1.0,
            project_id: None,
        });

        assert!(
            matches!(result, Err(HistoryMutationError::Internal(message)) if message.contains("History audio target already exists"))
        );
        assert_query_not_found(store.load_transcript("promote-fail"), "promote-fail");
        assert_eq!(std::fs::read(&target_path).unwrap(), vec![9, 9, 9]);
        let staged_entries = std::fs::read_dir(root.path().join("history"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .contains(STAGED_AUDIO_MARKER)
            })
            .count();
        assert_eq!(staged_entries, 0);
    }

    #[test]
    fn mutation_readiness_filesystem_failures_remain_internal_errors() {
        let root = tempdir().unwrap();
        let blocked_app_data = root.path().join("app-data-file");
        std::fs::write(&blocked_app_data, [1]).unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(blocked_app_data, db);

        let result = store.create_live_draft(HistoryCreateLiveDraftRequest {
            id: Some("draft-1".to_string()),
            audio_extension: "wav".to_string(),
            project_id: None,
            icon: None,
        });

        assert!(matches!(result, Err(HistoryMutationError::Internal(_))));
    }

    #[test]
    fn history_file_lock_serializes_staging_cleanup_across_store_handles() {
        use std::sync::mpsc;
        use std::thread;
        use std::time::{Duration, Instant};

        let root = tempdir().unwrap();
        let first = SqliteHistoryStore::with_db(
            root.path().to_path_buf(),
            Database::open_in_memory().unwrap(),
        );
        let second = SqliteHistoryStore::with_db(
            root.path().to_path_buf(),
            Database::open_in_memory().unwrap(),
        );
        let history_dir = root.path().join(HISTORY_DIR_NAME);
        let staging_path = history_dir.join(format!("active{STAGED_AUDIO_MARKER}test"));
        let staging_path_for_first = staging_path.clone();
        let (staged_tx, staged_rx) = mpsc::channel();

        thread::scope(|scope| {
            let first_handle = scope.spawn(move || {
                first
                    .with_history_file_lock(|| {
                        std::fs::create_dir_all(&history_dir)
                            .map_err(|error| HistoryMutationError::Internal(error.to_string()))?;
                        std::fs::write(&staging_path_for_first, [1, 2, 3])
                            .map_err(|error| HistoryMutationError::Internal(error.to_string()))?;
                        staged_tx.send(()).unwrap();
                        thread::sleep(Duration::from_millis(150));
                        assert!(staging_path_for_first.exists());
                        Ok(())
                    })
                    .unwrap();
            });
            let second_handle = scope.spawn(move || {
                staged_rx.recv().unwrap();
                let started = Instant::now();
                second
                    .with_history_file_lock(|| {
                        second
                            .cleanup_stale_staged_audio_files()
                            .map_err(HistoryMutationError::from)
                    })
                    .unwrap();
                assert!(started.elapsed() >= Duration::from_millis(100));
            });
            first_handle.join().unwrap();
            second_handle.join().unwrap();
        });

        assert!(!staging_path.exists());
    }

    #[test]
    fn test_sqlite_store_crud() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        // Insert referenced project first to satisfy foreign key constraint
        store
            .get_db()
            .unwrap()
            .with_connection(|conn| {
                conn.execute(
                    "INSERT INTO projects (id, name, icon, color, sort_order, created_at, updated_at)
                     VALUES ('project-1', 'Project One', 'folder', '', 0, 1000, 1000)",
                    [],
                )?;
                Ok(())
            })
            .unwrap();

        // 1. Save recording
        let recording = store
            .save_recording(HistorySaveRecordingRequest {
                segments: json!([segment_value("seg-1", "Hello world", 0.0, 2.0)]),
                duration: 2.0,
                project_id: Some("project-1".to_string()),
                audio_bytes: Some(vec![1, 2, 3]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();

        assert_eq!(recording.preview_text, "Hello world...");
        assert_eq!(recording.search_content, "Hello world");
        assert_eq!(recording.project_id.as_deref(), Some("project-1"));
        assert_eq!(recording.audio_status, HistoryAudioStatus::Available);

        // 2. Load transcript
        let transcript = store.load_transcript(&recording.id).unwrap().unwrap();
        assert_eq!(transcript.len(), 1);
        assert_eq!(transcript[0].text, "Hello world");

        // 3. Update transcript
        let updated = store
            .update_transcript(
                &recording.id,
                json!([segment_value("seg-1", "Hello updated", 0.0, 3.0)]),
            )
            .unwrap();
        assert_eq!(updated.preview_text, "Hello updated...");

        // 4. Update item metadata
        store
            .update_item_meta(&recording.id, json!({ "title": "New Title" }))
            .unwrap();
        let items = store.list_items().unwrap();
        assert_eq!(items[0].title, "New Title");

        // 5. Load summary & save summary
        assert_eq!(store.load_summary(&recording.id).unwrap(), None);
        store
            .save_summary(&recording.id, json!({ "activeTemplateId": "summary-1" }))
            .unwrap();
        assert_eq!(
            store.load_summary(&recording.id).unwrap(),
            Some(json!({ "activeTemplateId": "summary-1" }))
        );

        // 6. Delete item
        store
            .delete_items(std::slice::from_ref(&recording.id))
            .unwrap();
        let items = store.list_items().unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn update_transcript_missing_history_returns_not_found() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        assert_mutation_not_found(
            store.update_transcript(
                "missing-history",
                json!([segment_value("seg-1", "Missing", 0.0, 1.0)]),
            ),
            "missing-history",
        );
    }

    #[test]
    fn complete_live_draft_missing_history_returns_not_found() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        assert_mutation_not_found(
            store.complete_live_draft(
                "missing-history",
                json!([segment_value("seg-1", "Missing", 0.0, 1.0)]),
                1.0,
            ),
            "missing-history",
        );
    }

    #[test]
    fn update_item_meta_missing_history_returns_not_found() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        assert_mutation_not_found(
            store.update_item_meta("missing-history", json!({ "title": "Missing" })),
            "missing-history",
        );
    }

    #[test]
    fn create_transcript_snapshot_missing_history_returns_not_found() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        assert_mutation_not_found(
            store.create_transcript_snapshot(
                "missing-history",
                TranscriptSnapshotReason::Polish,
                json!([segment_value("seg-1", "Missing", 0.0, 1.0)]),
            ),
            "missing-history",
        );
    }

    #[test]
    fn save_imported_file_duplicate_id_does_not_overwrite_existing_audio() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        let first_source = root.path().join("first.wav");
        let second_source = root.path().join("second.wav");
        std::fs::write(&first_source, [1, 2, 3]).unwrap();
        std::fs::write(&second_source, [9, 8, 7]).unwrap();

        let item = store
            .save_imported_file(HistorySaveImportedFileRequest {
                id: Some("import-1".to_string()),
                source_path: first_source.to_string_lossy().to_string(),
                segments: json!([segment_value("seg-1", "Original import", 0.0, 1.0)]),
                duration: 1.0,
                project_id: None,
                converted_source_path: None,
            })
            .unwrap();
        let audio_path = root.path().join("history").join(&item.audio_path);
        assert_eq!(std::fs::read(&audio_path).unwrap(), vec![1, 2, 3]);

        let duplicate = store.save_imported_file(HistorySaveImportedFileRequest {
            id: Some("import-1".to_string()),
            source_path: second_source.to_string_lossy().to_string(),
            segments: json!([segment_value("seg-2", "Duplicate import", 0.0, 1.0)]),
            duration: 1.0,
            project_id: None,
            converted_source_path: None,
        });

        assert!(duplicate.is_err());
        assert_eq!(std::fs::read(&audio_path).unwrap(), vec![1, 2, 3]);
        assert_eq!(store.list_items().unwrap().len(), 1);
    }

    #[test]
    fn resolve_audio_path_marks_available_item_missing_without_deleting_text() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        let recording = store
            .save_recording(HistorySaveRecordingRequest {
                segments: json!([segment_value("seg-1", "Keep text", 0.0, 1.0)]),
                duration: 1.0,
                project_id: None,
                audio_bytes: Some(vec![1]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();

        std::fs::remove_file(root.path().join("history").join(&recording.audio_path)).unwrap();

        assert_eq!(store.resolve_audio_path(&recording.id).unwrap(), None);
        let item = store
            .list_items()
            .unwrap()
            .into_iter()
            .find(|item| item.id == recording.id)
            .unwrap();
        assert_eq!(item.audio_status, HistoryAudioStatus::Missing);
        let transcript = store.load_transcript(&recording.id).unwrap().unwrap();
        assert_eq!(transcript[0].text, "Keep text");
    }

    #[test]
    fn resolve_audio_path_preserves_removed_status_when_file_is_missing() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        let recording = store
            .save_recording(HistorySaveRecordingRequest {
                segments: json!([segment_value("seg-1", "Removed audio text", 0.0, 1.0)]),
                duration: 1.0,
                project_id: None,
                audio_bytes: Some(vec![1]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();
        store
            .update_item_meta(&recording.id, json!({ "audioStatus": "removed" }))
            .unwrap();
        std::fs::remove_file(root.path().join("history").join(&recording.audio_path)).unwrap();

        assert_eq!(store.resolve_audio_path(&recording.id).unwrap(), None);
        let item = store
            .list_items()
            .unwrap()
            .into_iter()
            .find(|item| item.id == recording.id)
            .unwrap();
        assert_eq!(item.audio_status, HistoryAudioStatus::Removed);
    }

    #[test]
    fn ensure_ready_removes_stale_staging_files_without_removing_final_audio() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);

        let history_dir = root.path().join("history");
        std::fs::create_dir_all(&history_dir).unwrap();
        let final_audio = history_dir.join("orphan.wav");
        let staged_audio = history_dir.join("orphan.wav.sona-staging-old");
        std::fs::write(&final_audio, [1]).unwrap();
        std::fs::write(&staged_audio, [2]).unwrap();

        store.ensure_ready().unwrap();

        assert!(final_audio.exists());
        assert!(!staged_audio.exists());
    }

    #[test]
    fn test_sqlite_store_cascades() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        let recording = store
            .save_recording(HistorySaveRecordingRequest {
                segments: json!([segment_value("seg-1", "Hello", 0.0, 1.0)]),
                duration: 1.0,
                project_id: None,
                audio_bytes: Some(vec![1]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();

        // Save summary
        store.save_summary(&recording.id, json!({})).unwrap();
        // Create transcript snapshot
        store
            .create_transcript_snapshot(&recording.id, TranscriptSnapshotReason::Polish, json!([]))
            .unwrap();

        // Verify they exist in DB
        store
            .get_db()
            .unwrap()
            .with_connection(|conn| {
                let t_count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM history_transcripts WHERE history_id = ?1",
                    [&recording.id],
                    |r| r.get(0),
                )?;
                let s_count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM history_summaries WHERE history_id = ?1",
                    [&recording.id],
                    |r| r.get(0),
                )?;
                let snap_count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM transcript_snapshots WHERE history_id = ?1",
                    [&recording.id],
                    |r| r.get(0),
                )?;
                assert_eq!(t_count, 1);
                assert_eq!(s_count, 1);
                assert_eq!(snap_count, 1);
                Ok(())
            })
            .unwrap();

        // Delete parent item
        store
            .delete_items(std::slice::from_ref(&recording.id))
            .unwrap();

        // Verify child tables are automatically pruned by ON DELETE CASCADE
        store
            .get_db()
            .unwrap()
            .with_connection(|conn| {
                let t_count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM history_transcripts WHERE history_id = ?1",
                    [&recording.id],
                    |r| r.get(0),
                )?;
                let s_count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM history_summaries WHERE history_id = ?1",
                    [&recording.id],
                    |r| r.get(0),
                )?;
                let snap_count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM transcript_snapshots WHERE history_id = ?1",
                    [&recording.id],
                    |r| r.get(0),
                )?;
                assert_eq!(t_count, 0);
                assert_eq!(s_count, 0);
                assert_eq!(snap_count, 0);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn test_sqlite_store_workspace_query() {
        use sona_core::history::{
            HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType, HistoryWorkspaceScope,
            HistoryWorkspaceSortOrder,
        };

        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        // Insert referenced project first to satisfy foreign key constraint
        store
            .get_db()
            .unwrap()
            .with_connection(|conn| {
                conn.execute(
                    "INSERT INTO projects (id, name, icon, color, sort_order, created_at, updated_at)
                     VALUES ('project-1', 'Project One', 'folder', '', 0, 1000, 1000)",
                    [],
                )?;
                Ok(())
            })
            .unwrap();

        // Create alpha item
        let _alpha = store
            .save_recording(HistorySaveRecordingRequest {
                segments: json!([segment_value(
                    "seg-1",
                    "Alpha roadmap discussion",
                    0.0,
                    10.0
                )]),
                duration: 10.0,
                project_id: Some("project-1".to_string()),
                audio_bytes: Some(vec![1]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();

        // Create batch item
        let source_file = root.path().join("import.wav");
        std::fs::write(&source_file, [1, 2, 3]).unwrap();
        let _beta = store
            .save_imported_file(HistorySaveImportedFileRequest {
                id: None,
                source_path: source_file.to_string_lossy().to_string(),
                segments: json!([segment_value("seg-2", "Beta notes", 0.0, 20.0)]),
                duration: 20.0,
                project_id: Some("project-1".to_string()),
                converted_source_path: None,
            })
            .unwrap();

        // Query workspace
        let result = store
            .query_workspace(HistoryWorkspaceQueryRequest {
                scope: HistoryWorkspaceScope::Project {
                    project_id: "project-1".to_string(),
                },
                query: "roadmap".to_string(),
                filter_type: HistoryWorkspaceFilterType::Recording,
                date_filter: HistoryWorkspaceDateFilter::All,
                sort_order: HistoryWorkspaceSortOrder::TitleAsc,
                limit: 100,
                offset: 0,
            })
            .unwrap();

        assert_eq!(result.filtered_items.len(), 1);
        assert_eq!(
            result.filtered_items[0].preview_text,
            "Alpha roadmap discussion..."
        );
    }

    #[test]
    fn workspace_query_applies_each_sort_before_pagination() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        insert_workspace_item(&store, "a", 100, 5.0, "Zulu", "recording");
        insert_workspace_item(&store, "b", 300, 2.0, "alpha", "recording");
        insert_workspace_item(&store, "c", 200, 2.0, "Beta", "recording");
        insert_workspace_item(&store, "d", 300, 7.0, "alpha", "recording");
        insert_workspace_item(&store, "batch", 400, 1.0, "Ignored", "batch");

        let cases = [
            (HistoryWorkspaceSortOrder::Newest, ["d", "c"]),
            (HistoryWorkspaceSortOrder::Oldest, ["c", "b"]),
            (HistoryWorkspaceSortOrder::DurationDesc, ["a", "b"]),
            (HistoryWorkspaceSortOrder::DurationAsc, ["c", "a"]),
            (HistoryWorkspaceSortOrder::TitleAsc, ["d", "c"]),
        ];

        for (sort_order, expected_ids) in cases {
            let result = store
                .query_workspace(HistoryWorkspaceQueryRequest {
                    scope: HistoryWorkspaceScope::All,
                    query: String::new(),
                    filter_type: HistoryWorkspaceFilterType::Recording,
                    date_filter: HistoryWorkspaceDateFilter::All,
                    sort_order,
                    limit: 2,
                    offset: 1,
                })
                .unwrap();

            assert_eq!(
                result
                    .filtered_items
                    .iter()
                    .map(|item| item.id.as_str())
                    .collect::<Vec<_>>(),
                expected_ids
            );
            assert_eq!(result.filtered_item_count, 4);
            assert!(result.has_more);
            assert_eq!(result.summary.total_items, 5);
        }
    }

    #[test]
    fn workspace_query_rejects_invalid_pagination() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        for (limit, offset) in [(0, 0), (201, 0), (1, usize::MAX)] {
            let result = store.query_workspace(HistoryWorkspaceQueryRequest {
                scope: HistoryWorkspaceScope::All,
                query: String::new(),
                filter_type: HistoryWorkspaceFilterType::All,
                date_filter: HistoryWorkspaceDateFilter::All,
                sort_order: HistoryWorkspaceSortOrder::Newest,
                limit,
                offset,
            });

            assert!(matches!(result, Err(HistoryStoreError::InvalidRequest(_))));
        }
    }

    #[test]
    fn workspace_query_paginates_exact_search_matches() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        insert_workspace_item(
            &store,
            "newest-miss",
            500,
            1.0,
            "Status update",
            "recording",
        );
        insert_workspace_item(
            &store,
            "first-match",
            400,
            1.0,
            "Roadmap alpha",
            "recording",
        );
        insert_workspace_item(
            &store,
            "middle-miss",
            300,
            1.0,
            "Release notes",
            "recording",
        );
        insert_workspace_item(
            &store,
            "second-match",
            200,
            1.0,
            "Roadmap beta",
            "recording",
        );

        let result = store
            .query_workspace(HistoryWorkspaceQueryRequest {
                scope: HistoryWorkspaceScope::All,
                query: "roadmap".to_string(),
                filter_type: HistoryWorkspaceFilterType::All,
                date_filter: HistoryWorkspaceDateFilter::All,
                sort_order: HistoryWorkspaceSortOrder::Newest,
                limit: 1,
                offset: 1,
            })
            .unwrap();

        assert_eq!(result.filtered_item_count, 2);
        assert!(!result.has_more);
        assert_eq!(result.filtered_items[0].id, "second-match");
        assert_eq!(
            result
                .search_match_by_item_id
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["second-match".to_string()]
        );
    }

    #[test]
    fn workspace_query_search_matches_nfkc_equivalent_text() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        insert_workspace_item(
            &store,
            "full-width",
            100,
            1.0,
            "\u{ff21}\u{ff22}\u{ff23} planning",
            "recording",
        );

        let result = store
            .query_workspace(HistoryWorkspaceQueryRequest {
                scope: HistoryWorkspaceScope::All,
                query: "abc".to_string(),
                filter_type: HistoryWorkspaceFilterType::All,
                date_filter: HistoryWorkspaceDateFilter::All,
                sort_order: HistoryWorkspaceSortOrder::Newest,
                limit: 100,
                offset: 0,
            })
            .unwrap();

        assert_eq!(result.filtered_item_count, 1);
        assert_eq!(result.filtered_items[0].id, "full-width");
        assert!(result.search_match_by_item_id["full-width"].is_some());
    }

    #[test]
    fn workspace_query_builds_metadata_and_page_from_one_read_snapshot() {
        let root = tempdir().unwrap();
        let store = SqliteHistoryStore::with_db(
            root.path().to_path_buf(),
            Database::open(root.path()).unwrap(),
        );
        store.ensure_ready().unwrap();
        insert_workspace_item(
            &store,
            "existing",
            100,
            1.0,
            "snapshotmarker existing",
            "recording",
        );

        let (start_write_tx, start_write_rx) = std::sync::mpsc::channel();
        let (write_done_tx, write_done_rx) = std::sync::mpsc::channel();
        let db_path = root.path().join("sona.db");
        let writer = std::thread::spawn(move || {
            start_write_rx.recv().unwrap();
            let conn = rusqlite::Connection::open(db_path).unwrap();
            conn.execute_batch("PRAGMA busy_timeout=5000;").unwrap();
            conn.execute(
                "INSERT INTO history_items (
                    id, timestamp, duration, title, preview_text, search_content, kind
                 ) VALUES ('concurrent', 200, 1.0, 'snapshotmarker concurrent',
                           'snapshotmarker concurrent', 'snapshotmarker concurrent', 'recording')",
                [],
            )
            .unwrap();
            write_done_tx.send(()).unwrap();
        });

        crate::set_workspace_match_test_hook(
            "snapshotmarker",
            Box::new(move || {
                start_write_tx.send(()).unwrap();
                write_done_rx.recv().unwrap();
            }),
        );

        let result = store
            .query_workspace(HistoryWorkspaceQueryRequest {
                scope: HistoryWorkspaceScope::All,
                query: "snapshotmarker".to_string(),
                filter_type: HistoryWorkspaceFilterType::All,
                date_filter: HistoryWorkspaceDateFilter::All,
                sort_order: HistoryWorkspaceSortOrder::Newest,
                limit: 100,
                offset: 0,
            })
            .unwrap();
        writer.join().unwrap();

        assert_eq!(result.filtered_item_count, 1);
        assert_eq!(result.filtered_items.len(), 1);
        assert_eq!(result.filtered_items[0].id, "existing");
        assert_eq!(result.summary.total_items, 1);
        assert_eq!(result.item_counts.inbox, 1);
    }

    #[test]
    fn test_workspace_query_with_reconciliation() {
        use sona_core::history::{
            HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType, HistoryWorkspaceScope,
            HistoryWorkspaceSortOrder,
        };

        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        // Insert referenced project first to satisfy foreign key constraint
        store
            .get_db()
            .unwrap()
            .with_connection(|conn| {
                conn.execute(
                    "INSERT INTO projects (id, name, icon, color, sort_order, created_at, updated_at)
                     VALUES ('project-1', 'Project One', 'folder', '', 0, 1000, 1000)",
                    [],
                )?;
                Ok(())
            })
            .unwrap();

        // 1. Create a live draft item
        let draft_res = store
            .create_live_draft(HistoryCreateLiveDraftRequest {
                id: None,
                audio_extension: "wav".to_string(),
                project_id: Some("project-1".to_string()),
                icon: None,
            })
            .unwrap();

        // At this point, the draft is in the database with status = 'draft' and draft_source = 'live_record'.
        // But since there is no audio file yet (or segments in transcripts), reconcile_live_drafts should skip it.
        let result = store
            .query_workspace(HistoryWorkspaceQueryRequest {
                scope: HistoryWorkspaceScope::All,
                query: "".to_string(),
                filter_type: HistoryWorkspaceFilterType::All,
                date_filter: HistoryWorkspaceDateFilter::All,
                sort_order: HistoryWorkspaceSortOrder::Newest,
                limit: 100,
                offset: 0,
            })
            .unwrap();
        assert_eq!(result.filtered_items.len(), 1);
        assert_eq!(result.filtered_items[0].status, HistoryItemStatus::Draft);

        // 2. Write the audio file so reconcile_live_drafts will process it
        let audio_path = root.path().join("history").join(&draft_res.item.audio_path);
        if let Some(parent) = audio_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&audio_path, [1, 2, 3]).unwrap();

        // 3. Insert some dummy transcript segments into history_transcripts
        let segments_str = serde_json::to_string(&json!([
            {
                "id": "seg-1",
                "text": "Hello world from reconciled draft",
                "start": 0.0,
                "end": 5.0
            }
        ]))
        .unwrap();
        store
            .get_db()
            .unwrap()
            .with_transaction(|tx| {
                tx.execute(
                "INSERT OR REPLACE INTO history_transcripts (history_id, segments) VALUES (?1, ?2)",
                rusqlite::params![draft_res.item.id, segments_str]
            )?;
                Ok(())
            })
            .unwrap();

        // 4. Query workspace again, which should trigger reconciliation
        let result = store
            .query_workspace(HistoryWorkspaceQueryRequest {
                scope: HistoryWorkspaceScope::All,
                query: "reconciled".to_string(),
                filter_type: HistoryWorkspaceFilterType::All,
                date_filter: HistoryWorkspaceDateFilter::All,
                sort_order: HistoryWorkspaceSortOrder::Newest,
                limit: 100,
                offset: 0,
            })
            .unwrap();

        assert_eq!(result.filtered_items.len(), 1);
        assert_eq!(result.filtered_items[0].status, HistoryItemStatus::Complete);
        assert_eq!(
            result.filtered_items[0].preview_text,
            "Hello world from reconciled draft..."
        );
    }

    #[test]
    fn test_sqlite_store_fts_workspace_query() {
        use sona_core::history::{
            HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType, HistoryWorkspaceScope,
            HistoryWorkspaceSortOrder,
        };

        let root = tempfile::TempDir::new().unwrap();
        let store = SqliteHistoryStore::with_db(
            root.path().to_path_buf(),
            Database::open_in_memory().unwrap(),
        );
        store.ensure_ready().unwrap();

        // Save test item with Chinese and English text
        let item = store
            .save_recording(HistorySaveRecordingRequest {
                segments: serde_json::json!([
                    segment_value("seg-1", "你好世界，这是一个测试", 0.0, 2.0),
                    segment_value("seg-2", "Fuzzy matching should be fast", 2.0, 4.0)
                ]),
                duration: 4.0,
                project_id: None,
                audio_bytes: Some(vec![1, 2, 3]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();

        // Save second test item with full-width Chinese punctuation for punctuation matching
        let item_punc = store
            .save_recording(HistorySaveRecordingRequest {
                segments: serde_json::json!([segment_value(
                    "seg-3",
                    "你好，世界，这是一个测试",
                    0.0,
                    2.0
                )]),
                duration: 2.0,
                project_id: None,
                audio_bytes: Some(vec![1, 2, 3]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();

        // 1. Test Chinese search match
        let req_zh = HistoryWorkspaceQueryRequest {
            scope: HistoryWorkspaceScope::All,
            query: "你好世界".to_string(),
            filter_type: HistoryWorkspaceFilterType::All,
            date_filter: HistoryWorkspaceDateFilter::All,
            sort_order: HistoryWorkspaceSortOrder::Newest,
            limit: 100,
            offset: 0,
        };
        let res_zh = store.query_workspace(req_zh).unwrap();
        assert_eq!(res_zh.filtered_items.len(), 1);
        assert_eq!(res_zh.filtered_items[0].id, item.id);
        // New assertions verifying optimized behavior:
        assert_eq!(res_zh.filtered_item_count, 1);
        assert_eq!(res_zh.summary.total_items, 2); // Summary correctly counts all items in the scope

        // 2. Test fuzzy/substring match
        let req_fuzzy = HistoryWorkspaceQueryRequest {
            scope: HistoryWorkspaceScope::All,
            query: "fuzzy".to_string(),
            filter_type: HistoryWorkspaceFilterType::All,
            date_filter: HistoryWorkspaceDateFilter::All,
            sort_order: HistoryWorkspaceSortOrder::Newest,
            limit: 100,
            offset: 0,
        };
        let res_fuzzy = store.query_workspace(req_fuzzy).unwrap();
        assert_eq!(res_fuzzy.filtered_items.len(), 1);
        assert_eq!(res_fuzzy.filtered_items[0].id, item.id);

        // 3. Test non-matching query
        let req_none = HistoryWorkspaceQueryRequest {
            scope: HistoryWorkspaceScope::All,
            query: "unrelated".to_string(),
            filter_type: HistoryWorkspaceFilterType::All,
            date_filter: HistoryWorkspaceDateFilter::All,
            sort_order: HistoryWorkspaceSortOrder::Newest,
            limit: 100,
            offset: 0,
        };
        let res_none = store.query_workspace(req_none).unwrap();
        assert_eq!(res_none.filtered_items.len(), 0);

        // 4. Test queries with full-width Chinese punctuation successfully matching
        let req_punc1 = HistoryWorkspaceQueryRequest {
            scope: HistoryWorkspaceScope::All,
            query: "你好，世界".to_string(),
            filter_type: HistoryWorkspaceFilterType::All,
            date_filter: HistoryWorkspaceDateFilter::All,
            sort_order: HistoryWorkspaceSortOrder::Newest,
            limit: 100,
            offset: 0,
        };
        let res_punc1 = store.query_workspace(req_punc1).unwrap();
        assert_eq!(res_punc1.filtered_items.len(), 1);
        assert_eq!(res_punc1.filtered_items[0].id, item_punc.id);

        let req_punc2 = HistoryWorkspaceQueryRequest {
            scope: HistoryWorkspaceScope::All,
            query: "你好、世界".to_string(),
            filter_type: HistoryWorkspaceFilterType::All,
            date_filter: HistoryWorkspaceDateFilter::All,
            sort_order: HistoryWorkspaceSortOrder::Newest,
            limit: 100,
            offset: 0,
        };
        let res_punc2 = store.query_workspace(req_punc2).unwrap();
        assert_eq!(res_punc2.filtered_items.len(), 1);
        assert_eq!(res_punc2.filtered_items[0].id, item_punc.id);

        // 5. Test short Chinese query (6 bytes / 2 chars): valid for
        //    byte-level trigram — FTS should be used for both items
        let req_short_zh = HistoryWorkspaceQueryRequest {
            scope: HistoryWorkspaceScope::All,
            query: "你好".to_string(), // 2 chars, 6 bytes
            filter_type: HistoryWorkspaceFilterType::All,
            date_filter: HistoryWorkspaceDateFilter::All,
            sort_order: HistoryWorkspaceSortOrder::Newest,
            limit: 100,
            offset: 0,
        };
        let res_short_zh = store.query_workspace(req_short_zh).unwrap();
        let short_zh_ids: std::collections::HashSet<&str> = res_short_zh
            .filtered_items
            .iter()
            .map(|i| i.id.as_str())
            .collect();
        assert!(short_zh_ids.contains(item.id.as_str()));
        assert!(short_zh_ids.contains(item_punc.id.as_str()));

        // 6. Test short ASCII query (< 3 bytes): must fall back to in-memory
        //    matching on search_content loaded from DB
        let req_short_en = HistoryWorkspaceQueryRequest {
            scope: HistoryWorkspaceScope::All,
            query: "fa".to_string(), // 2 bytes — too short for trigram
            filter_type: HistoryWorkspaceFilterType::All,
            date_filter: HistoryWorkspaceDateFilter::All,
            sort_order: HistoryWorkspaceSortOrder::Newest,
            limit: 100,
            offset: 0,
        };
        let res_short_en = store.query_workspace(req_short_en).unwrap();
        assert_eq!(res_short_en.filtered_items.len(), 1);
        assert_eq!(res_short_en.filtered_items[0].id, item.id);
    }
}
