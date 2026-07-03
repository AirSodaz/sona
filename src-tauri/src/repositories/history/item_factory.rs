use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use super::{
    HistoryAudioStatus, HistoryCreateLiveDraftRequest, HistoryDraftSource, HistoryItemKind,
    HistoryItemRecord, HistoryItemStatus,
};

pub(super) fn current_time_millis() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .map_err(|error| error.to_string())
}

pub(super) fn create_live_draft_item(
    request: HistoryCreateLiveDraftRequest,
) -> Result<HistoryItemRecord, String> {
    let id = request.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let timestamp = current_time_millis()?;
    let audio_extension = sanitize_audio_extension(&request.audio_extension, "webm");
    let mut item = build_history_item_record(
        id,
        timestamp,
        0.0,
        &audio_extension,
        build_recording_title(timestamp),
        HistoryItemKind::Recording,
        request.project_id,
        request.icon,
    );
    item.status = HistoryItemStatus::Draft;
    item.draft_source = Some(HistoryDraftSource::LiveRecord);
    Ok(item)
}

pub(super) fn create_recording_item(
    duration: f64,
    project_id: Option<String>,
    audio_extension: Option<&str>,
    native_audio_path: Option<&str>,
) -> Result<HistoryItemRecord, String> {
    let timestamp = current_time_millis()?;
    let id = Uuid::new_v4().to_string();
    let audio_extension = audio_extension
        .map(|extension| sanitize_audio_extension(extension, "webm"))
        .or_else(|| native_audio_path.map(|path| extension_from_path(path, "wav")))
        .unwrap_or_else(|| "webm".to_string());

    Ok(build_history_item_record(
        id,
        timestamp,
        duration,
        &audio_extension,
        build_recording_title(timestamp),
        HistoryItemKind::Recording,
        project_id,
        None,
    ))
}

pub(super) struct ImportedFileItem {
    pub(super) item: HistoryItemRecord,
    pub(super) copy_source_path: String,
}

pub(super) fn create_imported_file_item(
    id: Option<String>,
    source_path: String,
    converted_source_path: Option<String>,
    duration: f64,
    project_id: Option<String>,
) -> Result<ImportedFileItem, String> {
    let timestamp = current_time_millis()?;
    let id = id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let title_file_name = file_name_from_path(&source_path, "Imported File");
    let copy_source_path = converted_source_path.unwrap_or_else(|| source_path.clone());
    let audio_extension = extension_from_path(&copy_source_path, "wav");
    let item = build_history_item_record(
        id,
        timestamp,
        duration,
        &audio_extension,
        format!("Batch {title_file_name}"),
        HistoryItemKind::Batch,
        project_id,
        None,
    );

    Ok(ImportedFileItem {
        item,
        copy_source_path,
    })
}

fn sanitize_audio_extension(extension: &str, fallback: &str) -> String {
    let extension = extension
        .trim()
        .trim_start_matches('.')
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>();
    if extension.is_empty() {
        fallback.to_string()
    } else {
        extension.to_ascii_lowercase()
    }
}

fn extension_from_path(path: &str, fallback: &str) -> String {
    PathBuf::from(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| sanitize_audio_extension(extension, fallback))
        .unwrap_or_else(|| fallback.to_string())
}

fn file_name_from_path(path: &str, fallback: &str) -> String {
    PathBuf::from(path)
        .file_name()
        .and_then(|file_name| file_name.to_str())
        .filter(|file_name| !file_name.trim().is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn build_recording_title(timestamp: u64) -> String {
    let local_time = chrono::DateTime::<chrono::Local>::from(
        UNIX_EPOCH + std::time::Duration::from_millis(timestamp),
    );
    format!("Recording {}", local_time.format("%Y-%m-%d %H-%M-%S"))
}

#[allow(clippy::too_many_arguments)]
fn build_history_item_record(
    id: String,
    timestamp: u64,
    duration: f64,
    audio_extension: &str,
    title: String,
    kind: HistoryItemKind,
    project_id: Option<String>,
    icon: Option<String>,
) -> HistoryItemRecord {
    HistoryItemRecord {
        audio_path: format!("{id}.{audio_extension}"),
        transcript_path: format!("{id}.json"),
        id,
        timestamp,
        duration: duration.max(0.0),
        title,
        preview_text: String::new(),
        audio_status: HistoryAudioStatus::Available,
        icon,
        kind,
        search_content: String::new(),
        project_id,
        status: HistoryItemStatus::Complete,
        draft_source: None,
    }
}
