use sona_core::ports::asr::{
    AsrEngine, AsrMode, BatchSegmentationMode, OnlineAsrProviderRequest, VolcengineDoubaoAsrConfig,
};
use sona_core::runtime::{RuntimePathKind, RuntimePathStatus};

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiRuntimePathKind {
    File,
    Directory,
    Missing,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiRuntimePathStatus {
    pub path: String,
    pub kind: FfiRuntimePathKind,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiAsrEngine {
    LocalSherpa,
    Online,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiAsrMode {
    Streaming,
    Batch,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiBatchSegmentationMode {
    Vad,
    Whole,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiOnlineAsrProviderRequest {
    pub provider_id: String,
    pub profile_id: String,
    pub config_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiVolcengineDoubaoAsrConfig {
    pub api_key: String,
    pub streaming_endpoint: String,
    pub streaming_resource_id: String,
    pub batch_endpoint: String,
    pub batch_resource_id: String,
}

pub fn runtime_path_status_to_ffi(status: RuntimePathStatus) -> FfiRuntimePathStatus {
    FfiRuntimePathStatus {
        path: status.path,
        kind: runtime_path_kind_to_ffi(status.kind),
        error: status.error,
    }
}

fn runtime_path_kind_to_ffi(kind: RuntimePathKind) -> FfiRuntimePathKind {
    match kind {
        RuntimePathKind::File => FfiRuntimePathKind::File,
        RuntimePathKind::Directory => FfiRuntimePathKind::Directory,
        RuntimePathKind::Missing => FfiRuntimePathKind::Missing,
        RuntimePathKind::Unknown => FfiRuntimePathKind::Unknown,
    }
}

pub fn asr_engine_to_ffi(engine: AsrEngine) -> FfiAsrEngine {
    match engine {
        AsrEngine::LocalSherpa => FfiAsrEngine::LocalSherpa,
        AsrEngine::Online => FfiAsrEngine::Online,
    }
}

pub fn asr_mode_to_ffi(mode: AsrMode) -> FfiAsrMode {
    match mode {
        AsrMode::Streaming => FfiAsrMode::Streaming,
        AsrMode::Batch => FfiAsrMode::Batch,
    }
}

pub fn batch_segmentation_mode_to_ffi(mode: BatchSegmentationMode) -> FfiBatchSegmentationMode {
    match mode {
        BatchSegmentationMode::Vad => FfiBatchSegmentationMode::Vad,
        BatchSegmentationMode::Whole => FfiBatchSegmentationMode::Whole,
    }
}

pub fn online_asr_provider_request_to_ffi(
    request: OnlineAsrProviderRequest,
) -> FfiOnlineAsrProviderRequest {
    FfiOnlineAsrProviderRequest {
        provider_id: request.provider_id,
        profile_id: request.profile_id,
        config_json: request.config.to_string(),
    }
}

pub fn volcengine_doubao_asr_config_to_ffi(
    config: VolcengineDoubaoAsrConfig,
) -> FfiVolcengineDoubaoAsrConfig {
    FfiVolcengineDoubaoAsrConfig {
        api_key: config.api_key,
        streaming_endpoint: config.streaming_endpoint,
        streaming_resource_id: config.streaming_resource_id,
        batch_endpoint: config.batch_endpoint,
        batch_resource_id: config.batch_resource_id,
    }
}
