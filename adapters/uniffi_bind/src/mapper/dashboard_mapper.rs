use sona_core::dashboard::models::{
    ContentStats, ContentTrendPoint, DashboardSnapshotDomainModel, DashboardUsageBucket,
    LlmUsageDashboardStats, OverviewStats, SpeakerLeader, SpeakerStats, UsageBreakdown,
    UsageTrendPoint,
};

// Dashboard records are read-only projections: Core renders every number
// alongside a preformatted `*_display` string so each surface shows identical
// text. Both travel across the binding for the same reason.

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiDashboardUsageBucketV1 {
    pub call_count: u64,
    pub call_count_display: String,
    pub calls_with_usage: u64,
    pub calls_with_usage_display: String,
    pub calls_without_usage: u64,
    pub calls_without_usage_display: String,
    pub prompt_tokens: u64,
    pub prompt_tokens_display: String,
    pub completion_tokens: u64,
    pub completion_tokens_display: String,
    pub total_tokens: u64,
    pub total_tokens_display: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiUsageBreakdownV1 {
    pub key: String,
    pub label: String,
    pub stats: FfiDashboardUsageBucketV1,
    pub value: u64,
    pub value_display: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiUsageTrendPointV1 {
    pub date: String,
    pub date_label: String,
    pub stats: FfiDashboardUsageBucketV1,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiLlmUsageDashboardStatsV1 {
    pub started_at: Option<String>,
    pub last_updated_at: Option<String>,
    pub tracking_since_display: Option<String>,
    pub last_updated_display: Option<String>,
    pub totals: FfiDashboardUsageBucketV1,
    pub by_provider: Vec<FfiUsageBreakdownV1>,
    pub by_provider_top_rows: Vec<FfiUsageBreakdownV1>,
    pub by_provider_max_value: u64,
    pub by_category: Vec<FfiUsageBreakdownV1>,
    pub by_category_top_rows: Vec<FfiUsageBreakdownV1>,
    pub by_category_max_value: u64,
    pub recent_daily: Vec<FfiUsageTrendPointV1>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiContentTrendPointV1 {
    pub date: String,
    pub date_label: String,
    pub item_count: u64,
    pub item_count_display: String,
    pub duration_seconds: f64,
    pub duration_display: String,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiOverviewStatsV1 {
    pub item_count: u64,
    pub item_count_display: String,
    pub tag_count: u64,
    pub tag_count_display: String,
    pub total_duration_seconds: f64,
    pub total_duration_display: String,
    pub transcript_character_count: Option<u64>,
    pub transcript_character_count_display: Option<String>,
    pub recording_count: u64,
    pub recording_count_display: String,
    pub batch_count: u64,
    pub batch_count_display: String,
    pub untagged_count: u64,
    pub untagged_count_display: String,
    pub tagged_count: u64,
    pub tagged_count_display: String,
    pub recent_daily_items: Vec<FfiContentTrendPointV1>,
    pub is_deep_loaded: bool,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiSpeakerLeaderV1 {
    pub speaker_id: String,
    pub label: String,
    pub duration_seconds: f64,
    pub duration_display: String,
    pub segment_count: u64,
    pub segment_count_display: String,
    pub item_count: u64,
    pub item_count_display: String,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiSpeakerStatsV1 {
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
    pub top_identified_speakers: Vec<FfiSpeakerLeaderV1>,
    pub top_identified_speaker_rows: Vec<FfiSpeakerLeaderV1>,
    pub top_identified_speaker_max_value: f64,
    pub is_deep_loaded: bool,
}

/// `speakers` is absent on a shallow load, which is not the same as a load that
/// found no speakers, so the option is part of the contract.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiContentStatsV1 {
    pub overview: FfiOverviewStatsV1,
    pub speakers: Option<FfiSpeakerStatsV1>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiDashboardSnapshotV1 {
    pub content: FfiContentStatsV1,
    pub llm_usage: FfiLlmUsageDashboardStatsV1,
    pub generated_at: String,
}

impl From<DashboardUsageBucket> for FfiDashboardUsageBucketV1 {
    fn from(value: DashboardUsageBucket) -> Self {
        Self {
            call_count: value.call_count,
            call_count_display: value.call_count_display,
            calls_with_usage: value.calls_with_usage,
            calls_with_usage_display: value.calls_with_usage_display,
            calls_without_usage: value.calls_without_usage,
            calls_without_usage_display: value.calls_without_usage_display,
            prompt_tokens: value.prompt_tokens,
            prompt_tokens_display: value.prompt_tokens_display,
            completion_tokens: value.completion_tokens,
            completion_tokens_display: value.completion_tokens_display,
            total_tokens: value.total_tokens,
            total_tokens_display: value.total_tokens_display,
        }
    }
}

impl From<UsageBreakdown> for FfiUsageBreakdownV1 {
    fn from(value: UsageBreakdown) -> Self {
        Self {
            key: value.key,
            label: value.label,
            stats: value.stats.into(),
            value: value.value,
            value_display: value.value_display,
        }
    }
}

impl From<UsageTrendPoint> for FfiUsageTrendPointV1 {
    fn from(value: UsageTrendPoint) -> Self {
        Self {
            date: value.date,
            date_label: value.date_label,
            stats: value.stats.into(),
        }
    }
}

impl From<LlmUsageDashboardStats> for FfiLlmUsageDashboardStatsV1 {
    fn from(value: LlmUsageDashboardStats) -> Self {
        Self {
            started_at: value.started_at,
            last_updated_at: value.last_updated_at,
            tracking_since_display: value.tracking_since_display,
            last_updated_display: value.last_updated_display,
            totals: value.totals.into(),
            by_provider: value.by_provider.into_iter().map(Into::into).collect(),
            by_provider_top_rows: value
                .by_provider_top_rows
                .into_iter()
                .map(Into::into)
                .collect(),
            by_provider_max_value: value.by_provider_max_value,
            by_category: value.by_category.into_iter().map(Into::into).collect(),
            by_category_top_rows: value
                .by_category_top_rows
                .into_iter()
                .map(Into::into)
                .collect(),
            by_category_max_value: value.by_category_max_value,
            recent_daily: value.recent_daily.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<ContentTrendPoint> for FfiContentTrendPointV1 {
    fn from(value: ContentTrendPoint) -> Self {
        Self {
            date: value.date,
            date_label: value.date_label,
            item_count: value.item_count,
            item_count_display: value.item_count_display,
            duration_seconds: value.duration_seconds,
            duration_display: value.duration_display,
        }
    }
}

impl From<OverviewStats> for FfiOverviewStatsV1 {
    fn from(value: OverviewStats) -> Self {
        Self {
            item_count: value.item_count,
            item_count_display: value.item_count_display,
            tag_count: value.tag_count,
            tag_count_display: value.tag_count_display,
            total_duration_seconds: value.total_duration_seconds,
            total_duration_display: value.total_duration_display,
            transcript_character_count: value.transcript_character_count,
            transcript_character_count_display: value.transcript_character_count_display,
            recording_count: value.recording_count,
            recording_count_display: value.recording_count_display,
            batch_count: value.batch_count,
            batch_count_display: value.batch_count_display,
            untagged_count: value.untagged_count,
            untagged_count_display: value.untagged_count_display,
            tagged_count: value.tagged_count,
            tagged_count_display: value.tagged_count_display,
            recent_daily_items: value
                .recent_daily_items
                .into_iter()
                .map(Into::into)
                .collect(),
            is_deep_loaded: value.is_deep_loaded,
        }
    }
}

impl From<SpeakerLeader> for FfiSpeakerLeaderV1 {
    fn from(value: SpeakerLeader) -> Self {
        Self {
            speaker_id: value.speaker_id,
            label: value.label,
            duration_seconds: value.duration_seconds,
            duration_display: value.duration_display,
            segment_count: value.segment_count,
            segment_count_display: value.segment_count_display,
            item_count: value.item_count,
            item_count_display: value.item_count_display,
        }
    }
}

impl From<SpeakerStats> for FfiSpeakerStatsV1 {
    fn from(value: SpeakerStats) -> Self {
        Self {
            annotated_item_count: value.annotated_item_count,
            annotated_item_count_display: value.annotated_item_count_display,
            speaker_attributed_duration: value.speaker_attributed_duration,
            speaker_attributed_duration_display: value.speaker_attributed_duration_display,
            identified_speaker_count: value.identified_speaker_count,
            identified_speaker_count_display: value.identified_speaker_count_display,
            anonymous_speaker_slot_count: value.anonymous_speaker_slot_count,
            anonymous_speaker_slot_count_display: value.anonymous_speaker_slot_count_display,
            speaker_tagged_segment_count: value.speaker_tagged_segment_count,
            speaker_tagged_segment_count_display: value.speaker_tagged_segment_count_display,
            total_segment_count: value.total_segment_count,
            total_segment_count_display: value.total_segment_count_display,
            total_segment_duration: value.total_segment_duration,
            total_segment_duration_display: value.total_segment_duration_display,
            identified_duration: value.identified_duration,
            identified_duration_display: value.identified_duration_display,
            anonymous_duration: value.anonymous_duration,
            anonymous_duration_display: value.anonymous_duration_display,
            segment_coverage_ratio: value.segment_coverage_ratio,
            segment_coverage_label: value.segment_coverage_label,
            duration_coverage_ratio: value.duration_coverage_ratio,
            duration_coverage_label: value.duration_coverage_label,
            top_identified_speakers: value
                .top_identified_speakers
                .into_iter()
                .map(Into::into)
                .collect(),
            top_identified_speaker_rows: value
                .top_identified_speaker_rows
                .into_iter()
                .map(Into::into)
                .collect(),
            top_identified_speaker_max_value: value.top_identified_speaker_max_value,
            is_deep_loaded: value.is_deep_loaded,
        }
    }
}

impl From<ContentStats> for FfiContentStatsV1 {
    fn from(value: ContentStats) -> Self {
        Self {
            overview: value.overview.into(),
            speakers: value.speakers.map(Into::into),
        }
    }
}

impl From<DashboardSnapshotDomainModel> for FfiDashboardSnapshotV1 {
    fn from(value: DashboardSnapshotDomainModel) -> Self {
        Self {
            content: value.content.into(),
            llm_usage: value.llm_usage.into(),
            generated_at: value.generated_at,
        }
    }
}
