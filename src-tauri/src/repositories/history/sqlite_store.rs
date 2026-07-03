use super::fs_utils::{
    ensure_json_array_value, ensure_safe_file_name, optional_history_child_path,
};
use super::transcript_payload::normalize_history_transcript_segments;
use crate::core::database::{Database, DatabaseError};
use crate::core::history_store::HistoryStore;
use crate::integrations::asr::TranscriptSegment;
use crate::repositories::history::{
    HistoryBackupSnapshot, HistoryCreateLiveDraftRequest, HistoryDraftSource, HistoryItemKind,
    HistoryItemRecord, HistoryItemStatus, HistoryListOptions, HistorySaveImportedFileRequest,
    HistorySaveRecordingRequest, HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType,
    HistoryWorkspaceItemCounts, HistoryWorkspaceQueryRequest, HistoryWorkspaceQueryResult,
    HistoryWorkspaceScope, LiveRecordingDraftResult, TranscriptSnapshotMetadata,
    TranscriptSnapshotReason, TranscriptSnapshotRecord,
};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use chrono::{Datelike, Duration, Local, LocalResult, TimeZone};
use rusqlite::types::ToSql;

fn compute_date_threshold_millis(date_filter: HistoryWorkspaceDateFilter) -> Option<i64> {
    use HistoryWorkspaceDateFilter::*;
    match date_filter {
        All => None,
        _ => {
            let now = Local::now();
            let today = match Local.with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0) {
                LocalResult::Single(value) => value,
                LocalResult::Ambiguous(earliest, _) => earliest,
                LocalResult::None => return None,
            };
            let threshold = match date_filter {
                Today => today,
                Week => today - Duration::days(7),
                Month => {
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
                        LocalResult::Single(value) => value,
                        LocalResult::Ambiguous(earliest, _) => earliest,
                        LocalResult::None => today - Duration::days(30),
                    }
                }
                _ => unreachable!(),
            };
            Some(threshold.timestamp_millis())
        }
    }
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
    clauses: &mut Vec<String>,
    params: &mut Vec<Box<dyn ToSql>>,
) {
    match &request.scope {
        HistoryWorkspaceScope::All => {}
        HistoryWorkspaceScope::Inbox => {
            clauses.push("project_id IS NULL".to_string());
        }
        HistoryWorkspaceScope::Project { project_id } => {
            clauses.push("project_id = ?".to_string());
            params.push(Box::new(project_id.clone()));
        }
    }
    match request.filter_type {
        HistoryWorkspaceFilterType::All => {}
        HistoryWorkspaceFilterType::Recording => {
            clauses.push("kind = 'recording'".to_string());
        }
        HistoryWorkspaceFilterType::Batch => {
            clauses.push("kind = 'batch'".to_string());
        }
    }
    if let Some(threshold) = compute_date_threshold_millis(request.date_filter) {
        clauses.push("timestamp >= ?".to_string());
        params.push(Box::new(threshold));
    }
}

#[derive(Clone)]
pub struct SqliteHistoryStore {
    app_local_data_dir: PathBuf,
    db: crate::core::database::DbProvider,
}

impl SqliteHistoryStore {
    pub fn new(app_local_data_dir: PathBuf) -> Self {
        Self {
            app_local_data_dir,
            db: crate::core::database::DbProvider::default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_db(app_local_data_dir: PathBuf, db: Database) -> Self {
        Self {
            app_local_data_dir,
            db: crate::core::database::DbProvider::new(Some(std::sync::Arc::new(db))),
        }
    }

    fn get_db(&self) -> Result<&Database, DatabaseError> {
        self.db.get()
    }

    fn history_dir(&self) -> PathBuf {
        self.app_local_data_dir
            .join(crate::repositories::history::HISTORY_DIR_NAME)
    }

    fn audio_path(&self, file_name: &str) -> Result<PathBuf, DatabaseError> {
        Ok(self.history_dir().join(
            ensure_safe_file_name(file_name, "History audio path")
                .map_err(DatabaseError::Internal)?,
        ))
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
    ) -> Result<(), DatabaseError> {
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

    fn reconcile_live_drafts(&self) -> Result<(), DatabaseError> {
        let history_dir = self.history_dir();
        #[allow(clippy::type_complexity)]
        let draft_items: Vec<(HistoryItemRecord, Option<Value>)> = self.get_db()?.with_connection(|conn| {
            let mut stmt = conn.prepare_cached(
                "SELECT h.id, h.timestamp, h.duration, h.audio_path, h.transcript_path, h.title, h.preview_text, h.icon, h.kind, '' AS search_content, h.project_id, h.status, h.draft_source, t.segments
                 FROM history_items h
                 LEFT JOIN history_transcripts t ON h.id = t.history_id
                 WHERE h.status = 'draft' AND h.draft_source = 'live_record'"
            )?;
            let rows = stmt.query_map([], |row| {
                let item = map_row_to_item(row)?;
                let segments: Option<String> = row.get(13)?;
                let segments_val = segments
                    .and_then(|s| serde_json::from_str(&s).ok());
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
            self.get_db()?.with_transaction(|tx| {
                for (item, segments_str) in &verified_updates {
                    tx.execute(
                        "UPDATE history_items SET preview_text = ?1, search_content = ?2, duration = ?3, status = 'complete', draft_source = NULL WHERE id = ?4",
                        rusqlite::params![item.preview_text, item.search_content, item.duration, item.id]
                    )?;

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
    fn ensure_ready(&self) -> Result<(), DatabaseError> {
        fs::create_dir_all(self.history_dir()).map_err(|e| DatabaseError::Internal(e.to_string()))
    }

    fn list_items(&self) -> Result<Vec<HistoryItemRecord>, DatabaseError> {
        self.get_db()?.with_connection(|conn| {
            let mut stmt = conn.prepare_cached(
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

    fn list_items_with_reconciled_live_drafts(
        &self,
    ) -> Result<Vec<HistoryItemRecord>, DatabaseError> {
        self.reconcile_live_drafts()?;
        self.get_db()?.with_connection(|conn| {
            let mut stmt = conn.prepare_cached(
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

    fn list_items_paginated(
        &self,
        opts: HistoryListOptions,
    ) -> Result<Vec<HistoryItemRecord>, DatabaseError> {
        self.get_db()?.with_connection(|conn| {
            let limit = opts.limit.map(|l| l as i64).unwrap_or(-1);
            let offset = opts.offset.map(|o| o as i64).unwrap_or(0);
            let mut stmt = conn.prepare_cached(
                "SELECT id, timestamp, duration, audio_path, transcript_path, title, preview_text, icon, kind, search_content, project_id, status, draft_source
                 FROM history_items
                 ORDER BY timestamp DESC
                 LIMIT ?1 OFFSET ?2"
            )?;
            let rows = stmt.query_map(rusqlite::params![limit, offset], map_row_to_item)?;
            let mut items = Vec::new();
            for row in rows {
                items.push(row?);
            }
            Ok(items)
        })
    }

    fn list_items_with_reconciled_live_drafts_paginated(
        &self,
        opts: HistoryListOptions,
    ) -> Result<Vec<HistoryItemRecord>, DatabaseError> {
        self.reconcile_live_drafts()?;
        self.list_items_paginated(opts)
    }

    fn query_workspace(
        &self,
        request: HistoryWorkspaceQueryRequest,
    ) -> Result<HistoryWorkspaceQueryResult, DatabaseError> {
        self.ensure_ready()?;
        self.reconcile_live_drafts()?;

        self.get_db()?.with_connection(|conn| {
            // 1. Compute item counts via aggregate query (index-only, lightweight)
            let item_counts = {
                let mut stmt = conn.prepare_cached(
                    "SELECT project_id, COUNT(*) FROM history_items GROUP BY project_id"
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
                        Some(id) => { by_project_id.insert(id, cnt); }
                    }
                }
                HistoryWorkspaceItemCounts { inbox, by_project_id }
            };

            // 2. Build dynamic WHERE clause + params from request filters
            let mut conditions: Vec<String> = Vec::new();
            let mut params: Vec<Box<dyn ToSql>> = Vec::new();
            add_workspace_query_conditions(&request, &mut conditions, &mut params);

            // 3. Perform FTS pre-filtering if query exists
            let trimmed_query = request.query.trim();
            let mut search_contents = std::collections::HashMap::new();
            let mut fts_used = false;

            if !trimmed_query.is_empty() {
                let fts_expr = build_fts_query(trimmed_query);
                if !fts_expr.is_empty() {
                    let mut fts_conditions = vec!["history_items_fts MATCH ?".to_string()];
                    let mut fts_params: Vec<Box<dyn ToSql>> = vec![Box::new(fts_expr)];
                    add_workspace_query_conditions(&request, &mut fts_conditions, &mut fts_params);

                    let fts_sql = format!(
                        "SELECT h.id, h.search_content FROM history_items h
                         JOIN history_items_fts f ON h.rowid = f.rowid
                         WHERE {}",
                        fts_conditions.join(" AND ")
                    );
                    let mut stmt = conn.prepare_cached(&fts_sql)?;
                    let fts_param_refs: Vec<&dyn ToSql> =
                        fts_params.iter().map(|p| p.as_ref()).collect();
                    let mut rows = stmt.query(fts_param_refs.as_slice())?;
                    while let Some(row) = rows.next()? {
                        let id: String = row.get(0)?;
                        let content: String = row.get(1)?;
                        search_contents.insert(id, content);
                    }
                    fts_used = true;
                }
            }

            // 4. Load workspace items with SQL-level filtering
            //    When FTS was used, load empty search_content (populated from
            //    FTS results). When FTS was skipped, load the actual column so
            //    in-memory matching works for short queries.
            let column_select = if fts_used {
                "id, timestamp, duration, audio_path, transcript_path, title, preview_text, icon, kind, '' AS search_content, project_id, status, draft_source"
            } else {
                "id, timestamp, duration, audio_path, transcript_path, title, preview_text, icon, kind, search_content, project_id, status, draft_source"
            };
            let where_clause = if conditions.is_empty() {
                String::new()
            } else {
                format!("WHERE {}", conditions.join(" AND "))
            };
            let sql = format!(
                "SELECT {} FROM history_items {} ORDER BY timestamp DESC",
                column_select, where_clause
            );
            let mut stmt = conn.prepare_cached(&sql)?;
            let param_refs: Vec<&dyn ToSql> = params.iter().map(|p| p.as_ref()).collect();

            let rows = stmt.query_map(param_refs.as_slice(), map_row_to_item)?;
            let mut items = Vec::new();
            for row in rows {
                let mut item = row?;
                if let Some(content) = search_contents.remove(&item.id) {
                    item.search_content = content;
                }
                items.push(item);
            }

            // 5. In-memory text search matching (highlighting/snippets) and sorting
            Ok(super::workspace_query::query_workspace_items_with_counts(
                items, request, item_counts,
            ))
        })
    }

    fn create_live_draft(
        &self,
        request: HistoryCreateLiveDraftRequest,
    ) -> Result<LiveRecordingDraftResult, DatabaseError> {
        self.ensure_ready()?;
        let item = super::item_factory::create_live_draft_item(request)
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
    ) -> Result<HistoryItemRecord, DatabaseError> {
        let normalized_transcript =
            normalize_history_transcript_segments(segments).map_err(DatabaseError::Internal)?;
        let segments_str = serde_json::to_string(&normalized_transcript.segments)?;

        let exists: bool = self.get_db()?.with_connection(|conn| {
            Ok(conn
                .query_row(
                    "SELECT 1 FROM history_items WHERE id = ?1",
                    [history_id],
                    |_| Ok(()),
                )
                .is_ok())
        })?;

        if !exists {
            return Err(DatabaseError::NotFoundError(format!(
                "History item not found: {history_id}"
            )));
        }

        self.get_db()?.with_transaction(|tx| {
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

        self.get_db()?.with_connection(|conn| {
            let mut stmt = conn.prepare_cached(
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
    ) -> Result<HistoryItemRecord, DatabaseError> {
        self.ensure_ready()?;
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
        let mut item = super::item_factory::create_recording_item(
            duration,
            project_id,
            audio_extension.as_deref(),
            native_audio_path.as_deref(),
        )
        .map_err(DatabaseError::Internal)?;
        item.preview_text = normalized_transcript.preview_text;
        item.search_content = normalized_transcript.search_content;

        match (audio_bytes, native_audio_path) {
            (Some(bytes), _) => {
                let target_path = self.audio_path(&item.audio_path)?;
                crate::repositories::storage::write_binary_atomic(&target_path, &bytes)
                    .map_err(DatabaseError::Internal)?;
            }
            (None, Some(native_path)) => {
                let source_path = PathBuf::from(&native_path);
                if !source_path.is_file() {
                    return Err(DatabaseError::Internal(format!(
                        "Native recording source file does not exist: {}",
                        source_path.to_string_lossy()
                    )));
                }
                let target_path = self.audio_path(&item.audio_path)?;
                if source_path != target_path {
                    if let Some(parent) = target_path.parent() {
                        fs::create_dir_all(parent)
                            .map_err(|e| DatabaseError::Internal(e.to_string()))?;
                    }
                    fs::copy(&source_path, &target_path)
                        .map_err(|e| DatabaseError::Internal(e.to_string()))?;
                }
            }
            (None, None) => {
                return Err(DatabaseError::Internal(
                    "History recording save requires audio bytes or a native audio path."
                        .to_string(),
                ));
            }
        }

        let segments_str = serde_json::to_string(&normalized_transcript.segments)?;

        self.get_db()?.with_transaction(|tx| {
            Self::insert_history_item_and_transcript(tx, &item, &segments_str)?;
            Ok(())
        })?;

        Ok(item)
    }

    fn save_imported_file(
        &self,
        request: HistorySaveImportedFileRequest,
    ) -> Result<HistoryItemRecord, DatabaseError> {
        self.ensure_ready()?;
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
        let imported = super::item_factory::create_imported_file_item(
            id,
            source_path,
            converted_source_path,
            duration,
            project_id,
        )
        .map_err(DatabaseError::Internal)?;
        let mut item = imported.item;
        item.preview_text = normalized_transcript.preview_text;
        item.search_content = normalized_transcript.search_content;

        let source = PathBuf::from(imported.copy_source_path);
        if !source.is_file() {
            return Err(DatabaseError::Internal(format!(
                "Imported source file does not exist: {}",
                source.to_string_lossy()
            )));
        }

        let target_path = self.audio_path(&item.audio_path)?;
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|e| DatabaseError::Internal(e.to_string()))?;
        }
        fs::copy(&source, &target_path).map_err(|e| DatabaseError::Internal(e.to_string()))?;

        let segments_str = serde_json::to_string(&normalized_transcript.segments)?;

        self.get_db()?.with_transaction(|tx| {
            Self::insert_history_item_and_transcript(tx, &item, &segments_str)?;
            Ok(())
        })?;

        Ok(item)
    }

    fn delete_items(&self, ids: &[String]) -> Result<(), DatabaseError> {
        if ids.is_empty() {
            return Ok(());
        }

        let audio_paths = self.get_db()?.with_connection(|conn| {
            let mut audio_paths = Vec::new();
            let mut stmt =
                conn.prepare_cached("SELECT audio_path FROM history_items WHERE id = ?1")?;
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
                crate::repositories::storage::remove_path_if_exists(&path)
                    .map_err(DatabaseError::Internal)?;
            }
        }

        self.get_db()?.with_transaction(|tx| {
            let mut stmt = tx.prepare_cached("DELETE FROM history_items WHERE id = ?1")?;
            for id in ids {
                stmt.execute([id])?;
            }
            Ok(())
        })?;

        Ok(())
    }

    fn load_transcript(
        &self,
        history_id: &str,
    ) -> Result<Option<Vec<TranscriptSegment>>, DatabaseError> {
        self.get_db()?.with_connection(|conn| {
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
        })
    }

    fn update_transcript(
        &self,
        history_id: &str,
        segments: Value,
    ) -> Result<HistoryItemRecord, DatabaseError> {
        let normalized_transcript =
            normalize_history_transcript_segments(segments).map_err(DatabaseError::Internal)?;
        let segments_str = serde_json::to_string(&normalized_transcript.segments)?;

        let exists: bool = self.get_db()?.with_connection(|conn| {
            Ok(conn
                .query_row(
                    "SELECT 1 FROM history_items WHERE id = ?1",
                    [history_id],
                    |_| Ok(()),
                )
                .is_ok())
        })?;

        if !exists {
            return Err(DatabaseError::NotFoundError(format!(
                "History item not found: {history_id}"
            )));
        }

        self.get_db()?.with_transaction(|tx| {
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

        self.get_db()?.with_connection(|conn| {
            let mut stmt = conn.prepare_cached(
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
    ) -> Result<TranscriptSnapshotMetadata, DatabaseError> {
        validate_id(history_id, "History ID").map_err(DatabaseError::Internal)?;
        let normalized_transcript =
            normalize_history_transcript_segments(segments).map_err(DatabaseError::Internal)?;
        let parsed_segments = normalized_transcript.segments;

        let created_at =
            super::item_factory::current_time_millis().map_err(DatabaseError::Internal)?;
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

        let segments_str = serde_json::to_string(&parsed_segments)?;

        self.get_db()?.with_transaction(|tx| {
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM history_items WHERE id = ?1)",
                [history_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(DatabaseError::NotFoundError(format!(
                    "History item not found: {history_id}"
                )));
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
    ) -> Result<Vec<TranscriptSnapshotMetadata>, DatabaseError> {
        validate_id(history_id, "History ID").map_err(DatabaseError::Internal)?;
        self.get_db()?.with_connection(|conn| {
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
        })
    }

    fn load_transcript_snapshot(
        &self,
        history_id: &str,
        snapshot_id: &str,
    ) -> Result<Option<TranscriptSnapshotRecord>, DatabaseError> {
        validate_id(history_id, "History ID").map_err(DatabaseError::Internal)?;
        validate_id(snapshot_id, "Snapshot ID").map_err(DatabaseError::Internal)?;

        self.get_db()?.with_connection(|conn| {
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
        })
    }

    fn update_item_meta(&self, history_id: &str, updates: Value) -> Result<(), DatabaseError> {
        let updates = updates.as_object().ok_or_else(|| {
            DatabaseError::Internal("History item updates must be an object.".to_string())
        })?;

        self.get_db()?.with_transaction(|tx| {
            let mut stmt = tx.prepare_cached(
                "SELECT id, timestamp, duration, audio_path, transcript_path, title, preview_text, icon, kind, search_content, project_id, status, draft_source
                 FROM history_items
                 WHERE id = ?1"
            )?;
            let mut item = match stmt.query_row([history_id], map_row_to_item) {
                Ok(item) => item,
                Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(()),
                Err(error) => return Err(DatabaseError::QueryError(error)),
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
    ) -> Result<(), DatabaseError> {
        if ids.is_empty() {
            return Ok(());
        }

        self.get_db()?.with_transaction(|tx| {
            let mut stmt =
                tx.prepare_cached("UPDATE history_items SET project_id = ?1 WHERE id = ?2")?;
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
    ) -> Result<(), DatabaseError> {
        self.get_db()?.with_connection(|conn| {
            conn.execute(
                "UPDATE history_items SET project_id = ?1 WHERE project_id = ?2",
                rusqlite::params![next_project_id, current_project_id],
            )?;
            Ok(())
        })
    }

    fn load_summary(&self, history_id: &str) -> Result<Option<Value>, DatabaseError> {
        validate_id(history_id, "History ID").map_err(DatabaseError::Internal)?;

        self.get_db()?.with_connection(|conn| {
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
        })
    }

    fn save_summary(&self, history_id: &str, summary_payload: Value) -> Result<(), DatabaseError> {
        validate_id(history_id, "History ID").map_err(DatabaseError::Internal)?;

        let summary_payload = crate::repositories::history::fs_utils::ensure_json_object_value(
            summary_payload,
            "History summary payload",
        )
        .map_err(DatabaseError::Internal)?;

        let payload_str = serde_json::to_string(&summary_payload)?;

        self.get_db()?.with_connection(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO history_summaries (history_id, payload) VALUES (?1, ?2)",
                rusqlite::params![history_id, payload_str],
            )?;
            Ok(())
        })
    }

    fn delete_summary(&self, history_id: &str) -> Result<(), DatabaseError> {
        validate_id(history_id, "History ID").map_err(DatabaseError::Internal)?;

        self.get_db()?.with_connection(|conn| {
            conn.execute(
                "DELETE FROM history_summaries WHERE history_id = ?1",
                [history_id],
            )?;
            Ok(())
        })
    }

    fn resolve_audio_path(&self, history_id: &str) -> Result<Option<String>, DatabaseError> {
        validate_id(history_id, "History ID").map_err(DatabaseError::Internal)?;

        let audio_path_opt: Option<String> = self.get_db()?.with_connection(|conn| {
            let mut stmt =
                conn.prepare_cached("SELECT audio_path FROM history_items WHERE id = ?1")?;
            let mut rows = stmt.query([history_id])?;
            if let Some(row) = rows.next()? {
                let p: String = row.get(0)?;
                Ok(Some(p))
            } else {
                Ok(None)
            }
        })?;

        let Some(audio_path_str) = audio_path_opt else {
            return Err(DatabaseError::NotFoundError(format!(
                "History item not found: {history_id}"
            )));
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
            Err(error) => Err(DatabaseError::Internal(error.to_string())),
        }
    }

    fn history_snapshot_for_backup(&self) -> Result<HistoryBackupSnapshot, DatabaseError> {
        let items = self.get_db()?.with_connection(|conn| {
            let mut stmt = conn.prepare_cached(
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

        let (transcript_files, summary_files, snapshot_files) = self.get_db()?.with_connection(|conn| {
            // Bulk fetch transcripts
            let mut transcript_files: Vec<(String, Value)> = Vec::with_capacity(items.len());

            {
                let sql = format!(
                    "SELECT history_id, segments FROM history_transcripts WHERE history_id IN ({})",
                    placeholders
                );
                let mut stmt = conn.prepare_cached(&sql)?;
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
                        ).map_err(DatabaseError::Internal)?,
                        None => {
                            return Err(DatabaseError::NotFoundError(
                                format!("History item \"{}\" is missing its transcript file.", item.title),
                            ));
                        }
                    };
                    transcript_files.push((format!("{}.json", item.id), transcript_val));
                }
            }

            // Bulk fetch summaries
            let mut summary_files: Vec<(String, Value)> = Vec::new();
            {
                let sql = format!(
                    "SELECT history_id, payload FROM history_summaries WHERE history_id IN ({})",
                    placeholders
                );
                let mut stmt = conn.prepare_cached(&sql)?;
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
                let mut stmt = conn.prepare_cached(&sql)?;
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
                            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e)))
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
                            serde_json::to_value(&meta_list)?,
                        ));
                        for rec in records {
                            snapshot_files.push((
                                format!("versions/{}/{}.json", item.id, rec.metadata.id),
                                serde_json::to_value(&rec)?,
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

fn build_fts_query(query: &str) -> String {
    let normalized = super::workspace_query::normalize_workspace_search_text(query);
    let terms: Vec<&str> = normalized
        .text
        .split(|c: char| c.is_whitespace() || ",.?!;:()[]{}<>'/\\|~`-=_+\"".contains(c))
        .filter(|s| !s.is_empty())
        .collect();

    // FTS5 trigram tokenizer operates at the byte level and needs at least 3
    // bytes per term to form a trigram. Shorter terms would silently match
    // nothing, causing the entire AND query to return zero results.
    if terms.iter().any(|t| t.len() < 3) {
        return String::new();
    }

    terms
        .iter()
        .map(|s| {
            let escaped = s.replace('"', "\"\"");
            format!("\"{}\"", escaped)
        })
        .collect::<Vec<_>>()
        .join(" AND ")
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

    #[test]
    fn test_build_fts_query() {
        assert_eq!(build_fts_query("hello world"), "\"hello\" AND \"world\"");
        assert_eq!(build_fts_query("hello, world!"), "\"hello\" AND \"world\"");
        assert_eq!(
            build_fts_query("hello \"world\""),
            "\"hello\" AND \"world\""
        );
        assert_eq!(build_fts_query("  "), "");

        // Short queries (< 3 bytes) are skipped — trigram can't form a match
        assert_eq!(build_fts_query("ab"), "");
        assert_eq!(build_fts_query("a"), "");
        assert_eq!(build_fts_query("ab cd"), "");
        assert_eq!(build_fts_query("hello ab"), "");

        // 3-byte minimum: "abc" has exactly 3 bytes → one trigram possible
        assert_eq!(build_fts_query("abc"), "\"abc\"");
    }

    #[test]
    fn test_workspace_query_with_reconciliation() {
        use crate::repositories::history::{
            HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType, HistoryWorkspaceScope,
            HistoryWorkspaceSortOrder,
        };

        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

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
        use crate::repositories::history::{
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
        };
        let res_zh = store.query_workspace(req_zh).unwrap();
        assert_eq!(res_zh.filtered_items.len(), 1);
        assert_eq!(res_zh.filtered_items[0].id, item.id);

        // 2. Test fuzzy/substring match
        let req_fuzzy = HistoryWorkspaceQueryRequest {
            scope: HistoryWorkspaceScope::All,
            query: "fuzzy".to_string(),
            filter_type: HistoryWorkspaceFilterType::All,
            date_filter: HistoryWorkspaceDateFilter::All,
            sort_order: HistoryWorkspaceSortOrder::Newest,
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
        };
        let res_short_en = store.query_workspace(req_short_en).unwrap();
        assert_eq!(res_short_en.filtered_items.len(), 1);
        assert_eq!(res_short_en.filtered_items[0].id, item.id);
    }
}
