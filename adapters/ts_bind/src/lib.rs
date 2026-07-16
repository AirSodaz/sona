//! TypeScript-facing metadata for Sona core bindings.
//!
//! This adapter owns the transport-neutral core type registry, TypeScript rendering,
//! numeric transport validation, and desktop output-path metadata. The Tauri host
//! only writes the generated output and invokes validation at its IPC boundary.

pub use sona_core::dashboard::models::{
    ContentStats, ContentTrendPoint, DashboardSnapshotDomainModel, DashboardUsageBucket,
    LlmUsageDashboardStats, OverviewStats, SpeakerLeader, SpeakerStats, UsageBreakdown,
    UsageTrendPoint,
};
pub use sona_core::domain::{LlmProvider, PolishPresetId, SummaryTemplateId};
pub use sona_core::llm::provider_protocol::{
    LlmModelSummary, MessageRole, StandardLlmRequest, StandardLlmResponse, StandardMessage,
};
pub use sona_core::llm::requests::{
    HistorySummaryPayload, LlmConfig, LlmGenerateRequest, LlmModelsRequest, LlmUsageEventPayload,
    PolishSegmentsRequest, SummarizeTranscriptRequest, TranscriptLlmJobRequest,
    TranscriptSummaryRecordPayload, TranslateSegmentsRequest,
};
pub use sona_core::llm::tasks::{
    LlmProviderStrategy, LlmSegmentInput, LlmTaskChunkPayload, LlmTaskProgressPayload,
    LlmTaskTextPayload, LlmTaskType, PolishedSegment, SummarySegmentInput, SummaryTemplateConfig,
    TranscriptSummaryResult, TranslatedSegment,
};
pub use sona_core::llm::usage::{LlmGenerateSource, LlmUsageCategory, TokenUsage};
pub use sona_core::models::config::ModelFileConfig;
pub use sona_core::ports::asr::{
    AsrEngine, AsrEngineConfig, AsrMode, AsrTranscriptionRequest, BatchSegmentationMode,
    OnlineAsrBatchCapability, OnlineAsrCapability, OnlineAsrLocalFileBatchMode, OnlineAsrProvider,
    OnlineAsrProviderRequest, TranscriptNormalizationOptions, TranscriptPostprocessOptions,
    TranscriptTextReplacementRule, TranscriptTextReplacementRuleSet, VolcengineDoubaoAsrConfig,
};
pub use sona_core::runtime::environment::{
    RuntimeEnvironmentStatus, RuntimePathKind, RuntimePathStatus,
};
pub use sona_core::task_ledger::types::{
    TaskLedgerKind, TaskLedgerPatch, TaskLedgerRecord, TaskLedgerSnapshot, TaskLedgerStatus,
};
pub use sona_core::transcription::speaker::{
    SpeakerProcessingConfig, SpeakerProfile, SpeakerProfileSample,
};
pub use sona_core::transcription::transcript::{
    SpeakerAttribution, SpeakerCandidate, SpeakerTag, TranscriptSegment, TranscriptTiming,
    TranscriptTimingLevel, TranscriptTimingSource, TranscriptTimingUnit, TranscriptUpdate,
};

pub const DESKTOP_BINDINGS_OUTPUT: &str = "src/bindings.ts";
pub const TYPESCRIPT_MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
pub use specta_typescript::Error as TypescriptExportError;

pub fn desktop_bindings_output(frontend_root: impl AsRef<std::path::Path>) -> std::path::PathBuf {
    frontend_root.as_ref().join(DESKTOP_BINDINGS_OUTPUT)
}

pub fn render_desktop_typescript_bindings() -> Result<String, TypescriptExportError> {
    specta_typescript::Typescript::default().export(&desktop_types(), specta_serde::PhasesFormat)
}

pub fn validate_typescript_safe_integers<T>(value: &T) -> Result<(), String>
where
    T: serde::Serialize + ?Sized,
{
    let value = serde_json::to_value(value)
        .map_err(|error| format!("Failed to inspect TypeScript transport value: {error}"))?;
    validate_json_safe_integers(&value, "$")
}

pub fn validate_dashboard_snapshot_for_typescript(
    snapshot: &DashboardSnapshotDomainModel,
) -> Result<(), String> {
    validate_typescript_safe_integers(snapshot)?;

    let overview = &snapshot.content.overview;
    validate_finite_typescript_number(
        "$.content.overview.totalDurationSeconds",
        overview.total_duration_seconds,
    )?;
    for (index, point) in overview.recent_daily_items.iter().enumerate() {
        validate_finite_typescript_number(
            &format!("$.content.overview.recentDailyItems[{index}].durationSeconds"),
            point.duration_seconds,
        )?;
    }

    if let Some(speakers) = snapshot.content.speakers.as_ref() {
        for (path, value) in [
            (
                "$.content.speakers.speakerAttributedDuration",
                speakers.speaker_attributed_duration,
            ),
            (
                "$.content.speakers.totalSegmentDuration",
                speakers.total_segment_duration,
            ),
            (
                "$.content.speakers.identifiedDuration",
                speakers.identified_duration,
            ),
            (
                "$.content.speakers.anonymousDuration",
                speakers.anonymous_duration,
            ),
            (
                "$.content.speakers.segmentCoverageRatio",
                speakers.segment_coverage_ratio,
            ),
            (
                "$.content.speakers.durationCoverageRatio",
                speakers.duration_coverage_ratio,
            ),
            (
                "$.content.speakers.topIdentifiedSpeakerMaxValue",
                speakers.top_identified_speaker_max_value,
            ),
        ] {
            validate_finite_typescript_number(path, value)?;
        }
        validate_speaker_leaders(
            "$.content.speakers.topIdentifiedSpeakers",
            &speakers.top_identified_speakers,
        )?;
        validate_speaker_leaders(
            "$.content.speakers.topIdentifiedSpeakerRows",
            &speakers.top_identified_speaker_rows,
        )?;
    }

    Ok(())
}

pub fn validate_task_ledger_record_for_typescript(record: &TaskLedgerRecord) -> Result<(), String> {
    validate_typescript_safe_integers(record)?;
    validate_task_ledger_record_numbers("$", record)
}

pub fn validate_task_ledger_patch_for_typescript(patch: &TaskLedgerPatch) -> Result<(), String> {
    validate_typescript_safe_integers(patch)?;
    if let Some(progress) = patch.progress {
        validate_finite_typescript_number("$.progress", progress)?;
    }
    Ok(())
}

pub fn validate_task_ledger_snapshot_for_typescript(
    snapshot: &TaskLedgerSnapshot,
) -> Result<(), String> {
    validate_typescript_safe_integers(snapshot)?;
    for (index, record) in snapshot.tasks.iter().enumerate() {
        validate_task_ledger_record_numbers(&format!("$.tasks[{index}]"), record)?;
    }
    Ok(())
}

fn validate_task_ledger_record_numbers(
    path: &str,
    record: &TaskLedgerRecord,
) -> Result<(), String> {
    validate_finite_typescript_number(&format!("{path}.progress"), record.progress)
}

fn validate_speaker_leaders(path: &str, speakers: &[SpeakerLeader]) -> Result<(), String> {
    for (index, speaker) in speakers.iter().enumerate() {
        validate_finite_typescript_number(
            &format!("{path}[{index}].durationSeconds"),
            speaker.duration_seconds,
        )?;
    }
    Ok(())
}

fn validate_finite_typescript_number(path: &str, value: f64) -> Result<(), String> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(format!(
            "Number at {path} is not finite and cannot cross the TypeScript transport: {value}"
        ))
    }
}

fn validate_json_safe_integers(value: &serde_json::Value, path: &str) -> Result<(), String> {
    match value {
        serde_json::Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                validate_json_safe_integers(value, &format!("{path}[{index}]"))?;
            }
        }
        serde_json::Value::Object(values) => {
            for (key, value) in values {
                validate_json_safe_integers(value, &format!("{path}.{key}"))?;
            }
        }
        serde_json::Value::Number(number) => {
            let is_unsafe = number
                .as_u64()
                .is_some_and(|value| value > TYPESCRIPT_MAX_SAFE_INTEGER)
                || number
                    .as_i64()
                    .is_some_and(|value| value.unsigned_abs() > TYPESCRIPT_MAX_SAFE_INTEGER)
                || number.as_f64().is_some_and(|value| {
                    value.fract() == 0.0 && value.abs() > TYPESCRIPT_MAX_SAFE_INTEGER as f64
                });
            if is_unsafe {
                return Err(format!(
                    "Integer at {path} exceeds TypeScript's safe range: {number}"
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

/// Core-owned types currently emitted into the desktop TypeScript bindings.
///
/// The Tauri host owns the concrete exporter and command registration, while
/// this adapter owns the transport-neutral core type boundary.
pub fn desktop_types() -> specta::Types {
    specta::Types::default()
        .register::<LlmProvider>()
        .register::<PolishPresetId>()
        .register::<SummaryTemplateId>()
        .register::<LlmTaskType>()
        .register::<LlmSegmentInput>()
        .register::<SummarySegmentInput>()
        .register::<PolishedSegment>()
        .register::<TranslatedSegment>()
        .register::<TranscriptSummaryResult>()
        .register::<LlmTaskProgressPayload>()
        .register::<LlmTaskChunkPayload<PolishedSegment>>()
        .register::<LlmTaskTextPayload>()
        .register::<DashboardUsageBucket>()
        .register::<UsageBreakdown>()
        .register::<UsageTrendPoint>()
        .register::<LlmUsageDashboardStats>()
        .register::<ContentTrendPoint>()
        .register::<OverviewStats>()
        .register::<SpeakerLeader>()
        .register::<SpeakerStats>()
        .register::<ContentStats>()
        .register::<DashboardSnapshotDomainModel>()
        .register::<TaskLedgerKind>()
        .register::<TaskLedgerStatus>()
        .register::<TaskLedgerRecord>()
        .register::<TaskLedgerPatch>()
        .register::<TaskLedgerSnapshot>()
}

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
    "DashboardUsageBucket",
    "UsageBreakdown",
    "UsageTrendPoint",
    "LlmUsageDashboardStats",
    "ContentTrendPoint",
    "OverviewStats",
    "SpeakerLeader",
    "SpeakerStats",
    "ContentStats",
    "DashboardSnapshotDomainModel",
    "TaskLedgerKind",
    "TaskLedgerStatus",
    "TaskLedgerRecord",
    "TaskLedgerPatch",
    "TaskLedgerSnapshot",
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
    fn resolves_desktop_binding_output_from_the_frontend_root() {
        assert_eq!(
            desktop_bindings_output("frontend"),
            std::path::PathBuf::from("frontend").join("src/bindings.ts")
        );
    }

    #[test]
    fn desktop_type_registry_contains_the_frontend_contracts() {
        let types = desktop_types();
        let names = types
            .into_sorted_iter()
            .map(|datatype| datatype.name.as_ref())
            .collect::<Vec<_>>();

        for expected in [
            "LlmProvider",
            "PolishPresetId",
            "SummaryTemplateId",
            "LlmTaskType",
            "LlmSegmentInput",
            "SummarySegmentInput",
            "PolishedSegment",
            "TranslatedSegment",
            "TranscriptSummaryResult",
            "LlmTaskProgressPayload",
            "LlmTaskChunkPayload",
            "LlmTaskTextPayload",
            "DashboardUsageBucket",
            "UsageBreakdown",
            "UsageTrendPoint",
            "LlmUsageDashboardStats",
            "ContentTrendPoint",
            "OverviewStats",
            "SpeakerLeader",
            "SpeakerStats",
            "ContentStats",
            "DashboardSnapshotDomainModel",
            "TaskLedgerKind",
            "TaskLedgerStatus",
            "TaskLedgerRecord",
            "TaskLedgerPatch",
            "TaskLedgerSnapshot",
        ] {
            assert!(names.contains(&expected), "missing {expected}");
        }
    }

    #[test]
    fn desktop_type_registry_is_typescript_exportable() {
        render_desktop_typescript_bindings().unwrap();
    }

    #[test]
    fn committed_desktop_bindings_match_the_type_registry() {
        let frontend_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("platforms/desktop/frontend");
        let bindings_path = desktop_bindings_output(frontend_root);
        let committed = std::fs::read_to_string(&bindings_path).unwrap_or_else(|error| {
            panic!(
                "failed to read committed desktop bindings at {}: {error}",
                bindings_path.display()
            )
        });
        let generated = render_desktop_typescript_bindings().unwrap();

        assert_eq!(
            committed, generated,
            "desktop bindings are stale; run `pnpm run generate:desktop-bindings`"
        );
    }

    #[test]
    fn typescript_integer_validation_rejects_nested_unsafe_values() {
        let safe = serde_json::json!({"value": TYPESCRIPT_MAX_SAFE_INTEGER});
        validate_typescript_safe_integers(&safe).unwrap();

        let unsafe_value = serde_json::json!({
            "nested": [TYPESCRIPT_MAX_SAFE_INTEGER + 1]
        });
        let error = validate_typescript_safe_integers(&unsafe_value).unwrap_err();

        assert!(error.contains("$.nested[0]"), "{error}");
        assert!(error.contains("exceeds TypeScript's safe range"), "{error}");
    }

    #[test]
    fn typescript_number_validation_rejects_non_finite_values() {
        let error = validate_finite_typescript_number("$.duration", f64::NAN).unwrap_err();

        assert!(error.contains("$.duration"), "{error}");
        assert!(error.contains("is not finite"), "{error}");
    }

    #[test]
    fn task_ledger_validation_rejects_unsafe_timestamps_and_non_finite_progress() {
        let snapshot = TaskLedgerSnapshot {
            version: 1,
            updated_at: Some(TYPESCRIPT_MAX_SAFE_INTEGER + 1),
            tasks: Vec::new(),
        };
        let timestamp_error = validate_task_ledger_snapshot_for_typescript(&snapshot).unwrap_err();
        assert!(timestamp_error.contains("$.updatedAt"), "{timestamp_error}");

        let patch = TaskLedgerPatch {
            progress: Some(f64::INFINITY),
            ..Default::default()
        };
        let progress_error = validate_task_ledger_patch_for_typescript(&patch).unwrap_err();
        assert!(progress_error.contains("$.progress"), "{progress_error}");
        assert!(progress_error.contains("is not finite"), "{progress_error}");
    }

    #[test]
    fn runtime_types_are_specta_exportable_through_ts_bindings() {
        fn assert_specta_type<T: specta::Type>() {}

        assert_specta_type::<sona_core::runtime::environment::RuntimeEnvironmentStatus>();
        assert_specta_type::<sona_core::runtime::environment::RuntimePathKind>();
        assert_specta_type::<sona_core::runtime::environment::RuntimePathStatus>();
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
        assert_specta_type::<DashboardUsageBucket>();
        assert_specta_type::<UsageBreakdown>();
        assert_specta_type::<UsageTrendPoint>();
        assert_specta_type::<LlmUsageDashboardStats>();
        assert_specta_type::<ContentTrendPoint>();
        assert_specta_type::<OverviewStats>();
        assert_specta_type::<SpeakerLeader>();
        assert_specta_type::<SpeakerStats>();
        assert_specta_type::<ContentStats>();
        assert_specta_type::<DashboardSnapshotDomainModel>();
        assert_specta_type::<TaskLedgerKind>();
        assert_specta_type::<TaskLedgerStatus>();
        assert_specta_type::<TaskLedgerRecord>();
        assert_specta_type::<TaskLedgerPatch>();
        assert_specta_type::<TaskLedgerSnapshot>();
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
