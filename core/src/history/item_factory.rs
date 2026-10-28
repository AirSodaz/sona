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

pub fn is_network_url(path: &str) -> bool {
    let trimmed = path.trim();
    for scheme in &["http://", "https://", "rtmp://", "rtsp://"] {
        if let Some(prefix) = trimmed.get(..scheme.len())
            && prefix.eq_ignore_ascii_case(scheme)
        {
            let rest = trimmed[scheme.len()..].trim_start();
            let host_part = rest.split(['/', '?', '#']).next().unwrap_or("");
            return !host_part.trim().is_empty();
        }
    }
    false
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
        request.project_id,
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
    project_id: Option<String>,
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
        project_id,
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
    project_id: Option<String>,
) -> ImportedFileItem {
    let is_url_stream = is_network_url(&source_path) && converted_source_path.is_none();
    let timestamp = generated.timestamp;
    let id = id.unwrap_or(generated.fallback_id);
    let title_file_name = file_name_from_path(&source_path, "Imported File");
    let copy_source_path = converted_source_path.unwrap_or_else(|| {
        if is_url_stream {
            String::new()
        } else {
            source_path.clone()
        }
    });
    let audio_extension = if is_url_stream {
        String::new()
    } else {
        extension_from_path(&copy_source_path, "wav")
    };
    let mut item = build_history_item_record(
        id,
        timestamp,
        duration,
        &audio_extension,
        format!("Batch {title_file_name}"),
        HistoryItemKind::Batch,
        tag_ids,
        project_id,
        None,
    );
    if is_url_stream {
        item.audio_path = String::new();
        item.audio_status = HistoryAudioStatus::Removed;
    }
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
    let path = path.trim();
    if is_network_url(path) {
        let clean = path
            .split('?')
            .next()
            .unwrap_or(path)
            .split('#')
            .next()
            .unwrap_or(path);
        let without_scheme = clean
            .split_once("://")
            .map(|(_, rest)| rest)
            .unwrap_or(clean)
            .trim_end_matches('/');
        if without_scheme.contains('/') {
            let segment = without_scheme.rsplit('/').next().unwrap_or("");
            let segment = segment.trim();
            if !segment.is_empty() {
                return segment.to_string();
            }
        }
        return fallback.to_string();
    }
    PathBuf::from(path)
        .file_name()
        .and_then(|file_name| file_name.to_str())
        .map(|file_name| file_name.trim())
        .filter(|file_name| !file_name.is_empty())
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
        tag_ids,
        deleted_at: None,
        status: HistoryItemStatus::Complete,
        draft_source: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_network_url() {
        assert!(is_network_url("http://example.com/stream"));
        assert!(is_network_url("https://example.com/audio.mp3?token=123"));
        assert!(is_network_url("RTMP://live.example.com/app/stream"));
        assert!(is_network_url("rtsp://192.168.1.1:554/live"));
        assert!(!is_network_url("http://"));
        assert!(!is_network_url("https://  "));
        assert!(!is_network_url("rtmp:///live"));
        assert!(!is_network_url("C:\\path\\to\\audio.wav"));
        assert!(!is_network_url("/home/user/audio.mp3"));
        assert!(!is_network_url("relative/audio.flac"));
        assert!(!is_network_url("会议录音.wav"));
        assert!(!is_network_url("/tmp/录音.wav"));
    }

    #[test]
    fn test_file_name_from_path_for_urls() {
        assert_eq!(
            file_name_from_path(
                "https://example.com/audio/podcast.mp3?token=123",
                "Fallback"
            ),
            "podcast.mp3"
        );
        assert_eq!(
            file_name_from_path("https://live.example.com/live-stream", "Fallback"),
            "live-stream"
        );
        assert_eq!(
            file_name_from_path("https://example.com/", "Fallback"),
            "Fallback"
        );
        assert_eq!(
            file_name_from_path("  https://example.com/audio/live.mp3  ", "Fallback"),
            "live.mp3"
        );
        assert_eq!(
            file_name_from_path("https://example.com/audio/%20segment%20/ ", "Fallback"),
            "%20segment%20"
        );
        assert_eq!(
            file_name_from_path(
                "http://proxy.example.com/stream/https://origin.example.com/audio.aac",
                "Fallback"
            ),
            "audio.aac"
        );
    }
    #[test]
    fn test_create_imported_file_item_stream_url() {
        let generated = HistoryItemGeneratedValues {
            fallback_id: "test-id-1".to_string(),
            timestamp: 1234567890,
            recording_title: "Default Title".to_string(),
        };
        let imported = create_imported_file_item(
            None,
            "https://example.com/audio/podcast.mp3?token=secret".to_string(),
            None,
            120.0,
            vec![],
            generated,
            None,
        );
        assert_eq!(imported.item.id, "test-id-1");
        assert_eq!(imported.item.audio_path, "");
        assert_eq!(imported.item.audio_status, HistoryAudioStatus::Removed);
        assert_eq!(imported.item.title, "Batch podcast.mp3");
        assert_eq!(imported.copy_source_path, "");
    }
}
