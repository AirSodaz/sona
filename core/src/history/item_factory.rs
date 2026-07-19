use std::path::PathBuf;

use super::{
    HistoryAudioStatus, HistoryCreateLiveDraftRequest, HistoryDraftSource, HistoryItemKind,
    HistoryItemRecord, HistoryItemStatus,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryItemGeneratedValues {
    pub fallback_id: String,
    pub timestamp: u64,
    pub recording_title: String,
}

pub fn create_live_draft_item(
    request: HistoryCreateLiveDraftRequest,
    generated: HistoryItemGeneratedValues,
) -> HistoryItemRecord {
    let id = request.id.unwrap_or(generated.fallback_id);
    let timestamp = generated.timestamp;
    let recording_title = generated.recording_title;
    let audio_extension = sanitize_audio_extension(&request.audio_extension, "webm");
    let mut item = build_history_item_record(
        id,
        timestamp,
        0.0,
        &audio_extension,
        recording_title,
        HistoryItemKind::Recording,
        request.tag_ids,
        request.icon,
    );
    item.status = HistoryItemStatus::Draft;
    item.draft_source = Some(HistoryDraftSource::LiveRecord);
    item
}

pub fn create_recording_item(
    generated: HistoryItemGeneratedValues,
    duration: f64,
    tag_ids: Vec<String>,
    audio_extension: Option<&str>,
    native_audio_path: Option<&str>,
) -> HistoryItemRecord {
    let timestamp = generated.timestamp;
    let id = generated.fallback_id;
    let recording_title = generated.recording_title;
    let audio_extension = audio_extension
        .map(|extension| sanitize_audio_extension(extension, "webm"))
        .or_else(|| native_audio_path.map(|path| extension_from_path(path, "wav")))
        .unwrap_or_else(|| "webm".to_string());

    build_history_item_record(
        id,
        timestamp,
        duration,
        &audio_extension,
        recording_title,
        HistoryItemKind::Recording,
        tag_ids,
        None,
    )
}

pub struct ImportedFileItem {
    pub item: HistoryItemRecord,
    pub copy_source_path: String,
}

pub fn create_imported_file_item(
    id: Option<String>,
    source_path: String,
    converted_source_path: Option<String>,
    duration: f64,
    tag_ids: Vec<String>,
    generated: HistoryItemGeneratedValues,
) -> ImportedFileItem {
    let timestamp = generated.timestamp;
    let id = id.unwrap_or(generated.fallback_id);
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
        tag_ids,
        None,
    );
    ImportedFileItem {
        item,
        copy_source_path,
    }
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

#[allow(clippy::too_many_arguments)]
fn build_history_item_record(
    id: String,
    timestamp: u64,
    duration: f64,
    audio_extension: &str,
    title: String,
    kind: HistoryItemKind,
    tag_ids: Vec<String>,
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
        tag_ids,
        deleted_at: None,
        status: HistoryItemStatus::Complete,
        draft_source: None,
    }
}
