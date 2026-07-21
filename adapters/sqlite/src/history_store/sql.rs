use chrono::Local;
use sona_core::history::item_factory::HistoryItemGeneratedValues;
use sona_core::history::mutation_repository::HistoryMutationError;
use sona_core::history::{HistoryIdGenerator, HistoryItemRecord};
use sona_core::ports::fs::{FileSystemError, FileSystemOperation};
use sona_core::ports::time::{ClockError, UnixMillisClock};
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use crate::DatabaseError;


pub(super) fn require_history_source_file(
    path: &Path,
    missing_message: impl FnOnce() -> String,
) -> Result<(), HistoryMutationError> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => Err(HistoryMutationError::Internal(missing_message())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(HistoryMutationError::Internal(missing_message()))
        }
        Err(error) => {
            Err(FileSystemError::new(FileSystemOperation::Metadata, path, error.to_string()).into())
        }
    }
}

pub(super) fn new_history_item_generated_values(
    clock: &dyn UnixMillisClock,
    ids: &dyn HistoryIdGenerator,
) -> Result<HistoryItemGeneratedValues, ClockError> {
    let timestamp = clock.now_ms()?;
    Ok(HistoryItemGeneratedValues {
        fallback_id: ids.generate_id(),
        timestamp,
        recording_title: build_recording_title(timestamp),
    })
}

pub(super) fn build_recording_title(timestamp: u64) -> String {
    let local_time =
        chrono::DateTime::<Local>::from(UNIX_EPOCH + std::time::Duration::from_millis(timestamp));
    format!("Recording {}", local_time.format("%Y-%m-%d %H-%M-%S"))
}

pub(crate) const HISTORY_ITEM_COLUMNS: [&str; 15] = [
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
    "tag_ids",
    "deleted_at",
    "status",
    "draft_source",
];

pub(super) const HISTORY_ITEM_ROW_COLUMNS: [&str; 14] = [
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
    "deleted_at",
    "status",
    "draft_source",
];

pub(super) fn column_list(columns: &[&str]) -> String {
    columns.join(", ")
}

pub(super) fn named_param_list(columns: &[&str]) -> String {
    columns
        .iter()
        .map(|column| format!(":{column}"))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn history_select_columns(alias: Option<&str>, overrides: &[(&str, &str)]) -> String {
    HISTORY_ITEM_COLUMNS
        .iter()
        .map(|column| {
            if let Some((_, expression)) = overrides
                .iter()
                .find(|(override_column, _)| override_column == column)
            {
                return format!("{expression} AS {column}");
            }

            if *column == "tag_ids" {
                let history_id = alias.map_or("history_items.id".to_string(), |alias| {
                    format!("{alias}.id")
                });
                return format!(
                    "COALESCE((SELECT json_group_array(tag_id) FROM (SELECT hit.tag_id FROM history_item_tags hit JOIN tags t ON t.id = hit.tag_id WHERE hit.history_id = {history_id} ORDER BY t.sort_order, t.id)), '[]') AS tag_ids"
                );
            }

            match alias {
                Some(alias) => format!("{alias}.{column} AS {column}"),
                None => (*column).to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn history_insert_sql() -> String {
    format!(
        "INSERT INTO history_items ({}) VALUES ({})",
        column_list(&HISTORY_ITEM_ROW_COLUMNS),
        named_param_list(&HISTORY_ITEM_ROW_COLUMNS)
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
    let deleted_at = item
        .deleted_at
        .map(i64::try_from)
        .transpose()
        .map_err(|_| {
            DatabaseError::Internal("History deleted timestamp exceeds SQLite range.".into())
        })?;
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
            ":deleted_at": deleted_at,
            ":status": status_str,
            ":draft_source": draft_source_str.as_deref(),
        },
    )?;

    replace_history_item_tags(tx, &item.id, &item.tag_ids)
}

pub(super) fn replace_history_item_tags(
    tx: &rusqlite::Transaction<'_>,
    history_id: &str,
    tag_ids: &[String],
) -> Result<(), DatabaseError> {
    tx.execute(
        "DELETE FROM history_item_tags WHERE history_id = ?1",
        [history_id],
    )?;
    let mut insert =
        tx.prepare_cached("INSERT INTO history_item_tags (history_id, tag_id) VALUES (?1, ?2)")?;
    for tag_id in tag_ids {
        insert.execute(rusqlite::params![history_id, tag_id])?;
    }
    Ok(())
}

pub(super) fn load_tag_ids(
    conn: &rusqlite::Connection,
    history_id: &str,
) -> Result<Vec<String>, DatabaseError> {
    let mut statement = conn.prepare_cached(
        "SELECT hit.tag_id FROM history_item_tags hit JOIN tags t ON t.id = hit.tag_id WHERE hit.history_id = ?1 ORDER BY t.sort_order, t.id",
    )?;
    statement
        .query_map([history_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(DatabaseError::QueryError)
}
