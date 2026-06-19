use serde_json::{Map, Value, to_value};
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use uuid::Uuid;

use super::fs_utils::{
    ensure_json_array_value, ensure_json_object_value, ensure_safe_file_name,
    optional_history_child_path, read_json_value, remove_path_if_exists, write_binary_atomic,
    write_json_pretty_atomic,
};
use super::item_factory::{
    create_imported_file_item, create_live_draft_item, create_recording_item, current_time_millis,
};
use super::transcript_payload::normalize_history_transcript_segments;
use super::types::HistoryBackupSnapshot;
use super::{
    HISTORY_DIR_NAME, HISTORY_INDEX_FILE_NAME, HISTORY_VERSIONS_DIR_NAME, SUMMARY_FILE_SUFFIX,
    TRANSCRIPT_SNAPSHOT_RETENTION_LIMIT,
};
use super::{
    HistoryCreateLiveDraftRequest, HistoryDraftSource, HistoryItemKind, HistoryItemRecord,
    HistoryItemStatus, HistorySaveImportedFileRequest, HistorySaveRecordingRequest,
    HistoryWorkspaceQueryRequest, HistoryWorkspaceQueryResult, LiveRecordingDraftResult,
    TranscriptSnapshotMetadata, TranscriptSnapshotReason, TranscriptSnapshotRecord,
};

#[derive(Clone)]
pub struct HistoryRepository {
    app_local_data_dir: PathBuf,
}

impl HistoryRepository {
    pub fn new(app_local_data_dir: PathBuf) -> Self {
        Self { app_local_data_dir }
    }

    pub(crate) fn history_dir(&self) -> PathBuf {
        self.app_local_data_dir.join(HISTORY_DIR_NAME)
    }

    pub(crate) fn history_index_path(&self) -> PathBuf {
        self.history_dir().join(HISTORY_INDEX_FILE_NAME)
    }

    pub(crate) fn ensure_ready(&self) -> Result<(), String> {
        let history_dir = self.history_dir();
        fs::create_dir_all(&history_dir).map_err(|error| error.to_string())?;

        let index_path = self.history_index_path();
        if !index_path.exists() {
            write_json_pretty_atomic(&index_path, &Value::Array(Vec::new()))?;
        }

        Ok(())
    }

    pub fn list_items(&self) -> Result<Vec<HistoryItemRecord>, String> {
        self.read_index_items()
    }

    pub(crate) fn list_items_with_reconciled_live_drafts(
        &self,
    ) -> Result<Vec<HistoryItemRecord>, String> {
        let mut items = self.read_index_items()?;
        let mut changed = false;

        for item in &mut items {
            changed |= self.reconcile_live_draft_item(item)?;
        }

        if changed {
            self.write_index(&items)?;
        }

        Ok(items)
    }

    fn read_index_items(&self) -> Result<Vec<HistoryItemRecord>, String> {
        self.ensure_ready()?;
        let raw = read_json_value(&self.history_index_path())?;
        let items = raw
            .as_array()
            .ok_or_else(|| "History index must be an array.".to_string())?
            .iter()
            .map(normalize_history_item_value)
            .collect();
        Ok(items)
    }

    fn reconcile_live_draft_item(&self, item: &mut HistoryItemRecord) -> Result<bool, String> {
        if item.status != HistoryItemStatus::Draft
            || item.draft_source != Some(HistoryDraftSource::LiveRecord)
        {
            return Ok(false);
        }

        let Some(audio_path) = optional_history_child_path(&self.history_dir(), &item.audio_path)
        else {
            return Ok(false);
        };
        let audio_metadata = match fs::metadata(&audio_path) {
            Ok(metadata) if metadata.is_file() && metadata.len() > 0 => metadata,
            _ => return Ok(false),
        };

        let Some(transcript_path) =
            optional_history_child_path(&self.history_dir(), &item.transcript_path)
        else {
            return Ok(false);
        };
        if !transcript_path.exists() {
            return Ok(false);
        }

        let transcript_value = match read_json_value(&transcript_path) {
            Ok(value) => value,
            Err(error) => {
                log::warn!(
                    "[History] Skipping live draft reconciliation for {}: {}",
                    item.id,
                    error
                );
                return Ok(false);
            }
        };
        let normalized_transcript = match normalize_history_transcript_segments(transcript_value) {
            Ok(transcript) if !transcript.segments.is_empty() => transcript,
            Ok(_) => return Ok(false),
            Err(error) => {
                log::warn!(
                    "[History] Skipping live draft reconciliation for {}: {}",
                    item.id,
                    error
                );
                return Ok(false);
            }
        };

        write_json_pretty_atomic(&transcript_path, &normalized_transcript.segments)?;
        item.preview_text = normalized_transcript.preview_text;
        item.search_content = normalized_transcript.search_content;
        let transcript_duration = normalized_transcript
            .segments
            .iter()
            .filter_map(|segment| segment.end.is_finite().then_some(segment.end))
            .fold(0.0, f64::max);
        item.duration = item.duration.max(transcript_duration).max(0.0);
        if audio_metadata.len() == 0 {
            return Ok(false);
        }
        item.status = HistoryItemStatus::Complete;
        item.draft_source = None;

        Ok(true)
    }

    pub(crate) fn query_workspace(
        &self,
        request: HistoryWorkspaceQueryRequest,
    ) -> Result<HistoryWorkspaceQueryResult, String> {
        let items = self.list_items_with_reconciled_live_drafts()?;
        Ok(super::workspace_query::query_workspace_items(
            items, request,
        ))
    }

    pub(crate) fn insert_item_at_front(
        &self,
        item: HistoryItemRecord,
    ) -> Result<HistoryItemRecord, String> {
        let mut items = self.list_items()?;
        items.retain(|existing| existing.id != item.id);
        items.insert(0, item.clone());
        self.write_index(&items)?;
        Ok(item)
    }

    pub(crate) fn write_index(&self, items: &[HistoryItemRecord]) -> Result<(), String> {
        write_json_pretty_atomic(&self.history_index_path(), items)
    }

    pub(crate) fn transcript_path(&self, file_name: &str) -> Result<PathBuf, String> {
        Ok(self
            .history_dir()
            .join(ensure_safe_file_name(file_name, "History transcript path")?))
    }

    pub(crate) fn audio_path(&self, file_name: &str) -> Result<PathBuf, String> {
        Ok(self
            .history_dir()
            .join(ensure_safe_file_name(file_name, "History audio path")?))
    }

    pub(crate) fn summary_path(&self, history_id: &str) -> Result<PathBuf, String> {
        let safe_history_id = ensure_safe_file_name(history_id, "History summary id")?;
        Ok(self
            .history_dir()
            .join(format!("{safe_history_id}{SUMMARY_FILE_SUFFIX}")))
    }

    pub(crate) fn transcript_versions_dir(&self, history_id: &str) -> Result<PathBuf, String> {
        let safe_history_id = ensure_safe_file_name(history_id, "History version id")?;
        Ok(self
            .history_dir()
            .join(HISTORY_VERSIONS_DIR_NAME)
            .join(safe_history_id))
    }

    pub(crate) fn transcript_snapshot_index_path(
        &self,
        history_id: &str,
    ) -> Result<PathBuf, String> {
        Ok(self
            .transcript_versions_dir(history_id)?
            .join(HISTORY_INDEX_FILE_NAME))
    }

    pub(crate) fn transcript_snapshot_path(
        &self,
        history_id: &str,
        snapshot_id: &str,
    ) -> Result<PathBuf, String> {
        let safe_snapshot_id = ensure_safe_file_name(snapshot_id, "Transcript snapshot id")?;
        Ok(self
            .transcript_versions_dir(history_id)?
            .join(format!("{safe_snapshot_id}.json")))
    }

    pub(crate) fn create_live_draft(
        &self,
        request: HistoryCreateLiveDraftRequest,
    ) -> Result<LiveRecordingDraftResult, String> {
        self.ensure_ready()?;
        let item = create_live_draft_item(request)?;
        write_json_pretty_atomic(
            &self.transcript_path(&item.transcript_path)?,
            &Value::Array(Vec::new()),
        )?;
        let item = self.insert_item_at_front(item)?;
        let audio_absolute_path = self
            .audio_path(&item.audio_path)?
            .to_string_lossy()
            .into_owned();
        Ok(LiveRecordingDraftResult {
            item,
            audio_absolute_path,
        })
    }

    pub(crate) fn complete_live_draft(
        &self,
        history_id: &str,
        segments: Value,
        duration: f64,
    ) -> Result<HistoryItemRecord, String> {
        let normalized_transcript = normalize_history_transcript_segments(segments)?;
        let mut items = self.list_items()?;
        let item = items
            .iter_mut()
            .find(|entry| entry.id == history_id)
            .ok_or_else(|| format!("History item not found: {history_id}"))?;
        write_json_pretty_atomic(
            &self.transcript_path(&item.transcript_path)?,
            &normalized_transcript.segments,
        )?;
        item.preview_text = normalized_transcript.preview_text;
        item.search_content = normalized_transcript.search_content;
        item.duration = duration.max(0.0);
        item.status = HistoryItemStatus::Complete;
        item.draft_source = None;
        let updated = item.clone();
        self.write_index(&items)?;
        Ok(updated)
    }

    pub(crate) fn save_recording(
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
        let mut item = create_recording_item(
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
                write_binary_atomic(&target_path, &bytes)?;
            }
            (None, Some(native_audio_path)) => {
                let source_path = PathBuf::from(native_audio_path);
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

        write_json_pretty_atomic(
            &self.transcript_path(&item.transcript_path)?,
            &normalized_transcript.segments,
        )?;
        self.insert_item_at_front(item)
    }

    pub(crate) fn save_imported_file(
        &self,
        request: HistorySaveImportedFileRequest,
    ) -> Result<HistoryItemRecord, String> {
        self.ensure_ready()?;
        let HistorySaveImportedFileRequest {
            source_path,
            segments,
            duration,
            project_id,
            converted_source_path,
        } = request;
        let normalized_transcript = normalize_history_transcript_segments(segments)?;
        let imported =
            create_imported_file_item(source_path, converted_source_path, duration, project_id)?;
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
        write_json_pretty_atomic(
            &self.transcript_path(&item.transcript_path)?,
            &normalized_transcript.segments,
        )?;
        self.insert_item_at_front(item)
    }

    pub(crate) fn delete_items(&self, ids: &[String]) -> Result<(), String> {
        if ids.is_empty() {
            return Ok(());
        }

        let id_set = ids
            .iter()
            .cloned()
            .collect::<std::collections::HashSet<_>>();
        let items = self.list_items()?;
        let items_to_delete = items
            .iter()
            .filter(|item| id_set.contains(&item.id))
            .cloned()
            .collect::<Vec<_>>();

        for item in &items_to_delete {
            if let Some(path) = optional_history_child_path(&self.history_dir(), &item.audio_path) {
                remove_path_if_exists(&path)?;
            }
            if let Some(path) =
                optional_history_child_path(&self.history_dir(), &item.transcript_path)
            {
                remove_path_if_exists(&path)?;
            }
            if let Ok(path) = self.summary_path(&item.id) {
                remove_path_if_exists(&path)?;
            }
            if let Ok(path) = self.transcript_versions_dir(&item.id) {
                remove_path_if_exists(&path)?;
            }
        }

        let next_items = items
            .into_iter()
            .filter(|item| !id_set.contains(&item.id))
            .collect::<Vec<_>>();
        self.write_index(&next_items)?;
        Ok(())
    }

    pub fn load_transcript(
        &self,
        file_name: &str,
    ) -> Result<Option<Vec<crate::integrations::asr::TranscriptSegment>>, String> {
        let transcript_path = self.transcript_path(file_name)?;
        if !transcript_path.exists() {
            return Ok(None);
        }

        let value = read_json_value(&transcript_path)?;
        Ok(Some(normalize_history_transcript_segments(value)?.segments))
    }

    pub(crate) fn create_transcript_snapshot(
        &self,
        history_id: &str,
        reason: TranscriptSnapshotReason,
        segments: Value,
    ) -> Result<TranscriptSnapshotMetadata, String> {
        self.ensure_ready()?;
        let normalized_transcript = normalize_history_transcript_segments(segments)?;
        let segments = normalized_transcript.segments;
        let items = self.list_items()?;
        if !items.iter().any(|entry| entry.id == history_id) {
            return Err(format!("History item not found: {history_id}"));
        }

        let created_at = current_time_millis()?;
        let metadata = TranscriptSnapshotMetadata {
            id: format!("{created_at}-{}", Uuid::new_v4()),
            history_id: history_id.to_string(),
            reason,
            created_at,
            segment_count: segments.len() as u64,
        };

        let versions_dir = self.transcript_versions_dir(history_id)?;
        fs::create_dir_all(&versions_dir).map_err(|error| error.to_string())?;

        let record = TranscriptSnapshotRecord {
            metadata: metadata.clone(),
            segments,
        };
        write_json_pretty_atomic(
            &self.transcript_snapshot_path(history_id, &metadata.id)?,
            &record,
        )?;

        let mut index = self.list_transcript_snapshots(history_id)?;
        index.retain(|entry| entry.id != metadata.id);
        index.insert(0, metadata.clone());
        index.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| b.id.cmp(&a.id))
        });

        let pruned = if index.len() > TRANSCRIPT_SNAPSHOT_RETENTION_LIMIT {
            index.split_off(TRANSCRIPT_SNAPSHOT_RETENTION_LIMIT)
        } else {
            Vec::new()
        };

        for snapshot in &pruned {
            remove_path_if_exists(&self.transcript_snapshot_path(history_id, &snapshot.id)?)?;
        }

        write_json_pretty_atomic(&self.transcript_snapshot_index_path(history_id)?, &index)?;
        Ok(metadata)
    }

    pub(crate) fn list_transcript_snapshots(
        &self,
        history_id: &str,
    ) -> Result<Vec<TranscriptSnapshotMetadata>, String> {
        let index_path = self.transcript_snapshot_index_path(history_id)?;
        if !index_path.exists() {
            return Ok(Vec::new());
        }

        let raw = read_json_value(&index_path)?;
        let mut entries: Vec<TranscriptSnapshotMetadata> =
            serde_json::from_value(raw).map_err(|error| error.to_string())?;
        entries.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| b.id.cmp(&a.id))
        });
        Ok(entries)
    }

    pub(crate) fn load_transcript_snapshot(
        &self,
        history_id: &str,
        snapshot_id: &str,
    ) -> Result<Option<TranscriptSnapshotRecord>, String> {
        let snapshot_path = self.transcript_snapshot_path(history_id, snapshot_id)?;
        if !snapshot_path.exists() {
            return Ok(None);
        }

        let raw = read_json_value(&snapshot_path)?;
        let mut record: TranscriptSnapshotRecord =
            serde_json::from_value(raw).map_err(|error| error.to_string())?;
        record.segments = normalize_history_transcript_segments(
            to_value(record.segments).map_err(|error| error.to_string())?,
        )?
        .segments;
        Ok(Some(record))
    }

    pub(crate) fn update_transcript(
        &self,
        history_id: &str,
        segments: Value,
    ) -> Result<HistoryItemRecord, String> {
        let normalized_transcript = normalize_history_transcript_segments(segments)?;
        let mut items = self.list_items()?;
        let item = items
            .iter_mut()
            .find(|entry| entry.id == history_id)
            .ok_or_else(|| format!("History item not found: {history_id}"))?;
        write_json_pretty_atomic(
            &self.transcript_path(&item.transcript_path)?,
            &normalized_transcript.segments,
        )?;
        item.preview_text = normalized_transcript.preview_text;
        item.search_content = normalized_transcript.search_content;
        let updated = item.clone();
        self.write_index(&items)?;
        Ok(updated)
    }

    pub(crate) fn update_item_meta(&self, history_id: &str, updates: Value) -> Result<(), String> {
        let updates = updates
            .as_object()
            .ok_or_else(|| "History item updates must be an object.".to_string())?;
        let mut items = self.list_items()?;
        let Some(item) = items.iter_mut().find(|entry| entry.id == history_id) else {
            return Ok(());
        };

        apply_history_item_updates(item, updates);
        self.write_index(&items)?;
        Ok(())
    }

    pub(crate) fn update_project_assignments(
        &self,
        ids: &[String],
        project_id: Option<String>,
    ) -> Result<(), String> {
        if ids.is_empty() {
            return Ok(());
        }

        let id_set = ids
            .iter()
            .cloned()
            .collect::<std::collections::HashSet<_>>();
        let mut items = self.list_items()?;
        for item in &mut items {
            if id_set.contains(&item.id) {
                item.project_id = project_id.clone();
            }
        }
        self.write_index(&items)?;
        Ok(())
    }

    pub(crate) fn reassign_project(
        &self,
        current_project_id: String,
        next_project_id: Option<String>,
    ) -> Result<(), String> {
        let mut items = self.list_items()?;
        for item in &mut items {
            if item.project_id.as_deref() == Some(current_project_id.as_str()) {
                item.project_id = next_project_id.clone();
            }
        }
        self.write_index(&items)?;
        Ok(())
    }

    pub(crate) fn load_summary(&self, history_id: &str) -> Result<Option<Value>, String> {
        let summary_path = self.summary_path(history_id)?;
        if !summary_path.exists() {
            return Ok(None);
        }

        let value = read_json_value(&summary_path)?;
        Ok(Some(ensure_json_object_value(
            value,
            "History summary file",
        )?))
    }

    pub(crate) fn save_summary(
        &self,
        history_id: &str,
        summary_payload: Value,
    ) -> Result<(), String> {
        self.ensure_ready()?;
        let summary_payload = ensure_json_object_value(summary_payload, "History summary payload")?;
        write_json_pretty_atomic(&self.summary_path(history_id)?, &summary_payload)
    }

    pub(crate) fn delete_summary(&self, history_id: &str) -> Result<(), String> {
        let summary_path = self.summary_path(history_id)?;
        remove_path_if_exists(&summary_path)
    }

    pub(crate) fn resolve_audio_path(&self, file_name: &str) -> Result<Option<String>, String> {
        let Some(audio_path) = optional_history_child_path(&self.history_dir(), file_name) else {
            return Ok(None);
        };

        match fs::metadata(&audio_path) {
            Ok(metadata) if metadata.is_file() && metadata.len() > 0 => {
                Ok(Some(audio_path.to_string_lossy().into_owned()))
            }
            Ok(_) => Ok(None),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    pub(crate) fn history_snapshot_for_backup(&self) -> Result<HistoryBackupSnapshot, String> {
        let items = self
            .list_items()?
            .into_iter()
            .filter(|item| item.status != HistoryItemStatus::Draft)
            .map(|mut item| {
                item.status = HistoryItemStatus::Complete;
                item.draft_source = None;
                item
            })
            .collect::<Vec<_>>();

        let mut transcript_files = Vec::with_capacity(items.len());
        let mut summary_files = Vec::new();
        let mut snapshot_files = Vec::new();
        let history_dir = self.history_dir();

        for item in &items {
            let transcript_path = self.transcript_path(&item.transcript_path)?;
            if !transcript_path.exists() {
                return Err(format!(
                    "History item \"{}\" is missing its transcript file.",
                    item.title
                ));
            }
            let transcript = ensure_json_array_value(
                read_json_value(&transcript_path)?,
                &format!("Transcript for history item {}", item.id),
            )?;
            transcript_files.push((item.transcript_path.clone(), transcript));

            let summary_path = self.summary_path(&item.id)?;
            if summary_path.exists() {
                let summary = ensure_json_object_value(
                    read_json_value(&summary_path)?,
                    &format!("Summary for history item {}", item.id),
                )?;
                summary_files.push((item.id.clone(), summary));
            }

            let snapshots = self.list_transcript_snapshots(&item.id)?;
            if !snapshots.is_empty() {
                let snapshot_index_path = self.transcript_snapshot_index_path(&item.id)?;
                let snapshot_index_relative = snapshot_index_path
                    .strip_prefix(&history_dir)
                    .map_err(|error| error.to_string())?
                    .to_string_lossy()
                    .into_owned();
                snapshot_files.push((
                    snapshot_index_relative,
                    to_value(&snapshots).map_err(|error| error.to_string())?,
                ));
            }

            for snapshot in snapshots {
                let record = self
                    .load_transcript_snapshot(&item.id, &snapshot.id)?
                    .ok_or_else(|| {
                        format!(
                            "Transcript snapshot \"{}\" for history item \"{}\" is missing.",
                            snapshot.id, item.id
                        )
                    })?;
                let snapshot_path = self.transcript_snapshot_path(&item.id, &snapshot.id)?;
                let snapshot_relative = snapshot_path
                    .strip_prefix(&history_dir)
                    .map_err(|error| error.to_string())?
                    .to_string_lossy()
                    .into_owned();
                snapshot_files.push((
                    snapshot_relative,
                    to_value(&record).map_err(|error| error.to_string())?,
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
}

pub(crate) fn normalize_history_item_value(value: &Value) -> HistoryItemRecord {
    let object = value.as_object();

    HistoryItemRecord {
        id: object
            .and_then(|map| map.get("id"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        timestamp: object
            .and_then(|map| map.get("timestamp"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        duration: object
            .and_then(|map| map.get("duration"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            .max(0.0),
        audio_path: object
            .and_then(|map| map.get("audioPath"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        transcript_path: object
            .and_then(|map| map.get("transcriptPath"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        title: object
            .and_then(|map| map.get("title"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        preview_text: object
            .and_then(|map| map.get("previewText"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        icon: object
            .and_then(|map| map.get("icon"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        kind: match object
            .and_then(|map| map.get("type"))
            .and_then(Value::as_str)
        {
            Some("batch") => HistoryItemKind::Batch,
            _ => HistoryItemKind::Recording,
        },
        search_content: object
            .and_then(|map| map.get("searchContent"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        project_id: object
            .and_then(|map| map.get("projectId"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        status: match object
            .and_then(|map| map.get("status"))
            .and_then(Value::as_str)
        {
            Some("draft") => HistoryItemStatus::Draft,
            _ => HistoryItemStatus::Complete,
        },
        draft_source: match object
            .and_then(|map| map.get("draftSource"))
            .and_then(Value::as_str)
        {
            Some("live_record") => Some(HistoryDraftSource::LiveRecord),
            _ => None,
        },
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::history::fs_utils::write_json_pretty_atomic;
    use crate::repositories::history::test_support::sample_history_item;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;
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
    fn history_delete_items_removes_files_and_updates_index() {
        let root = tempdir().unwrap();
        let repository = HistoryRepository::new(root.path().to_path_buf());
        repository.ensure_ready().unwrap();

        let keep_item = sample_history_item("keep", HistoryItemStatus::Complete);
        let delete_item = sample_history_item("delete", HistoryItemStatus::Complete);
        repository
            .write_index(&[keep_item.clone(), delete_item.clone()])
            .unwrap();
        fs::write(
            repository.audio_path(&delete_item.audio_path).unwrap(),
            b"audio",
        )
        .unwrap();
        write_json_pretty_atomic(
            &repository
                .transcript_path(&delete_item.transcript_path)
                .unwrap(),
            &json!([]),
        )
        .unwrap();
        write_json_pretty_atomic(
            &repository.summary_path(&delete_item.id).unwrap(),
            &json!({}),
        )
        .unwrap();

        repository
            .delete_items(std::slice::from_ref(&delete_item.id))
            .unwrap();

        let items = repository.list_items().unwrap();
        assert_eq!(items, vec![keep_item]);
        assert!(
            !repository
                .audio_path(&delete_item.audio_path)
                .unwrap()
                .exists()
        );
        assert!(
            !repository
                .transcript_path(&delete_item.transcript_path)
                .unwrap()
                .exists()
        );
        assert!(!repository.summary_path(&delete_item.id).unwrap().exists());
    }

    #[test]
    fn live_draft_round_trip_updates_index_and_transcript() {
        let root = tempdir().unwrap();
        let repository = HistoryRepository::new(root.path().to_path_buf());

        let draft = repository
            .create_live_draft(HistoryCreateLiveDraftRequest {
                audio_extension: "webm".to_string(),
                project_id: Some("project-1".to_string()),
                icon: Some("system:mic".to_string()),
            })
            .unwrap();
        assert_eq!(draft.item.status, HistoryItemStatus::Draft);
        assert!(!draft.item.id.is_empty());
        assert!(draft.item.audio_path.ends_with(".webm"));
        assert_eq!(draft.item.audio_path, format!("{}.webm", draft.item.id));
        assert_eq!(
            draft.item.transcript_path,
            format!("{}.json", draft.item.id)
        );
        assert!(draft.item.title.starts_with("Recording "));
        assert_eq!(draft.item.project_id.as_deref(), Some("project-1"));
        assert_eq!(draft.item.icon.as_deref(), Some("system:mic"));
        assert_eq!(draft.item.kind, HistoryItemKind::Recording);
        assert_eq!(
            draft.item.draft_source,
            Some(HistoryDraftSource::LiveRecord)
        );
        assert!(PathBuf::from(&draft.audio_absolute_path).ends_with(&draft.item.audio_path));

        let completed = repository
            .complete_live_draft(
                &draft.item.id,
                json!([segment_value("seg-1", "hello", 0.0, 3.0)]),
                3.0,
            )
            .unwrap();

        assert_eq!(completed.status, HistoryItemStatus::Complete);
        assert_eq!(completed.preview_text, "hello...");
        let transcript = repository
            .load_transcript(&draft.item.transcript_path)
            .unwrap()
            .unwrap();
        assert_eq!(transcript.len(), 1);
    }

    #[test]
    fn list_items_reconciles_restart_live_drafts_with_saved_audio_and_transcript() {
        let root = tempdir().unwrap();
        let repository = HistoryRepository::new(root.path().to_path_buf());

        let draft = repository
            .create_live_draft(HistoryCreateLiveDraftRequest {
                audio_extension: "wav".to_string(),
                project_id: None,
                icon: Some("system:mic".to_string()),
            })
            .unwrap();
        fs::write(
            repository.audio_path(&draft.item.audio_path).unwrap(),
            b"audio",
        )
        .unwrap();
        write_json_pretty_atomic(
            &repository
                .transcript_path(&draft.item.transcript_path)
                .unwrap(),
            &json!([segment_value("seg-1", "Recovered text", 0.0, 2.5)]),
        )
        .unwrap();

        let items = repository.list_items_with_reconciled_live_drafts().unwrap();

        assert_eq!(items[0].id, draft.item.id);
        assert_eq!(items[0].status, HistoryItemStatus::Complete);
        assert_eq!(items[0].draft_source, None);
        assert_eq!(items[0].preview_text, "Recovered text...");
        assert_eq!(items[0].search_content, "Recovered text");
        assert_eq!(items[0].duration, 2.5);

        let persisted = repository.list_items().unwrap();
        assert_eq!(persisted[0].status, HistoryItemStatus::Complete);
        assert_eq!(persisted[0].draft_source, None);
    }

    #[test]
    fn list_items_keeps_empty_or_missing_audio_live_drafts_as_drafts() {
        let root = tempdir().unwrap();
        let repository = HistoryRepository::new(root.path().to_path_buf());

        let draft = repository
            .create_live_draft(HistoryCreateLiveDraftRequest {
                audio_extension: "wav".to_string(),
                project_id: None,
                icon: Some("system:mic".to_string()),
            })
            .unwrap();

        let items = repository.list_items_with_reconciled_live_drafts().unwrap();

        assert_eq!(items[0].id, draft.item.id);
        assert_eq!(items[0].status, HistoryItemStatus::Draft);
        assert_eq!(items[0].draft_source, Some(HistoryDraftSource::LiveRecord));
    }

    #[test]
    fn list_items_keeps_empty_transcript_live_drafts_as_drafts() {
        let root = tempdir().unwrap();
        let repository = HistoryRepository::new(root.path().to_path_buf());

        let draft = repository
            .create_live_draft(HistoryCreateLiveDraftRequest {
                audio_extension: "wav".to_string(),
                project_id: None,
                icon: Some("system:mic".to_string()),
            })
            .unwrap();
        fs::write(
            repository.audio_path(&draft.item.audio_path).unwrap(),
            b"audio",
        )
        .unwrap();
        write_json_pretty_atomic(
            &repository
                .transcript_path(&draft.item.transcript_path)
                .unwrap(),
            &json!([]),
        )
        .unwrap();

        let items = repository.list_items_with_reconciled_live_drafts().unwrap();

        assert_eq!(items[0].id, draft.item.id);
        assert_eq!(items[0].status, HistoryItemStatus::Draft);
        assert_eq!(items[0].draft_source, Some(HistoryDraftSource::LiveRecord));
    }

    #[test]
    fn update_transcript_derives_metadata_from_normalized_segments() {
        let root = tempdir().unwrap();
        let repository = HistoryRepository::new(root.path().to_path_buf());
        repository.ensure_ready().unwrap();

        let mut item = sample_history_item("history-1", HistoryItemStatus::Complete);
        item.preview_text = "stale preview".to_string();
        item.search_content = "stale search".to_string();
        repository.write_index(&[item.clone()]).unwrap();

        let updated = repository
            .update_transcript(
                &item.id,
                json!([
                    segment_value("seg-1", "Alpha", 0.0, 1.0),
                    segment_value("seg-2", "Beta", 1.0, 2.0)
                ]),
            )
            .unwrap();

        assert_eq!(updated.preview_text, "Alpha Beta...");
        assert_eq!(updated.search_content, "Alpha Beta");

        let items = repository.list_items().unwrap();
        assert_eq!(items[0].preview_text, "Alpha Beta...");
        assert_eq!(items[0].search_content, "Alpha Beta");
    }

    #[test]
    fn save_recording_and_import_paths_derive_metadata_from_segments() {
        let root = tempdir().unwrap();
        let repository = HistoryRepository::new(root.path().to_path_buf());

        let recording = repository
            .save_recording(HistorySaveRecordingRequest {
                segments: json!([segment_value("seg-1", "Recorded text", 0.0, 1.0)]),
                duration: 2.0,
                project_id: Some("project-1".to_string()),
                audio_bytes: Some(vec![0, 1, 2]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();

        assert_eq!(recording.preview_text, "Recorded text...");
        assert_eq!(recording.search_content, "Recorded text");
        assert!(!recording.id.is_empty());
        assert_eq!(recording.audio_path, format!("{}.wav", recording.id));
        assert_eq!(recording.transcript_path, format!("{}.json", recording.id));
        assert!(recording.title.starts_with("Recording "));
        assert_eq!(recording.project_id.as_deref(), Some("project-1"));
        assert_eq!(recording.kind, HistoryItemKind::Recording);
        assert_eq!(recording.status, HistoryItemStatus::Complete);
        assert_eq!(recording.draft_source, None);

        let source_path = root.path().join("import-source.wav");
        fs::write(&source_path, [3, 4, 5]).unwrap();

        let imported = repository
            .save_imported_file(HistorySaveImportedFileRequest {
                source_path: source_path.to_string_lossy().into_owned(),
                segments: json!([segment_value("seg-1", "Imported text", 0.0, 1.0)]),
                duration: 2.0,
                project_id: Some("project-1".to_string()),
                converted_source_path: None,
            })
            .unwrap();

        assert_eq!(imported.preview_text, "Imported text...");
        assert_eq!(imported.search_content, "Imported text");
        assert_eq!(imported.audio_path, format!("{}.wav", imported.id));
        assert_eq!(imported.transcript_path, format!("{}.json", imported.id));
        assert_eq!(imported.title, "Batch import-source.wav");
        assert_eq!(imported.project_id.as_deref(), Some("project-1"));
        assert_eq!(imported.kind, HistoryItemKind::Batch);
    }

    #[test]
    fn imported_file_uses_original_name_for_title_and_converted_source_for_copy() {
        let root = tempdir().unwrap();
        let repository = HistoryRepository::new(root.path().to_path_buf());
        let original_path = root.path().join("meeting.mp3");
        let converted_path = root.path().join("converted.wav");
        fs::write(&original_path, [1, 2, 3]).unwrap();
        fs::write(&converted_path, [4, 5, 6]).unwrap();

        let imported = repository
            .save_imported_file(HistorySaveImportedFileRequest {
                source_path: original_path.to_string_lossy().into_owned(),
                segments: json!([segment_value("seg-1", "Converted text", 0.0, 1.0)]),
                duration: 1.0,
                project_id: None,
                converted_source_path: Some(converted_path.to_string_lossy().into_owned()),
            })
            .unwrap();

        assert_eq!(imported.title, "Batch meeting.mp3");
        assert_eq!(imported.audio_path, format!("{}.wav", imported.id));
        assert_eq!(
            fs::read(repository.audio_path(&imported.audio_path).unwrap()).unwrap(),
            vec![4, 5, 6]
        );
    }

    #[test]
    fn legacy_segment_timing_is_normalized_before_persisting() {
        let root = tempdir().unwrap();
        let repository = HistoryRepository::new(root.path().to_path_buf());

        let saved = repository
            .save_recording(HistorySaveRecordingRequest {
                segments: json!([{
                    "id": "seg-1",
                    "text": "\u{4f60}\u{597d}",
                    "start": 0.0,
                    "end": 1.0,
                    "isFinal": true,
                    "tokens": ["\u{4f60}", "\u{597d}"],
                    "timestamps": [0.0, 0.5],
                    "durations": [0.5, 0.5]
                }]),
                duration: 2.0,
                project_id: None,
                audio_bytes: Some(vec![0, 1, 2]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();

        let transcript = repository
            .load_transcript(&saved.transcript_path)
            .unwrap()
            .unwrap();
        let timing = transcript[0].timing.as_ref().unwrap();
        assert_eq!(
            timing.level,
            crate::integrations::asr::TranscriptTimingLevel::Token
        );
        assert_eq!(
            timing.source,
            crate::integrations::asr::TranscriptTimingSource::Model
        );
        assert_eq!(timing.units[0].text, "\u{4f60}");
        assert_eq!(timing.units[0].start, 0.0);
        assert_eq!(timing.units[1].text, "\u{597d}");
        assert_eq!(timing.units[1].end, 1.0);
    }

    #[test]
    fn load_transcript_normalizes_legacy_timing_fields_from_disk() {
        let root = tempdir().unwrap();
        let repository = HistoryRepository::new(root.path().to_path_buf());
        repository.ensure_ready().unwrap();

        write_json_pretty_atomic(
            &repository.transcript_path("legacy.json").unwrap(),
            &json!([{
                "id": "seg-1",
                "text": "\u{4f60}\u{597d}",
                "start": 0.0,
                "end": 1.0,
                "isFinal": true,
                "tokens": ["\u{4f60}", "\u{597d}"],
                "timestamps": [0.0, 0.5],
                "durations": [0.5, 0.5]
            }]),
        )
        .unwrap();

        let transcript = repository.load_transcript("legacy.json").unwrap().unwrap();
        let timing = transcript[0].timing.as_ref().unwrap();
        assert_eq!(
            timing.level,
            crate::integrations::asr::TranscriptTimingLevel::Token
        );
        assert_eq!(
            timing.source,
            crate::integrations::asr::TranscriptTimingSource::Model
        );
        assert_eq!(timing.units[0].text, "\u{4f60}");
        assert_eq!(timing.units[0].start, 0.0);
        assert_eq!(timing.units[1].text, "\u{597d}");
        assert_eq!(timing.units[1].end, 1.0);
    }

    #[test]
    fn summary_round_trip_saves_loads_and_deletes() {
        let root = tempdir().unwrap();
        let repository = HistoryRepository::new(root.path().to_path_buf());
        repository.ensure_ready().unwrap();

        repository
            .save_summary("history-1", json!({ "activeTemplateId": "general" }))
            .unwrap();
        assert_eq!(
            repository.load_summary("history-1").unwrap(),
            Some(json!({ "activeTemplateId": "general" }))
        );

        repository.delete_summary("history-1").unwrap();
        assert_eq!(repository.load_summary("history-1").unwrap(), None);
    }

    #[test]
    fn transcript_snapshot_round_trip_lists_newest_first() {
        let root = tempdir().unwrap();
        let repository = HistoryRepository::new(root.path().to_path_buf());
        repository.ensure_ready().unwrap();

        let item = sample_history_item("history-1", HistoryItemStatus::Complete);
        repository.write_index(std::slice::from_ref(&item)).unwrap();

        let first = repository
            .create_transcript_snapshot(
                &item.id,
                TranscriptSnapshotReason::Polish,
                json!([{ "id": "seg-1", "text": "before" }]),
            )
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let second = repository
            .create_transcript_snapshot(
                &item.id,
                TranscriptSnapshotReason::Translate,
                json!([{ "id": "seg-1", "text": "after" }]),
            )
            .unwrap();

        let snapshots = repository.list_transcript_snapshots(&item.id).unwrap();
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].id, second.id);
        assert_eq!(snapshots[1].id, first.id);

        let record = repository
            .load_transcript_snapshot(&item.id, &first.id)
            .unwrap()
            .unwrap();
        assert_eq!(record.metadata.reason, TranscriptSnapshotReason::Polish);
        assert_eq!(record.segments.len(), 1);
    }

    #[test]
    fn transcript_snapshot_paths_normalize_legacy_timing_fields() {
        let root = tempdir().unwrap();
        let repository = HistoryRepository::new(root.path().to_path_buf());
        repository.ensure_ready().unwrap();

        let item = sample_history_item("history-1", HistoryItemStatus::Complete);
        repository.write_index(std::slice::from_ref(&item)).unwrap();

        let metadata = repository
            .create_transcript_snapshot(
                &item.id,
                TranscriptSnapshotReason::Polish,
                json!([{
                    "id": "seg-1",
                    "text": "\u{4f60}\u{597d}",
                    "start": 0.0,
                    "end": 1.0,
                    "isFinal": true,
                    "tokens": ["\u{4f60}", "\u{597d}"],
                    "timestamps": [0.0, 0.5],
                    "durations": [0.5, 0.5]
                }]),
            )
            .unwrap();

        let record = repository
            .load_transcript_snapshot(&item.id, &metadata.id)
            .unwrap()
            .unwrap();
        let timing = record.segments[0].timing.as_ref().unwrap();
        assert_eq!(
            timing.level,
            crate::integrations::asr::TranscriptTimingLevel::Token
        );
        assert_eq!(
            timing.source,
            crate::integrations::asr::TranscriptTimingSource::Model
        );
        assert_eq!(timing.units[0].text, "\u{4f60}");
        assert_eq!(timing.units[1].end, 1.0);
    }

    #[test]
    fn transcript_snapshots_are_pruned_to_retention_limit() {
        let root = tempdir().unwrap();
        let repository = HistoryRepository::new(root.path().to_path_buf());
        repository.ensure_ready().unwrap();

        let item = sample_history_item("history-1", HistoryItemStatus::Complete);
        repository.write_index(std::slice::from_ref(&item)).unwrap();

        for index in 0..(TRANSCRIPT_SNAPSHOT_RETENTION_LIMIT + 2) {
            repository
                .create_transcript_snapshot(
                    &item.id,
                    TranscriptSnapshotReason::Polish,
                    json!([{ "id": format!("seg-{index}"), "text": format!("text {index}") }]),
                )
                .unwrap();
        }

        let snapshots = repository.list_transcript_snapshots(&item.id).unwrap();
        assert_eq!(snapshots.len(), TRANSCRIPT_SNAPSHOT_RETENTION_LIMIT);

        let versions_dir = repository.transcript_versions_dir(&item.id).unwrap();
        let json_file_count = fs::read_dir(versions_dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.path().extension().and_then(|value| value.to_str()) == Some("json")
            })
            .count();
        assert_eq!(json_file_count, TRANSCRIPT_SNAPSHOT_RETENTION_LIMIT + 1);
    }

    #[test]
    fn transcript_snapshot_paths_reject_unsafe_ids() {
        let root = tempdir().unwrap();
        let repository = HistoryRepository::new(root.path().to_path_buf());

        assert!(repository.list_transcript_snapshots("../bad").is_err());
        assert!(
            repository
                .load_transcript_snapshot("history-1", "../bad")
                .is_err()
        );
    }

    #[test]
    fn resolve_audio_path_rejects_paths_outside_history_dir() {
        let root = tempdir().unwrap();
        let repository = HistoryRepository::new(root.path().to_path_buf());
        repository.ensure_ready().unwrap();

        fs::write(repository.audio_path("safe.wav").unwrap(), b"audio").unwrap();

        assert!(
            repository
                .resolve_audio_path("safe.wav")
                .unwrap()
                .unwrap()
                .ends_with("safe.wav")
        );
        assert_eq!(
            repository.resolve_audio_path("../outside.wav").unwrap(),
            None
        );
        assert_eq!(
            repository.resolve_audio_path("nested/outside.wav").unwrap(),
            None
        );
        assert_eq!(
            repository
                .resolve_audio_path("C:\\Users\\asoda\\secret.wav")
                .unwrap(),
            None
        );
    }

    #[test]
    fn history_delete_items_removes_snapshot_versions() {
        let root = tempdir().unwrap();
        let repository = HistoryRepository::new(root.path().to_path_buf());
        repository.ensure_ready().unwrap();

        let item = sample_history_item("delete", HistoryItemStatus::Complete);
        repository.write_index(std::slice::from_ref(&item)).unwrap();
        repository
            .create_transcript_snapshot(
                &item.id,
                TranscriptSnapshotReason::Polish,
                json!([{ "id": "seg-1", "text": "before" }]),
            )
            .unwrap();
        let versions_dir = repository.transcript_versions_dir(&item.id).unwrap();
        assert!(versions_dir.exists());

        repository
            .delete_items(std::slice::from_ref(&item.id))
            .unwrap();

        assert!(!versions_dir.exists());
    }

    #[test]
    fn workspace_query_filters_searches_sorts_and_counts() {
        use crate::repositories::history::{
            HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType, HistoryWorkspaceQueryRequest,
            HistoryWorkspaceScope, HistoryWorkspaceSortOrder,
        };

        let root = tempdir().unwrap();
        let repository = HistoryRepository::new(root.path().to_path_buf());
        repository.ensure_ready().unwrap();

        let mut alpha = sample_history_item("alpha", HistoryItemStatus::Complete);
        alpha.timestamp = 3;
        alpha.duration = 30.0;
        alpha.title = "Alpha Plan".to_string();
        alpha.preview_text = "Roadmap preview".to_string();
        alpha.search_content = "Roadmap preview".to_string();
        alpha.project_id = Some("project-1".to_string());

        let mut batch = sample_history_item("batch", HistoryItemStatus::Complete);
        batch.timestamp = 4;
        batch.duration = 60.0;
        batch.title = "Batch Import".to_string();
        batch.kind = HistoryItemKind::Batch;
        batch.project_id = Some("project-1".to_string());

        let mut inbox = sample_history_item("inbox", HistoryItemStatus::Complete);
        inbox.timestamp = 5;
        inbox.title = "Inbox Note".to_string();
        inbox.project_id = None;

        repository
            .write_index(&[inbox.clone(), batch.clone(), alpha.clone()])
            .unwrap();

        let result = repository
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

        assert_eq!(result.scoped_item_ids, vec!["batch", "alpha"]);
        assert_eq!(result.filtered_items, vec![alpha.clone()]);
        assert_eq!(result.summary.total_items, 2);
        assert_eq!(result.summary.total_duration, 90.0);
        assert_eq!(result.summary.latest_timestamp, Some(4));
        assert_eq!(result.summary.recording_count, 1);
        assert_eq!(result.summary.batch_count, 1);
        assert_eq!(result.item_counts.inbox, 1);
        assert_eq!(result.item_counts.by_project_id.get("project-1"), Some(&2));

        let search_match = result.search_match_by_item_id.get("alpha").unwrap();
        assert_eq!(search_match.as_ref().unwrap().matched_field, "previewText");
        assert!(
            search_match
                .as_ref()
                .unwrap()
                .display_snippet
                .text
                .contains("Roadmap")
        );
    }

    #[test]
    fn workspace_query_uses_nfkc_search_normalization() {
        use crate::repositories::history::{
            HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType, HistoryWorkspaceQueryRequest,
            HistoryWorkspaceScope, HistoryWorkspaceSortOrder,
        };

        let root = tempdir().unwrap();
        let repository = HistoryRepository::new(root.path().to_path_buf());
        repository.ensure_ready().unwrap();

        let mut item = sample_history_item("ligature", HistoryItemStatus::Complete);
        item.preview_text = "\u{4f1a}\u{8bae} of\u{fb01}ce notes".to_string();
        item.search_content = item.preview_text.clone();
        repository.write_index(&[item.clone()]).unwrap();

        let result = repository
            .query_workspace(HistoryWorkspaceQueryRequest {
                scope: HistoryWorkspaceScope::All,
                query: "office".to_string(),
                filter_type: HistoryWorkspaceFilterType::All,
                date_filter: HistoryWorkspaceDateFilter::All,
                sort_order: HistoryWorkspaceSortOrder::Newest,
            })
            .unwrap();

        assert_eq!(result.filtered_items, vec![item.clone()]);
        let search_match = result.search_match_by_item_id.get("ligature").unwrap();
        assert_eq!(search_match.as_ref().unwrap().matched_field, "previewText");
        assert_eq!(
            search_match.as_ref().unwrap().display_snippet.text,
            "\u{4f1a}\u{8bae} of\u{fb01}ce notes"
        );
        assert_eq!(
            search_match
                .as_ref()
                .unwrap()
                .display_snippet
                .highlight_start,
            3
        );
        assert_eq!(
            search_match.as_ref().unwrap().display_snippet.highlight_end,
            8
        );
    }
}
