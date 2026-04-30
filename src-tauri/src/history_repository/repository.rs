use serde_json::{Map, Value};
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use super::fs_utils::{
    ensure_json_array_value, ensure_json_object_value, ensure_safe_file_name,
    optional_history_child_path, read_json_value, remove_path_if_exists, write_binary_atomic,
    write_json_pretty_atomic,
};
use super::types::HistoryBackupSnapshot;
use super::{
    HistoryDraftSource, HistoryItemKind, HistoryItemRecord, HistoryItemStatus,
    LiveRecordingDraftResult,
};
use super::{HISTORY_DIR_NAME, HISTORY_INDEX_FILE_NAME, SUMMARY_FILE_SUFFIX};

#[derive(Clone)]
pub(super) struct HistoryRepository {
    app_local_data_dir: PathBuf,
}

impl HistoryRepository {
    pub(super) fn new(app_local_data_dir: PathBuf) -> Self {
        Self { app_local_data_dir }
    }

    pub(super) fn history_dir(&self) -> PathBuf {
        self.app_local_data_dir.join(HISTORY_DIR_NAME)
    }

    pub(super) fn history_index_path(&self) -> PathBuf {
        self.history_dir().join(HISTORY_INDEX_FILE_NAME)
    }

    pub(super) fn ensure_ready(&self) -> Result<(), String> {
        let history_dir = self.history_dir();
        fs::create_dir_all(&history_dir).map_err(|error| error.to_string())?;

        let index_path = self.history_index_path();
        if !index_path.exists() {
            write_json_pretty_atomic(&index_path, &Value::Array(Vec::new()))?;
        }

        Ok(())
    }

    pub(super) fn list_items(&self) -> Result<Vec<HistoryItemRecord>, String> {
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

    pub(super) fn insert_item_at_front(
        &self,
        item: HistoryItemRecord,
    ) -> Result<HistoryItemRecord, String> {
        let mut items = self.list_items()?;
        items.retain(|existing| existing.id != item.id);
        items.insert(0, item.clone());
        self.write_index(&items)?;
        Ok(item)
    }

    pub(super) fn write_index(&self, items: &[HistoryItemRecord]) -> Result<(), String> {
        write_json_pretty_atomic(&self.history_index_path(), items)
    }

    pub(super) fn transcript_path(&self, file_name: &str) -> Result<PathBuf, String> {
        Ok(self
            .history_dir()
            .join(ensure_safe_file_name(file_name, "History transcript path")?))
    }

    pub(super) fn audio_path(&self, file_name: &str) -> Result<PathBuf, String> {
        Ok(self
            .history_dir()
            .join(ensure_safe_file_name(file_name, "History audio path")?))
    }

    pub(super) fn summary_path(&self, history_id: &str) -> Result<PathBuf, String> {
        let safe_history_id = ensure_safe_file_name(history_id, "History summary id")?;
        Ok(self
            .history_dir()
            .join(format!("{safe_history_id}{SUMMARY_FILE_SUFFIX}")))
    }

    pub(super) fn create_live_draft(
        &self,
        item_value: Value,
    ) -> Result<LiveRecordingDraftResult, String> {
        self.ensure_ready()?;
        let mut item = normalize_history_item_value(&item_value);
        item.status = HistoryItemStatus::Draft;
        item.draft_source = Some(HistoryDraftSource::LiveRecord);
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

    pub(super) fn complete_live_draft(
        &self,
        history_id: &str,
        segments: Value,
        preview_text: String,
        search_content: String,
        duration: f64,
    ) -> Result<HistoryItemRecord, String> {
        let segments = ensure_json_array_value(segments, "History transcript segments")?;
        let mut items = self.list_items()?;
        let item = items
            .iter_mut()
            .find(|entry| entry.id == history_id)
            .ok_or_else(|| format!("History item not found: {history_id}"))?;
        write_json_pretty_atomic(&self.transcript_path(&item.transcript_path)?, &segments)?;
        item.preview_text = preview_text;
        item.search_content = search_content;
        item.duration = duration.max(0.0);
        item.status = HistoryItemStatus::Complete;
        item.draft_source = None;
        let updated = item.clone();
        self.write_index(&items)?;
        Ok(updated)
    }

    pub(super) fn save_recording(
        &self,
        item_value: Value,
        segments: Value,
        audio_bytes: Option<Vec<u8>>,
        native_audio_path: Option<String>,
    ) -> Result<HistoryItemRecord, String> {
        self.ensure_ready()?;
        let segments = ensure_json_array_value(segments, "History transcript segments")?;
        let mut item = normalize_history_item_value(&item_value);
        item.status = HistoryItemStatus::Complete;
        item.draft_source = None;

        match (audio_bytes, native_audio_path) {
            (Some(bytes), _) => {
                let target_path = self.audio_path(&item.audio_path)?;
                write_binary_atomic(&target_path, &bytes)?;
            }
            (None, Some(_)) => {
                // Native capture already wrote the file. We only persist transcript/index here.
            }
            (None, None) => {
                return Err(
                    "History recording save requires audio bytes or a native audio path."
                        .to_string(),
                );
            }
        }

        write_json_pretty_atomic(&self.transcript_path(&item.transcript_path)?, &segments)?;
        self.insert_item_at_front(item)
    }

    pub(super) fn save_imported_file(
        &self,
        item_value: Value,
        segments: Value,
        source_path: String,
    ) -> Result<HistoryItemRecord, String> {
        self.ensure_ready()?;
        let segments = ensure_json_array_value(segments, "History transcript segments")?;
        let mut item = normalize_history_item_value(&item_value);
        item.status = HistoryItemStatus::Complete;
        item.draft_source = None;

        let source = PathBuf::from(source_path);
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
        write_json_pretty_atomic(&self.transcript_path(&item.transcript_path)?, &segments)?;
        self.insert_item_at_front(item)
    }

    pub(super) fn delete_items(&self, ids: &[String]) -> Result<(), String> {
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
        }

        let next_items = items
            .into_iter()
            .filter(|item| !id_set.contains(&item.id))
            .collect::<Vec<_>>();
        self.write_index(&next_items)?;
        Ok(())
    }

    pub(super) fn load_transcript(&self, file_name: &str) -> Result<Option<Value>, String> {
        let transcript_path = self.transcript_path(file_name)?;
        if !transcript_path.exists() {
            return Ok(None);
        }

        let value = read_json_value(&transcript_path)?;
        Ok(Some(ensure_json_array_value(
            value,
            "History transcript file",
        )?))
    }

    pub(super) fn update_transcript(
        &self,
        history_id: &str,
        segments: Value,
        preview_text: String,
        search_content: String,
    ) -> Result<(), String> {
        let segments = ensure_json_array_value(segments, "History transcript segments")?;
        let mut items = self.list_items()?;
        let item = items
            .iter_mut()
            .find(|entry| entry.id == history_id)
            .ok_or_else(|| format!("History item not found: {history_id}"))?;
        write_json_pretty_atomic(&self.transcript_path(&item.transcript_path)?, &segments)?;
        item.preview_text = preview_text;
        item.search_content = search_content;
        self.write_index(&items)?;
        Ok(())
    }

    pub(super) fn update_item_meta(&self, history_id: &str, updates: Value) -> Result<(), String> {
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

    pub(super) fn update_project_assignments(
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

    pub(super) fn reassign_project(
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

    pub(super) fn load_summary(&self, history_id: &str) -> Result<Option<Value>, String> {
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

    pub(super) fn save_summary(
        &self,
        history_id: &str,
        summary_payload: Value,
    ) -> Result<(), String> {
        self.ensure_ready()?;
        let summary_payload = ensure_json_object_value(summary_payload, "History summary payload")?;
        write_json_pretty_atomic(&self.summary_path(history_id)?, &summary_payload)
    }

    pub(super) fn delete_summary(&self, history_id: &str) -> Result<(), String> {
        let summary_path = self.summary_path(history_id)?;
        remove_path_if_exists(&summary_path)
    }

    pub(super) fn resolve_audio_path(&self, file_name: &str) -> Result<Option<String>, String> {
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

    pub(super) fn history_snapshot_for_backup(&self) -> Result<HistoryBackupSnapshot, String> {
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
        }

        Ok(HistoryBackupSnapshot {
            items,
            transcript_files,
            summary_files,
        })
    }
}

pub(super) fn normalize_history_item_value(value: &Value) -> HistoryItemRecord {
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
    use crate::history_repository::fs_utils::write_json_pretty_atomic;
    use crate::history_repository::test_support::sample_history_item;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn history_delete_items_removes_files_and_updates_index() {
        let root = tempdir().unwrap();
        let repository = HistoryRepository::new(root.path().to_path_buf());
        repository.ensure_ready().unwrap();

        let keep_item = sample_history_item("keep", HistoryItemStatus::Complete);
        let delete_item = sample_history_item("delete", HistoryItemStatus::Complete);
        repository
            .write_index(&vec![keep_item.clone(), delete_item.clone()])
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
            .delete_items(&vec![delete_item.id.clone()])
            .unwrap();

        let items = repository.list_items().unwrap();
        assert_eq!(items, vec![keep_item]);
        assert!(!repository
            .audio_path(&delete_item.audio_path)
            .unwrap()
            .exists());
        assert!(!repository
            .transcript_path(&delete_item.transcript_path)
            .unwrap()
            .exists());
        assert!(!repository.summary_path(&delete_item.id).unwrap().exists());
    }

    #[test]
    fn live_draft_round_trip_updates_index_and_transcript() {
        let root = tempdir().unwrap();
        let repository = HistoryRepository::new(root.path().to_path_buf());

        let draft = repository
            .create_live_draft(json!({
                "id": "draft-1",
                "timestamp": 1,
                "duration": 0,
                "audioPath": "draft-1.webm",
                "transcriptPath": "draft-1.json",
                "title": "Draft",
                "previewText": "",
                "type": "recording",
                "searchContent": "",
                "projectId": null,
                "status": "draft",
                "draftSource": "live_record"
            }))
            .unwrap();
        assert_eq!(draft.item.status, HistoryItemStatus::Draft);
        assert!(PathBuf::from(&draft.audio_absolute_path).ends_with("draft-1.webm"));

        let completed = repository
            .complete_live_draft(
                "draft-1",
                json!([{ "id": "seg-1", "text": "hello" }]),
                "hello".to_string(),
                "hello".to_string(),
                3.0,
            )
            .unwrap();

        assert_eq!(completed.status, HistoryItemStatus::Complete);
        assert_eq!(completed.preview_text, "hello");
        let transcript = repository.load_transcript("draft-1.json").unwrap().unwrap();
        assert_eq!(transcript.as_array().unwrap().len(), 1);
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
}
