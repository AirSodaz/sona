use std::path::Path;
use std::sync::Arc;

use serde_json::{Value, to_value};

use crate::history::mutation_repository::{
    HistoryCompleteLiveDraftRequest, HistoryCreateTranscriptSnapshotRequest,
    HistoryDeleteItemsRequest, HistoryMutationError, HistoryMutationRepository,
    HistoryReassignProjectRequest, HistoryUpdateItemMetaRequest,
    HistoryUpdateProjectAssignmentsRequest, HistoryUpdateTranscriptRequest,
};
use crate::history::transcript_payload::normalize_history_transcript_segments;
use crate::history::{
    HistoryCreateLiveDraftRequest, HistoryItemRecord, HistorySaveImportedFileRequest,
    HistorySaveRecordingRequest, LiveRecordingDraftResult, TranscriptSnapshotMetadata,
};

const MAX_HISTORY_ID_UTF16_UNITS: usize = 238;
const MAX_MANAGED_FILE_NAME_UTF16_UNITS: usize = 255;

pub struct HistoryMutationService {
    repository: Arc<dyn HistoryMutationRepository>,
}

impl HistoryMutationService {
    pub fn new(repository: Arc<dyn HistoryMutationRepository>) -> Self {
        Self { repository }
    }

    pub fn create_live_draft(
        &self,
        request: HistoryCreateLiveDraftRequest,
    ) -> Result<LiveRecordingDraftResult, HistoryMutationError> {
        validate_optional_history_id("history ID", request.id.as_deref())?;
        validate_audio_extension("audio extension", &request.audio_extension)?;
        validate_optional_opaque_id("project ID", request.project_id.as_deref())?;
        self.repository.create_live_draft(request)
    }

    pub fn complete_live_draft(
        &self,
        mut request: HistoryCompleteLiveDraftRequest,
    ) -> Result<HistoryItemRecord, HistoryMutationError> {
        validate_history_id("history ID", &request.history_id)?;
        validate_duration(request.duration)?;
        request.segments = canonical_transcript(request.segments)?;
        self.repository.complete_live_draft(request)
    }

    pub fn save_recording(
        &self,
        mut request: HistorySaveRecordingRequest,
    ) -> Result<HistoryItemRecord, HistoryMutationError> {
        validate_duration(request.duration)?;
        validate_optional_opaque_id("project ID", request.project_id.as_deref())?;
        if let Some(extension) = request.audio_extension.as_deref() {
            validate_audio_extension("audio extension", extension)?;
        }
        match (&request.audio_bytes, &request.native_audio_path) {
            (Some(bytes), None) => {
                if bytes.is_empty() {
                    return invalid("recording audio bytes must not be empty");
                }
            }
            (None, Some(path)) => {
                if path.trim().is_empty() {
                    return invalid("native recording path must not be empty");
                }
            }
            (Some(_), Some(_)) => {
                return invalid("recording must use either audio bytes or a native path, not both");
            }
            (None, None) => return invalid("recording requires audio bytes or a native path"),
        }
        request.segments = canonical_transcript(request.segments)?;
        self.repository.save_recording(request)
    }

    pub fn save_imported_file(
        &self,
        mut request: HistorySaveImportedFileRequest,
    ) -> Result<HistoryItemRecord, HistoryMutationError> {
        validate_optional_history_id("history ID", request.id.as_deref())?;
        validate_nonempty("source path", &request.source_path)?;
        if let Some(path) = request.converted_source_path.as_deref() {
            validate_nonempty("converted source path", path)?;
        }
        validate_import_audio_extension(
            request
                .converted_source_path
                .as_deref()
                .unwrap_or(&request.source_path),
        )?;
        validate_duration(request.duration)?;
        validate_optional_opaque_id("project ID", request.project_id.as_deref())?;
        request.segments = canonical_transcript(request.segments)?;
        self.repository.save_imported_file(request)
    }

    pub fn delete_items(
        &self,
        request: HistoryDeleteItemsRequest,
    ) -> Result<(), HistoryMutationError> {
        validate_ids(&request.ids)?;
        if request.ids.is_empty() {
            return Ok(());
        }
        self.repository.delete_items(request)
    }

    pub fn update_transcript(
        &self,
        mut request: HistoryUpdateTranscriptRequest,
    ) -> Result<HistoryItemRecord, HistoryMutationError> {
        validate_history_id("history ID", &request.history_id)?;
        request.segments = canonical_transcript(request.segments)?;
        self.repository.update_transcript(request)
    }

    pub fn create_transcript_snapshot(
        &self,
        mut request: HistoryCreateTranscriptSnapshotRequest,
    ) -> Result<TranscriptSnapshotMetadata, HistoryMutationError> {
        validate_history_id("history ID", &request.history_id)?;
        request.segments = canonical_transcript(request.segments)?;
        self.repository.create_transcript_snapshot(request)
    }

    pub fn update_item_meta(
        &self,
        request: HistoryUpdateItemMetaRequest,
    ) -> Result<(), HistoryMutationError> {
        validate_history_id("history ID", &request.history_id)?;
        validate_metadata_updates(&request.updates)?;
        self.repository.update_item_meta(request)
    }

    pub fn update_project_assignments(
        &self,
        request: HistoryUpdateProjectAssignmentsRequest,
    ) -> Result<(), HistoryMutationError> {
        validate_ids(&request.ids)?;
        validate_optional_opaque_id("project ID", request.project_id.as_deref())?;
        if request.ids.is_empty() {
            return Ok(());
        }
        self.repository.update_project_assignments(request)
    }

    pub fn reassign_project(
        &self,
        request: HistoryReassignProjectRequest,
    ) -> Result<(), HistoryMutationError> {
        validate_nonempty("current project ID", &request.current_project_id)?;
        validate_optional_opaque_id("next project ID", request.next_project_id.as_deref())?;
        self.repository.reassign_project(request)
    }
}

fn canonical_transcript(segments: Value) -> Result<Value, HistoryMutationError> {
    let normalized = normalize_history_transcript_segments(segments)
        .map_err(HistoryMutationError::InvalidRequest)?;
    Ok(to_value(normalized.segments)?)
}

fn validate_duration(duration: f64) -> Result<(), HistoryMutationError> {
    if duration.is_finite() && duration >= 0.0 {
        Ok(())
    } else {
        invalid("duration must be a finite non-negative number")
    }
}

fn validate_ids(ids: &[String]) -> Result<(), HistoryMutationError> {
    for id in ids {
        validate_history_id("history ID", id)?;
    }
    Ok(())
}

fn validate_optional_history_id(
    label: &str,
    value: Option<&str>,
) -> Result<(), HistoryMutationError> {
    if let Some(value) = value {
        validate_history_id(label, value)?;
    }
    Ok(())
}

fn validate_optional_opaque_id(
    label: &str,
    value: Option<&str>,
) -> Result<(), HistoryMutationError> {
    if let Some(value) = value {
        validate_nonempty(label, value)?;
    }
    Ok(())
}

fn validate_history_id(label: &str, value: &str) -> Result<(), HistoryMutationError> {
    validate_managed_file_name(label, value, MAX_HISTORY_ID_UTF16_UNITS)
}

fn validate_nonempty(label: &str, value: &str) -> Result<(), HistoryMutationError> {
    if value.trim().is_empty() {
        invalid(format!("{label} must not be empty"))
    } else {
        Ok(())
    }
}

fn validate_audio_extension(label: &str, value: &str) -> Result<(), HistoryMutationError> {
    let extension = value.trim().trim_start_matches('.');
    if extension.is_empty()
        || extension.len() > 16
        || !extension
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
    {
        invalid(format!(
            "{label} must contain only 1-16 ASCII letters or digits"
        ))
    } else {
        Ok(())
    }
}

fn validate_import_audio_extension(path: &str) -> Result<(), HistoryMutationError> {
    let Some(extension) = Path::new(path).extension().and_then(|value| value.to_str()) else {
        return Ok(());
    };
    let sanitized = extension
        .trim()
        .trim_start_matches('.')
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>();
    if sanitized.is_empty() {
        Ok(())
    } else {
        validate_audio_extension("imported audio extension", &sanitized)
    }
}

fn validate_metadata_updates(updates: &Value) -> Result<(), HistoryMutationError> {
    let updates = updates.as_object().ok_or_else(|| {
        HistoryMutationError::InvalidRequest("history item updates must be an object".to_string())
    })?;
    for (key, value) in updates {
        validate_metadata_update(key, value)?;
    }
    Ok(())
}

fn validate_metadata_update(key: &str, value: &Value) -> Result<(), HistoryMutationError> {
    let valid = match key {
        "timestamp" => value.as_u64().is_some(),
        "duration" => value
            .as_f64()
            .is_some_and(|value| value.is_finite() && value >= 0.0),
        "audioPath" | "transcriptPath" => value.as_str().is_some_and(|value| {
            validate_managed_file_name(key, value, MAX_MANAGED_FILE_NAME_UTF16_UNITS).is_ok()
        }),
        "audioStatus" => matches!(value.as_str(), Some("available" | "missing" | "removed")),
        "title" | "previewText" | "searchContent" => value.is_string(),
        "icon" => value.is_null() || value.is_string(),
        "type" => matches!(value.as_str(), Some("batch" | "recording")),
        "projectId" => optional_nonempty_string(value),
        "status" => matches!(value.as_str(), Some("draft" | "complete")),
        "draftSource" => value.is_null() || value.as_str() == Some("live_record"),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        invalid(format!(
            "history item update `{key}` has an unsupported value"
        ))
    }
}

fn optional_nonempty_string(value: &Value) -> bool {
    value.is_null() || value.as_str().is_some_and(|value| !value.trim().is_empty())
}

fn validate_managed_file_name(
    label: &str,
    value: &str,
    max_utf16_units: usize,
) -> Result<(), HistoryMutationError> {
    validate_nonempty(label, value)?;
    if value.trim() != value
        || value.ends_with('.')
        || value.encode_utf16().count() > max_utf16_units
        || value.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
        })
        || is_windows_reserved_file_name(value)
    {
        return invalid(format!("{label} contains an unsupported file name"));
    }
    Ok(())
}

fn is_windows_reserved_file_name(value: &str) -> bool {
    let stem = value
        .split('.')
        .next()
        .unwrap_or(value)
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .is_some_and(is_windows_reserved_port_number)
        || stem
            .strip_prefix("LPT")
            .is_some_and(is_windows_reserved_port_number)
}

fn is_windows_reserved_port_number(suffix: &str) -> bool {
    matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
}

fn invalid<T>(reason: impl Into<String>) -> Result<T, HistoryMutationError> {
    Err(HistoryMutationError::InvalidRequest(reason.into()))
}
