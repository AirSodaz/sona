use sona_core::history::mutation_repository::{HistoryItemMetaPatch, HistoryMutationError};
use sona_core::history::{
    HistoryAudioStatus, HistoryDraftSource, HistoryItemKind, HistoryItemRecord, HistoryItemStatus,
};
use sona_core::history_store::HistoryStoreError;
use rusqlite::types::Type;
use std::str::FromStr;

use crate::history_fs_utils::ensure_safe_file_name;


pub(super) fn validate_id(id: &str, label: &str) -> Result<(), String> {
    ensure_safe_file_name(id, label).map_err(|error| error.to_string())?;
    Ok(())
}

pub(super) fn history_store_mutation_error(error: HistoryMutationError) -> HistoryStoreError {
    match error {
        HistoryMutationError::InvalidRequest(reason) => HistoryStoreError::InvalidRequest(reason),
        HistoryMutationError::Database(reason) => HistoryStoreError::Database(reason),
        HistoryMutationError::Serialization(error) => HistoryStoreError::Serialization(error),
        HistoryMutationError::Clock(error) => HistoryStoreError::Clock(error),
        HistoryMutationError::FileSystem(error) => HistoryStoreError::FileSystem(error),
        HistoryMutationError::NotFound(reason) | HistoryMutationError::Internal(reason) => {
            HistoryStoreError::Internal(reason)
        }
    }
}

pub(super) fn map_row_to_item(row: &rusqlite::Row) -> rusqlite::Result<HistoryItemRecord> {
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
    let tag_ids_json: String = row.get("tag_ids")?;
    let deleted_at_value: Option<i64> = row.get("deleted_at")?;
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

    let tag_ids = serde_json::from_str::<Vec<String>>(&tag_ids_json)
        .map_err(|error| invalid_history_column(row, "tag_ids", Type::Text, error.to_string()))?;
    let deleted_at = deleted_at_value
        .map(|value| checked_history_u64_column(row, "deleted_at", value))
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
        tag_ids,
        deleted_at,
        status,
        draft_source,
    })
}

pub(super) fn checked_history_u64_column(
    row: &rusqlite::Row<'_>,
    column: &str,
    value: i64,
) -> rusqlite::Result<u64> {
    let column_index = row.as_ref().column_index(column)?;
    u64::try_from(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(column_index, Type::Integer, Box::new(error))
    })
}

pub(super) fn invalid_history_column(
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

pub(super) fn apply_history_item_updates(item: &mut HistoryItemRecord, updates: &HistoryItemMetaPatch) {
    if let Some(timestamp) = updates.timestamp {
        item.timestamp = timestamp;
    }
    if let Some(duration) = updates.duration {
        item.duration = duration.max(0.0);
    }
    if let Some(audio_path) = updates.audio_path.as_ref() {
        item.audio_path.clone_from(audio_path);
    }
    if let Some(audio_status) = updates.audio_status {
        item.audio_status = audio_status;
    }
    if let Some(transcript_path) = updates.transcript_path.as_ref() {
        item.transcript_path.clone_from(transcript_path);
    }
    if let Some(title) = updates.title.as_ref() {
        item.title.clone_from(title);
    }
    if let Some(preview_text) = updates.preview_text.as_ref() {
        item.preview_text.clone_from(preview_text);
    }
    if let Some(icon) = updates.icon.as_ref() {
        item.icon.clone_from(icon);
    }
    if let Some(kind) = updates.kind {
        item.kind = kind;
    }
    if let Some(search_content) = updates.search_content.as_ref() {
        item.search_content.clone_from(search_content);
    }
    if let Some(status) = updates.status {
        item.status = status;
    }
    if let Some(draft_source) = updates.draft_source.as_ref() {
        item.draft_source = *draft_source;
    }
}
