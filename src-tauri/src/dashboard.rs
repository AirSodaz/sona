use chrono::{Duration, Local, NaiveDate, SecondsFormat, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use tauri::{Manager, Runtime};

const HISTORY_INDEX_PATH: &str = "history/index.json";
const PROJECT_INDEX_PATH: &str = "projects/index.json";
const RECENT_DAILY_WINDOW: i64 = 30;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSnapshotRequest {
    pub deep: bool,
}

#[derive(Clone, Debug)]
struct HistoryItem {
    id: String,
    timestamp: f64,
    duration: f64,
    transcript_path: String,
    kind: String,
    project_id: Option<String>,
}

#[derive(Clone, Debug)]
struct ParsedTranscriptSegment {
    text: String,
    duration_seconds: f64,
    speaker: Option<SpeakerTag>,
}

#[derive(Clone, Debug)]
struct SpeakerTag {
    id: String,
    label: String,
    kind: SpeakerKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SpeakerKind {
    Identified,
    Anonymous,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ContentTrendPoint {
    date: String,
    date_label: String,
    item_count: u64,
    item_count_display: String,
    duration_seconds: f64,
    duration_display: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OverviewStats {
    item_count: u64,
    item_count_display: String,
    project_count: u64,
    project_count_display: String,
    total_duration_seconds: f64,
    total_duration_display: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    transcript_character_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transcript_character_count_display: Option<String>,
    recording_count: u64,
    recording_count_display: String,
    batch_count: u64,
    batch_count_display: String,
    inbox_count: u64,
    inbox_count_display: String,
    project_assigned_count: u64,
    project_assigned_count_display: String,
    recent_daily_items: Vec<ContentTrendPoint>,
    is_deep_loaded: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpeakerLeader {
    speaker_id: String,
    label: String,
    duration_seconds: f64,
    duration_display: String,
    segment_count: u64,
    segment_count_display: String,
    item_count: u64,
    item_count_display: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpeakerStats {
    annotated_item_count: u64,
    annotated_item_count_display: String,
    speaker_attributed_duration: f64,
    speaker_attributed_duration_display: String,
    identified_speaker_count: u64,
    identified_speaker_count_display: String,
    anonymous_speaker_slot_count: u64,
    anonymous_speaker_slot_count_display: String,
    speaker_tagged_segment_count: u64,
    speaker_tagged_segment_count_display: String,
    total_segment_count: u64,
    total_segment_count_display: String,
    total_segment_duration: f64,
    total_segment_duration_display: String,
    identified_duration: f64,
    identified_duration_display: String,
    anonymous_duration: f64,
    anonymous_duration_display: String,
    segment_coverage_ratio: f64,
    segment_coverage_label: String,
    duration_coverage_ratio: f64,
    duration_coverage_label: String,
    top_identified_speakers: Vec<SpeakerLeader>,
    top_identified_speaker_rows: Vec<SpeakerLeader>,
    top_identified_speaker_max_value: f64,
    is_deep_loaded: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ContentStats {
    overview: OverviewStats,
    speakers: Option<SpeakerStats>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DashboardSnapshot {
    content: ContentStats,
    llm_usage: crate::integrations::llm::llm_usage::LlmUsageDashboardStats,
    generated_at: String,
}

#[tauri::command]
pub async fn get_dashboard_snapshot<R: Runtime>(
    app: tauri::AppHandle<R>,
    request: DashboardSnapshotRequest,
) -> Result<Value, String> {
    let app_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())?;

    tauri::async_runtime::spawn_blocking(move || build_dashboard_snapshot(&app_dir, request.deep))
        .await
        .map_err(|error| error.to_string())?
}

fn build_dashboard_snapshot(app_dir: &Path, deep: bool) -> Result<Value, String> {
    let history_items = read_history_items(app_dir)?;
    let project_count = read_project_count(app_dir)?;
    let llm_usage = crate::integrations::llm::llm_usage::read_dashboard_stats(app_dir);
    let mut overview = create_overview(&history_items, project_count, deep);
    let speakers = if deep {
        let transcript_analytics = aggregate_transcript_analytics(app_dir, &history_items);
        overview.transcript_character_count = Some(transcript_analytics.transcript_character_count);
        overview.transcript_character_count_display = Some(format_number(
            transcript_analytics.transcript_character_count,
        ));
        Some(transcript_analytics.speakers)
    } else {
        None
    };

    serde_json::to_value(DashboardSnapshot {
        content: ContentStats { overview, speakers },
        llm_usage,
        generated_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
    })
    .map_err(|error| error.to_string())
}

fn read_json_value(path: &Path) -> Result<Value, String> {
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&content).map_err(|error| error.to_string())
}

fn read_index_array(app_dir: &Path, relative_path: &str) -> Result<Vec<Value>, String> {
    let value = read_json_value(&app_dir.join(relative_path))?;
    value
        .as_array()
        .cloned()
        .ok_or_else(|| format!("{relative_path} is not a JSON array"))
}

fn read_history_items(app_dir: &Path) -> Result<Vec<HistoryItem>, String> {
    Ok(read_index_array(app_dir, HISTORY_INDEX_PATH)?
        .into_iter()
        .filter_map(|item| normalize_history_item(&item))
        .collect())
}

fn read_project_count(app_dir: &Path) -> Result<u64, String> {
    Ok(read_index_array(app_dir, PROJECT_INDEX_PATH)?.len() as u64)
}

fn normalize_history_item(input: &Value) -> Option<HistoryItem> {
    let source = input.as_object()?;
    let id = non_empty_string(source.get("id"))?;

    Some(HistoryItem {
        id,
        timestamp: non_negative_number(source.get("timestamp")),
        duration: non_negative_number(source.get("duration")),
        transcript_path: non_empty_string(source.get("transcriptPath")).unwrap_or_default(),
        kind: if source.get("type").and_then(Value::as_str) == Some("batch") {
            "batch".to_string()
        } else {
            "recording".to_string()
        },
        project_id: non_empty_string(source.get("projectId")),
    })
}

fn create_overview(
    history_items: &[HistoryItem],
    project_count: u64,
    is_deep_loaded: bool,
) -> OverviewStats {
    let recording_count = history_items
        .iter()
        .filter(|item| item.kind != "batch")
        .count() as u64;
    let item_count = history_items.len() as u64;
    let inbox_count = history_items
        .iter()
        .filter(|item| item.project_id.is_none())
        .count() as u64;

    let total_duration_seconds = history_items.iter().map(|item| item.duration).sum();
    let batch_count = item_count.saturating_sub(recording_count);
    let project_assigned_count = item_count.saturating_sub(inbox_count);

    OverviewStats {
        item_count,
        item_count_display: format_number(item_count),
        project_count,
        project_count_display: format_number(project_count),
        total_duration_seconds,
        total_duration_display: format_duration(total_duration_seconds),
        transcript_character_count: None,
        transcript_character_count_display: None,
        recording_count,
        recording_count_display: format_number(recording_count),
        batch_count,
        batch_count_display: format_number(batch_count),
        inbox_count,
        inbox_count_display: format_number(inbox_count),
        project_assigned_count,
        project_assigned_count_display: format_number(project_assigned_count),
        recent_daily_items: create_recent_daily_trend(history_items),
        is_deep_loaded,
    }
}

fn create_recent_daily_trend(history_items: &[HistoryItem]) -> Vec<ContentTrendPoint> {
    let mut aggregates: HashMap<String, ContentTrendPoint> = HashMap::new();
    for item in history_items {
        let key = local_date_key_from_timestamp(item.timestamp);
        aggregates
            .entry(key.clone())
            .and_modify(|existing| {
                existing.item_count += 1;
                existing.duration_seconds += item.duration;
                existing.item_count_display = format_number(existing.item_count);
                existing.duration_display = format_duration(existing.duration_seconds);
            })
            .or_insert(ContentTrendPoint {
                date: key.clone(),
                date_label: format_date_key(&key),
                item_count: 1,
                item_count_display: format_number(1),
                duration_seconds: item.duration,
                duration_display: format_duration(item.duration),
            });
    }

    let today = Local::now().date_naive();
    (0..RECENT_DAILY_WINDOW)
        .rev()
        .map(|offset| {
            let key = (today - Duration::days(offset))
                .format("%Y-%m-%d")
                .to_string();
            aggregates.remove(&key).unwrap_or(ContentTrendPoint {
                date: key.clone(),
                date_label: format_date_key(&key),
                item_count: 0,
                item_count_display: format_number(0),
                duration_seconds: 0.0,
                duration_display: format_duration(0.0),
            })
        })
        .collect()
}

struct TranscriptAnalytics {
    transcript_character_count: u64,
    speakers: SpeakerStats,
}

fn aggregate_transcript_analytics(
    app_dir: &Path,
    history_items: &[HistoryItem],
) -> TranscriptAnalytics {
    let mut speakers = create_empty_speaker_stats(true);
    let mut transcript_character_count = 0_u64;
    let mut identified_speaker_ids: HashSet<String> = HashSet::new();
    let mut leader_map: HashMap<String, SpeakerLeaderAccumulator> = HashMap::new();

    for item in history_items {
        if item.transcript_path.trim().is_empty() {
            continue;
        }

        let transcript_path = app_dir.join("history").join(&item.transcript_path);
        let segments = match read_json_value(&transcript_path) {
            Ok(value) => parse_transcript_segments(&value),
            Err(error) => {
                log::warn!(
                    "[Dashboard] Skipping transcript during deep scan: {} {error}",
                    item.transcript_path
                );
                continue;
            }
        };

        let mut item_has_speaker = false;
        let mut anonymous_ids_in_item: HashSet<String> = HashSet::new();

        for segment in segments {
            transcript_character_count += segment.text.encode_utf16().count() as u64;
            speakers.total_segment_count += 1;
            speakers.total_segment_duration += segment.duration_seconds;

            let Some(speaker) = segment.speaker else {
                continue;
            };

            item_has_speaker = true;
            speakers.speaker_tagged_segment_count += 1;
            speakers.speaker_attributed_duration += segment.duration_seconds;

            if speaker.kind == SpeakerKind::Identified {
                speakers.identified_duration += segment.duration_seconds;
                identified_speaker_ids.insert(speaker.id.clone());
                leader_map
                    .entry(speaker.id.clone())
                    .and_modify(|leader| {
                        leader.duration_seconds += segment.duration_seconds;
                        leader.segment_count += 1;
                        leader.item_ids.insert(item.id.clone());
                    })
                    .or_insert_with(|| SpeakerLeaderAccumulator {
                        speaker_id: speaker.id,
                        label: speaker.label,
                        duration_seconds: segment.duration_seconds,
                        segment_count: 1,
                        item_ids: HashSet::from([item.id.clone()]),
                    });
            } else {
                speakers.anonymous_duration += segment.duration_seconds;
                anonymous_ids_in_item.insert(speaker.id);
            }
        }

        if item_has_speaker {
            speakers.annotated_item_count += 1;
        }
        speakers.anonymous_speaker_slot_count += anonymous_ids_in_item.len() as u64;
    }

    speakers.identified_speaker_count = identified_speaker_ids.len() as u64;
    speakers.top_identified_speakers = to_sorted_leaders(leader_map);
    refresh_speaker_view_fields(&mut speakers);

    TranscriptAnalytics {
        transcript_character_count,
        speakers,
    }
}

#[derive(Clone, Debug)]
struct SpeakerLeaderAccumulator {
    speaker_id: String,
    label: String,
    duration_seconds: f64,
    segment_count: u64,
    item_ids: HashSet<String>,
}

fn to_sorted_leaders(leader_map: HashMap<String, SpeakerLeaderAccumulator>) -> Vec<SpeakerLeader> {
    let mut leaders: Vec<SpeakerLeader> = leader_map
        .into_values()
        .map(|leader| SpeakerLeader {
            speaker_id: leader.speaker_id,
            label: leader.label,
            duration_seconds: leader.duration_seconds,
            duration_display: format_duration(leader.duration_seconds),
            segment_count: leader.segment_count,
            segment_count_display: format_number(leader.segment_count),
            item_count: leader.item_ids.len() as u64,
            item_count_display: format_number(leader.item_ids.len() as u64),
        })
        .collect();

    leaders.sort_by(|left, right| {
        right
            .duration_seconds
            .partial_cmp(&left.duration_seconds)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.segment_count.cmp(&left.segment_count))
            .then_with(|| right.item_count.cmp(&left.item_count))
            .then_with(|| left.label.cmp(&right.label))
    });
    leaders
}

fn create_empty_speaker_stats(is_deep_loaded: bool) -> SpeakerStats {
    SpeakerStats {
        annotated_item_count: 0,
        annotated_item_count_display: format_number(0),
        speaker_attributed_duration: 0.0,
        speaker_attributed_duration_display: format_duration(0.0),
        identified_speaker_count: 0,
        identified_speaker_count_display: format_number(0),
        anonymous_speaker_slot_count: 0,
        anonymous_speaker_slot_count_display: format_number(0),
        speaker_tagged_segment_count: 0,
        speaker_tagged_segment_count_display: format_number(0),
        total_segment_count: 0,
        total_segment_count_display: format_number(0),
        total_segment_duration: 0.0,
        total_segment_duration_display: format_duration(0.0),
        identified_duration: 0.0,
        identified_duration_display: format_duration(0.0),
        anonymous_duration: 0.0,
        anonymous_duration_display: format_duration(0.0),
        segment_coverage_ratio: 0.0,
        segment_coverage_label: format_percent(0.0),
        duration_coverage_ratio: 0.0,
        duration_coverage_label: format_percent(0.0),
        top_identified_speakers: Vec::new(),
        top_identified_speaker_rows: Vec::new(),
        top_identified_speaker_max_value: 0.0,
        is_deep_loaded,
    }
}

fn refresh_speaker_view_fields(speakers: &mut SpeakerStats) {
    speakers.annotated_item_count_display = format_number(speakers.annotated_item_count);
    speakers.speaker_attributed_duration_display =
        format_duration(speakers.speaker_attributed_duration);
    speakers.identified_speaker_count_display = format_number(speakers.identified_speaker_count);
    speakers.anonymous_speaker_slot_count_display =
        format_number(speakers.anonymous_speaker_slot_count);
    speakers.speaker_tagged_segment_count_display =
        format_number(speakers.speaker_tagged_segment_count);
    speakers.total_segment_count_display = format_number(speakers.total_segment_count);
    speakers.total_segment_duration_display = format_duration(speakers.total_segment_duration);
    speakers.identified_duration_display = format_duration(speakers.identified_duration);
    speakers.anonymous_duration_display = format_duration(speakers.anonymous_duration);
    speakers.segment_coverage_ratio = coverage_ratio(
        speakers.speaker_tagged_segment_count as f64,
        speakers.total_segment_count as f64,
    );
    speakers.segment_coverage_label = format_percent(speakers.segment_coverage_ratio);
    speakers.duration_coverage_ratio = coverage_ratio(
        speakers.speaker_attributed_duration,
        speakers.total_segment_duration,
    );
    speakers.duration_coverage_label = format_percent(speakers.duration_coverage_ratio);
    speakers.top_identified_speaker_rows = speakers
        .top_identified_speakers
        .iter()
        .take(5)
        .cloned()
        .collect();
    speakers.top_identified_speaker_max_value = speakers
        .top_identified_speaker_rows
        .iter()
        .map(|speaker| speaker.duration_seconds)
        .fold(0.0, f64::max);
}

fn parse_transcript_segments(input: &Value) -> Vec<ParsedTranscriptSegment> {
    input
        .as_array()
        .map(|segments| {
            segments
                .iter()
                .filter_map(|segment| {
                    let source = segment.as_object()?;
                    let text = source
                        .get("text")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let start = non_negative_number(source.get("start"));
                    let end = non_negative_number(source.get("end"));

                    Some(ParsedTranscriptSegment {
                        text,
                        duration_seconds: (end - start).max(0.0),
                        speaker: normalize_speaker_tag(source.get("speaker")),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn normalize_speaker_tag(input: Option<&Value>) -> Option<SpeakerTag> {
    let source = input?.as_object()?;
    let id = non_empty_string(source.get("id"))?;
    let label = non_empty_string(source.get("label"))?;
    let kind = if source.get("kind").and_then(Value::as_str) == Some("identified") {
        SpeakerKind::Identified
    } else {
        SpeakerKind::Anonymous
    };

    Some(SpeakerTag { id, label, kind })
}

fn non_negative_number(input: Option<&Value>) -> f64 {
    match input.and_then(Value::as_f64) {
        Some(value) if value.is_finite() => value.max(0.0),
        _ => 0.0,
    }
}

fn non_empty_string(input: Option<&Value>) -> Option<String> {
    input.and_then(non_empty_string_ref)
}

fn non_empty_string_ref(input: &Value) -> Option<String> {
    let trimmed = input.as_str()?.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn local_date_key_from_timestamp(timestamp: f64) -> String {
    let millis = if timestamp.is_finite() && timestamp >= 0.0 {
        timestamp.min(i64::MAX as f64).round() as i64
    } else {
        0
    };
    Local
        .timestamp_millis_opt(millis)
        .single()
        .unwrap_or_else(Local::now)
        .format("%Y-%m-%d")
        .to_string()
}

fn format_number(value: u64) -> String {
    let raw = value.to_string();
    let mut formatted = String::new();
    for (index, character) in raw.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            formatted.push(',');
        }
        formatted.push(character);
    }
    formatted.chars().rev().collect()
}

fn format_duration(seconds: f64) -> String {
    let total_minutes = (seconds / 60.0).round().max(0.0) as u64;
    let hours = total_minutes / 60;
    let minutes = total_minutes % 60;

    if hours > 0 {
        return format!("{hours}h {minutes}m");
    }

    format!("{total_minutes}m")
}

fn coverage_ratio(numerator: f64, denominator: f64) -> f64 {
    if denominator <= 0.0 || !numerator.is_finite() || !denominator.is_finite() {
        return 0.0;
    }

    (numerator / denominator).clamp(0.0, 1.0)
}

fn format_percent(value: f64) -> String {
    format!("{}%", (value.clamp(0.0, 1.0) * 100.0).round() as u64)
}

fn format_date_key(date_key: &str) -> String {
    NaiveDate::parse_from_str(date_key, "%Y-%m-%d")
        .map(|date| date.format("%m-%d").to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn history_item(id: &str, transcript_path: &str) -> HistoryItem {
        HistoryItem {
            id: id.to_string(),
            timestamp: 1_776_668_400_000.0,
            duration: 60.0,
            transcript_path: transcript_path.to_string(),
            kind: "recording".to_string(),
            project_id: None,
        }
    }

    #[test]
    fn aggregates_transcript_characters_and_speaker_insights() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(temp_dir.path().join("history")).expect("history dir");
        fs::write(
            temp_dir.path().join("history/hist-1.json"),
            serde_json::to_string(&json!([
                {
                    "start": 0,
                    "end": 30,
                    "text": "hello world",
                    "speaker": { "id": "speaker-alice", "label": "Alice", "kind": "identified" }
                },
                {
                    "start": 30,
                    "end": 60,
                    "text": "anonymous part",
                    "speaker": { "id": "anonymous-1", "label": "Speaker 1", "kind": "anonymous" }
                },
                {
                    "start": 60,
                    "end": 120,
                    "text": "plain text"
                }
            ]))
            .expect("json"),
        )
        .expect("write transcript");

        let analytics = aggregate_transcript_analytics(
            temp_dir.path(),
            &[history_item("hist-1", "hist-1.json")],
        );

        assert_eq!(
            analytics.transcript_character_count,
            ("hello world".len() + "anonymous part".len() + "plain text".len()) as u64
        );
        assert_eq!(analytics.speakers.annotated_item_count, 1);
        assert_eq!(analytics.speakers.speaker_attributed_duration, 60.0);
        assert_eq!(analytics.speakers.identified_speaker_count, 1);
        assert_eq!(analytics.speakers.anonymous_speaker_slot_count, 1);
        assert_eq!(analytics.speakers.speaker_tagged_segment_count, 2);
        assert_eq!(analytics.speakers.total_segment_count, 3);
        assert_eq!(analytics.speakers.identified_duration, 30.0);
        assert_eq!(analytics.speakers.anonymous_duration, 30.0);
        assert_eq!(analytics.speakers.top_identified_speakers.len(), 1);
        assert_eq!(
            analytics.speakers.top_identified_speakers[0].speaker_id,
            "speaker-alice"
        );
    }

    #[test]
    fn skips_malformed_transcripts_during_deep_analytics() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(temp_dir.path().join("history")).expect("history dir");
        fs::write(temp_dir.path().join("history/bad.json"), "{not-json").expect("write bad");

        let analytics =
            aggregate_transcript_analytics(temp_dir.path(), &[history_item("bad", "bad.json")]);

        assert_eq!(analytics.transcript_character_count, 0);
        assert_eq!(analytics.speakers.total_segment_count, 0);
        assert_eq!(analytics.speakers.top_identified_speakers.len(), 0);
    }

    #[test]
    fn normalizes_llm_usage_stats_for_dashboard() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(temp_dir.path().join("analytics")).expect("analytics dir");
        fs::write(
            temp_dir.path().join("analytics/llm-usage.json"),
            serde_json::to_string(&json!({
                "startedAt": "2026-04-01T00:00:00.000Z",
                "lastUpdatedAt": "2026-04-28T00:00:00.000Z",
                "totals": {
                    "callCount": 2.4,
                    "callsWithUsage": 2,
                    "callsWithoutUsage": -1,
                    "promptTokens": 10,
                    "completionTokens": 5,
                    "totalTokens": 15
                },
                "byProvider": {
                    "ollama": { "callCount": 1, "totalTokens": 5 },
                    "open_ai": { "callCount": 1, "totalTokens": 10 }
                },
                "byCategory": {
                    "summary": { "callCount": 1, "totalTokens": 10 }
                },
                "daily": {}
            }))
            .expect("json"),
        )
        .expect("write usage");

        let stats = crate::integrations::llm::llm_usage::read_dashboard_stats(temp_dir.path());

        assert_eq!(
            stats.started_at.as_deref(),
            Some("2026-04-01T00:00:00.000Z")
        );
        assert_eq!(stats.totals.call_count, 2);
        assert_eq!(stats.totals.calls_without_usage, 0);
        assert_eq!(stats.by_provider[0].key, "open_ai");
        assert_eq!(stats.by_provider[1].key, "ollama");
        assert_eq!(stats.by_category[0].key, "summary");
        assert_eq!(stats.recent_daily.len(), RECENT_DAILY_WINDOW as usize);
    }

    #[test]
    fn fast_snapshot_omits_deep_speaker_details() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(temp_dir.path().join("history")).expect("history dir");
        fs::create_dir_all(temp_dir.path().join("projects")).expect("projects dir");
        fs::write(
            temp_dir.path().join(HISTORY_INDEX_PATH),
            serde_json::to_string(&json!([
                {
                    "id": "hist-1",
                    "timestamp": 1776668400000_u64,
                    "duration": 120,
                    "transcriptPath": "hist-1.json",
                    "type": "batch",
                    "projectId": "project-1"
                }
            ]))
            .expect("json"),
        )
        .expect("write history");
        fs::write(
            temp_dir.path().join(PROJECT_INDEX_PATH),
            serde_json::to_string(&json!([{ "id": "project-1" }])).expect("json"),
        )
        .expect("write projects");

        let snapshot = build_dashboard_snapshot(temp_dir.path(), false).expect("snapshot");

        assert_eq!(snapshot["content"]["overview"]["itemCount"], 1);
        assert_eq!(snapshot["content"]["overview"]["batchCount"], 1);
        assert_eq!(snapshot["content"]["overview"]["isDeepLoaded"], false);
        assert!(snapshot["content"]["overview"]["transcriptCharacterCount"].is_null());
        assert!(snapshot["content"]["speakers"].is_null());
    }

    #[test]
    fn snapshot_includes_view_ready_display_fields_alongside_raw_stats() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let today_timestamp = Local::now().timestamp_millis() as u64;
        fs::create_dir_all(temp_dir.path().join("history")).expect("history dir");
        fs::create_dir_all(temp_dir.path().join("projects")).expect("projects dir");
        fs::write(
            temp_dir.path().join(HISTORY_INDEX_PATH),
            serde_json::to_string(&json!([
                {
                    "id": "hist-1",
                    "timestamp": today_timestamp,
                    "duration": 3720,
                    "transcriptPath": "hist-1.json",
                    "type": "recording",
                    "projectId": "project-1"
                },
                {
                    "id": "hist-2",
                    "timestamp": today_timestamp,
                    "duration": 60,
                    "transcriptPath": "hist-2.json",
                    "type": "batch"
                }
            ]))
            .expect("json"),
        )
        .expect("write history");
        fs::write(
            temp_dir.path().join(PROJECT_INDEX_PATH),
            serde_json::to_string(&json!([{ "id": "project-1" }])).expect("json"),
        )
        .expect("write projects");
        fs::write(
            temp_dir.path().join("history/hist-1.json"),
            serde_json::to_string(&json!([
                {
                    "start": 0,
                    "end": 120,
                    "text": "speaker text",
                    "speaker": { "id": "speaker-alice", "label": "Alice", "kind": "identified" }
                },
                {
                    "start": 120,
                    "end": 180,
                    "text": "anonymous text",
                    "speaker": { "id": "anonymous-1", "label": "Speaker 1", "kind": "anonymous" }
                },
                {
                    "start": 180,
                    "end": 240,
                    "text": "untagged text"
                }
            ]))
            .expect("json"),
        )
        .expect("write transcript 1");
        fs::write(
            temp_dir.path().join("history/hist-2.json"),
            serde_json::to_string(&json!([
                {
                    "start": 0,
                    "end": 60,
                    "text": "more speaker text",
                    "speaker": { "id": "speaker-alice", "label": "Alice", "kind": "identified" }
                }
            ]))
            .expect("json"),
        )
        .expect("write transcript 2");
        fs::create_dir_all(temp_dir.path().join("analytics")).expect("analytics dir");
        fs::write(
            temp_dir.path().join("analytics/llm-usage.json"),
            serde_json::to_string(&json!({
                "startedAt": "2026-04-01T00:00:00.000Z",
                "lastUpdatedAt": "2026-04-28T00:00:00.000Z",
                "totals": {
                    "callCount": 2,
                    "callsWithUsage": 1,
                    "callsWithoutUsage": 1,
                    "promptTokens": 1200,
                    "completionTokens": 600,
                    "totalTokens": 1800
                },
                "byProvider": {
                    "ollama": { "callCount": 1, "totalTokens": 400 },
                    "open_ai": { "callCount": 2, "totalTokens": 1800 }
                },
                "byCategory": {},
                "daily": {}
            }))
            .expect("json"),
        )
        .expect("write usage");

        let snapshot = build_dashboard_snapshot(temp_dir.path(), true).expect("snapshot");
        let overview = &snapshot["content"]["overview"];
        let speakers = &snapshot["content"]["speakers"];
        let llm_usage = &snapshot["llmUsage"];

        assert_eq!(overview["itemCount"], 2);
        assert_eq!(overview["itemCountDisplay"], "2");
        assert_eq!(overview["totalDurationSeconds"], 3780.0);
        assert_eq!(overview["totalDurationDisplay"], "1h 3m");
        assert_eq!(overview["recentDailyItems"][29]["itemCountDisplay"], "2");
        assert_eq!(overview["recentDailyItems"][29]["durationDisplay"], "1h 3m");

        assert_eq!(speakers["segmentCoverageRatio"], 0.75);
        assert_eq!(speakers["segmentCoverageLabel"], "75%");
        assert_eq!(speakers["durationCoverageRatio"], 0.8);
        assert_eq!(speakers["durationCoverageLabel"], "80%");
        assert_eq!(
            speakers["topIdentifiedSpeakers"][0]["durationDisplay"],
            "3m"
        );
        assert_eq!(
            speakers["topIdentifiedSpeakerRows"][0]["durationDisplay"],
            "3m"
        );
        assert_eq!(speakers["topIdentifiedSpeakerMaxValue"], 180.0);

        assert_eq!(llm_usage["totals"]["totalTokensDisplay"], "1,800");
        assert_eq!(llm_usage["trackingSinceDisplay"], "2026-04-01");
        assert_eq!(llm_usage["byProvider"][0]["label"], "open_ai");
        assert_eq!(llm_usage["byProviderTopRows"][0]["value"], 1800);
        assert_eq!(llm_usage["byProviderMaxValue"], 1800);
    }
}
