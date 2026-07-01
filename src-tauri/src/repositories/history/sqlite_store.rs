use crate::core::database::Database;
use crate::core::history_store::HistoryStore;
use crate::integrations::asr::TranscriptSegment;
use crate::repositories::history::{
    HistoryBackupSnapshot, HistoryCreateLiveDraftRequest, HistoryDraftSource, HistoryItemKind,
    HistoryItemRecord, HistoryItemStatus, HistorySaveImportedFileRequest,
    HistorySaveRecordingRequest, HistoryWorkspaceQueryRequest, HistoryWorkspaceQueryResult,
    LiveRecordingDraftResult, TranscriptSnapshotMetadata, TranscriptSnapshotReason,
    TranscriptSnapshotRecord,
};
use serde_json::{Map, Value};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use super::fs_utils::{
    ensure_json_array_value, ensure_safe_file_name, optional_history_child_path,
};
use super::transcript_payload::normalize_history_transcript_segments;

#[derive(Clone)]
pub struct SqliteHistoryStore {
    app_local_data_dir: PathBuf,
    db: Option<Arc<Database>>,
}

impl SqliteHistoryStore {
    pub fn new(app_local_data_dir: PathBuf) -> Self {
        Self {
            app_local_data_dir,
            db: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_db(app_local_data_dir: PathBuf, db: Database) -> Self {
        Self {
            app_local_data_dir,
            db: Some(Arc::new(db)),
        }
    }

    fn get_db(&self) -> &Database {
        if let Some(ref db) = self.db {
            db
        } else {
            Database::global()
        }
    }

    fn history_dir(&self) -> PathBuf {
        self.app_local_data_dir
            .join(crate::repositories::history::HISTORY_DIR_NAME)
    }

    fn audio_path(&self, file_name: &str) -> Result<PathBuf, String> {
        Ok(self
            .history_dir()
            .join(ensure_safe_file_name(file_name, "History audio path")?))
    }

    fn enum_to_kind_str(kind: HistoryItemKind) -> &'static str {
        match kind {
            HistoryItemKind::Batch => "batch",
            HistoryItemKind::Recording => "recording",
        }
    }

    fn enum_to_status_str(status: HistoryItemStatus) -> &'static str {
        match status {
            HistoryItemStatus::Draft => "draft",
            HistoryItemStatus::Complete => "complete",
        }
    }

    fn enum_to_draft_source_str(source: HistoryDraftSource) -> &'static str {
        match source {
            HistoryDraftSource::LiveRecord => "live_record",
        }
    }

    fn insert_history_item_and_transcript(
        tx: &rusqlite::Transaction,
        item: &HistoryItemRecord,
        segments_str: &str,
    ) -> Result<(), rusqlite::Error> {
        let kind_str = Self::enum_to_kind_str(item.kind);
        let status_str = Self::enum_to_status_str(item.status);
        let draft_source_str = item.draft_source.map(Self::enum_to_draft_source_str);

        tx.execute(
            "INSERT INTO history_items (id, timestamp, duration, audio_path, transcript_path, title, preview_text, icon, kind, search_content, project_id, status, draft_source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            rusqlite::params![
                item.id,
                item.timestamp as i64,
                item.duration,
                item.audio_path,
                item.transcript_path,
                item.title,
                item.preview_text,
                item.icon,
                kind_str,
                item.search_content,
                item.project_id,
                status_str,
                draft_source_str,
            ],
        )?;

        tx.execute(
            "INSERT INTO history_transcripts (history_id, segments) VALUES (?1, ?2)",
            rusqlite::params![item.id, segments_str],
        )?;

        Ok(())
    }
}

fn validate_id(id: &str, label: &str) -> Result<(), String> {
    ensure_safe_file_name(id, label)?;
    Ok(())
}

fn map_row_to_item(row: &rusqlite::Row) -> rusqlite::Result<HistoryItemRecord> {
    let id: String = row.get(0)?;
    let timestamp_val: i64 = row.get(1)?;
    let duration: f64 = row.get(2)?;
    let audio_path: String = row.get(3)?;
    let transcript_path: String = row.get(4)?;
    let title: String = row.get(5)?;
    let preview_text: String = row.get(6)?;
    let icon: Option<String> = row.get(7)?;
    let kind_str: String = row.get(8)?;
    let search_content: String = row.get(9)?;
    let project_id: Option<String> = row.get(10)?;
    let status_str: String = row.get(11)?;
    let draft_source_str: Option<String> = row.get(12)?;

    let kind = if kind_str == "batch" {
        HistoryItemKind::Batch
    } else {
        HistoryItemKind::Recording
    };

    let status = if status_str == "draft" {
        HistoryItemStatus::Draft
    } else {
        HistoryItemStatus::Complete
    };

    let draft_source = match draft_source_str.as_deref() {
        Some("live_record") => Some(HistoryDraftSource::LiveRecord),
        _ => None,
    };

    Ok(HistoryItemRecord {
        id,
        timestamp: timestamp_val as u64,
        duration,
        audio_path,
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
        item.kind = if kind == "batch" {
            HistoryItemKind::Batch
        } else {
            HistoryItemKind::Recording
        };
    }
    if let Some(search_content) = updates.get("searchContent").and_then(Value::as_str) {
        item.search_content = search_content.to_string();
    }
    if let Some(project_id) = updates.get("projectId") {
        item.project_id = project_id.as_str().map(ToString::to_string);
    }
    if let Some(status) = updates.get("status").and_then(Value::as_str) {
        item.status = if status == "draft" {
            HistoryItemStatus::Draft
        } else {
            HistoryItemStatus::Complete
        };
    }
    if let Some(draft_source) = updates.get("draftSource") {
        item.draft_source = match draft_source.as_str() {
            Some("live_record") => Some(HistoryDraftSource::LiveRecord),
            _ => None,
        };
    }
}

impl HistoryStore for SqliteHistoryStore {
    fn ensure_ready(&self) -> Result<(), String> {
        fs::create_dir_all(self.history_dir()).map_err(|error| error.to_string())
    }

    fn list_items(&self) -> Result<Vec<HistoryItemRecord>, String> {
        self.get_db().with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, timestamp, duration, audio_path, transcript_path, title, preview_text, icon, kind, search_content, project_id, status, draft_source
                 FROM history_items
                 ORDER BY timestamp DESC"
            )?;
            let rows = stmt.query_map([], map_row_to_item)?;
            let mut items = Vec::new();
            for row in rows {
                items.push(row?);
            }
            Ok(items)
        })
    }

    fn list_items_with_reconciled_live_drafts(&self) -> Result<Vec<HistoryItemRecord>, String> {
        self.get_db().with_transaction(|tx| {
            let mut items = {
                let mut stmt = tx.prepare(
                    "SELECT id, timestamp, duration, audio_path, transcript_path, title, preview_text, icon, kind, search_content, project_id, status, draft_source
                     FROM history_items
                     ORDER BY timestamp DESC"
                )?;
                let rows = stmt.query_map([], map_row_to_item)?;
                let mut items = Vec::new();
                for row in rows {
                    items.push(row?);
                }
                items
            };
            let mut changed = false;

            for item in &mut items {
                if item.status != HistoryItemStatus::Draft
                    || item.draft_source != Some(HistoryDraftSource::LiveRecord)
                {
                    continue;
                }

                let Some(audio_path) =
                    optional_history_child_path(&self.history_dir(), &item.audio_path)
                else {
                    continue;
                };
                let _audio_metadata = match fs::metadata(&audio_path) {
                    Ok(metadata) if metadata.is_file() && metadata.len() > 0 => metadata,
                    _ => continue,
                };

                let segments_val: Option<Value> = {
                    let mut stmt =
                        tx.prepare("SELECT segments FROM history_transcripts WHERE history_id = ?1")?;
                    let mut rows = stmt.query([&item.id])?;
                    let result: Result<Option<Value>, rusqlite::Error> = (|| {
                        if let Some(row) = rows.next()? {
                            let segments_str: String = row.get(0)?;
                            let val: Value = serde_json::from_str(&segments_str)
                                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                            Ok(Some(val))
                        } else {
                            Ok(None)
                        }
                    })();
                    result
                }?;

                let Some(segments_val) = segments_val else {
                    continue;
                };

                let normalized_transcript = match normalize_history_transcript_segments(segments_val) {
                    Ok(transcript) if !transcript.segments.is_empty() => transcript,
                    _ => continue,
                };

                item.preview_text = normalized_transcript.preview_text.clone();
                item.search_content = normalized_transcript.search_content.clone();
                let transcript_duration = normalized_transcript
                    .segments
                    .iter()
                    .filter_map(|segment| segment.end.is_finite().then_some(segment.end))
                    .fold(0.0, f64::max);
                item.duration = item.duration.max(transcript_duration).max(0.0);
                item.status = HistoryItemStatus::Complete;
                item.draft_source = None;

                let segments_str = serde_json::to_string(&normalized_transcript.segments)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

                tx.execute(
                    "UPDATE history_items SET preview_text = ?1, search_content = ?2, duration = ?3, status = 'complete', draft_source = NULL WHERE id = ?4",
                    rusqlite::params![
                        item.preview_text,
                        item.search_content,
                        item.duration,
                        item.id
                    ]
                )?;
                tx.execute(
                    "INSERT OR REPLACE INTO history_transcripts (history_id, segments) VALUES (?1, ?2)",
                    rusqlite::params![item.id, segments_str]
                )?;

                changed = true;
            }

            if changed {
                let mut stmt = tx.prepare(
                    "SELECT id, timestamp, duration, audio_path, transcript_path, title, preview_text, icon, kind, search_content, project_id, status, draft_source
                     FROM history_items
                     ORDER BY timestamp DESC"
                )?;
                let rows = stmt.query_map([], map_row_to_item)?;
                let mut items = Vec::new();
                for row in rows {
                    items.push(row?);
                }
                return Ok(items);
            }

            Ok(items)
        })
    }

    fn query_workspace(
        &self,
        request: HistoryWorkspaceQueryRequest,
    ) -> Result<HistoryWorkspaceQueryResult, String> {
        let items = self.list_items_with_reconciled_live_drafts()?;
        Ok(super::workspace_query::query_workspace_items(
            items, request,
        ))
    }

    fn create_live_draft(
        &self,
        request: HistoryCreateLiveDraftRequest,
    ) -> Result<LiveRecordingDraftResult, String> {
        self.ensure_ready()?;
        let item = super::item_factory::create_live_draft_item(request)?;
        let audio_absolute_path = self
            .audio_path(&item.audio_path)?
            .to_string_lossy()
            .into_owned();

        self.get_db().with_transaction(|tx| {
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
    ) -> Result<HistoryItemRecord, String> {
        let normalized_transcript = normalize_history_transcript_segments(segments)?;
        let segments_str =
            serde_json::to_string(&normalized_transcript.segments).map_err(|e| e.to_string())?;

        let exists: bool = self.get_db().with_connection(|conn| {
            Ok(conn
                .query_row(
                    "SELECT 1 FROM history_items WHERE id = ?1",
                    [history_id],
                    |_| Ok(()),
                )
                .is_ok())
        })?;

        if !exists {
            return Err(format!("History item not found: {history_id}"));
        }

        self.get_db().with_transaction(|tx| {
            tx.execute(
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

            tx.execute(
                "INSERT OR REPLACE INTO history_transcripts (history_id, segments) VALUES (?1, ?2)",
                rusqlite::params![history_id, segments_str],
            )?;

            Ok(())
        })?;

        self.get_db().with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, timestamp, duration, audio_path, transcript_path, title, preview_text, icon, kind, search_content, project_id, status, draft_source
                 FROM history_items
                 WHERE id = ?1"
            )?;
            let item = stmt.query_row([history_id], map_row_to_item)?;
            Ok(item)
        })
    }

    fn save_recording(
        &self,
        request: HistorySaveRecordingRequest,
    ) -> Result<HistoryItemRecord, String> {
        self.ensure_ready()?;
        let HistorySaveRecordingRequest {
            segments,
            duration,
            project_id,
            audio_bytes,
            native_audio_path,
            audio_extension,
        } = request;

        let normalized_transcript = normalize_history_transcript_segments(segments)?;
        let mut item = super::item_factory::create_recording_item(
            duration,
            project_id,
            audio_extension.as_deref(),
            native_audio_path.as_deref(),
        )?;
        item.preview_text = normalized_transcript.preview_text;
        item.search_content = normalized_transcript.search_content;

        match (audio_bytes, native_audio_path) {
            (Some(bytes), _) => {
                let target_path = self.audio_path(&item.audio_path)?;
                crate::repositories::storage::write_binary_atomic(&target_path, &bytes)?;
            }
            (None, Some(native_path)) => {
                let source_path = PathBuf::from(&native_path);
                if !source_path.is_file() {
                    return Err(format!(
                        "Native recording source file does not exist: {}",
                        source_path.to_string_lossy()
                    ));
                }
                let target_path = self.audio_path(&item.audio_path)?;
                if source_path != target_path {
                    if let Some(parent) = target_path.parent() {
                        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                    }
                    fs::copy(&source_path, &target_path).map_err(|error| error.to_string())?;
                }
            }
            (None, None) => {
                return Err(
                    "History recording save requires audio bytes or a native audio path."
                        .to_string(),
                );
            }
        }

        let segments_str =
            serde_json::to_string(&normalized_transcript.segments).map_err(|e| e.to_string())?;

        self.get_db().with_transaction(|tx| {
            Self::insert_history_item_and_transcript(tx, &item, &segments_str)?;
            Ok(())
        })?;

        Ok(item)
    }

    fn save_imported_file(
        &self,
        request: HistorySaveImportedFileRequest,
    ) -> Result<HistoryItemRecord, String> {
        self.ensure_ready()?;
        let HistorySaveImportedFileRequest {
            id,
            source_path,
            segments,
            duration,
            project_id,
            converted_source_path,
        } = request;

        let normalized_transcript = normalize_history_transcript_segments(segments)?;
        let imported = super::item_factory::create_imported_file_item(
            id,
            source_path,
            converted_source_path,
            duration,
            project_id,
        )?;
        let mut item = imported.item;
        item.preview_text = normalized_transcript.preview_text;
        item.search_content = normalized_transcript.search_content;

        let source = PathBuf::from(imported.copy_source_path);
        if !source.is_file() {
            return Err(format!(
                "Imported source file does not exist: {}",
                source.to_string_lossy()
            ));
        }

        let target_path = self.audio_path(&item.audio_path)?;
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::copy(&source, &target_path).map_err(|error| error.to_string())?;

        let segments_str =
            serde_json::to_string(&normalized_transcript.segments).map_err(|e| e.to_string())?;

        self.get_db().with_transaction(|tx| {
            Self::insert_history_item_and_transcript(tx, &item, &segments_str)?;
            Ok(())
        })?;

        Ok(item)
    }

    fn delete_items(&self, ids: &[String]) -> Result<(), String> {
        if ids.is_empty() {
            return Ok(());
        }

        let audio_paths = self.get_db().with_connection(|conn| {
            let mut audio_paths = Vec::new();
            let mut stmt = conn.prepare("SELECT audio_path FROM history_items WHERE id = ?1")?;
            for id in ids {
                let mut rows = stmt.query([id])?;
                if let Some(row) = rows.next()? {
                    let audio_path: String = row.get(0)?;
                    audio_paths.push(audio_path);
                }
            }
            Ok(audio_paths)
        })?;

        for audio_path_str in audio_paths {
            if let Some(path) = optional_history_child_path(&self.history_dir(), &audio_path_str) {
                crate::repositories::storage::remove_path_if_exists(&path)?;
            }
        }

        self.get_db().with_transaction(|tx| {
            let mut stmt = tx.prepare("DELETE FROM history_items WHERE id = ?1")?;
            for id in ids {
                stmt.execute([id])?;
            }
            Ok(())
        })?;

        Ok(())
    }

    fn load_transcript(&self, history_id: &str) -> Result<Option<Vec<TranscriptSegment>>, String> {
        self.get_db().with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT t.segments
                 FROM history_items i
                 JOIN history_transcripts t ON i.id = t.history_id
                 WHERE i.id = ?1 OR i.transcript_path = ?1"
            )?;
            let mut rows = stmt.query([history_id])?;
            if let Some(row) = rows.next()? {
                let segments_str: String = row.get(0)?;
                let parsed_val: Value = serde_json::from_str(&segments_str)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                let normalized = normalize_history_transcript_segments(parsed_val)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e))))?;
                Ok(Some(normalized.segments))
            } else {
                let exists: bool = conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM history_items WHERE id = ?1 OR transcript_path = ?1)",
                    [history_id],
                    |row| row.get(0),
                )?;
                if exists {
                    Ok(None)
                } else {
                    Err(rusqlite::Error::ToSqlConversionFailure(
                        Box::new(std::io::Error::other(format!("History item not found: {history_id}")))
                    ))
                }
            }
        })
    }

    fn update_transcript(
        &self,
        history_id: &str,
        segments: Value,
    ) -> Result<HistoryItemRecord, String> {
        let normalized_transcript = normalize_history_transcript_segments(segments)?;
        let segments_str =
            serde_json::to_string(&normalized_transcript.segments).map_err(|e| e.to_string())?;

        let exists: bool = self.get_db().with_connection(|conn| {
            Ok(conn
                .query_row(
                    "SELECT 1 FROM history_items WHERE id = ?1",
                    [history_id],
                    |_| Ok(()),
                )
                .is_ok())
        })?;

        if !exists {
            return Err(format!("History item not found: {history_id}"));
        }

        self.get_db().with_transaction(|tx| {
            tx.execute(
                "UPDATE history_items
                 SET preview_text = ?1, search_content = ?2
                 WHERE id = ?3",
                rusqlite::params![
                    normalized_transcript.preview_text,
                    normalized_transcript.search_content,
                    history_id,
                ],
            )?;

            tx.execute(
                "INSERT OR REPLACE INTO history_transcripts (history_id, segments) VALUES (?1, ?2)",
                rusqlite::params![history_id, segments_str],
            )?;

            Ok(())
        })?;

        self.get_db().with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, timestamp, duration, audio_path, transcript_path, title, preview_text, icon, kind, search_content, project_id, status, draft_source
                 FROM history_items
                 WHERE id = ?1"
            )?;
            let item = stmt.query_row([history_id], map_row_to_item)?;
            Ok(item)
        })
    }

    fn create_transcript_snapshot(
        &self,
        history_id: &str,
        reason: TranscriptSnapshotReason,
        segments: Value,
    ) -> Result<TranscriptSnapshotMetadata, String> {
        validate_id(history_id, "History ID")?;
        let normalized_transcript = normalize_history_transcript_segments(segments)?;
        let parsed_segments = normalized_transcript.segments;

        let created_at = super::item_factory::current_time_millis()?;
        let metadata = TranscriptSnapshotMetadata {
            id: format!("{created_at}-{}", uuid::Uuid::new_v4()),
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

        let segments_str = serde_json::to_string(&parsed_segments).map_err(|e| e.to_string())?;

        self.get_db().with_transaction(|tx| {
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM history_items WHERE id = ?1)",
                [history_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(rusqlite::Error::ToSqlConversionFailure(
                    Box::new(std::io::Error::other(format!("History item not found: {history_id}")))
                ));
            }

            tx.execute(
                "INSERT INTO transcript_snapshots (id, history_id, reason, created_at, segment_count, segments)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    metadata.id,
                    metadata.history_id,
                    reason_str,
                    metadata.created_at as i64,
                    metadata.segment_count as i64,
                    segments_str,
                ],
            )?;

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
                    crate::repositories::history::TRANSCRIPT_SNAPSHOT_RETENTION_LIMIT as i64
                ],
            )?;

            Ok(metadata.clone())
        })
    }

    fn list_transcript_snapshots(
        &self,
        history_id: &str,
    ) -> Result<Vec<TranscriptSnapshotMetadata>, String> {
        validate_id(history_id, "History ID")?;
        self.get_db().with_connection(|conn| {
            let mut stmt = conn.prepare(
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
        })
    }

    fn load_transcript_snapshot(
        &self,
        history_id: &str,
        snapshot_id: &str,
    ) -> Result<Option<TranscriptSnapshotRecord>, String> {
        validate_id(history_id, "History ID")?;
        validate_id(snapshot_id, "Snapshot ID")?;

        self.get_db().with_connection(|conn| {
            let mut stmt = conn.prepare(
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

                let parsed_val: Value = serde_json::from_str(&segments_str)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

                let normalized =
                    normalize_history_transcript_segments(parsed_val).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(
                            e,
                        )))
                    })?;

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
        })
    }

    fn update_item_meta(&self, history_id: &str, updates: Value) -> Result<(), String> {
        let updates = updates
            .as_object()
            .ok_or_else(|| "History item updates must be an object.".to_string())?;

        self.get_db().with_transaction(|tx| {
            let mut stmt = tx.prepare(
                "SELECT id, timestamp, duration, audio_path, transcript_path, title, preview_text, icon, kind, search_content, project_id, status, draft_source
                 FROM history_items
                 WHERE id = ?1"
            )?;
            let mut item = match stmt.query_row([history_id], map_row_to_item) {
                Ok(item) => item,
                Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(()),
                Err(error) => return Err(error),
            };

            apply_history_item_updates(&mut item, updates);

            let kind_str = match item.kind {
                HistoryItemKind::Batch => "batch",
                HistoryItemKind::Recording => "recording",
            };
            let status_str = match item.status {
                HistoryItemStatus::Draft => "draft",
                HistoryItemStatus::Complete => "complete",
            };
            let draft_source_str = item.draft_source.map(|s| match s {
                HistoryDraftSource::LiveRecord => "live_record",
            });

            tx.execute(
                "UPDATE history_items
                 SET id = ?1, timestamp = ?2, duration = ?3, audio_path = ?4, transcript_path = ?5, title = ?6, preview_text = ?7, icon = ?8, kind = ?9, search_content = ?10, project_id = ?11, status = ?12, draft_source = ?13
                 WHERE id = ?14",
                rusqlite::params![
                    item.id,
                    item.timestamp as i64,
                    item.duration,
                    item.audio_path,
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

            Ok(())
        })
    }

    fn update_project_assignments(
        &self,
        ids: &[String],
        project_id: Option<String>,
    ) -> Result<(), String> {
        if ids.is_empty() {
            return Ok(());
        }

        self.get_db().with_transaction(|tx| {
            let mut stmt = tx.prepare("UPDATE history_items SET project_id = ?1 WHERE id = ?2")?;
            for id in ids {
                stmt.execute(rusqlite::params![project_id, id])?;
            }
            Ok(())
        })
    }

    fn reassign_project(
        &self,
        current_project_id: String,
        next_project_id: Option<String>,
    ) -> Result<(), String> {
        self.get_db().with_connection(|conn| {
            conn.execute(
                "UPDATE history_items SET project_id = ?1 WHERE project_id = ?2",
                rusqlite::params![next_project_id, current_project_id],
            )?;
            Ok(())
        })
    }

    fn load_summary(&self, history_id: &str) -> Result<Option<Value>, String> {
        validate_id(history_id, "History ID")?;

        self.get_db().with_connection(|conn| {
            let mut stmt =
                conn.prepare("SELECT payload FROM history_summaries WHERE history_id = ?1")?;
            let mut rows = stmt.query([history_id])?;
            if let Some(row) = rows.next()? {
                let payload_str: String = row.get(0)?;
                let val: Value = serde_json::from_str(&payload_str)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                Ok(Some(val))
            } else {
                Ok(None)
            }
        })
    }

    fn save_summary(&self, history_id: &str, summary_payload: Value) -> Result<(), String> {
        validate_id(history_id, "History ID")?;

        let summary_payload = crate::repositories::history::fs_utils::ensure_json_object_value(
            summary_payload,
            "History summary payload",
        )?;

        let payload_str = serde_json::to_string(&summary_payload).map_err(|e| e.to_string())?;

        self.get_db().with_connection(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO history_summaries (history_id, payload) VALUES (?1, ?2)",
                rusqlite::params![history_id, payload_str],
            )?;
            Ok(())
        })
    }

    fn delete_summary(&self, history_id: &str) -> Result<(), String> {
        validate_id(history_id, "History ID")?;

        self.get_db().with_connection(|conn| {
            conn.execute(
                "DELETE FROM history_summaries WHERE history_id = ?1",
                [history_id],
            )?;
            Ok(())
        })
    }

    fn resolve_audio_path(&self, history_id: &str) -> Result<Option<String>, String> {
        validate_id(history_id, "History ID")?;

        let audio_path_opt: Option<String> = self.get_db().with_connection(|conn| {
            let mut stmt = conn
                .prepare("SELECT audio_path FROM history_items WHERE id = ?1 OR audio_path = ?1")?;
            let mut rows = stmt.query([history_id])?;
            if let Some(row) = rows.next()? {
                let p: String = row.get(0)?;
                Ok(Some(p))
            } else {
                Ok(None)
            }
        })?;

        let Some(audio_path_str) = audio_path_opt else {
            return Err(format!("History item not found: {history_id}"));
        };

        let Some(audio_path) = optional_history_child_path(&self.history_dir(), &audio_path_str)
        else {
            return Ok(None);
        };

        match fs::metadata(&audio_path) {
            Ok(metadata) if metadata.is_file() && metadata.len() > 0 => {
                Ok(Some(audio_path.to_string_lossy().into_owned()))
            }
            Ok(_) => Ok(None),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    fn history_snapshot_for_backup(&self) -> Result<HistoryBackupSnapshot, String> {
        let items = self.get_db().with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, timestamp, duration, audio_path, transcript_path, title, preview_text, icon, kind, search_content, project_id, status, draft_source
                 FROM history_items
                 WHERE status != 'draft'
                 ORDER BY timestamp DESC"
            )?;
            let rows = stmt.query_map([], map_row_to_item)?;
            let mut items = Vec::new();
            for r in rows {
                let mut item = r?;
                item.status = HistoryItemStatus::Complete;
                item.draft_source = None;
                items.push(item);
            }
            Ok(items)
        })?;

        if items.is_empty() {
            return Ok(HistoryBackupSnapshot {
                items,
                transcript_files: vec![],
                summary_files: vec![],
                snapshot_files: vec![],
            });
        }

        let ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");

        let (transcript_files, summary_files, snapshot_files) = self.get_db().with_connection(|conn| {
            // Bulk fetch transcripts
            let mut transcript_files: Vec<(String, Value)> = Vec::with_capacity(items.len());

            {
                let sql = format!(
                    "SELECT history_id, segments FROM history_transcripts WHERE history_id IN ({})",
                    placeholders
                );
                let mut stmt = conn.prepare(&sql)?;
                let rows = stmt.query_map(rusqlite::params_from_iter(ids.iter()), |row| {
                    let history_id: String = row.get(0)?;
                    let segments_str: String = row.get(1)?;
                    let val: Value = serde_json::from_str(&segments_str)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                    Ok((history_id, val))
                })?;

                let mut transcript_map: std::collections::HashMap<String, Value> =
                    std::collections::HashMap::new();
                for r in rows {
                    let (hid, val) = r?;
                    transcript_map.insert(hid, val);
                }

                for item in &items {
                    let transcript_val = match transcript_map.remove(item.id.as_str()) {
                        Some(val) => ensure_json_array_value(
                            val,
                            &format!("Transcript for history item {}", item.id),
                        ).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e))))?,
                        None => {
                            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(
                                format!("History item \"{}\" is missing its transcript file.", item.title),
                            ))));
                        }
                    };
                    transcript_files.push((item.transcript_path.clone(), transcript_val));
                }
            }

            // Bulk fetch summaries
            let mut summary_files: Vec<(String, Value)> = Vec::new();
            {
                let sql = format!(
                    "SELECT history_id, payload FROM history_summaries WHERE history_id IN ({})",
                    placeholders
                );
                let mut stmt = conn.prepare(&sql)?;
                let rows = stmt.query_map(rusqlite::params_from_iter(ids.iter()), |row| {
                    let history_id: String = row.get(0)?;
                    let payload_str: String = row.get(1)?;
                    let val: Value = serde_json::from_str(&payload_str)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                    Ok((history_id, val))
                })?;

                for r in rows {
                    let (hid, val) = r?;
                    summary_files.push((hid, val));
                }
            }

            // Bulk fetch snapshots with segments
            let mut snapshot_files: Vec<(String, Value)> = Vec::new();
            {
                let sql = format!(
                    "SELECT id, history_id, reason, created_at, segment_count, segments
                     FROM transcript_snapshots
                     WHERE history_id IN ({})
                     ORDER BY created_at DESC, id DESC",
                    placeholders
                );
                let mut stmt = conn.prepare(&sql)?;
                let rows = stmt.query_map(rusqlite::params_from_iter(ids.iter()), |row| {
                    let snapshot_id: String = row.get(0)?;
                    let history_id: String = row.get(1)?;
                    let reason_str: String = row.get(2)?;
                    let created_at: i64 = row.get(3)?;
                    let segment_count: i64 = row.get(4)?;
                    let segments_str: String = row.get(5)?;

                    let reason = match reason_str.as_str() {
                        "polish" => TranscriptSnapshotReason::Polish,
                        "translate" => TranscriptSnapshotReason::Translate,
                        "retranscribe" => TranscriptSnapshotReason::Retranscribe,
                        "restore" => TranscriptSnapshotReason::Restore,
                        _ => TranscriptSnapshotReason::Polish,
                    };

                    let parsed_val: Value = serde_json::from_str(&segments_str)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

                    let normalized =
                        normalize_history_transcript_segments(parsed_val).map_err(|e| {
                            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(
                                e,
                            )))
                        })?;

                    let metadata = TranscriptSnapshotMetadata {
                        id: snapshot_id.clone(),
                        history_id: history_id.clone(),
                        reason,
                        created_at: created_at as u64,
                        segment_count: segment_count as u64,
                    };

                    Ok((history_id, metadata, normalized.segments))
                })?;

                let mut snapshots_by_item: std::collections::HashMap<String, Vec<TranscriptSnapshotRecord>> =
                    std::collections::HashMap::new();
                for r in rows {
                    let (hid, metadata, segments) = r?;
                    snapshots_by_item
                        .entry(hid)
                        .or_default()
                        .push(TranscriptSnapshotRecord { metadata, segments });
                }

                for item in &items {
                    if let Some(records) = snapshots_by_item.remove(item.id.as_str()) {
                        let meta_list: Vec<TranscriptSnapshotMetadata> = records.iter().map(|r| r.metadata.clone()).collect();
                        snapshot_files.push((
                            format!("versions/{}/index.json", item.id),
                            serde_json::to_value(&meta_list)
                                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                        ));
                        for rec in records {
                            snapshot_files.push((
                                format!("versions/{}/{}.json", item.id, rec.metadata.id),
                                serde_json::to_value(&rec)
                                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                            ));
                        }
                    }
                }
            }

            Ok((transcript_files, summary_files, snapshot_files))
        })?;

        Ok(HistoryBackupSnapshot {
            items,
            transcript_files,
            summary_files,
            snapshot_files,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
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

    #[test]
    fn test_sqlite_store_crud() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

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
        use crate::repositories::history::{
            HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType, HistoryWorkspaceScope,
            HistoryWorkspaceSortOrder,
        };

        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

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
            })
            .unwrap();

        assert_eq!(result.filtered_items.len(), 1);
        assert_eq!(
            result.filtered_items[0].preview_text,
            "Alpha roadmap discussion..."
        );
    }
}
