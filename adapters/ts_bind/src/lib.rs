//! TypeScript-facing metadata for Sona core bindings.
//!
//! Desktop currently generates concrete Tauri command bindings from
//! `src-tauri` with tauri-specta. This crate keeps core-owned TS binding
//! metadata in the workspace so future non-Tauri consumers can depend on the
//! same pure Rust types without reaching into the desktop crate.

pub use sona_core::domain::{LlmProvider, PolishPresetId, SummaryTemplateId};
pub use sona_core::model_config::ModelFileConfig;
pub use sona_core::ports::asr::{
    AsrEngine, AsrEngineConfig, AsrMode, AsrTranscriptionRequest, BatchSegmentationMode,
    OnlineAsrProviderRequest, TranscriptNormalizationOptions, TranscriptPostprocessOptions,
    TranscriptTextReplacementRule, TranscriptTextReplacementRuleSet, VolcengineDoubaoAsrConfig,
};
pub use sona_core::runtime::{RuntimeEnvironmentStatus, RuntimePathKind, RuntimePathStatus};
pub use sona_core::speaker::{SpeakerProcessingConfig, SpeakerProfile, SpeakerProfileSample};

pub const DESKTOP_BINDINGS_OUTPUT: &str = "src/bindings.ts";

const EXPORTED_CORE_TYPE_NAMES: &[&str] = &[
    "LlmProvider",
    "PolishPresetId",
    "SummaryTemplateId",
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
        assert_specta_type::<VolcengineDoubaoAsrConfig>();
    }
}
