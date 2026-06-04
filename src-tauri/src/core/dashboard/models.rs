use serde::Serialize;

#[derive(Clone, Debug)]
pub struct ParsedTranscriptSegment {
    pub text: String,
    pub duration_seconds: f64,
    pub speaker: Option<SpeakerTag>,
}

#[derive(Clone, Debug)]
pub struct SpeakerTag {
    pub id: String,
    pub label: String,
    pub kind: SpeakerKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpeakerKind {
    Identified,
    Anonymous,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentTrendPoint {
    pub date: String,
    pub date_label: String,
    pub item_count: u64,
    pub item_count_display: String,
    pub duration_seconds: f64,
    pub duration_display: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverviewStats {
    pub item_count: u64,
    pub item_count_display: String,
    pub project_count: u64,
    pub project_count_display: String,
    pub total_duration_seconds: f64,
    pub total_duration_display: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript_character_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript_character_count_display: Option<String>,
    pub recording_count: u64,
    pub recording_count_display: String,
    pub batch_count: u64,
    pub batch_count_display: String,
    pub inbox_count: u64,
    pub inbox_count_display: String,
    pub project_assigned_count: u64,
    pub project_assigned_count_display: String,
    pub recent_daily_items: Vec<ContentTrendPoint>,
    pub is_deep_loaded: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerLeader {
    pub speaker_id: String,
    pub label: String,
    pub duration_seconds: f64,
    pub duration_display: String,
    pub segment_count: u64,
    pub segment_count_display: String,
    pub item_count: u64,
    pub item_count_display: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerStats {
    pub annotated_item_count: u64,
    pub annotated_item_count_display: String,
    pub speaker_attributed_duration: f64,
    pub speaker_attributed_duration_display: String,
    pub identified_speaker_count: u64,
    pub identified_speaker_count_display: String,
    pub anonymous_speaker_slot_count: u64,
    pub anonymous_speaker_slot_count_display: String,
    pub speaker_tagged_segment_count: u64,
    pub speaker_tagged_segment_count_display: String,
    pub total_segment_count: u64,
    pub total_segment_count_display: String,
    pub total_segment_duration: f64,
    pub total_segment_duration_display: String,
    pub identified_duration: f64,
    pub identified_duration_display: String,
    pub anonymous_duration: f64,
    pub anonymous_duration_display: String,
    pub segment_coverage_ratio: f64,
    pub segment_coverage_label: String,
    pub duration_coverage_ratio: f64,
    pub duration_coverage_label: String,
    pub top_identified_speakers: Vec<SpeakerLeader>,
    pub top_identified_speaker_rows: Vec<SpeakerLeader>,
    pub top_identified_speaker_max_value: f64,
    pub is_deep_loaded: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentStats {
    pub overview: OverviewStats,
    pub speakers: Option<SpeakerStats>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSnapshotDomainModel {
    pub content: ContentStats,
    pub llm_usage: crate::integrations::llm::llm_usage::LlmUsageDashboardStats,
    pub generated_at: String,
}
