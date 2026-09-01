use crate::{
    FfiAsrInferenceMetric, FfiAsrModelLoadMetric, FfiRuntimePathKind, FfiRuntimePathStatus,
};
use sona_core::runtime::diagnostics::{
    DeviceOptionInput, DeviceProbeInput, DiagnosticsConfigInput, DiagnosticsCoreInput,
    DiagnosticsCoreSnapshot, ModelRuleInput, ModelRulesInput, ModelSummaryInput, PathStatusesInput,
    SelectedModelsInput, VoiceTypingReadinessInput,
};
use sona_core::runtime::environment::{
    RuntimeEnvironmentStatus, RuntimePathKind, RuntimePathStatus,
};
use sona_core::transcription::asr_metrics::{
    AsrInferenceMetric, AsrModelLoadMetric, AsrRuntimeMetricsSnapshot,
};

// Diagnostics is the one snapshot domain with a typed *input*: the host reports
// what it observed (device probes, permission state, resolved model paths) and
// Core enriches it. Both directions are therefore mapped.

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiDiagnosticsConfigV1 {
    pub streaming_model_path: String,
    pub batch_model_path: String,
    pub vad_model_path: String,
    pub punctuation_model_path: String,
    pub microphone_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiDiagnosticsModelSummaryV1 {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiDiagnosticsSelectedModelsV1 {
    pub live: Option<FfiDiagnosticsModelSummaryV1>,
    pub batch: Option<FfiDiagnosticsModelSummaryV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiDiagnosticsModelRuleV1 {
    pub requires_vad: bool,
    pub requires_punctuation: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiDiagnosticsModelRulesV1 {
    pub live: Option<FfiDiagnosticsModelRuleV1>,
    pub batch: Option<FfiDiagnosticsModelRuleV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiDiagnosticsPathStatusesV1 {
    pub live_model: Option<FfiRuntimePathStatus>,
    pub batch_model: Option<FfiRuntimePathStatus>,
    pub vad: Option<FfiRuntimePathStatus>,
    pub punctuation: Option<FfiRuntimePathStatus>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiDiagnosticsDeviceOptionV1 {
    pub label: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiDiagnosticsDeviceProbeV1 {
    pub options: Vec<FfiDiagnosticsDeviceOptionV1>,
    pub available: bool,
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiVoiceTypingReadinessV1 {
    pub state: String,
    pub last_error_message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiRuntimeEnvironmentStatusV1 {
    pub ffmpeg_path: String,
    pub ffmpeg_exists: bool,
    pub log_dir_path: String,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiAsrRuntimeMetricsSnapshotV1 {
    pub model_load: Option<FfiAsrModelLoadMetric>,
    pub live_inference: Option<FfiAsrInferenceMetric>,
    pub batch_inference: Option<FfiAsrInferenceMetric>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiDiagnosticsInputV1 {
    pub config: FfiDiagnosticsConfigV1,
    pub selected_models: FfiDiagnosticsSelectedModelsV1,
    pub model_rules: FfiDiagnosticsModelRulesV1,
    pub path_statuses: FfiDiagnosticsPathStatusesV1,
    pub permission_state: String,
    pub microphone_probe: FfiDiagnosticsDeviceProbeV1,
    pub system_audio_probe: FfiDiagnosticsDeviceProbeV1,
    pub voice_typing_readiness: FfiVoiceTypingReadinessV1,
    pub runtime_environment: FfiRuntimeEnvironmentStatusV1,
    pub asr_runtime_metrics: FfiAsrRuntimeMetricsSnapshotV1,
    pub onboarding_ready: bool,
    pub punctuation_required: bool,
}

/// The enriched snapshot: the input echoed back with `scanned_at` and with
/// `path_statuses` and `selected_models` resolved against the models directory.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiDiagnosticsSnapshotV1 {
    pub scanned_at: String,
    pub config: FfiDiagnosticsConfigV1,
    pub selected_models: FfiDiagnosticsSelectedModelsV1,
    pub model_rules: FfiDiagnosticsModelRulesV1,
    pub path_statuses: FfiDiagnosticsPathStatusesV1,
    pub permission_state: String,
    pub microphone_probe: FfiDiagnosticsDeviceProbeV1,
    pub system_audio_probe: FfiDiagnosticsDeviceProbeV1,
    pub voice_typing_readiness: FfiVoiceTypingReadinessV1,
    pub runtime_environment: FfiRuntimeEnvironmentStatusV1,
    pub asr_runtime_metrics: FfiAsrRuntimeMetricsSnapshotV1,
    pub onboarding_ready: bool,
    pub punctuation_required: bool,
}

// ---------------------------------------------------------------- to core ---

impl From<FfiDiagnosticsConfigV1> for DiagnosticsConfigInput {
    fn from(value: FfiDiagnosticsConfigV1) -> Self {
        Self {
            streaming_model_path: value.streaming_model_path,
            batch_model_path: value.batch_model_path,
            vad_model_path: value.vad_model_path,
            punctuation_model_path: value.punctuation_model_path,
            microphone_id: value.microphone_id,
            ffmpeg_path: "".to_string(),
        }
    }
}

impl From<FfiDiagnosticsModelSummaryV1> for ModelSummaryInput {
    fn from(value: FfiDiagnosticsModelSummaryV1) -> Self {
        Self {
            id: value.id,
            name: value.name,
        }
    }
}

impl From<FfiDiagnosticsSelectedModelsV1> for SelectedModelsInput {
    fn from(value: FfiDiagnosticsSelectedModelsV1) -> Self {
        Self {
            live: value.live.map(Into::into),
            batch: value.batch.map(Into::into),
        }
    }
}

impl From<FfiDiagnosticsModelRuleV1> for ModelRuleInput {
    fn from(value: FfiDiagnosticsModelRuleV1) -> Self {
        Self {
            requires_vad: value.requires_vad,
            requires_punctuation: value.requires_punctuation,
        }
    }
}

impl From<FfiDiagnosticsModelRulesV1> for ModelRulesInput {
    fn from(value: FfiDiagnosticsModelRulesV1) -> Self {
        Self {
            live: value.live.map(Into::into),
            batch: value.batch.map(Into::into),
        }
    }
}

impl From<FfiRuntimePathKind> for RuntimePathKind {
    fn from(value: FfiRuntimePathKind) -> Self {
        match value {
            FfiRuntimePathKind::File => Self::File,
            FfiRuntimePathKind::Directory => Self::Directory,
            FfiRuntimePathKind::Missing => Self::Missing,
            FfiRuntimePathKind::Unknown => Self::Unknown,
        }
    }
}

impl From<FfiRuntimePathStatus> for RuntimePathStatus {
    fn from(value: FfiRuntimePathStatus) -> Self {
        Self {
            path: value.path,
            kind: value.kind.into(),
            error: value.error,
        }
    }
}

impl From<FfiDiagnosticsPathStatusesV1> for PathStatusesInput {
    fn from(value: FfiDiagnosticsPathStatusesV1) -> Self {
        Self {
            live_model: value.live_model.map(Into::into),
            batch_model: value.batch_model.map(Into::into),
            vad: value.vad.map(Into::into),
            punctuation: value.punctuation.map(Into::into),
        }
    }
}

impl From<FfiDiagnosticsDeviceOptionV1> for DeviceOptionInput {
    fn from(value: FfiDiagnosticsDeviceOptionV1) -> Self {
        Self {
            label: value.label,
            value: value.value,
        }
    }
}

impl From<FfiDiagnosticsDeviceProbeV1> for DeviceProbeInput {
    fn from(value: FfiDiagnosticsDeviceProbeV1) -> Self {
        Self {
            options: value.options.into_iter().map(Into::into).collect(),
            available: value.available,
            error_message: value.error_message,
        }
    }
}

impl From<FfiVoiceTypingReadinessV1> for VoiceTypingReadinessInput {
    fn from(value: FfiVoiceTypingReadinessV1) -> Self {
        Self {
            state: value.state,
            last_error_message: value.last_error_message,
        }
    }
}

impl From<FfiRuntimeEnvironmentStatusV1> for RuntimeEnvironmentStatus {
    fn from(value: FfiRuntimeEnvironmentStatusV1) -> Self {
        Self {
            ffmpeg_path: value.ffmpeg_path,
            ffmpeg_exists: value.ffmpeg_exists,
            log_dir_path: value.log_dir_path,
        }
    }
}

impl From<FfiAsrModelLoadMetric> for AsrModelLoadMetric {
    fn from(value: FfiAsrModelLoadMetric) -> Self {
        Self {
            occurred_at_ms: value.occurred_at_ms,
            instance_id: value.instance_id,
            model_path: value.model_path,
            model_type: value.model_type,
            recognizer_kind: value.recognizer_kind,
            num_threads: value.num_threads,
            reused_from_pool: value.reused_from_pool,
            load_ms: value.load_ms,
            rss_before_mb: value.rss_before_mb,
            rss_after_mb: value.rss_after_mb,
            rss_delta_mb: value.rss_delta_mb,
            process_rss_mb: value.process_rss_mb,
        }
    }
}

impl From<FfiAsrInferenceMetric> for AsrInferenceMetric {
    fn from(value: FfiAsrInferenceMetric) -> Self {
        Self {
            occurred_at_ms: value.occurred_at_ms,
            source: value.source,
            instance_id: value.instance_id,
            stage: value.stage,
            is_final: value.is_final,
            audio_duration_ms: value.audio_duration_ms,
            buffered_samples: value.buffered_samples as usize,
            audio_extract_ms: value.audio_extract_ms,
            decode_ms: value.decode_ms,
            emit_latency_ms: value.emit_latency_ms,
            total_ms: value.total_ms,
            rtf: value.rtf,
            segment_count: value.segment_count.map(|count| count as usize),
            process_rss_mb: value.process_rss_mb,
        }
    }
}

impl From<FfiAsrRuntimeMetricsSnapshotV1> for AsrRuntimeMetricsSnapshot {
    fn from(value: FfiAsrRuntimeMetricsSnapshotV1) -> Self {
        Self {
            model_load: value.model_load.map(Into::into),
            live_inference: value.live_inference.map(Into::into),
            batch_inference: value.batch_inference.map(Into::into),
        }
    }
}

impl From<FfiDiagnosticsInputV1> for DiagnosticsCoreInput {
    fn from(value: FfiDiagnosticsInputV1) -> Self {
        Self {
            config: value.config.into(),
            selected_models: value.selected_models.into(),
            model_rules: value.model_rules.into(),
            path_statuses: value.path_statuses.into(),
            permission_state: value.permission_state,
            microphone_probe: value.microphone_probe.into(),
            system_audio_probe: value.system_audio_probe.into(),
            voice_typing_readiness: value.voice_typing_readiness.into(),
            runtime_environment: value.runtime_environment.into(),
            asr_runtime_metrics: value.asr_runtime_metrics.into(),
            live_transcription: Default::default(),
            onboarding_ready: value.onboarding_ready,
            punctuation_required: value.punctuation_required,
        }
    }
}

// ----------------------------------------------------------------- to ffi ---

impl From<DiagnosticsConfigInput> for FfiDiagnosticsConfigV1 {
    fn from(value: DiagnosticsConfigInput) -> Self {
        Self {
            streaming_model_path: value.streaming_model_path,
            batch_model_path: value.batch_model_path,
            vad_model_path: value.vad_model_path,
            punctuation_model_path: value.punctuation_model_path,
            microphone_id: value.microphone_id,
        }
    }
}

impl From<ModelSummaryInput> for FfiDiagnosticsModelSummaryV1 {
    fn from(value: ModelSummaryInput) -> Self {
        Self {
            id: value.id,
            name: value.name,
        }
    }
}

impl From<SelectedModelsInput> for FfiDiagnosticsSelectedModelsV1 {
    fn from(value: SelectedModelsInput) -> Self {
        Self {
            live: value.live.map(Into::into),
            batch: value.batch.map(Into::into),
        }
    }
}

impl From<ModelRuleInput> for FfiDiagnosticsModelRuleV1 {
    fn from(value: ModelRuleInput) -> Self {
        Self {
            requires_vad: value.requires_vad,
            requires_punctuation: value.requires_punctuation,
        }
    }
}

impl From<ModelRulesInput> for FfiDiagnosticsModelRulesV1 {
    fn from(value: ModelRulesInput) -> Self {
        Self {
            live: value.live.map(Into::into),
            batch: value.batch.map(Into::into),
        }
    }
}

impl From<PathStatusesInput> for FfiDiagnosticsPathStatusesV1 {
    fn from(value: PathStatusesInput) -> Self {
        Self {
            live_model: value
                .live_model
                .map(super::runtime_mapper::runtime_path_status_to_ffi),
            batch_model: value
                .batch_model
                .map(super::runtime_mapper::runtime_path_status_to_ffi),
            vad: value
                .vad
                .map(super::runtime_mapper::runtime_path_status_to_ffi),
            punctuation: value
                .punctuation
                .map(super::runtime_mapper::runtime_path_status_to_ffi),
        }
    }
}

impl From<DeviceOptionInput> for FfiDiagnosticsDeviceOptionV1 {
    fn from(value: DeviceOptionInput) -> Self {
        Self {
            label: value.label,
            value: value.value,
        }
    }
}

impl From<DeviceProbeInput> for FfiDiagnosticsDeviceProbeV1 {
    fn from(value: DeviceProbeInput) -> Self {
        Self {
            options: value.options.into_iter().map(Into::into).collect(),
            available: value.available,
            error_message: value.error_message,
        }
    }
}

impl From<VoiceTypingReadinessInput> for FfiVoiceTypingReadinessV1 {
    fn from(value: VoiceTypingReadinessInput) -> Self {
        Self {
            state: value.state,
            last_error_message: value.last_error_message,
        }
    }
}

impl From<RuntimeEnvironmentStatus> for FfiRuntimeEnvironmentStatusV1 {
    fn from(value: RuntimeEnvironmentStatus) -> Self {
        Self {
            ffmpeg_path: value.ffmpeg_path,
            ffmpeg_exists: value.ffmpeg_exists,
            log_dir_path: value.log_dir_path,
        }
    }
}

impl From<AsrRuntimeMetricsSnapshot> for FfiAsrRuntimeMetricsSnapshotV1 {
    fn from(value: AsrRuntimeMetricsSnapshot) -> Self {
        Self {
            model_load: value
                .model_load
                .as_ref()
                .map(super::asr_streaming_mapper::asr_model_load_metric_to_ffi),
            live_inference: value
                .live_inference
                .as_ref()
                .map(super::asr_streaming_mapper::asr_inference_metric_to_ffi),
            batch_inference: value
                .batch_inference
                .as_ref()
                .map(super::asr_streaming_mapper::asr_inference_metric_to_ffi),
        }
    }
}

impl From<DiagnosticsCoreSnapshot> for FfiDiagnosticsSnapshotV1 {
    fn from(value: DiagnosticsCoreSnapshot) -> Self {
        Self {
            scanned_at: value.scanned_at,
            config: value.config.into(),
            selected_models: value.selected_models.into(),
            model_rules: value.model_rules.into(),
            path_statuses: value.path_statuses.into(),
            permission_state: value.permission_state,
            microphone_probe: value.microphone_probe.into(),
            system_audio_probe: value.system_audio_probe.into(),
            voice_typing_readiness: value.voice_typing_readiness.into(),
            runtime_environment: value.runtime_environment.into(),
            asr_runtime_metrics: value.asr_runtime_metrics.into(),
            onboarding_ready: value.onboarding_ready,
            punctuation_required: value.punctuation_required,
        }
    }
}
