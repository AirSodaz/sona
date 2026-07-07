use serde_json::{Value, to_value};
use std::path::PathBuf;

use sona_core::history::fs_utils::{ensure_safe_file_name, read_json_value};
use sona_core::history::transcript_payload::normalize_history_transcript_segments;
use sona_core::history::{
    HistoryAudioStatus, HistoryDraftSource, HistoryItemKind, HistoryItemRecord, HistoryItemStatus,
    TranscriptSnapshotMetadata, TranscriptSnapshotRecord,
};

const HISTORY_DIR_NAME: &str = "history";
const HISTORY_INDEX_FILE_NAME: &str = "index.json";
const HISTORY_VERSIONS_DIR_NAME: &str = "versions";

/// Reader for the legacy portable history layout used inside backup archives.
#[derive(Clone)]
pub struct HistoryRepository {
    app_local_data_dir: PathBuf,
}

impl HistoryRepository {
    pub fn new(app_local_data_dir: PathBuf) -> Self {
        Self { app_local_data_dir }
    }

    fn history_dir(&self) -> PathBuf {
        self.app_local_data_dir.join(HISTORY_DIR_NAME)
    }

    fn transcript_versions_dir(&self, history_id: &str) -> Result<PathBuf, String> {
        let safe_history_id = ensure_safe_file_name(history_id, "History version id")?;
        Ok(self
            .history_dir()
            .join(HISTORY_VERSIONS_DIR_NAME)
            .join(safe_history_id))
    }

    fn transcript_snapshot_index_path(&self, history_id: &str) -> Result<PathBuf, String> {
        Ok(self
            .transcript_versions_dir(history_id)?
            .join(HISTORY_INDEX_FILE_NAME))
    }

    fn transcript_snapshot_path(
        &self,
        history_id: &str,
        snapshot_id: &str,
    ) -> Result<PathBuf, String> {
        let safe_snapshot_id = ensure_safe_file_name(snapshot_id, "Transcript snapshot id")?;
        Ok(self
            .transcript_versions_dir(history_id)?
            .join(format!("{safe_snapshot_id}.json")))
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
        audio_status: match object
            .and_then(|map| map.get("audioStatus"))
            .and_then(Value::as_str)
        {
            Some("missing") => HistoryAudioStatus::Missing,
            Some("removed") => HistoryAudioStatus::Removed,
            _ => HistoryAudioStatus::Available,
        },
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use sona_core::history::fs_utils::write_json_pretty_atomic;
    use tempfile::tempdir;

    #[test]
    fn transcript_snapshot_reader_lists_newest_first_and_normalizes_segments() {
        let root = tempdir().unwrap();
        let repository = HistoryRepository::new(root.path().to_path_buf());
        let versions_dir = repository.transcript_versions_dir("history-1").unwrap();
        std::fs::create_dir_all(&versions_dir).unwrap();

        write_json_pretty_atomic(
            &versions_dir.join(HISTORY_INDEX_FILE_NAME),
            &json!([
                {
                    "id": "snapshot-older",
                    "historyId": "history-1",
                    "reason": "polish",
                    "createdAt": 100,
                    "segmentCount": 1
                },
                {
                    "id": "snapshot-newer",
                    "historyId": "history-1",
                    "reason": "translate",
                    "createdAt": 200,
                    "segmentCount": 1
                }
            ]),
        )
        .unwrap();
        write_json_pretty_atomic(
            &versions_dir.join("snapshot-newer.json"),
            &json!({
                "metadata": {
                    "id": "snapshot-newer",
                    "historyId": "history-1",
                    "reason": "translate",
                    "createdAt": 200,
                    "segmentCount": 1
                },
                "segments": [{
                    "id": "seg-1",
                    "text": "\u{4f60}\u{597d}",
                    "start": 0.0,
                    "end": 1.0,
                    "isFinal": true,
                    "tokens": ["\u{4f60}", "\u{597d}"],
                    "timestamps": [0.0, 0.5],
                    "durations": [0.5, 0.5]
                }]
            }),
        )
        .unwrap();

        let snapshots = repository.list_transcript_snapshots("history-1").unwrap();
        assert_eq!(snapshots[0].id, "snapshot-newer");
        assert_eq!(snapshots[1].id, "snapshot-older");

        let record = repository
            .load_transcript_snapshot("history-1", "snapshot-newer")
            .unwrap()
            .unwrap();
        let timing = record.segments[0].timing.as_ref().unwrap();
        assert_eq!(record.segments[0].text, "\u{4f60}\u{597d}");
        assert_eq!(timing.units[0].text, "\u{4f60}");
        assert_eq!(timing.units[1].end, 1.0);
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
}
