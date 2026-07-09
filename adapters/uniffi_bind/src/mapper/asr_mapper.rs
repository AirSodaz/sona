use sona_core::ports::asr::{
    AsrEngine, AsrMode, BatchSegmentationMode, OnlineAsrBatchCapability, OnlineAsrCapability,
    OnlineAsrLocalFileBatchMode, OnlineAsrProvider, OnlineAsrProviderRequest,
    VolcengineDoubaoAsrConfig,
};

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
pub struct FfiOnlineAsrCapability {
    pub supported: Option<bool>,
    pub requires_api_key: bool,
    pub required_config_fields: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiOnlineAsrLocalFileBatchMode {
    pub supported: bool,
    pub endpoint: String,
    pub resource_id: String,
    pub unsupported_message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiOnlineAsrBatchCapability {
    pub requires_api_key: bool,
    pub required_config_fields: Vec<String>,
    pub local_file_mode: FfiOnlineAsrLocalFileBatchMode,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiOnlineAsrProvider {
    pub id: String,
    pub profile_id: String,
    pub defaults_json: String,
    pub streaming: FfiOnlineAsrCapability,
    pub batch: FfiOnlineAsrBatchCapability,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiVolcengineDoubaoAsrConfig {
    pub api_key: String,
    pub streaming_endpoint: String,
    pub streaming_resource_id: String,
    pub batch_endpoint: String,
    pub batch_resource_id: String,
}

#[expect(dead_code)]
pub fn asr_engine_to_ffi(engine: AsrEngine) -> FfiAsrEngine {
    match engine {
        AsrEngine::LocalSherpa => FfiAsrEngine::LocalSherpa,
        AsrEngine::Online => FfiAsrEngine::Online,
    }
}

#[expect(dead_code)]
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

fn online_asr_capability_to_ffi(capability: &OnlineAsrCapability) -> FfiOnlineAsrCapability {
    FfiOnlineAsrCapability {
        supported: capability.supported,
        requires_api_key: capability.requires_api_key,
        required_config_fields: capability.required_config_fields.clone(),
    }
}

fn online_asr_local_file_batch_mode_to_ffi(
    mode: &OnlineAsrLocalFileBatchMode,
) -> FfiOnlineAsrLocalFileBatchMode {
    FfiOnlineAsrLocalFileBatchMode {
        supported: mode.supported,
        endpoint: mode.endpoint.clone(),
        resource_id: mode.resource_id.clone(),
        unsupported_message: mode.unsupported_message.clone(),
    }
}

fn online_asr_batch_capability_to_ffi(
    capability: &OnlineAsrBatchCapability,
) -> FfiOnlineAsrBatchCapability {
    FfiOnlineAsrBatchCapability {
        requires_api_key: capability.requires_api_key,
        required_config_fields: capability.required_config_fields.clone(),
        local_file_mode: online_asr_local_file_batch_mode_to_ffi(&capability.local_file_mode),
    }
}

pub fn online_asr_provider_to_ffi(provider: &OnlineAsrProvider) -> FfiOnlineAsrProvider {
    FfiOnlineAsrProvider {
        id: provider.id.clone(),
        profile_id: provider.profile_id.clone(),
        defaults_json: provider.defaults.to_string(),
        streaming: online_asr_capability_to_ffi(&provider.streaming),
        batch: online_asr_batch_capability_to_ffi(&provider.batch),
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
