//! TypeScript-facing metadata for Sona core bindings.
//!
//! This adapter owns the transport-neutral core type registry, TypeScript rendering,
//! numeric transport validation, and desktop output-path metadata. The Tauri host
//! only writes the generated output and invokes validation at its IPC boundary.

pub use sona_core::automation::repository::{
    AutomationProcessedInput, AutomationProcessedRecord, AutomationRepositoryInput,
    AutomationRepositoryState, AutomationRuleInput, AutomationRuleInputExportConfig,
    AutomationRuleInputStageConfig, AutomationRuleRecord, AutomationRuleRecordExportConfig,
    AutomationRuleRecordStageConfig,
};
pub use sona_core::automation::{
    AutomationRule, AutomationRuleExportConfig, AutomationRuleStageConfig,
    AutomationRuleValidationResult, AutomationRuntimeCandidatePayload,
    AutomationRuntimePathCollectionOutcome, AutomationRuntimePathCollectionResult,
    AutomationRuntimeReplaceResult, AutomationRuntimeRuleConfig,
};
pub use sona_core::dashboard::models::{
    ContentStats, ContentTrendPoint, DashboardSnapshotDomainModel, DashboardUsageBucket,
    LlmUsageDashboardStats, OverviewStats, SpeakerLeader, SpeakerStats, UsageBreakdown,
    UsageTrendPoint,
};
pub use sona_core::domain::{LlmProvider, PolishPresetId, SummaryTemplateId};
pub use sona_core::export::{
    ExportFormat, ExportMode, ExportTranscriptFileRequest, ExportTranscriptFileResult,
};
pub use sona_core::history::mutation_repository::{
    HistoryCompleteLiveDraftRequest, HistoryCreateTranscriptSnapshotRequest,
    HistoryDeleteItemsRequest, HistoryItemMetaPatch, HistoryReplaceTagAssignmentsRequest,
    HistoryTrashItemsRequest, HistoryUpdateItemMetaRequest, HistoryUpdateTagAssignmentsRequest,
    HistoryUpdateTranscriptRequest,
};
pub use sona_core::history::{
    HistoryAudioCleanupReport, HistoryAudioCleanupRequest, HistoryAudioStatus,
    HistoryCreateLiveDraftRequest, HistoryDraftSource, HistoryItemKind, HistoryItemRecord,
    HistoryItemStatus, HistorySaveImportedFileRequest, HistorySaveRecordingRequest,
    HistorySummaryPayload, HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType,
    HistoryWorkspaceItemCounts, HistoryWorkspaceItemSearchMatch, HistoryWorkspaceQueryRequest,
    HistoryWorkspaceQueryResult, HistoryWorkspaceScope, HistoryWorkspaceSearchRange,
    HistoryWorkspaceSearchSnippet, HistoryWorkspaceSortOrder, HistoryWorkspaceSummary,
    LiveRecordingDraftResult, TranscriptDiffResult, TranscriptDiffRow, TranscriptDiffStatus,
    TranscriptSnapshotMetadata, TranscriptSnapshotReason, TranscriptSnapshotRecord,
    TranscriptSummaryRecordPayload,
};
pub use sona_core::llm::provider_protocol::{
    LlmModelSummary, MessageRole, StandardLlmRequest, StandardLlmResponse, StandardMessage,
};
pub use sona_core::llm::requests::{
    LlmConfig, LlmGenerateRequest, LlmModelsRequest, LlmUsageEventPayload, PolishSegmentsRequest,
    SummarizeTranscriptRequest, TranscriptLlmJobRequest, TranslateSegmentsRequest,
};
pub use sona_core::llm::tasks::{
    LlmProviderStrategy, LlmSegmentInput, LlmTaskChunkPayload, LlmTaskProgressPayload,
    LlmTaskTextPayload, LlmTaskType, PolishedSegment, SummarySegmentInput, SummaryTemplateConfig,
    TranscriptSummaryResult, TranslatedSegment,
};
pub use sona_core::llm::usage::{LlmGenerateSource, LlmUsageCategory, TokenUsage};
pub use sona_core::models::config::ModelFileConfig;
pub use sona_core::models::preset_models::{
    ModelCatalogGroup, ModelCatalogModel, ModelCatalogPathMatchToken, ModelCatalogRestoreDefaults,
    ModelCatalogSection, ModelCatalogSectionType, ModelCatalogSelectedIds,
    ModelCatalogSelectionOptions, ModelCatalogSnapshot, ModelDependencyConfigKey,
    ModelDependencyRequest, ModelRules, ModelSelectionOption, ModelSelectionPaths,
    TimestampSupportHint,
};
pub use sona_core::ports::asr::{
    AsrEngine, AsrEngineConfig, AsrMode, AsrTranscriptionRequest, BatchSegmentationMode,
    OnlineAsrBatchCapability, OnlineAsrCapability, OnlineAsrLocalFileBatchMode, OnlineAsrProvider,
    OnlineAsrProviderRequest, TranscriptNormalizationOptions, TranscriptPostprocessOptions,
    TranscriptTextReplacementRule, TranscriptTextReplacementRuleSet, VolcengineDoubaoAsrConfig,
};
pub use sona_core::project::{
    ProjectCreateInput, ProjectDefaults, ProjectDefaultsInput, ProjectDefaultsPatch, ProjectRecord,
    ProjectRepositorySnapshot, ProjectUpdateInput,
};
pub use sona_core::recovery::types::{
    RecoveredQueueItem, RecoveredTranscriptSegment, RecoveredTranscriptTiming,
    RecoveredTranscriptTimingUnit, RecoveryFileStat, RecoveryItemInput, RecoveryItemStage,
    RecoveryResolution, RecoverySnapshot, RecoverySnapshotInput, RecoverySource,
};
pub use sona_core::runtime::diagnostics::{
    DeviceOptionInput, DeviceProbeInput, DiagnosticsConfigInput, DiagnosticsCoreInput,
    DiagnosticsCoreSnapshot, ModelRuleInput, ModelRulesInput, ModelSummaryInput, PathStatusesInput,
    SelectedModelsInput, VoiceTypingReadinessInput,
};
pub use sona_core::runtime::environment::{
    RuntimeEnvironmentStatus, RuntimePathKind, RuntimePathStatus,
};
pub use sona_core::storage_usage::{
    AudioUsageCategory, DatabaseUsageCategory, FileUsageCategory, SQLiteIndexUsageEntry,
    SQLiteUsageSummary, StorageUsageCategories, StorageUsageSnapshot,
    WebviewBrowsingDataClearResult, WebviewCacheUsageCategory,
};
pub use sona_core::tag::{
    TagCreateInput, TagDefaults, TagDefaultsInput, TagDefaultsPatch, TagRecord,
    TagRepositorySnapshot, TagUpdateInput,
};
pub use sona_core::task_ledger::types::{
    TaskLedgerKind, TaskLedgerPatch, TaskLedgerRecord, TaskLedgerSnapshot, TaskLedgerStatus,
};
pub use sona_core::transcription::asr_metrics::{
    AsrInferenceMetric, AsrModelLoadMetric, AsrRuntimeMetricsSnapshot,
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

pub fn validate_export_transcript_request_for_typescript(
    request: &ExportTranscriptFileRequest,
) -> Result<(), String> {
    for (index, segment) in request.segments.iter().enumerate() {
        validate_transcript_segment_numbers(&format!("$.segments[{index}]"), segment)?;
    }
    validate_typescript_safe_integers(request)
}

pub fn validate_export_transcript_result_for_typescript(
    result: &ExportTranscriptFileResult,
) -> Result<(), String> {
    validate_typescript_safe_integers(result)
}

pub fn validate_diagnostics_input_for_typescript(
    input: &DiagnosticsCoreInput,
) -> Result<(), String> {
    validate_typescript_safe_integers(input)?;
    validate_asr_runtime_metric_numbers("$.asrRuntimeMetrics", &input.asr_runtime_metrics)
}

pub fn validate_diagnostics_snapshot_for_typescript(
    snapshot: &DiagnosticsCoreSnapshot,
) -> Result<(), String> {
    validate_typescript_safe_integers(snapshot)?;
    validate_asr_runtime_metric_numbers("$.asrRuntimeMetrics", &snapshot.asr_runtime_metrics)
}

pub fn validate_asr_runtime_metrics_for_typescript(
    metrics: &AsrRuntimeMetricsSnapshot,
) -> Result<(), String> {
    validate_typescript_safe_integers(metrics)?;
    validate_asr_runtime_metric_numbers("$", metrics)
}

pub fn validate_storage_usage_snapshot_for_typescript(
    snapshot: &StorageUsageSnapshot,
) -> Result<(), String> {
    validate_typescript_safe_integers(snapshot)
}

pub fn validate_webview_browsing_data_clear_result_for_typescript(
    result: &WebviewBrowsingDataClearResult,
) -> Result<(), String> {
    validate_typescript_safe_integers(result)
}

pub fn validate_task_ledger_record_for_typescript(record: &TaskLedgerRecord) -> Result<(), String> {
    validate_typescript_safe_integers(record)?;
    validate_task_ledger_record_numbers("$", record)
}

pub fn validate_project_record_for_typescript(record: &ProjectRecord) -> Result<(), String> {
    validate_typescript_safe_integers(record)
}

pub fn validate_project_records_for_typescript(records: &[ProjectRecord]) -> Result<(), String> {
    validate_typescript_safe_integers(records)
}

pub fn validate_tag_record_for_typescript(record: &TagRecord) -> Result<(), String> {
    validate_typescript_safe_integers(record)
}

pub fn validate_tag_records_for_typescript(records: &[TagRecord]) -> Result<(), String> {
    validate_typescript_safe_integers(records)
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

fn validate_asr_runtime_metric_numbers(
    path: &str,
    metrics: &AsrRuntimeMetricsSnapshot,
) -> Result<(), String> {
    if let Some(metric) = metrics.model_load.as_ref() {
        let metric_path = format!("{path}.modelLoad");
        validate_finite_typescript_number(&format!("{metric_path}.loadMs"), metric.load_ms)?;
        for (field, value) in [
            ("rssBeforeMb", metric.rss_before_mb),
            ("rssAfterMb", metric.rss_after_mb),
            ("rssDeltaMb", metric.rss_delta_mb),
            ("processRssMb", metric.process_rss_mb),
        ] {
            if let Some(value) = value {
                validate_finite_typescript_number(&format!("{metric_path}.{field}"), value)?;
            }
        }
    }

    for (field, metric) in [
        ("liveInference", metrics.live_inference.as_ref()),
        ("batchInference", metrics.batch_inference.as_ref()),
    ] {
        let Some(metric) = metric else {
            continue;
        };
        let metric_path = format!("{path}.{field}");
        validate_finite_typescript_number(
            &format!("{metric_path}.audioDurationMs"),
            metric.audio_duration_ms,
        )?;
        validate_finite_typescript_number(&format!("{metric_path}.decodeMs"), metric.decode_ms)?;
        for (field, value) in [
            ("audioExtractMs", metric.audio_extract_ms),
            ("emitLatencyMs", metric.emit_latency_ms),
            ("totalMs", metric.total_ms),
            ("rtf", metric.rtf),
            ("processRssMb", metric.process_rss_mb),
        ] {
            if let Some(value) = value {
                validate_finite_typescript_number(&format!("{metric_path}.{field}"), value)?;
            }
        }
    }

    Ok(())
}

fn validate_transcript_segment_numbers(
    path: &str,
    segment: &TranscriptSegment,
) -> Result<(), String> {
    validate_finite_typescript_number(&format!("{path}.start"), segment.start)?;
    validate_finite_typescript_number(&format!("{path}.end"), segment.end)?;

    if let Some(timing) = segment.timing.as_ref() {
        for (index, unit) in timing.units.iter().enumerate() {
            let unit_path = format!("{path}.timing.units[{index}]");
            validate_finite_typescript_number(&format!("{unit_path}.start"), unit.start)?;
            validate_finite_typescript_number(&format!("{unit_path}.end"), unit.end)?;
        }
    }
    for (field, values) in [
        ("timestamps", segment.timestamps.as_deref()),
        ("durations", segment.durations.as_deref()),
    ] {
        if let Some(values) = values {
            for (index, value) in values.iter().enumerate() {
                validate_finite_typescript_number(
                    &format!("{path}.{field}[{index}]"),
                    f64::from(*value),
                )?;
            }
        }
    }
    if let Some(score) = segment.speaker.as_ref().and_then(|speaker| speaker.score) {
        validate_finite_typescript_number(&format!("{path}.speaker.score"), f64::from(score))?;
    }
    if let Some(attribution) = segment.speaker_attribution.as_ref() {
        for (index, candidate) in attribution.candidates.iter().enumerate() {
            validate_finite_typescript_number(
                &format!("{path}.speakerAttribution.candidates[{index}].score"),
                f64::from(candidate.score),
            )?;
        }
    }

    Ok(())
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
        .register::<ExportFormat>()
        .register::<ExportMode>()
        .register::<ExportTranscriptFileRequest>()
        .register::<ExportTranscriptFileResult>()
        .register::<TimestampSupportHint>()
        .register::<ModelRules>()
        .register::<ModelCatalogSnapshot>()
        .register::<ModelCatalogModel>()
        .register::<ModelCatalogSection>()
        .register::<ModelCatalogGroup>()
        .register::<ModelCatalogSectionType>()
        .register::<ModelCatalogSelectionOptions>()
        .register::<ModelSelectionOption>()
        .register::<ModelCatalogPathMatchToken>()
        .register::<ModelDependencyConfigKey>()
        .register::<ModelDependencyRequest>()
        .register::<ModelCatalogRestoreDefaults>()
        .register::<ModelSelectionPaths>()
        .register::<ModelCatalogSelectedIds>()
        .register::<DiagnosticsCoreInput>()
        .register::<DiagnosticsConfigInput>()
        .register::<SelectedModelsInput>()
        .register::<ModelSummaryInput>()
        .register::<ModelRulesInput>()
        .register::<ModelRuleInput>()
        .register::<PathStatusesInput>()
        .register::<DeviceProbeInput>()
        .register::<DeviceOptionInput>()
        .register::<VoiceTypingReadinessInput>()
        .register::<DiagnosticsCoreSnapshot>()
        .register::<AsrRuntimeMetricsSnapshot>()
        .register::<AsrModelLoadMetric>()
        .register::<AsrInferenceMetric>()
        .register::<StorageUsageSnapshot>()
        .register::<StorageUsageCategories>()
        .register::<AudioUsageCategory>()
        .register::<DatabaseUsageCategory>()
        .register::<FileUsageCategory>()
        .register::<WebviewCacheUsageCategory>()
        .register::<SQLiteUsageSummary>()
        .register::<SQLiteIndexUsageEntry>()
        .register::<WebviewBrowsingDataClearResult>()
        .register::<TaskLedgerKind>()
        .register::<TaskLedgerStatus>()
        .register::<TaskLedgerRecord>()
        .register::<TaskLedgerPatch>()
        .register::<TaskLedgerSnapshot>()
        .register::<ProjectDefaultsInput>()
        .register::<ProjectCreateInput>()
        .register::<ProjectDefaults>()
        .register::<ProjectDefaultsPatch>()
        .register::<ProjectUpdateInput>()
        .register::<ProjectRecord>()
        .register::<ProjectRepositorySnapshot>()
        .register::<TagDefaultsInput>()
        .register::<TagCreateInput>()
        .register::<TagDefaults>()
        .register::<TagDefaultsPatch>()
        .register::<TagUpdateInput>()
        .register::<TagRecord>()
        .register::<TagRepositorySnapshot>()
        .register::<AutomationRuleInputStageConfig>()
        .register::<AutomationRuleInputExportConfig>()
        .register::<AutomationRuleInput>()
        .register::<AutomationProcessedInput>()
        .register::<AutomationRepositoryInput>()
        .register::<AutomationRuleRecordStageConfig>()
        .register::<AutomationRuleRecordExportConfig>()
        .register::<AutomationRuleRecord>()
        .register::<AutomationProcessedRecord>()
        .register::<AutomationRepositoryState>()
        .register::<AutomationRuleStageConfig>()
        .register::<AutomationRuleExportConfig>()
        .register::<AutomationRule>()
        .register::<AutomationRuleValidationResult>()
        .register::<AutomationRuntimeRuleConfig>()
        .register::<AutomationRuntimeReplaceResult>()
        .register::<AutomationRuntimeCandidatePayload>()
        .register::<AutomationRuntimePathCollectionOutcome>()
        .register::<AutomationRuntimePathCollectionResult>()
        .register::<HistoryAudioStatus>()
        .register::<HistoryItemKind>()
        .register::<HistoryItemStatus>()
        .register::<HistoryDraftSource>()
        .register::<HistoryItemRecord>()
        .register::<HistoryWorkspaceScope>()
        .register::<HistoryWorkspaceFilterType>()
        .register::<HistoryWorkspaceDateFilter>()
        .register::<HistoryWorkspaceSortOrder>()
        .register::<HistoryWorkspaceQueryRequest>()
        .register::<HistoryWorkspaceSearchRange>()
        .register::<HistoryWorkspaceSearchSnippet>()
        .register::<HistoryWorkspaceItemSearchMatch>()
        .register::<HistoryWorkspaceSummary>()
        .register::<HistoryWorkspaceItemCounts>()
        .register::<HistoryWorkspaceQueryResult>()
        .register::<LiveRecordingDraftResult>()
        .register::<HistoryCreateLiveDraftRequest>()
        .register::<HistoryCompleteLiveDraftRequest>()
        .register::<HistorySaveRecordingRequest>()
        .register::<HistorySaveImportedFileRequest>()
        .register::<HistoryDeleteItemsRequest>()
        .register::<HistoryUpdateTranscriptRequest>()
        .register::<HistoryCreateTranscriptSnapshotRequest>()
        .register::<HistoryItemMetaPatch>()
        .register::<HistoryUpdateItemMetaRequest>()
        .register::<HistoryTrashItemsRequest>()
        .register::<HistoryUpdateTagAssignmentsRequest>()
        .register::<HistoryReplaceTagAssignmentsRequest>()
        .register::<HistoryAudioCleanupRequest>()
        .register::<HistoryAudioCleanupReport>()
        .register::<RecoverySource>()
        .register::<RecoveryResolution>()
        .register::<RecoveryItemStage>()
        .register::<RecoverySnapshotInput>()
        .register::<RecoveryItemInput>()
        .register::<RecoverySnapshot>()
        .register::<RecoveredQueueItem>()
        .register::<RecoveryFileStat>()
        .register::<RecoveredTranscriptSegment>()
        .register::<RecoveredTranscriptTiming>()
        .register::<RecoveredTranscriptTimingUnit>()
        .register::<TranscriptSnapshotReason>()
        .register::<TranscriptSnapshotMetadata>()
        .register::<TranscriptSnapshotRecord>()
        .register::<TranscriptDiffStatus>()
        .register::<TranscriptDiffRow>()
        .register::<TranscriptDiffResult>()
        .register::<TranscriptSummaryRecordPayload>()
        .register::<HistorySummaryPayload>()
        .register::<AsrEngine>()
        .register::<AsrMode>()
        .register::<BatchSegmentationMode>()
        .register::<ModelFileConfig>()
        .register::<SpeakerProcessingConfig>()
        .register::<SpeakerProfile>()
        .register::<SpeakerProfileSample>()
        .register::<TranscriptNormalizationOptions>()
        .register::<TranscriptPostprocessOptions>()
        .register::<TranscriptTextReplacementRule>()
        .register::<TranscriptTextReplacementRuleSet>()
        .register::<AsrTranscriptionRequest>()
        .register::<AsrEngineConfig>()
        .register::<OnlineAsrProviderRequest>()
        .register::<OnlineAsrProvider>()
        .register::<OnlineAsrCapability>()
        .register::<OnlineAsrBatchCapability>()
        .register::<OnlineAsrLocalFileBatchMode>()
        .register::<VolcengineDoubaoAsrConfig>()
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
    "ExportFormat",
    "ExportMode",
    "ExportTranscriptFileRequest",
    "ExportTranscriptFileResult",
    "TimestampSupportHint",
    "ModelRules",
    "ModelCatalogSnapshot",
    "ModelCatalogModel",
    "ModelCatalogSection",
    "ModelCatalogGroup",
    "ModelCatalogSectionType",
    "ModelCatalogSelectionOptions",
    "ModelSelectionOption",
    "ModelCatalogPathMatchToken",
    "ModelDependencyConfigKey",
    "ModelDependencyRequest",
    "ModelCatalogRestoreDefaults",
    "ModelSelectionPaths",
    "ModelCatalogSelectedIds",
    "DiagnosticsCoreInput",
    "DiagnosticsConfigInput",
    "SelectedModelsInput",
    "ModelSummaryInput",
    "ModelRulesInput",
    "ModelRuleInput",
    "PathStatusesInput",
    "DeviceProbeInput",
    "DeviceOptionInput",
    "VoiceTypingReadinessInput",
    "DiagnosticsCoreSnapshot",
    "AsrRuntimeMetricsSnapshot",
    "AsrModelLoadMetric",
    "AsrInferenceMetric",
    "StorageUsageSnapshot",
    "StorageUsageCategories",
    "AudioUsageCategory",
    "DatabaseUsageCategory",
    "FileUsageCategory",
    "WebviewCacheUsageCategory",
    "SQLiteUsageSummary",
    "SQLiteIndexUsageEntry",
    "WebviewBrowsingDataClearResult",
    "TaskLedgerKind",
    "TaskLedgerStatus",
    "TaskLedgerRecord",
    "TaskLedgerPatch",
    "TaskLedgerSnapshot",
    "ProjectDefaultsInput",
    "ProjectCreateInput",
    "ProjectDefaults",
    "ProjectDefaultsPatch",
    "ProjectUpdateInput",
    "ProjectRecord",
    "ProjectRepositorySnapshot",
    "TagDefaultsInput",
    "TagCreateInput",
    "TagDefaults",
    "TagDefaultsPatch",
    "TagUpdateInput",
    "TagRecord",
    "TagRepositorySnapshot",
    "AutomationRuleInputStageConfig",
    "AutomationRuleInputExportConfig",
    "AutomationRuleInput",
    "AutomationProcessedInput",
    "AutomationRepositoryInput",
    "AutomationRuleRecordStageConfig",
    "AutomationRuleRecordExportConfig",
    "AutomationRuleRecord",
    "AutomationProcessedRecord",
    "AutomationRepositoryState",
    "AutomationRuleStageConfig",
    "AutomationRuleExportConfig",
    "AutomationRule",
    "AutomationRuleValidationResult",
    "AutomationRuntimeRuleConfig",
    "AutomationRuntimeReplaceResult",
    "AutomationRuntimeCandidatePayload",
    "AutomationRuntimePathCollectionOutcome",
    "AutomationRuntimePathCollectionResult",
    "HistoryAudioStatus",
    "HistoryItemKind",
    "HistoryItemStatus",
    "HistoryDraftSource",
    "HistoryItemRecord",
    "HistoryWorkspaceScope",
    "HistoryWorkspaceFilterType",
    "HistoryWorkspaceDateFilter",
    "HistoryWorkspaceSortOrder",
    "HistoryWorkspaceQueryRequest",
    "HistoryWorkspaceSearchRange",
    "HistoryWorkspaceSearchSnippet",
    "HistoryWorkspaceItemSearchMatch",
    "HistoryWorkspaceSummary",
    "HistoryWorkspaceItemCounts",
    "HistoryWorkspaceQueryResult",
    "LiveRecordingDraftResult",
    "HistoryCreateLiveDraftRequest",
    "HistoryCompleteLiveDraftRequest",
    "HistorySaveRecordingRequest",
    "HistorySaveImportedFileRequest",
    "HistoryDeleteItemsRequest",
    "HistoryUpdateTranscriptRequest",
    "HistoryCreateTranscriptSnapshotRequest",
    "HistoryItemMetaPatch",
    "HistoryUpdateItemMetaRequest",
    "HistoryTrashItemsRequest",
    "HistoryUpdateTagAssignmentsRequest",
    "HistoryReplaceTagAssignmentsRequest",
    "HistoryAudioCleanupRequest",
    "HistoryAudioCleanupReport",
    "RecoverySource",
    "RecoveryResolution",
    "RecoveryItemStage",
    "RecoverySnapshotInput",
    "RecoveryItemInput",
    "RecoverySnapshot",
    "RecoveredQueueItem",
    "RecoveryFileStat",
    "RecoveredTranscriptSegment",
    "RecoveredTranscriptTiming",
    "RecoveredTranscriptTimingUnit",
    "TranscriptSnapshotReason",
    "TranscriptSnapshotMetadata",
    "TranscriptSnapshotRecord",
    "TranscriptDiffStatus",
    "TranscriptDiffRow",
    "TranscriptDiffResult",
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
            "ExportFormat",
            "ExportMode",
            "ExportTranscriptFileRequest",
            "ExportTranscriptFileResult",
            "TimestampSupportHint",
            "ModelRules",
            "ModelCatalogSnapshot",
            "ModelCatalogModel",
            "ModelCatalogSection",
            "ModelCatalogGroup",
            "ModelCatalogSectionType",
            "ModelCatalogSelectionOptions",
            "ModelSelectionOption",
            "ModelCatalogPathMatchToken",
            "ModelDependencyConfigKey",
            "ModelDependencyRequest",
            "ModelCatalogRestoreDefaults",
            "ModelSelectionPaths",
            "ModelCatalogSelectedIds",
            "DiagnosticsCoreInput",
            "DiagnosticsConfigInput",
            "SelectedModelsInput",
            "ModelSummaryInput",
            "ModelRulesInput",
            "ModelRuleInput",
            "PathStatusesInput",
            "DeviceProbeInput",
            "DeviceOptionInput",
            "VoiceTypingReadinessInput",
            "DiagnosticsCoreSnapshot",
            "AsrRuntimeMetricsSnapshot",
            "AsrModelLoadMetric",
            "AsrInferenceMetric",
            "StorageUsageSnapshot",
            "StorageUsageCategories",
            "AudioUsageCategory",
            "DatabaseUsageCategory",
            "FileUsageCategory",
            "WebviewCacheUsageCategory",
            "SQLiteUsageSummary",
            "SQLiteIndexUsageEntry",
            "WebviewBrowsingDataClearResult",
            "TaskLedgerKind",
            "TaskLedgerStatus",
            "TaskLedgerRecord",
            "TaskLedgerPatch",
            "TaskLedgerSnapshot",
            "ProjectDefaultsInput",
            "ProjectCreateInput",
            "ProjectDefaults",
            "ProjectDefaultsPatch",
            "ProjectUpdateInput",
            "ProjectRecord",
            "ProjectRepositorySnapshot",
            "AutomationRuleInput",
            "AutomationProcessedInput",
            "AutomationRepositoryState",
            "AutomationRuleValidationResult",
            "AutomationRuntimeRuleConfig",
            "AutomationRuntimeReplaceResult",
            "AutomationRuntimePathCollectionResult",
            "HistoryItemRecord",
            "HistoryWorkspaceQueryRequest",
            "HistoryWorkspaceQueryResult",
            "HistoryCompleteLiveDraftRequest",
            "HistorySaveRecordingRequest",
            "HistorySaveImportedFileRequest",
            "HistoryUpdateTranscriptRequest",
            "HistoryCreateTranscriptSnapshotRequest",
            "HistoryItemMetaPatch",
            "HistorySummaryPayload",
            "TranscriptSnapshotRecord",
            "TranscriptDiffResult",
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
        ] {
            assert!(names.contains(&expected), "missing {expected}");
        }
    }

    #[test]
    fn desktop_type_registry_is_typescript_exportable() {
        render_desktop_typescript_bindings().unwrap();
    }

    #[test]
    fn asr_types_keep_dynamic_json_unknown_and_required_floats_finite() {
        let bindings = render_desktop_typescript_bindings().unwrap();
        let speaker_profile_sample = bindings
            .split_once("export type SpeakerProfileSample = {")
            .expect("SpeakerProfileSample binding should be generated")
            .1
            .split_once("};")
            .expect("SpeakerProfileSample binding should have a closing delimiter")
            .0;

        assert!(bindings.contains("config?: unknown"));
        assert!(bindings.contains("defaults: unknown"));
        assert!(bindings.contains("vadBuffer: number"));
        assert!(speaker_profile_sample.contains("durationSeconds: number"));
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
    fn project_validation_rejects_unsafe_timestamps() {
        let project = ProjectRecord {
            id: "project-1".to_string(),
            name: "Project".to_string(),
            description: String::new(),
            icon: String::new(),
            created_at: TYPESCRIPT_MAX_SAFE_INTEGER + 1,
            updated_at: 1,
            defaults: ProjectDefaults {
                summary_template_id: "general".to_string(),
                translation_language: "zh".to_string(),
                polish_preset_id: "general".to_string(),
                polish_scenario: None,
                polish_context: None,
                export_file_name_prefix: String::new(),
                enabled_text_replacement_set_ids: Vec::new(),
                enabled_hotword_set_ids: Vec::new(),
                enabled_polish_keyword_set_ids: Vec::new(),
                enabled_speaker_profile_ids: Vec::new(),
            },
        };

        let error = validate_project_record_for_typescript(&project).unwrap_err();
        assert!(error.contains("$.createdAt"), "{error}");
    }

    #[test]
    fn history_transport_validation_rejects_unsafe_timestamps() {
        let item = HistoryItemRecord {
            id: "history-1".to_string(),
            timestamp: TYPESCRIPT_MAX_SAFE_INTEGER + 1,
            duration: 1.0,
            audio_path: "history-1.wav".to_string(),
            audio_status: HistoryAudioStatus::Available,
            transcript_path: "history-1.json".to_string(),
            title: "History".to_string(),
            preview_text: "hello".to_string(),
            icon: None,
            kind: HistoryItemKind::Recording,
            search_content: "hello".to_string(),
            tag_ids: Vec::new(),
            deleted_at: None,
            status: HistoryItemStatus::Complete,
            draft_source: None,
        };

        let error = validate_typescript_safe_integers(&item).unwrap_err();
        assert!(error.contains("$.timestamp"), "{error}");
    }

    #[test]
    fn automation_transport_validation_rejects_unsafe_file_metadata() {
        let candidate = AutomationRuntimeCandidatePayload {
            rule_id: "rule-1".to_string(),
            file_path: "C:\\watch\\audio.wav".to_string(),
            source_fingerprint: "fingerprint".to_string(),
            size: TYPESCRIPT_MAX_SAFE_INTEGER + 1,
            mtime_ms: 1,
        };

        let error = validate_typescript_safe_integers(&candidate).unwrap_err();
        assert!(error.contains("$.size"), "{error}");
    }

    #[test]
    fn recovery_transport_validation_rejects_unsafe_timestamps_and_file_stats() {
        let input = RecoverySnapshotInput {
            updated_at: Some(TYPESCRIPT_MAX_SAFE_INTEGER + 1),
            items: Vec::new(),
        };
        let error = validate_typescript_safe_integers(&input).unwrap_err();
        assert!(error.contains("$.updatedAt"), "{error}");

        let file_stat = RecoveryFileStat {
            size: TYPESCRIPT_MAX_SAFE_INTEGER + 1,
            mtime_ms: 1,
        };
        let error = validate_typescript_safe_integers(&file_stat).unwrap_err();
        assert!(error.contains("$.size"), "{error}");
    }

    #[test]
    fn export_transport_validation_rejects_unsafe_sizes_and_non_finite_timing() {
        let result = sona_core::export::ExportTranscriptFileResult {
            output_path: "C:/exports/transcript.vtt".to_string(),
            bytes_written: TYPESCRIPT_MAX_SAFE_INTEGER + 1,
        };
        let error = validate_export_transcript_result_for_typescript(&result).unwrap_err();
        assert!(error.contains("$.bytesWritten"), "{error}");

        let mut request: sona_core::export::ExportTranscriptFileRequest =
            serde_json::from_value(serde_json::json!({
                "segments": [{
                    "id": "segment-1",
                    "text": "Hello",
                    "start": 0.0,
                    "end": 1.25,
                    "isFinal": true
                }],
                "format": "vtt",
                "mode": "original",
                "outputPath": "C:/exports/transcript.vtt"
            }))
            .unwrap();
        request.segments[0].start = f64::INFINITY;

        let error = validate_export_transcript_request_for_typescript(&request).unwrap_err();
        assert!(error.contains("$.segments[0].start"), "{error}");
        assert!(error.contains("is not finite"), "{error}");
    }

    #[test]
    fn storage_usage_transport_validation_rejects_unsafe_byte_counts() {
        let mut snapshot = sona_core::storage_usage::StorageUsageSnapshot::default();
        snapshot.categories.database.sqlite.index_entries =
            vec![sona_core::storage_usage::SQLiteIndexUsageEntry {
                schema: "main".to_string(),
                name: "idx_history".to_string(),
                bytes: TYPESCRIPT_MAX_SAFE_INTEGER + 1,
            }];

        let error = validate_storage_usage_snapshot_for_typescript(&snapshot).unwrap_err();
        assert!(
            error.contains("$.categories.database.sqlite.indexEntries[0].bytes"),
            "{error}"
        );

        let clear_result = sona_core::storage_usage::WebviewBrowsingDataClearResult {
            before_bytes: Some(TYPESCRIPT_MAX_SAFE_INTEGER + 1),
            after_bytes: None,
            clear_requested: false,
        };
        let error =
            validate_webview_browsing_data_clear_result_for_typescript(&clear_result).unwrap_err();
        assert!(error.contains("$.beforeBytes"), "{error}");
    }

    #[test]
    fn diagnostics_transport_validation_rejects_unsafe_and_non_finite_metrics() {
        let mut input: sona_core::runtime::diagnostics::DiagnosticsCoreInput =
            serde_json::from_value(serde_json::json!({
                "config": {
                    "streamingModelPath": "",
                    "batchModelPath": ""
                },
                "permissionState": "prompt",
                "microphoneProbe": {
                    "options": [],
                    "available": false,
                    "errorMessage": null
                },
                "systemAudioProbe": {
                    "options": [],
                    "available": false,
                    "errorMessage": null
                },
                "voiceTypingReadiness": {
                    "state": "off",
                    "lastErrorMessage": null
                }
            }))
            .unwrap();
        input.asr_runtime_metrics.model_load =
            Some(sona_core::transcription::asr_metrics::AsrModelLoadMetric {
                occurred_at_ms: TYPESCRIPT_MAX_SAFE_INTEGER + 1,
                instance_id: "instance-1".to_string(),
                model_path: "model.onnx".to_string(),
                model_type: "streaming".to_string(),
                recognizer_kind: "online".to_string(),
                num_threads: 4,
                reused_from_pool: false,
                load_ms: 1.0,
                rss_before_mb: None,
                rss_after_mb: None,
                rss_delta_mb: None,
                process_rss_mb: None,
            });

        let error = validate_diagnostics_input_for_typescript(&input).unwrap_err();
        assert!(
            error.contains("$.asrRuntimeMetrics.modelLoad.occurredAtMs"),
            "{error}"
        );
        let metrics_error =
            validate_asr_runtime_metrics_for_typescript(&input.asr_runtime_metrics).unwrap_err();
        assert!(
            metrics_error.contains("$.modelLoad.occurredAtMs"),
            "{metrics_error}"
        );

        input
            .asr_runtime_metrics
            .model_load
            .as_mut()
            .unwrap()
            .occurred_at_ms = 1;
        input
            .asr_runtime_metrics
            .model_load
            .as_mut()
            .unwrap()
            .load_ms = f64::NAN;
        let error = validate_diagnostics_input_for_typescript(&input).unwrap_err();
        assert!(
            error.contains("$.asrRuntimeMetrics.modelLoad.loadMs"),
            "{error}"
        );
        assert!(error.contains("is not finite"), "{error}");
    }

    #[test]
    fn model_catalog_types_are_specta_exportable_through_ts_bindings() {
        fn assert_specta_type<T: specta::Type>() {}

        assert_specta_type::<sona_core::models::preset_models::TimestampSupportHint>();
        assert_specta_type::<sona_core::models::preset_models::ModelRules>();
        assert_specta_type::<sona_core::models::preset_models::ModelCatalogSnapshot>();
        assert_specta_type::<sona_core::models::preset_models::ModelCatalogModel>();
        assert_specta_type::<sona_core::models::preset_models::ModelCatalogSection>();
        assert_specta_type::<sona_core::models::preset_models::ModelCatalogGroup>();
        assert_specta_type::<sona_core::models::preset_models::ModelCatalogSectionType>();
        assert_specta_type::<sona_core::models::preset_models::ModelCatalogSelectionOptions>();
        assert_specta_type::<sona_core::models::preset_models::ModelSelectionOption>();
        assert_specta_type::<sona_core::models::preset_models::ModelCatalogPathMatchToken>();
        assert_specta_type::<sona_core::models::preset_models::ModelDependencyConfigKey>();
        assert_specta_type::<sona_core::models::preset_models::ModelDependencyRequest>();
        assert_specta_type::<sona_core::models::preset_models::ModelCatalogRestoreDefaults>();
        assert_specta_type::<sona_core::models::preset_models::ModelSelectionPaths>();
        assert_specta_type::<sona_core::models::preset_models::ModelCatalogSelectedIds>();
    }

    #[test]
    fn runtime_types_are_specta_exportable_through_ts_bindings() {
        fn assert_specta_type<T: specta::Type>() {}

        assert_specta_type::<sona_core::runtime::environment::RuntimeEnvironmentStatus>();
        assert_specta_type::<sona_core::runtime::environment::RuntimePathKind>();
        assert_specta_type::<sona_core::runtime::environment::RuntimePathStatus>();
        assert_specta_type::<sona_core::runtime::diagnostics::DiagnosticsCoreInput>();
        assert_specta_type::<sona_core::runtime::diagnostics::DiagnosticsConfigInput>();
        assert_specta_type::<sona_core::runtime::diagnostics::SelectedModelsInput>();
        assert_specta_type::<sona_core::runtime::diagnostics::ModelSummaryInput>();
        assert_specta_type::<sona_core::runtime::diagnostics::ModelRulesInput>();
        assert_specta_type::<sona_core::runtime::diagnostics::ModelRuleInput>();
        assert_specta_type::<sona_core::runtime::diagnostics::PathStatusesInput>();
        assert_specta_type::<sona_core::runtime::diagnostics::DeviceProbeInput>();
        assert_specta_type::<sona_core::runtime::diagnostics::DeviceOptionInput>();
        assert_specta_type::<sona_core::runtime::diagnostics::VoiceTypingReadinessInput>();
        assert_specta_type::<sona_core::runtime::diagnostics::DiagnosticsCoreSnapshot>();
        assert_specta_type::<sona_core::transcription::asr_metrics::AsrRuntimeMetricsSnapshot>();
        assert_specta_type::<sona_core::transcription::asr_metrics::AsrModelLoadMetric>();
        assert_specta_type::<sona_core::transcription::asr_metrics::AsrInferenceMetric>();
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
        assert_specta_type::<sona_core::export::ExportFormat>();
        assert_specta_type::<sona_core::export::ExportMode>();
        assert_specta_type::<sona_core::export::ExportTranscriptFileRequest>();
        assert_specta_type::<sona_core::export::ExportTranscriptFileResult>();
        assert_specta_type::<sona_core::storage_usage::StorageUsageSnapshot>();
        assert_specta_type::<sona_core::storage_usage::StorageUsageCategories>();
        assert_specta_type::<sona_core::storage_usage::AudioUsageCategory>();
        assert_specta_type::<sona_core::storage_usage::DatabaseUsageCategory>();
        assert_specta_type::<sona_core::storage_usage::FileUsageCategory>();
        assert_specta_type::<sona_core::storage_usage::WebviewCacheUsageCategory>();
        assert_specta_type::<sona_core::storage_usage::SQLiteUsageSummary>();
        assert_specta_type::<sona_core::storage_usage::SQLiteIndexUsageEntry>();
        assert_specta_type::<sona_core::storage_usage::WebviewBrowsingDataClearResult>();
        assert_specta_type::<TaskLedgerKind>();
        assert_specta_type::<TaskLedgerStatus>();
        assert_specta_type::<TaskLedgerRecord>();
        assert_specta_type::<TaskLedgerPatch>();
        assert_specta_type::<TaskLedgerSnapshot>();
        assert_specta_type::<ProjectDefaultsInput>();
        assert_specta_type::<ProjectCreateInput>();
        assert_specta_type::<ProjectDefaults>();
        assert_specta_type::<ProjectDefaultsPatch>();
        assert_specta_type::<ProjectUpdateInput>();
        assert_specta_type::<ProjectRecord>();
        assert_specta_type::<ProjectRepositorySnapshot>();
        assert_specta_type::<TagDefaultsInput>();
        assert_specta_type::<TagCreateInput>();
        assert_specta_type::<TagDefaults>();
        assert_specta_type::<TagDefaultsPatch>();
        assert_specta_type::<TagUpdateInput>();
        assert_specta_type::<TagRecord>();
        assert_specta_type::<TagRepositorySnapshot>();
        assert_specta_type::<AutomationRuleInputStageConfig>();
        assert_specta_type::<AutomationRuleInputExportConfig>();
        assert_specta_type::<AutomationRuleInput>();
        assert_specta_type::<AutomationProcessedInput>();
        assert_specta_type::<AutomationRepositoryInput>();
        assert_specta_type::<AutomationRuleRecordStageConfig>();
        assert_specta_type::<AutomationRuleRecordExportConfig>();
        assert_specta_type::<AutomationRuleRecord>();
        assert_specta_type::<AutomationProcessedRecord>();
        assert_specta_type::<AutomationRepositoryState>();
        assert_specta_type::<AutomationRuleStageConfig>();
        assert_specta_type::<AutomationRuleExportConfig>();
        assert_specta_type::<AutomationRule>();
        assert_specta_type::<AutomationRuleValidationResult>();
        assert_specta_type::<AutomationRuntimeRuleConfig>();
        assert_specta_type::<AutomationRuntimeReplaceResult>();
        assert_specta_type::<AutomationRuntimeCandidatePayload>();
        assert_specta_type::<AutomationRuntimePathCollectionOutcome>();
        assert_specta_type::<AutomationRuntimePathCollectionResult>();
        assert_specta_type::<HistoryAudioStatus>();
        assert_specta_type::<HistoryItemKind>();
        assert_specta_type::<HistoryItemStatus>();
        assert_specta_type::<HistoryDraftSource>();
        assert_specta_type::<HistoryItemRecord>();
        assert_specta_type::<HistoryWorkspaceScope>();
        assert_specta_type::<HistoryWorkspaceFilterType>();
        assert_specta_type::<HistoryWorkspaceDateFilter>();
        assert_specta_type::<HistoryWorkspaceSortOrder>();
        assert_specta_type::<HistoryWorkspaceQueryRequest>();
        assert_specta_type::<HistoryWorkspaceSearchRange>();
        assert_specta_type::<HistoryWorkspaceSearchSnippet>();
        assert_specta_type::<HistoryWorkspaceItemSearchMatch>();
        assert_specta_type::<HistoryWorkspaceSummary>();
        assert_specta_type::<HistoryWorkspaceItemCounts>();
        assert_specta_type::<HistoryWorkspaceQueryResult>();
        assert_specta_type::<LiveRecordingDraftResult>();
        assert_specta_type::<HistoryCreateLiveDraftRequest>();
        assert_specta_type::<HistoryCompleteLiveDraftRequest>();
        assert_specta_type::<HistorySaveRecordingRequest>();
        assert_specta_type::<HistorySaveImportedFileRequest>();
        assert_specta_type::<HistoryDeleteItemsRequest>();
        assert_specta_type::<HistoryUpdateTranscriptRequest>();
        assert_specta_type::<HistoryCreateTranscriptSnapshotRequest>();
        assert_specta_type::<HistoryItemMetaPatch>();
        assert_specta_type::<HistoryUpdateItemMetaRequest>();
        assert_specta_type::<HistoryTrashItemsRequest>();
        assert_specta_type::<HistoryUpdateTagAssignmentsRequest>();
        assert_specta_type::<HistoryReplaceTagAssignmentsRequest>();
        assert_specta_type::<HistoryAudioCleanupRequest>();
        assert_specta_type::<HistoryAudioCleanupReport>();
        assert_specta_type::<RecoverySource>();
        assert_specta_type::<RecoveryResolution>();
        assert_specta_type::<RecoveryItemStage>();
        assert_specta_type::<RecoverySnapshotInput>();
        assert_specta_type::<RecoveryItemInput>();
        assert_specta_type::<RecoverySnapshot>();
        assert_specta_type::<RecoveredQueueItem>();
        assert_specta_type::<RecoveryFileStat>();
        assert_specta_type::<RecoveredTranscriptSegment>();
        assert_specta_type::<RecoveredTranscriptTiming>();
        assert_specta_type::<RecoveredTranscriptTimingUnit>();
        assert_specta_type::<TranscriptSnapshotReason>();
        assert_specta_type::<TranscriptSnapshotMetadata>();
        assert_specta_type::<TranscriptSnapshotRecord>();
        assert_specta_type::<TranscriptDiffStatus>();
        assert_specta_type::<TranscriptDiffRow>();
        assert_specta_type::<TranscriptDiffResult>();
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
