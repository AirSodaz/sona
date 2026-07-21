use sona_core::history::transcript_payload::normalize_history_transcript_segments;
use sona_core::history::{
    HistoryBackupSnapshot, HistoryItemRecord, HistoryItemStatus, TranscriptSnapshotMetadata,
    TranscriptSnapshotReason, TranscriptSnapshotRecord,
};
use std::collections::{HashMap, HashSet};

use crate::DatabaseError;
use crate::history_fs_utils::ensure_safe_file_name;

use super::sql::insert_history_item_row;

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
        let safe_id = ensure_safe_file_name(&item.id, "History item ID")
            .map_err(|error| error.to_string())?;
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
        let safe_name = ensure_safe_file_name(file_name, "Backup transcript file name")
            .map_err(|error| error.to_string())?;
        if safe_name != *file_name {
            return Err("Backup transcript file name is invalid.".to_string());
        }
        let history_id = file_name
            .strip_suffix(".json")
            .filter(|id| item_ids.contains(*id))
            .ok_or_else(|| format!("Backup transcript is orphaned: {file_name}"))?;
        let normalized = normalize_history_transcript_segments(value.clone())
            .map_err(|error| error.to_string())?;
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
        let safe_id = ensure_safe_file_name(history_id, "Backup summary history ID")
            .map_err(|error| error.to_string())?;
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
        let safe_history_id = ensure_safe_file_name(history_id, "Snapshot history ID")
            .map_err(|error| error.to_string())?;
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
        let safe_snapshot_id = ensure_safe_file_name(snapshot_id, "Transcript snapshot ID")
            .map_err(|error| error.to_string())?;
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
            let safe_id = ensure_safe_file_name(&metadata.id, "Transcript snapshot ID")
                .map_err(|error| error.to_string())?;
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
            )
            .map_err(|error| error.to_string())?;
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

pub(super) fn transcript_snapshot_reason_str(reason: TranscriptSnapshotReason) -> &'static str {
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
