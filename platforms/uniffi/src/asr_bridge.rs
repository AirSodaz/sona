use crate::SonaCoreBindingResult;
use crate::json_bridge::parse_core_json;
use crate::mapper::{
    self, FfiBatchSegmentationMode, FfiOnlineAsrProvider, FfiOnlineAsrProviderRequest,
    FfiVolcengineDoubaoAsrConfig,
};
use sona_core::ports::asr::{
    BatchSegmentationMode, OnlineAsrProviderRequest, VolcengineDoubaoAsrConfig,
    find_online_asr_provider as core_find_online_asr_provider,
    online_asr_providers as core_online_asr_providers,
};

pub(crate) fn default_batch_segmentation_mode() -> FfiBatchSegmentationMode {
    mapper::batch_segmentation_mode_to_ffi(BatchSegmentationMode::default())
}

pub(crate) fn online_asr_providers() -> Vec<FfiOnlineAsrProvider> {
    core_online_asr_providers()
        .iter()
        .map(mapper::online_asr_provider_to_ffi)
        .collect()
}

pub(crate) fn find_online_asr_provider(provider_id: String) -> Option<FfiOnlineAsrProvider> {
    core_find_online_asr_provider(&provider_id).map(mapper::online_asr_provider_to_ffi)
}

pub(crate) fn online_asr_provider_request(
    provider_id: String,
    profile_id: String,
    config_json: String,
) -> SonaCoreBindingResult<FfiOnlineAsrProviderRequest> {
    let config = parse_core_json(&config_json, "ASR provider config")?;

    Ok(mapper::online_asr_provider_request_to_ffi(
        OnlineAsrProviderRequest {
            provider_id,
            profile_id,
            config,
        },
    ))
}

pub(crate) fn volcengine_doubao_asr_config_from_json(
    config_json: String,
) -> SonaCoreBindingResult<FfiVolcengineDoubaoAsrConfig> {
    let config: VolcengineDoubaoAsrConfig =
        parse_core_json(&config_json, "Volcengine Doubao ASR config")?;

    Ok(mapper::volcengine_doubao_asr_config_to_ffi(config))
}
