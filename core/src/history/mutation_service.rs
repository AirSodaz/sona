use std::path::Path;
use std::sync::Arc;

use crate::history::mutation_repository::{
    HistoryCompleteLiveDraftRequest, HistoryCreateTranscriptSnapshotRequest, HistoryItemMetaPatch,
    HistoryMutationError, HistoryMutationRepository, HistoryPurgeItemsRequest,
    HistoryReplaceTagAssignmentsRequest, HistoryRestoreItemsRequest, HistoryTrashItemsRequest,
    HistoryUpdateItemMetaRequest, HistoryUpdateTagAssignmentsRequest,
    HistoryUpdateTranscriptRequest,
};
use crate::history::transcript_payload::canonicalize_history_transcript_segments;
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
        validate_tag_ids(&request.tag_ids)?;
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
        validate_tag_ids(&request.tag_ids)?;
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
        validate_tag_ids(&request.tag_ids)?;
        request.segments = canonical_transcript(request.segments)?;
        self.repository.save_imported_file(request)
    }

    pub fn trash_items(
        &self,
        request: HistoryTrashItemsRequest,
    ) -> Result<(), HistoryMutationError> {
        validate_ids(&request.ids)?;
        if request.deleted_at == 0 {
            return invalid("deletedAt must be positive");
        }
        if request.ids.is_empty() {
            return Ok(());
        }
        self.repository.trash_items(request)
    }

    pub fn restore_items(
        &self,
        request: HistoryRestoreItemsRequest,
    ) -> Result<(), HistoryMutationError> {
        validate_ids(&request.ids)?;
        if request.ids.is_empty() {
            return Ok(());
        }
        self.repository.restore_items(request)
    }

    pub fn purge_items(
        &self,
        request: HistoryPurgeItemsRequest,
    ) -> Result<(), HistoryMutationError> {
        validate_ids(&request.ids)?;
        if request.ids.is_empty() {
            return Ok(());
        }
        self.repository.purge_items(request)
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

    pub fn update_tag_assignments(
        &self,
        request: HistoryUpdateTagAssignmentsRequest,
    ) -> Result<(), HistoryMutationError> {
        validate_ids(&request.ids)?;
        validate_tag_ids(&request.add_tag_ids)?;
        validate_tag_ids(&request.remove_tag_ids)?;
        if request.ids.is_empty() {
            return Ok(());
        }
        self.repository.update_tag_assignments(request)
    }

    pub fn replace_tag_assignments(
        &self,
        request: HistoryReplaceTagAssignmentsRequest,
    ) -> Result<(), HistoryMutationError> {
        validate_ids(&request.ids)?;
        validate_tag_ids(&request.tag_ids)?;
        if request.ids.is_empty() {
            return Ok(());
        }
        self.repository.replace_tag_assignments(request)
    }
}

fn canonical_transcript(
    segments: Vec<crate::transcription::transcript::TranscriptSegment>,
) -> Result<Vec<crate::transcription::transcript::TranscriptSegment>, HistoryMutationError> {
    validate_transcript_numbers(&segments)?;
    Ok(canonicalize_history_transcript_segments(segments).segments)
}

fn validate_transcript_numbers(
    segments: &[crate::transcription::transcript::TranscriptSegment],
) -> Result<(), HistoryMutationError> {
    for (segment_index, segment) in segments.iter().enumerate() {
        validate_finite_number(&format!("segments[{segment_index}].start"), segment.start)?;
        validate_finite_number(&format!("segments[{segment_index}].end"), segment.end)?;
        if let Some(timing) = segment.timing.as_ref() {
            for (unit_index, unit) in timing.units.iter().enumerate() {
                validate_finite_number(
                    &format!("segments[{segment_index}].timing.units[{unit_index}].start"),
                    unit.start,
                )?;
                validate_finite_number(
                    &format!("segments[{segment_index}].timing.units[{unit_index}].end"),
                    unit.end,
                )?;
            }
        }
        for (field, values) in [
            ("timestamps", segment.timestamps.as_deref()),
            ("durations", segment.durations.as_deref()),
        ] {
            if let Some(values) = values {
                for (value_index, value) in values.iter().enumerate() {
                    validate_finite_number(
                        &format!("segments[{segment_index}].{field}[{value_index}]"),
                        f64::from(*value),
                    )?;
                }
            }
        }
        if let Some(score) = segment.speaker.as_ref().and_then(|speaker| speaker.score) {
            validate_finite_number(
                &format!("segments[{segment_index}].speaker.score"),
                f64::from(score),
            )?;
        }
        if let Some(attribution) = segment.speaker_attribution.as_ref() {
            for (candidate_index, candidate) in attribution.candidates.iter().enumerate() {
                validate_finite_number(
                    &format!(
                        "segments[{segment_index}].speakerAttribution.candidates[{candidate_index}].score"
                    ),
                    f64::from(candidate.score),
                )?;
            }
        }
    }
    Ok(())
}

fn validate_finite_number(label: &str, value: f64) -> Result<(), HistoryMutationError> {
    if value.is_finite() {
        Ok(())
    } else {
        invalid(format!("{label} must be a finite number"))
    }
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

fn validate_metadata_updates(updates: &HistoryItemMetaPatch) -> Result<(), HistoryMutationError> {
    if let Some(duration) = updates.duration {
        validate_duration(duration)?;
    }
    if let Some(audio_path) = updates.audio_path.as_deref() {
        validate_managed_file_name("audioPath", audio_path, MAX_MANAGED_FILE_NAME_UTF16_UNITS)?;
    }
    if let Some(transcript_path) = updates.transcript_path.as_deref() {
        validate_managed_file_name(
            "transcriptPath",
            transcript_path,
            MAX_MANAGED_FILE_NAME_UTF16_UNITS,
        )?;
    }
    Ok(())
}

fn validate_tag_ids(tag_ids: &[String]) -> Result<(), HistoryMutationError> {
    let mut seen = std::collections::HashSet::new();
    for tag_id in tag_ids {
        validate_nonempty("tag ID", tag_id)?;
        if !seen.insert(tag_id) {
            return invalid(format!("duplicate tag ID: {tag_id}"));
        }
    }
    Ok(())
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
