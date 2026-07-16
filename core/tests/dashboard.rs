use async_trait::async_trait;
use chrono::NaiveDate;
use sona_core::dashboard::models::{DashboardUsageBucket, LlmUsageDashboardStats};
use sona_core::dashboard::ports::{AnalyticsRepository, HistoryRepository, TagRepository};
use sona_core::dashboard::{DashboardService, DashboardServiceError, DashboardSnapshotTime};
use sona_core::history::{
    HistoryAudioStatus, HistoryItemKind, HistoryItemRecord, HistoryItemStatus,
};
use sona_core::transcription::transcript::TranscriptSegment;
use std::sync::Arc;

struct TestHistoryRepository {
    items: Vec<HistoryItemRecord>,
    transcripts: Vec<(String, Vec<TranscriptSegment>)>,
}

#[async_trait]
impl HistoryRepository for TestHistoryRepository {
    async fn list_items(&self) -> Result<Vec<HistoryItemRecord>, DashboardServiceError> {
        Ok(self.items.clone())
    }

    async fn load_transcript(
        &self,
        history_id: &str,
    ) -> Result<Option<Vec<TranscriptSegment>>, DashboardServiceError> {
        Ok(self
            .transcripts
            .iter()
            .find(|(id, _)| id == history_id)
            .map(|(_, segments)| segments.clone()))
    }
}

struct TestTagRepository;

#[async_trait]
impl TagRepository for TestTagRepository {
    async fn count_tags(&self) -> Result<u64, DashboardServiceError> {
        Ok(3)
    }
}

struct TestAnalyticsRepository;

#[async_trait]
impl AnalyticsRepository for TestAnalyticsRepository {
    async fn read_dashboard_stats(&self) -> Result<LlmUsageDashboardStats, DashboardServiceError> {
        Ok(LlmUsageDashboardStats {
            started_at: None,
            last_updated_at: None,
            tracking_since_display: None,
            last_updated_display: None,
            totals: DashboardUsageBucket {
                call_count: 2,
                call_count_display: "2".to_string(),
                calls_with_usage: 2,
                calls_with_usage_display: "2".to_string(),
                calls_without_usage: 0,
                calls_without_usage_display: "0".to_string(),
                prompt_tokens: 10,
                prompt_tokens_display: "10".to_string(),
                completion_tokens: 20,
                completion_tokens_display: "20".to_string(),
                total_tokens: 30,
                total_tokens_display: "30".to_string(),
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

fn snapshot_time() -> DashboardSnapshotTime {
    DashboardSnapshotTime {
        generated_at: "2026-07-08T01:02:03.004Z".to_string(),
        today: NaiveDate::from_ymd_opt(2026, 7, 8).unwrap(),
        local_utc_offset_seconds: 8 * 60 * 60,
    }
}

fn history_item(id: &str, kind: HistoryItemKind, tag_id: Option<&str>) -> HistoryItemRecord {
    HistoryItemRecord {
        id: id.to_string(),
        timestamp: 1_783_441_800_000,
        duration: 120.0,
        audio_path: String::new(),
        audio_status: HistoryAudioStatus::Available,
        transcript_path: format!("{id}.json"),
        title: id.to_string(),
        preview_text: String::new(),
        icon: None,
        kind,
        search_content: String::new(),
        tag_ids: tag_id.map(str::to_string).into_iter().collect(),
        deleted_at: None,
        status: HistoryItemStatus::Complete,
        draft_source: None,
    }
}

#[tokio::test]
async fn dashboard_service_uses_core_ports_for_shallow_snapshot() {
    let service = DashboardService::new(
        Arc::new(TestHistoryRepository {
            items: vec![
                history_item("recording", HistoryItemKind::Recording, None),
                history_item("batch", HistoryItemKind::Batch, Some("tag-1")),
            ],
            transcripts: vec![],
        }),
        Arc::new(TestTagRepository),
        Arc::new(TestAnalyticsRepository),
    );

    let snapshot = service
        .build_snapshot_at(false, snapshot_time())
        .await
        .unwrap();

    assert_eq!(snapshot.content.overview.item_count, 2);
    assert_eq!(snapshot.content.overview.recording_count, 1);
    assert_eq!(snapshot.content.overview.batch_count, 1);
    assert_eq!(snapshot.content.overview.tag_count, 3);
    assert_eq!(snapshot.content.overview.untagged_count, 1);
    assert_eq!(snapshot.content.overview.tagged_count, 1);
    assert_eq!(snapshot.llm_usage.totals.total_tokens, 30);
    assert!(snapshot.content.speakers.is_none());
    assert_eq!(
        snapshot
            .content
            .overview
            .recent_daily_items
            .iter()
            .find(|point| point.item_count == 2)
            .map(|point| point.date.as_str()),
        Some("2026-07-08")
    );
}

#[tokio::test]
async fn dashboard_service_uses_supplied_snapshot_time() {
    let service = DashboardService::new(
        Arc::new(TestHistoryRepository {
            items: vec![],
            transcripts: vec![],
        }),
        Arc::new(TestTagRepository),
        Arc::new(TestAnalyticsRepository),
    );

    let snapshot = service
        .build_snapshot_at(false, snapshot_time())
        .await
        .unwrap();

    assert_eq!(snapshot.generated_at, "2026-07-08T01:02:03.004Z");
    assert_eq!(
        snapshot
            .content
            .overview
            .recent_daily_items
            .last()
            .map(|point| point.date.as_str()),
        Some("2026-07-08")
    );
}

fn transcript_segment(
    text: &str,
    end: f64,
    speaker: Option<sona_core::transcription::transcript::SpeakerTag>,
) -> TranscriptSegment {
    TranscriptSegment {
        id: format!("segment-{text}"),
        text: text.to_string(),
        start: 0.0,
        end,
        is_final: true,
        timing: None,
        tokens: None,
        timestamps: None,
        durations: None,
        translation: None,
        speaker,
        speaker_attribution: None,
    }
}

fn speaker(id: &str, label: &str, kind: &str) -> sona_core::transcription::transcript::SpeakerTag {
    sona_core::transcription::transcript::SpeakerTag {
        id: id.to_string(),
        label: label.to_string(),
        kind: kind.to_string(),
        score: None,
    }
}

#[tokio::test]
async fn dashboard_service_aggregates_deep_transcript_speaker_stats_from_core_port() {
    let service = DashboardService::new(
        Arc::new(TestHistoryRepository {
            items: vec![history_item("hist-1", HistoryItemKind::Recording, None)],
            transcripts: vec![(
                "hist-1".to_string(),
                vec![
                    transcript_segment(
                        "hello world",
                        30.0,
                        Some(speaker("speaker-alice", "Alice", "identified")),
                    ),
                    transcript_segment(
                        "anonymous part",
                        30.0,
                        Some(speaker("anonymous-1", "Speaker 1", "anonymous")),
                    ),
                    transcript_segment("plain text", 60.0, None),
                ],
            )],
        }),
        Arc::new(TestTagRepository),
        Arc::new(TestAnalyticsRepository),
    );

    let snapshot = service
        .build_snapshot_at(true, snapshot_time())
        .await
        .unwrap();
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
