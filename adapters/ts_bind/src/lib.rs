//! TypeScript-facing metadata for Sona core bindings.
//!
//! Desktop currently generates concrete Tauri command bindings from
//! `src-tauri` with tauri-specta. This crate keeps core-owned TS binding
//! metadata in the workspace so future non-Tauri consumers can depend on the
//! same pure Rust types without reaching into the desktop crate.

pub use sona_core::domain::{LlmProvider, PolishPresetId, SummaryTemplateId};
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
pub use sona_core::model_config::ModelFileConfig;
pub use sona_core::ports::asr::{
    AsrEngine, AsrEngineConfig, AsrMode, AsrTranscriptionRequest, BatchSegmentationMode,
    OnlineAsrBatchCapability, OnlineAsrCapability, OnlineAsrLocalFileBatchMode, OnlineAsrProvider,
    OnlineAsrProviderRequest, TranscriptNormalizationOptions, TranscriptPostprocessOptions,
    TranscriptTextReplacementRule, TranscriptTextReplacementRuleSet, VolcengineDoubaoAsrConfig,
};
pub use sona_core::runtime::{RuntimeEnvironmentStatus, RuntimePathKind, RuntimePathStatus};
pub use sona_core::speaker::{SpeakerProcessingConfig, SpeakerProfile, SpeakerProfileSample};
pub use sona_core::transcript::{
    SpeakerAttribution, SpeakerCandidate, SpeakerTag, TranscriptSegment, TranscriptTiming,
    TranscriptTimingLevel, TranscriptTimingSource, TranscriptTimingUnit, TranscriptUpdate,
};

pub const DESKTOP_BINDINGS_OUTPUT: &str = "src/bindings.ts";

const EXPORTED_CORE_TYPE_NAMES: &[&str] = &[
    "LlmProvider",
    "PolishPresetId",
    "SummaryTemplateId",
    "MessageRole",
    "StandardMessage",
    "StandardLlmRequest",
    "StandardLlmResponse",
    "LlmModelSummary",
    "LlmConfig",
    "LlmGenerateRequest",
    "LlmUsageEventPayload",
    "LlmModelsRequest",
    "PolishSegmentsRequest",
    "TranslateSegmentsRequest",
    "SummarizeTranscriptRequest",
    "TranscriptLlmJobRequest",
    "TranscriptSummaryRecordPayload",
    "HistorySummaryPayload",
    "LlmProviderStrategy",
    "LlmTaskType",
    "SummaryTemplateConfig",
    "LlmSegmentInput",
    "SummarySegmentInput",
    "PolishedSegment",
    "TranslatedSegment",
    "TranscriptSummaryResult",
    "LlmTaskProgressPayload",
    "LlmTaskChunkPayload",
    "LlmTaskTextPayload",
    "LlmGenerateSource",
    "LlmUsageCategory",
    "TokenUsage",
    "TranscriptTimingLevel",
    "TranscriptTimingSource",
    "TranscriptTimingUnit",
    "TranscriptTiming",
    "SpeakerTag",
    "SpeakerCandidate",
    "SpeakerAttribution",
    "TranscriptSegment",
    "TranscriptUpdate",
    "RuntimeEnvironmentStatus",
    "RuntimePathKind",
    "RuntimePathStatus",
    "AsrEngine",
    "AsrMode",
    "BatchSegmentationMode",
    "ModelFileConfig",
    "SpeakerProcessingConfig",
    "SpeakerProfile",
    "SpeakerProfileSample",
    "TranscriptNormalizationOptions",
    "TranscriptPostprocessOptions",
    "TranscriptTextReplacementRule",
    "TranscriptTextReplacementRuleSet",
    "AsrTranscriptionRequest",
    "AsrEngineConfig",
    "OnlineAsrProviderRequest",
    "OnlineAsrProvider",
    "OnlineAsrCapability",
    "OnlineAsrBatchCapability",
    "OnlineAsrLocalFileBatchMode",
    "VolcengineDoubaoAsrConfig",
];

pub fn exported_core_type_names() -> &'static [&'static str] {
    EXPORTED_CORE_TYPE_NAMES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_core_types_owned_by_ts_bindings() {
        assert_eq!(exported_core_type_names(), EXPORTED_CORE_TYPE_NAMES);
    }

    #[test]
    fn keeps_desktop_binding_output_explicit() {
        assert_eq!(DESKTOP_BINDINGS_OUTPUT, "src/bindings.ts");
    }

    #[test]
    fn runtime_types_are_specta_exportable_through_ts_bindings() {
        fn assert_specta_type<T: specta::Type>() {}

        assert_specta_type::<sona_core::runtime::RuntimeEnvironmentStatus>();
        assert_specta_type::<sona_core::runtime::RuntimePathKind>();
        assert_specta_type::<sona_core::runtime::RuntimePathStatus>();
        assert_specta_type::<sona_core::ports::asr::AsrEngine>();
        assert_specta_type::<sona_core::ports::asr::AsrMode>();
        assert_specta_type::<sona_core::ports::asr::BatchSegmentationMode>();
        assert_specta_type::<MessageRole>();
        assert_specta_type::<StandardMessage>();
        assert_specta_type::<StandardLlmRequest>();
        assert_specta_type::<StandardLlmResponse>();
        assert_specta_type::<LlmModelSummary>();
        assert_specta_type::<LlmConfig>();
        assert_specta_type::<LlmGenerateRequest>();
        assert_specta_type::<LlmUsageEventPayload>();
        assert_specta_type::<LlmModelsRequest>();
        assert_specta_type::<PolishSegmentsRequest>();
        assert_specta_type::<TranslateSegmentsRequest>();
        assert_specta_type::<SummarizeTranscriptRequest>();
        assert_specta_type::<TranscriptLlmJobRequest>();
        assert_specta_type::<TranscriptSummaryRecordPayload>();
        assert_specta_type::<HistorySummaryPayload>();
        assert_specta_type::<LlmProviderStrategy>();
        assert_specta_type::<LlmTaskType>();
        assert_specta_type::<SummaryTemplateConfig>();
        assert_specta_type::<LlmSegmentInput>();
        assert_specta_type::<SummarySegmentInput>();
        assert_specta_type::<PolishedSegment>();
        assert_specta_type::<TranslatedSegment>();
        assert_specta_type::<TranscriptSummaryResult>();
        assert_specta_type::<LlmTaskProgressPayload>();
        assert_specta_type::<LlmTaskChunkPayload<PolishedSegment>>();
        assert_specta_type::<LlmTaskTextPayload>();
        assert_specta_type::<LlmGenerateSource>();
        assert_specta_type::<LlmUsageCategory>();
        assert_specta_type::<TokenUsage>();
        assert_specta_type::<TranscriptTimingLevel>();
        assert_specta_type::<TranscriptTimingSource>();
        assert_specta_type::<TranscriptTimingUnit>();
        assert_specta_type::<TranscriptTiming>();
        assert_specta_type::<SpeakerTag>();
        assert_specta_type::<SpeakerCandidate>();
        assert_specta_type::<SpeakerAttribution>();
        assert_specta_type::<TranscriptSegment>();
        assert_specta_type::<TranscriptUpdate>();
        assert_specta_type::<ModelFileConfig>();
        assert_specta_type::<SpeakerProcessingConfig>();
        assert_specta_type::<SpeakerProfile>();
        assert_specta_type::<SpeakerProfileSample>();
        assert_specta_type::<TranscriptNormalizationOptions>();
        assert_specta_type::<TranscriptPostprocessOptions>();
        assert_specta_type::<TranscriptTextReplacementRule>();
        assert_specta_type::<TranscriptTextReplacementRuleSet>();
        assert_specta_type::<AsrTranscriptionRequest>();
        assert_specta_type::<AsrEngineConfig>();
        assert_specta_type::<OnlineAsrProviderRequest>();
        assert_specta_type::<OnlineAsrProvider>();
        assert_specta_type::<OnlineAsrCapability>();
        assert_specta_type::<OnlineAsrBatchCapability>();
        assert_specta_type::<OnlineAsrLocalFileBatchMode>();
        assert_specta_type::<VolcengineDoubaoAsrConfig>();
    }
}
