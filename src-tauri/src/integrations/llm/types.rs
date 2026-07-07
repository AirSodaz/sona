use serde::Serialize;

pub use sona_core::domain::LlmProvider;
pub use sona_core::llm_provider_protocol::{
    LlmModelSummary, MessageRole, StandardLlmRequest, StandardLlmResponse, StandardMessage,
};
pub use sona_core::llm_requests::{
    HistorySummaryPayload, LlmConfig, LlmGenerateRequest, LlmModelsRequest, LlmUsageEventPayload,
    PolishSegmentsRequest, SummarizeTranscriptRequest, TranscriptLlmJobRequest,
    TranscriptSummaryRecordPayload, TranslateSegmentsRequest,
};
pub use sona_core::llm_tasks::{
    LlmProviderStrategy, LlmSegmentInput, LlmTaskChunkPayload, LlmTaskProgressPayload,
    LlmTaskTextPayload, LlmTaskType, PolishedSegment, SummarySegmentInput, SummaryTemplateConfig,
    TranscriptSummaryResult, TranslatedSegment,
};
pub use sona_core::llm_usage::{LlmGenerateSource, LlmUsageCategory, TokenUsage};

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptLlmJobResult {
    pub task_id: String,
    pub task_type: LlmTaskType,
    pub job_history_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<crate::integrations::asr::TranscriptSegment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<HistorySummaryPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_item: Option<crate::platform::history_repository::HistoryItemRecord>,
}
