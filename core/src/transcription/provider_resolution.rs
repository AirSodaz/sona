use crate::ports::asr::{
    AsrEngineConfig, AsrMode, AsrPortError, AsrPortErrorKind, AsrTranscriptionRequest,
    find_online_asr_provider,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsrProviderCapability<'a> {
    pub provider_id: &'a str,
    pub supports_streaming: bool,
}

impl<'a> AsrProviderCapability<'a> {
    pub const fn new(provider_id: &'a str, supports_streaming: bool) -> Self {
        Self {
            provider_id,
            supports_streaming,
        }
    }
}

pub fn resolve_asr_provider_id<'a>(
    request: &AsrTranscriptionRequest,
    capabilities: &'a [AsrProviderCapability<'a>],
) -> Result<&'a str, AsrPortError> {
    resolve_asr_provider_id_for_mode(request, request.mode, capabilities)
}

pub fn resolve_asr_streaming_provider_id<'a>(
    request: &AsrTranscriptionRequest,
    capabilities: &'a [AsrProviderCapability<'a>],
) -> Result<&'a str, AsrPortError> {
    resolve_asr_provider_id_for_mode(request, AsrMode::Streaming, capabilities)
}

fn resolve_asr_provider_id_for_mode<'a>(
    request: &AsrTranscriptionRequest,
    mode: AsrMode,
    capabilities: &'a [AsrProviderCapability<'a>],
) -> Result<&'a str, AsrPortError> {
    let provider_id = request.provider_id();

    if matches!(&request.engine_config, AsrEngineConfig::Online { .. }) {
        let provider = find_online_asr_provider(provider_id).ok_or_else(|| {
            AsrPortError::new(
                AsrPortErrorKind::Unsupported,
                format!("不支持的在线 ASR provider：{provider_id}"),
            )
            .with_code("UNSUPPORTED_ONLINE_PROVIDER")
        })?;
        if mode == AsrMode::Streaming && provider.streaming.supported == Some(false) {
            return Err(AsrPortError::new(
                AsrPortErrorKind::Unsupported,
                format!("provider {provider_id} 不支持流式识别"),
            )
            .with_code("STREAMING_NOT_SUPPORTED"));
        }
    }

    let capability = capabilities
        .iter()
        .find(|capability| capability.provider_id == provider_id)
        .ok_or_else(|| {
            AsrPortError::new(
                AsrPortErrorKind::Unsupported,
                format!("不支持的在线 ASR provider：{provider_id}"),
            )
            .with_code("UNSUPPORTED_ONLINE_PROVIDER")
        })?;

    if mode == AsrMode::Streaming && !capability.supports_streaming {
        return Err(
            AsrPortError::new(
                AsrPortErrorKind::Unsupported,
                format!("provider {provider_id} 不支持流式识别"),
            )
            .with_code("STREAMING_NOT_SUPPORTED"),
        );
    }

    Ok(capability.provider_id)
}
