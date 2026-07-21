use serde_json::Value;
use sona_core::history::transcript_payload::normalize_history_transcript_segments;
use sona_core::history::{
    HistoryBackupSnapshot, HistoryItemStatus, TranscriptSnapshotMetadata, TranscriptSnapshotReason,
    TranscriptSnapshotRecord,
};
use std::collections::HashMap;

use crate::DatabaseError;
use crate::history_fs_utils::ensure_json_array_value;

use super::row_map::map_row_to_item;
use super::sql::history_select_columns;

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
        .map_err(|error| DatabaseError::Internal(error.to_string()))?;
        let normalized = normalize_history_transcript_segments(transcript)
            .map_err(|error| DatabaseError::Internal(error.to_string()))?;
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
            .map_err(|error| DatabaseError::Internal(error.to_string()))?;
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
