use serde::Deserialize;
use serde_json::Value;
use serde_with::{serde_as, DefaultOnError};
use std::fs;
use std::io::ErrorKind;
use std::time::SystemTime;

use super::types::{
    RecoveredQueueItem, RecoveredTranscriptSegment, RecoveredTranscriptTiming,
    RecoveredTranscriptTimingUnit, RecoveryFileStat, RecoverySnapshot, RECOVERY_VERSION,
};

enum SourcePathStatus {
    File,
    Directory,
    Missing,
    Unknown,
}

#[serde_as]
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRecoverySnapshot {
    #[serde_as(as = "DefaultOnError")]
    #[serde(default)]
    updated_at: Option<u64>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default)]
    items: Vec<RawRecoveredQueueItem>,
}

#[serde_as]
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRecoveredQueueItem {
    #[serde_as(as = "DefaultOnError")]
    #[serde(default)]
    id: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default)]
    recovery_id: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default)]
    filename: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default)]
    file_path: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default)]
    source: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default)]
    origin: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default)]
    resolution: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default)]
    status: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default)]
    progress: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default)]
    segments: Vec<RecoveredTranscriptSegment>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default)]
    project_id: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default)]
    history_id: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default)]
    history_title: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default)]
    last_known_stage: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default)]
    updated_at: Option<u64>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default)]
    has_source_file: Option<bool>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default)]
    can_resume: Option<bool>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default)]
    automation_rule_id: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default)]
    automation_rule_name: Option<String>,
    #[serde(default)]
    resolved_config_snapshot: Option<Value>,
    #[serde(default)]
    export_config: Option<Value>,
    #[serde(default)]
    stage_config: Option<Value>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default)]
    source_fingerprint: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default)]
    file_stat: Option<RecoveryFileStat>,
    #[serde_as(as = "DefaultOnError")]
    #[serde(default)]
    export_file_name_prefix: Option<String>,
}

impl RawRecoveredQueueItem {
    fn from_value(value: Value) -> Option<Self> {
        serde_json::from_value(value).ok()
    }

    fn normalize_recovered(self, now: u64) -> Option<RecoveredQueueItem> {
        let id = non_empty_string(self.id)?;
        let file_path = self.file_path.unwrap_or_default();
        let automation_rule_id = self.automation_rule_id;
        let resolution = normalize_resolution(self.resolution.as_deref());
        let saved_has_source_file = self.has_source_file.unwrap_or(true);
        let saved_can_resume = self
            .can_resume
            .unwrap_or(saved_has_source_file && resolution == "pending");
        let (has_source_file, can_resume) = resolve_source_flags(
            &file_path,
            &resolution,
            saved_has_source_file,
            saved_can_resume,
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
            project_id: self.project_id,
            history_id: self.history_id,
            history_title: self.history_title,
            last_known_stage: normalize_stage(self.last_known_stage.as_deref()),
            updated_at: self.updated_at.unwrap_or(now),
            has_source_file,
            can_resume,
            automation_rule_id,
            automation_rule_name: self.automation_rule_name,
            resolved_config_snapshot: self.resolved_config_snapshot,
            export_config: self.export_config.unwrap_or(Value::Null),
            stage_config: self.stage_config.unwrap_or(Value::Null),
            source_fingerprint: self.source_fingerprint,
            file_stat: self.file_stat,
            export_file_name_prefix: self.export_file_name_prefix,
        })
    }

    fn normalize_queue(self, now: u64) -> Option<RecoveredQueueItem> {
        let status = self.status.as_deref()?;
        if status != "pending" && status != "processing" {
            return None;
        }

        let queue_id = non_empty_string(self.id)?;
        let id = non_empty_string(self.recovery_id).unwrap_or(queue_id);
        let file_path = self.file_path.unwrap_or_default();
        let source =
            if self.origin.as_deref() == Some("automation") || self.automation_rule_id.is_some() {
                "automation"
            } else {
                "batch_import"
            }
            .to_string();
        let resolution = "pending".to_string();
        let (has_source_file, can_resume) =
            resolve_source_flags(&file_path, &resolution, true, true);

        Some(RecoveredQueueItem {
            id,
            filename: non_empty_string(self.filename)
                .unwrap_or_else(|| filename_from_path(&file_path)),
            file_path,
            source,
            resolution,
            progress: self.progress.unwrap_or(0.0),
            segments: normalize_segments(self.segments),
            project_id: self.project_id,
            history_id: self.history_id,
            history_title: self.history_title,
            last_known_stage: normalize_stage(self.last_known_stage.as_deref()),
            updated_at: now,
            has_source_file,
            can_resume,
            automation_rule_id: self.automation_rule_id,
            automation_rule_name: self.automation_rule_name,
            resolved_config_snapshot: self.resolved_config_snapshot,
            export_config: self.export_config.unwrap_or(Value::Null),
            stage_config: self.stage_config.unwrap_or(Value::Null),
            source_fingerprint: self.source_fingerprint,
            file_stat: self.file_stat,
            export_file_name_prefix: self.export_file_name_prefix,
        })
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn empty_snapshot() -> RecoverySnapshot {
    RecoverySnapshot {
        version: RECOVERY_VERSION,
        updated_at: None,
        items: Vec::new(),
    }
}

pub fn snapshot_from_items(items: Vec<RecoveredQueueItem>) -> RecoverySnapshot {
    RecoverySnapshot {
        version: RECOVERY_VERSION,
        updated_at: (!items.is_empty()).then(now_ms),
        items,
    }
}

pub fn snapshot_from_value(value: Value, include_only_pending: bool) -> RecoverySnapshot {
    let now = now_ms();
    let raw = serde_json::from_value::<RawRecoverySnapshot>(value).unwrap_or_default();
    let items = raw
        .items
        .into_iter()
        .filter_map(|item| item.normalize_recovered(now))
        .filter(|item| !include_only_pending || item.resolution == "pending")
        .collect::<Vec<_>>();

    RecoverySnapshot {
        version: RECOVERY_VERSION,
        updated_at: raw.updated_at.filter(|_| !items.is_empty()),
        items,
    }
}

pub fn recovered_item_from_queue_value(value: Value, now: u64) -> Option<RecoveredQueueItem> {
    RawRecoveredQueueItem::from_value(value)?.normalize_queue(now)
}

pub fn recovered_item_from_saved_value(value: Value, now: u64) -> Option<RecoveredQueueItem> {
    RawRecoveredQueueItem::from_value(value)?.normalize_recovered(now)
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
        .or_else(|| Some(build_segment_timing(&segment, "derived")));
    segment
}

fn normalize_existing_timing(
    segment: &RecoveredTranscriptSegment,
) -> Option<RecoveredTranscriptTiming> {
    let timing = segment.timing.as_ref()?;
    if !is_valid_timing_source(&timing.source) {
        return None;
    }

    if timing.level == "segment" {
        return Some(build_segment_timing(segment, &timing.source));
    }

    if timing.level != "token" {
        return None;
    }

    let units = normalize_timing_units(timing.units.clone(), segment.start, segment.end);
    (!units.is_empty()).then(|| RecoveredTranscriptTiming {
        level: "token".to_string(),
        source: timing.source.clone(),
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

    (!units.is_empty()).then(|| RecoveredTranscriptTiming {
        level: "token".to_string(),
        source: "model".to_string(),
        units,
    })
}

fn build_segment_timing(
    segment: &RecoveredTranscriptSegment,
    source: &str,
) -> RecoveredTranscriptTiming {
    RecoveredTranscriptTiming {
        level: "segment".to_string(),
        source: source.to_string(),
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
    let safe_start = start.max(0.0);
    let safe_end = end.max(safe_start);
    let unit_count = units.len();

    units
        .into_iter()
        .enumerate()
        .filter_map(|(index, unit)| {
            if unit.text.is_empty() {
                return None;
            }

            let unit_start = unit.start.max(safe_start).min(safe_end);
            let fallback_end = if index + 1 == unit_count {
                safe_end
            } else {
                unit_start
            };
            let unit_end = unit.end.max(fallback_end).min(safe_end).max(unit_start);

            Some(RecoveredTranscriptTimingUnit {
                text: unit.text,
                start: unit_start,
                end: unit_end,
            })
        })
        .collect()
}

fn is_valid_timing_source(source: &str) -> bool {
    source == "model" || source == "derived"
}

fn normalize_source(raw_source: Option<&str>, automation_rule_id: Option<&String>) -> String {
    match raw_source {
        Some("automation") => "automation".to_string(),
        Some("batch_import") => "batch_import".to_string(),
        _ if automation_rule_id.is_some() => "automation".to_string(),
        _ => "batch_import".to_string(),
    }
}

fn normalize_resolution(raw_resolution: Option<&str>) -> String {
    match raw_resolution {
        Some("resumed") => "resumed".to_string(),
        Some("discarded") => "discarded".to_string(),
        _ => "pending".to_string(),
    }
}

fn normalize_stage(raw_stage: Option<&str>) -> String {
    match raw_stage {
        Some("transcribing") => "transcribing".to_string(),
        Some("polishing") => "polishing".to_string(),
        Some("translating") => "translating".to_string(),
        Some("exporting") => "exporting".to_string(),
        _ => "queued".to_string(),
    }
}

fn resolve_source_flags(
    file_path: &str,
    resolution: &str,
    default_has_source_file: bool,
    default_can_resume: bool,
) -> (bool, bool) {
    match resolve_source_path_status(file_path) {
        SourcePathStatus::File => (true, resolution == "pending"),
        SourcePathStatus::Directory | SourcePathStatus::Missing => (false, false),
        SourcePathStatus::Unknown => (default_has_source_file, default_can_resume),
    }
}

fn resolve_source_path_status(path: &str) -> SourcePathStatus {
    if path.trim().is_empty() {
        return SourcePathStatus::Unknown;
    }

    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => SourcePathStatus::File,
        Ok(metadata) if metadata.is_dir() => SourcePathStatus::Directory,
        Ok(_) => SourcePathStatus::Unknown,
        Err(error) if error.kind() == ErrorKind::NotFound => SourcePathStatus::Missing,
        Err(_) => SourcePathStatus::Unknown,
    }
}

fn filename_from_path(path: &str) -> String {
    path.rsplit(|ch| ch == '/' || ch == '\\')
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or(path)
        .to_string()
}

fn non_empty_string(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}
