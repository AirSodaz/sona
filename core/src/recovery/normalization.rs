use serde_json::Value;

use crate::recovery::types::{
    RECOVERY_VERSION, RecoveredQueueItem, RecoveredTranscriptSegment, RecoveredTranscriptTiming,
    RecoveredTranscriptTimingUnit, RecoveryItemInput, RecoveryItemStage, RecoveryResolution,
    RecoverySnapshot, RecoverySnapshotInput, RecoverySource,
};
use crate::transcription::transcript::{TranscriptTimingLevel, TranscriptTimingSource};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourcePathStatus {
    File,
    Directory,
    Missing,
    Unknown,
}

pub trait SourcePathStatusProvider {
    fn status_for_path(&self, path: &str) -> SourcePathStatus;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct UnknownSourcePathStatusProvider;

impl SourcePathStatusProvider for UnknownSourcePathStatusProvider {
    fn status_for_path(&self, _path: &str) -> SourcePathStatus {
        SourcePathStatus::Unknown
    }
}

impl RecoveryItemInput {
    fn normalize_recovered(
        self,
        now: u64,
        source_paths: &impl SourcePathStatusProvider,
    ) -> Option<RecoveredQueueItem> {
        let id = non_empty_string(self.id)?;
        let file_path = self.file_path.unwrap_or_default();
        let automation_rule_id = self.automation_rule_id;
        let resolution = normalize_resolution(self.resolution.as_deref());
        let saved_has_source_file = self.has_source_file.unwrap_or(true);
        let saved_can_resume = self
            .can_resume
            .unwrap_or(saved_has_source_file && resolution == RecoveryResolution::Pending);
        let (has_source_file, can_resume) = resolve_source_flags(
            &file_path,
            resolution,
            saved_has_source_file,
            saved_can_resume,
            source_paths,
        );

        Some(RecoveredQueueItem {
            id,
            filename: non_empty_string(self.filename)
                .unwrap_or_else(|| filename_from_path(&file_path)),
            file_path,
            source: normalize_source(self.source.as_deref(), automation_rule_id.as_ref()),
            resolution,
            progress: self.progress.unwrap_or(0.0),
            segments: normalize_segments(self.segments),
            tag_ids: normalized_tag_ids(self.tag_ids, self.project_id),
            history_id: self.history_id,
            history_title: self.history_title,
            last_known_stage: normalize_stage(self.last_known_stage.as_deref()),
            updated_at: self.updated_at.unwrap_or(now),
            has_source_file,
            can_resume,
            automation_rule_id,
            automation_rule_name: self.automation_rule_name,
            resolved_config_snapshot: self.resolved_config_snapshot,
            automation_resolution_snapshot: self.automation_resolution_snapshot,
            export_config: self.export_config.unwrap_or(Value::Null),
            stage_config: self.stage_config.unwrap_or(Value::Null),
            source_fingerprint: self.source_fingerprint,
            file_stat: self.file_stat,
            export_file_name_prefix: self.export_file_name_prefix,
        })
    }

    fn normalize_queue(
        self,
        now: u64,
        source_paths: &impl SourcePathStatusProvider,
    ) -> Option<RecoveredQueueItem> {
        let status = self.status.as_deref()?;
        if status != "pending" && status != "processing" {
            return None;
        }

        let queue_id = non_empty_string(self.id)?;
        let id = non_empty_string(self.recovery_id).unwrap_or(queue_id);
        let file_path = self.file_path.unwrap_or_default();
        let source =
            if self.origin.as_deref() == Some("automation") || self.automation_rule_id.is_some() {
                RecoverySource::Automation
            } else {
                RecoverySource::BatchImport
            };
        let resolution = RecoveryResolution::Pending;
        let (has_source_file, can_resume) =
            resolve_source_flags(&file_path, resolution, true, true, source_paths);

        Some(RecoveredQueueItem {
            id,
            filename: non_empty_string(self.filename)
                .unwrap_or_else(|| filename_from_path(&file_path)),
            file_path,
            source,
            resolution,
            progress: self.progress.unwrap_or(0.0),
            segments: normalize_segments(self.segments),
            tag_ids: normalized_tag_ids(self.tag_ids, self.project_id),
            history_id: self.history_id,
            history_title: self.history_title,
            last_known_stage: normalize_stage(self.last_known_stage.as_deref()),
            updated_at: now,
            has_source_file,
            can_resume,
            automation_rule_id: self.automation_rule_id,
            automation_rule_name: self.automation_rule_name,
            resolved_config_snapshot: self.resolved_config_snapshot,
            automation_resolution_snapshot: self.automation_resolution_snapshot,
            export_config: self.export_config.unwrap_or(Value::Null),
            stage_config: self.stage_config.unwrap_or(Value::Null),
            source_fingerprint: self.source_fingerprint,
            file_stat: self.file_stat,
            export_file_name_prefix: self.export_file_name_prefix,
        })
    }
}

pub fn empty_snapshot() -> RecoverySnapshot {
    RecoverySnapshot {
        version: RECOVERY_VERSION,
        updated_at: None,
        items: Vec::new(),
    }
}

pub fn snapshot_from_items(items: Vec<RecoveredQueueItem>) -> RecoverySnapshot {
    snapshot_from_items_with_timestamp(items, 0)
}

pub fn snapshot_from_items_with_timestamp(
    items: Vec<RecoveredQueueItem>,
    timestamp: u64,
) -> RecoverySnapshot {
    RecoverySnapshot {
        version: RECOVERY_VERSION,
        updated_at: (!items.is_empty()).then_some(timestamp),
        items,
    }
}

pub fn snapshot_from_input(
    input: RecoverySnapshotInput,
    include_only_pending: bool,
) -> RecoverySnapshot {
    snapshot_from_input_at(input, include_only_pending, 0)
}

pub fn snapshot_from_input_at(
    input: RecoverySnapshotInput,
    include_only_pending: bool,
    timestamp: u64,
) -> RecoverySnapshot {
    snapshot_from_input_with_source_paths_at(
        input,
        include_only_pending,
        &UnknownSourcePathStatusProvider,
        timestamp,
    )
}

pub fn snapshot_from_input_with_source_paths(
    input: RecoverySnapshotInput,
    include_only_pending: bool,
    source_paths: &impl SourcePathStatusProvider,
) -> RecoverySnapshot {
    snapshot_from_input_with_source_paths_at(input, include_only_pending, source_paths, 0)
}

pub fn snapshot_from_input_with_source_paths_at(
    input: RecoverySnapshotInput,
    include_only_pending: bool,
    source_paths: &impl SourcePathStatusProvider,
    timestamp: u64,
) -> RecoverySnapshot {
    let items = input
        .items
        .into_iter()
        .filter_map(|item| item.normalize_recovered(timestamp, source_paths))
        .filter(|item| !include_only_pending || item.resolution == RecoveryResolution::Pending)
        .collect::<Vec<_>>();

    RecoverySnapshot {
        version: RECOVERY_VERSION,
        updated_at: input.updated_at.filter(|_| !items.is_empty()),
        items,
    }
}

pub fn recovered_item_from_queue_input(
    input: RecoveryItemInput,
    now: u64,
) -> Option<RecoveredQueueItem> {
    recovered_item_from_queue_input_with_source_paths(input, now, &UnknownSourcePathStatusProvider)
}

pub fn recovered_item_from_queue_input_with_source_paths(
    input: RecoveryItemInput,
    now: u64,
    source_paths: &impl SourcePathStatusProvider,
) -> Option<RecoveredQueueItem> {
    input.normalize_queue(now, source_paths)
}

pub fn recovered_item_from_saved_input(
    input: RecoveryItemInput,
    now: u64,
) -> Option<RecoveredQueueItem> {
    recovered_item_from_saved_input_with_source_paths(input, now, &UnknownSourcePathStatusProvider)
}

pub fn recovered_item_from_saved_input_with_source_paths(
    input: RecoveryItemInput,
    now: u64,
    source_paths: &impl SourcePathStatusProvider,
) -> Option<RecoveredQueueItem> {
    input.normalize_recovered(now, source_paths)
}

fn normalize_segments(
    segments: Vec<RecoveredTranscriptSegment>,
) -> Vec<RecoveredTranscriptSegment> {
    segments
        .into_iter()
        .map(normalize_transcript_segment)
        .collect()
}

fn normalize_transcript_segment(
    mut segment: RecoveredTranscriptSegment,
) -> RecoveredTranscriptSegment {
    segment.start = segment.start.max(0.0);
    segment.end = segment.end.max(segment.start);
    segment.timing = normalize_existing_timing(&segment)
        .or_else(|| build_token_timing_from_legacy(&segment))
        .or_else(|| {
            Some(build_segment_timing(
                &segment,
                TranscriptTimingSource::Derived,
            ))
        });
    segment
}

fn normalize_existing_timing(
    segment: &RecoveredTranscriptSegment,
) -> Option<RecoveredTranscriptTiming> {
    let timing = segment.timing.as_ref()?;
    if timing.level == TranscriptTimingLevel::Segment {
        return Some(build_segment_timing(segment, timing.source));
    }

    let units = normalize_timing_units(timing.units.clone(), segment.start, segment.end);
    (!units.is_empty()).then_some(RecoveredTranscriptTiming {
        level: TranscriptTimingLevel::Token,
        source: timing.source,
        units,
    })
}

fn build_token_timing_from_legacy(
    segment: &RecoveredTranscriptSegment,
) -> Option<RecoveredTranscriptTiming> {
    let tokens = segment.tokens.as_ref()?;
    let timestamps = segment.timestamps.as_ref()?;
    if tokens.is_empty() || tokens.len() != timestamps.len() {
        return None;
    }

    let durations = segment
        .durations
        .as_ref()
        .filter(|values| values.len() == tokens.len());
    let units = tokens
        .iter()
        .enumerate()
        .map(|(index, token)| {
            let start = timestamps[index];
            let explicit_end = durations
                .and_then(|values| values.get(index))
                .map(|duration| start + (*duration).max(0.0));
            let next_start = timestamps.get(index + 1).copied();
            let end = explicit_end
                .or(next_start)
                .unwrap_or(segment.end)
                .max(start);
            RecoveredTranscriptTimingUnit {
                text: token.clone(),
                start,
                end,
            }
        })
        .collect::<Vec<_>>();
    let units = normalize_timing_units(units, segment.start, segment.end);

    (!units.is_empty()).then_some(RecoveredTranscriptTiming {
        level: TranscriptTimingLevel::Token,
        source: TranscriptTimingSource::Model,
        units,
    })
}

fn build_segment_timing(
    segment: &RecoveredTranscriptSegment,
    source: TranscriptTimingSource,
) -> RecoveredTranscriptTiming {
    RecoveredTranscriptTiming {
        level: TranscriptTimingLevel::Segment,
        source,
        units: vec![RecoveredTranscriptTimingUnit {
            text: segment.text.clone(),
            start: segment.start,
            end: segment.end,
        }],
    }
}

fn normalize_timing_units(
    units: Vec<RecoveredTranscriptTimingUnit>,
    start: f64,
    end: f64,
) -> Vec<RecoveredTranscriptTimingUnit> {
    let tuples = units
        .into_iter()
        .map(|u| (u.text, u.start, u.end))
        .collect();
    crate::transcription::transcript::normalize_timing_units_impl(tuples, start, end)
        .into_iter()
        .map(|(text, start, end)| RecoveredTranscriptTimingUnit { text, start, end })
        .collect()
}

fn normalize_source(
    raw_source: Option<&str>,
    automation_rule_id: Option<&String>,
) -> RecoverySource {
    match raw_source {
        Some("automation") => RecoverySource::Automation,
        Some("batch_import") => RecoverySource::BatchImport,
        _ if automation_rule_id.is_some() => RecoverySource::Automation,
        _ => RecoverySource::BatchImport,
    }
}

fn normalize_resolution(raw_resolution: Option<&str>) -> RecoveryResolution {
    match raw_resolution {
        Some("resumed") => RecoveryResolution::Resumed,
        Some("discarded") => RecoveryResolution::Discarded,
        _ => RecoveryResolution::Pending,
    }
}

fn normalize_stage(raw_stage: Option<&str>) -> RecoveryItemStage {
    match raw_stage {
        Some("transcribing") => RecoveryItemStage::Transcribing,
        Some("polishing") => RecoveryItemStage::Polishing,
        Some("translating") => RecoveryItemStage::Translating,
        Some("exporting") => RecoveryItemStage::Exporting,
        _ => RecoveryItemStage::Queued,
    }
}

fn resolve_source_flags(
    file_path: &str,
    resolution: RecoveryResolution,
    default_has_source_file: bool,
    default_can_resume: bool,
    source_paths: &impl SourcePathStatusProvider,
) -> (bool, bool) {
    let status = if file_path.trim().is_empty() {
        SourcePathStatus::Unknown
    } else {
        source_paths.status_for_path(file_path)
    };

    match status {
        SourcePathStatus::File => (true, resolution == RecoveryResolution::Pending),
        SourcePathStatus::Directory | SourcePathStatus::Missing => (false, false),
        SourcePathStatus::Unknown => (default_has_source_file, default_can_resume),
    }
}

fn filename_from_path(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or(path)
        .to_string()
}

fn normalized_tag_ids(tag_ids: Vec<String>, project_id: Option<String>) -> Vec<String> {
    if !tag_ids.is_empty() {
        return tag_ids;
    }
    project_id
        .filter(|project_id| !matches!(project_id.as_str(), "" | "inbox" | "none"))
        .into_iter()
        .collect()
}

fn non_empty_string(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}
