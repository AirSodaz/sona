use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::{Duration, Local, NaiveDate, TimeZone};

use super::error::DashboardServiceError;
use super::models::*;
use super::ports::{AnalyticsRepository, ProjectRepository};
use crate::core::history_store::HistoryStore;
use crate::repositories::history::{HistoryItemKind, HistoryItemRecord};

const RECENT_DAILY_WINDOW: i64 = 30;

pub struct DashboardService<H, P, A>
where
    H: HistoryStore,
    P: ProjectRepository,
    A: AnalyticsRepository,
{
    history_repo: Arc<H>,
    project_repo: Arc<P>,
    analytics_repo: Arc<A>,
}

impl<H, P, A> DashboardService<H, P, A>
where
    H: HistoryStore,
    P: ProjectRepository,
    A: AnalyticsRepository,
{
    pub fn new(history_repo: Arc<H>, project_repo: Arc<P>, analytics_repo: Arc<A>) -> Self {
        Self {
            history_repo,
            project_repo,
            analytics_repo,
        }
    }

    pub async fn build_snapshot(
        &self,
        deep: bool,
    ) -> Result<DashboardSnapshotDomainModel, DashboardServiceError> {
        let history_items = self
            .history_repo
            .list_items()
            .map_err(DashboardServiceError::HistoryStore)?;
        let project_count = self.project_repo.count_projects().await?;
        let llm_usage = self.analytics_repo.read_dashboard_stats().await?;

        let mut overview = create_overview(&history_items, project_count, deep);

        let speakers = if deep {
            let transcript_analytics = self.aggregate_transcript_analytics(&history_items)?;
            overview.transcript_character_count =
                Some(transcript_analytics.transcript_character_count);
            overview.transcript_character_count_display = Some(format_number(
                transcript_analytics.transcript_character_count,
            ));
            Some(transcript_analytics.speakers)
        } else {
            None
        };

        let generated_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

        Ok(DashboardSnapshotDomainModel {
            content: ContentStats { overview, speakers },
            llm_usage,
            generated_at,
        })
    }

    fn aggregate_transcript_analytics(
        &self,
        history_items: &[HistoryItemRecord],
    ) -> Result<TranscriptAnalytics, DashboardServiceError> {
        let mut speakers = create_empty_speaker_stats(true);
        let mut transcript_character_count = 0_u64;
        let mut identified_speaker_ids: HashSet<String> = HashSet::new();
        let mut leader_map: HashMap<String, SpeakerLeaderAccumulator> = HashMap::new();

        for item in history_items {
            let transcript_segments = match self.history_repo.load_transcript(&item.id) {
                Ok(Some(segments)) => segments,
                Ok(None) => continue,
                Err(error) => {
                    log::warn!(
                        "[Dashboard] Skipping transcript during deep scan: {} {error}",
                        item.id
                    );
                    continue;
                }
            };

            let segments: Vec<ParsedTranscriptSegment> = transcript_segments
                .into_iter()
                .map(|s| {
                    let speaker = s.speaker.map(|sp| SpeakerTag {
                        id: sp.id.clone(),
                        label: sp.label.clone(),
                        kind: if sp.kind == "identified" {
                            SpeakerKind::Identified
                        } else {
                            SpeakerKind::Anonymous
                        },
                    });
                    ParsedTranscriptSegment {
                        text: s.text,
                        duration_seconds: (s.end - s.start).max(0.0),
                        speaker,
                    }
                })
                .collect();

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
                            speaker_id: speaker.id.clone(),
                            label: speaker.label.clone(),
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

        Ok(TranscriptAnalytics {
            transcript_character_count,
            speakers,
        })
    }
}

fn create_overview(
    history_items: &[HistoryItemRecord],
    project_count: u64,
    is_deep_loaded: bool,
) -> OverviewStats {
    let recording_count = history_items
        .iter()
        .filter(|item| item.kind != HistoryItemKind::Batch)
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

fn create_recent_daily_trend(history_items: &[HistoryItemRecord]) -> Vec<ContentTrendPoint> {
    let mut aggregates: HashMap<String, ContentTrendPoint> = HashMap::new();
    for item in history_items {
        let key = local_date_key_from_timestamp(item.timestamp as f64);
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
    use crate::core::dashboard::error::DashboardServiceError;
    use crate::integrations::asr::TranscriptSegment;
    use crate::integrations::llm::llm_usage::LlmUsageDashboardStats;
    use crate::repositories::history::{
        HistoryBackupSnapshot, HistoryCreateLiveDraftRequest, HistoryDraftSource, HistoryItemKind,
        HistoryItemStatus, HistoryListOptions, HistorySaveImportedFileRequest,
        HistorySaveRecordingRequest, HistoryWorkspaceItemCounts, HistoryWorkspaceQueryRequest,
        HistoryWorkspaceQueryResult, HistoryWorkspaceSummary, LiveRecordingDraftResult,
        TranscriptSnapshotMetadata, TranscriptSnapshotReason, TranscriptSnapshotRecord,
    };
    use serde_json::Value;
    use std::collections::BTreeMap;

    struct MockHistoryRepo {
        items: Vec<HistoryItemRecord>,
        segments: HashMap<String, Vec<ParsedTranscriptSegment>>,
    }

    impl HistoryStore for MockHistoryRepo {
        fn ensure_ready(&self) -> Result<(), String> {
            Ok(())
        }

        fn list_items(&self) -> Result<Vec<HistoryItemRecord>, String> {
            Ok(self.items.clone())
        }

        fn list_items_with_reconciled_live_drafts(&self) -> Result<Vec<HistoryItemRecord>, String> {
            Ok(self.items.clone())
        }

        fn list_items_paginated(
            &self,
            _opts: HistoryListOptions,
        ) -> Result<Vec<HistoryItemRecord>, String> {
            Ok(self.items.clone())
        }

        fn list_items_with_reconciled_live_drafts_paginated(
            &self,
            _opts: HistoryListOptions,
        ) -> Result<Vec<HistoryItemRecord>, String> {
            Ok(self.items.clone())
        }

        fn query_workspace(
            &self,
            _request: HistoryWorkspaceQueryRequest,
        ) -> Result<HistoryWorkspaceQueryResult, String> {
            Ok(HistoryWorkspaceQueryResult {
                filtered_items: vec![],
                scoped_items: vec![],
                scoped_item_ids: vec![],
                search_match_by_item_id: BTreeMap::new(),
                summary: HistoryWorkspaceSummary {
                    total_items: 0,
                    total_duration: 0.0,
                    latest_timestamp: None,
                    recording_count: 0,
                    batch_count: 0,
                },
                item_counts: HistoryWorkspaceItemCounts {
                    inbox: 0,
                    by_project_id: BTreeMap::new(),
                },
            })
        }

        fn create_live_draft(
            &self,
            _request: HistoryCreateLiveDraftRequest,
        ) -> Result<LiveRecordingDraftResult, String> {
            Ok(LiveRecordingDraftResult {
                item: HistoryItemRecord {
                    id: String::new(),
                    timestamp: 0,
                    duration: 0.0,
                    audio_path: String::new(),
                    transcript_path: String::new(),
                    title: String::new(),
                    preview_text: String::new(),
                    icon: None,
                    kind: HistoryItemKind::Recording,
                    search_content: String::new(),
                    project_id: None,
                    status: HistoryItemStatus::Draft,
                    draft_source: Some(HistoryDraftSource::LiveRecord),
                },
                audio_absolute_path: String::new(),
            })
        }

        fn complete_live_draft(
            &self,
            _history_id: &str,
            _segments: Value,
            _duration: f64,
        ) -> Result<HistoryItemRecord, String> {
            Ok(HistoryItemRecord {
                id: _history_id.to_string(),
                timestamp: 0,
                duration: _duration,
                audio_path: String::new(),
                transcript_path: String::new(),
                title: String::new(),
                preview_text: String::new(),
                icon: None,
                kind: HistoryItemKind::Recording,
                search_content: String::new(),
                project_id: None,
                status: HistoryItemStatus::Complete,
                draft_source: None,
            })
        }

        fn save_recording(
            &self,
            _request: HistorySaveRecordingRequest,
        ) -> Result<HistoryItemRecord, String> {
            Ok(HistoryItemRecord {
                id: String::new(),
                timestamp: 0,
                duration: _request.duration,
                audio_path: String::new(),
                transcript_path: String::new(),
                title: String::new(),
                preview_text: String::new(),
                icon: None,
                kind: HistoryItemKind::Recording,
                search_content: String::new(),
                project_id: None,
                status: HistoryItemStatus::Complete,
                draft_source: None,
            })
        }

        fn save_imported_file(
            &self,
            _request: HistorySaveImportedFileRequest,
        ) -> Result<HistoryItemRecord, String> {
            Ok(HistoryItemRecord {
                id: String::new(),
                timestamp: 0,
                duration: _request.duration,
                audio_path: String::new(),
                transcript_path: String::new(),
                title: String::new(),
                preview_text: String::new(),
                icon: None,
                kind: HistoryItemKind::Recording,
                search_content: String::new(),
                project_id: None,
                status: HistoryItemStatus::Complete,
                draft_source: None,
            })
        }

        fn delete_items(&self, _ids: &[String]) -> Result<(), String> {
            Ok(())
        }

        fn load_transcript(
            &self,
            history_id: &str,
        ) -> Result<Option<Vec<TranscriptSegment>>, String> {
            let parsed = self.segments.get(history_id).cloned().unwrap_or_default();
            Ok(Some(
                parsed
                    .into_iter()
                    .map(|s| TranscriptSegment {
                        id: String::new(),
                        text: s.text,
                        start: 0.0,
                        end: s.duration_seconds,
                        is_final: true,
                        timing: None,
                        tokens: None,
                        timestamps: None,
                        durations: None,
                        translation: None,
                        speaker: s
                            .speaker
                            .map(|sp| crate::integrations::speaker::SpeakerTag {
                                id: sp.id,
                                label: sp.label,
                                kind: if sp.kind == SpeakerKind::Identified {
                                    "identified".to_string()
                                } else {
                                    "anonymous".to_string()
                                },
                                score: None,
                            }),
                        speaker_attribution: None,
                    })
                    .collect(),
            ))
        }

        fn update_transcript(
            &self,
            history_id: &str,
            _segments: Value,
        ) -> Result<HistoryItemRecord, String> {
            Ok(HistoryItemRecord {
                id: history_id.to_string(),
                ..self
                    .list_items()?
                    .into_iter()
                    .find(|i| i.id == history_id)
                    .unwrap_or(HistoryItemRecord {
                        id: history_id.to_string(),
                        timestamp: 0,
                        duration: 0.0,
                        audio_path: String::new(),
                        transcript_path: String::new(),
                        title: String::new(),
                        preview_text: String::new(),
                        icon: None,
                        kind: HistoryItemKind::Recording,
                        search_content: String::new(),
                        project_id: None,
                        status: HistoryItemStatus::Complete,
                        draft_source: None,
                    })
            })
        }

        fn create_transcript_snapshot(
            &self,
            history_id: &str,
            reason: TranscriptSnapshotReason,
            _segments: Value,
        ) -> Result<TranscriptSnapshotMetadata, String> {
            Ok(TranscriptSnapshotMetadata {
                id: String::new(),
                history_id: history_id.to_string(),
                reason,
                created_at: 0,
                segment_count: 0,
            })
        }

        fn list_transcript_snapshots(
            &self,
            _history_id: &str,
        ) -> Result<Vec<TranscriptSnapshotMetadata>, String> {
            Ok(vec![])
        }

        fn load_transcript_snapshot(
            &self,
            _history_id: &str,
            _snapshot_id: &str,
        ) -> Result<Option<TranscriptSnapshotRecord>, String> {
            Ok(None)
        }

        fn update_item_meta(&self, _history_id: &str, _updates: Value) -> Result<(), String> {
            Ok(())
        }

        fn update_project_assignments(
            &self,
            _ids: &[String],
            _project_id: Option<String>,
        ) -> Result<(), String> {
            Ok(())
        }

        fn reassign_project(
            &self,
            _current_project_id: String,
            _next_project_id: Option<String>,
        ) -> Result<(), String> {
            Ok(())
        }

        fn load_summary(&self, _history_id: &str) -> Result<Option<Value>, String> {
            Ok(None)
        }

        fn save_summary(&self, _history_id: &str, _summary_payload: Value) -> Result<(), String> {
            Ok(())
        }

        fn delete_summary(&self, _history_id: &str) -> Result<(), String> {
            Ok(())
        }

        fn resolve_audio_path(&self, _history_id: &str) -> Result<Option<String>, String> {
            Ok(None)
        }

        fn history_snapshot_for_backup(&self) -> Result<HistoryBackupSnapshot, String> {
            Ok(HistoryBackupSnapshot {
                items: vec![],
                transcript_files: vec![],
                summary_files: vec![],
                snapshot_files: vec![],
            })
        }
    }

    struct MockProjectRepo {
        count: u64,
    }

    #[async_trait::async_trait]
    impl ProjectRepository for MockProjectRepo {
        async fn count_projects(&self) -> Result<u64, DashboardServiceError> {
            Ok(self.count)
        }
    }

    struct MockAnalyticsRepo;

    #[async_trait::async_trait]
    impl AnalyticsRepository for MockAnalyticsRepo {
        async fn read_dashboard_stats(
            &self,
        ) -> Result<LlmUsageDashboardStats, DashboardServiceError> {
            Ok(LlmUsageDashboardStats {
                started_at: None,
                last_updated_at: None,
                tracking_since_display: None,
                last_updated_display: None,
                totals: crate::integrations::llm::llm_usage::DashboardUsageBucket {
                    call_count: 0,
                    call_count_display: "0".to_string(),
                    calls_with_usage: 0,
                    calls_with_usage_display: "0".to_string(),
                    calls_without_usage: 0,
                    calls_without_usage_display: "0".to_string(),
                    prompt_tokens: 0,
                    prompt_tokens_display: "0".to_string(),
                    completion_tokens: 0,
                    completion_tokens_display: "0".to_string(),
                    total_tokens: 0,
                    total_tokens_display: "0".to_string(),
                },
                by_provider: vec![],
                by_provider_top_rows: vec![],
                by_provider_max_value: 0,
                by_category: vec![],
                by_category_top_rows: vec![],
                by_category_max_value: 0,
                recent_daily: vec![],
            })
        }
    }

    fn history_item(id: &str) -> HistoryItemRecord {
        HistoryItemRecord {
            id: id.to_string(),
            timestamp: 1_776_668_400_000,
            duration: 60.0,
            audio_path: "".to_string(),
            transcript_path: format!("{id}.json"),
            title: "".to_string(),
            preview_text: "".to_string(),
            icon: None,
            kind: HistoryItemKind::Recording,
            search_content: "".to_string(),
            project_id: None,
            status: HistoryItemStatus::Complete,
            draft_source: None,
        }
    }

    #[tokio::test]
    async fn aggregates_transcript_characters_and_speaker_insights() {
        let mut segments = HashMap::new();
        segments.insert(
            "hist-1".to_string(),
            vec![
                ParsedTranscriptSegment {
                    text: "hello world".to_string(),
                    duration_seconds: 30.0,
                    speaker: Some(SpeakerTag {
                        id: "speaker-alice".to_string(),
                        label: "Alice".to_string(),
                        kind: SpeakerKind::Identified,
                    }),
                },
                ParsedTranscriptSegment {
                    text: "anonymous part".to_string(),
                    duration_seconds: 30.0,
                    speaker: Some(SpeakerTag {
                        id: "anonymous-1".to_string(),
                        label: "Speaker 1".to_string(),
                        kind: SpeakerKind::Anonymous,
                    }),
                },
                ParsedTranscriptSegment {
                    text: "plain text".to_string(),
                    duration_seconds: 60.0,
                    speaker: None,
                },
            ],
        );

        let service = DashboardService::new(
            Arc::new(MockHistoryRepo {
                items: vec![history_item("hist-1")],
                segments,
            }),
            Arc::new(MockProjectRepo { count: 1 }),
            Arc::new(MockAnalyticsRepo),
        );

        let snapshot = service.build_snapshot(true).await.expect("snapshot");
        let analytics = snapshot.content.speakers.unwrap();

        assert_eq!(
            snapshot
                .content
                .overview
                .transcript_character_count
                .unwrap(),
            ("hello world".len() + "anonymous part".len() + "plain text".len()) as u64
        );
        assert_eq!(analytics.annotated_item_count, 1);
        assert_eq!(analytics.speaker_attributed_duration, 60.0);
        assert_eq!(analytics.identified_speaker_count, 1);
        assert_eq!(analytics.anonymous_speaker_slot_count, 1);
        assert_eq!(analytics.speaker_tagged_segment_count, 2);
        assert_eq!(analytics.total_segment_count, 3);
        assert_eq!(analytics.identified_duration, 30.0);
        assert_eq!(analytics.anonymous_duration, 30.0);
        assert_eq!(analytics.top_identified_speakers.len(), 1);
        assert_eq!(
            analytics.top_identified_speakers[0].speaker_id,
            "speaker-alice"
        );
    }
}
