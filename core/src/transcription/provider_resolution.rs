use crate::ports::asr::{
    AsrEngineConfig, AsrMode, AsrTranscriptionRequest, SherpaError, find_online_asr_provider,
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
) -> Result<&'a str, SherpaError> {
    resolve_asr_provider_id_for_mode(request, request.mode, capabilities)
}

pub fn resolve_asr_streaming_provider_id<'a>(
    request: &AsrTranscriptionRequest,
    capabilities: &'a [AsrProviderCapability<'a>],
) -> Result<&'a str, SherpaError> {
    resolve_asr_provider_id_for_mode(request, AsrMode::Streaming, capabilities)
}

fn resolve_asr_provider_id_for_mode<'a>(
    request: &AsrTranscriptionRequest,
    mode: AsrMode,
    capabilities: &'a [AsrProviderCapability<'a>],
) -> Result<&'a str, SherpaError> {
    let provider_id = request.provider_id();

    if matches!(&request.engine_config, AsrEngineConfig::Online { .. }) {
        let provider = find_online_asr_provider(provider_id).ok_or_else(|| {
            SherpaError::UnsupportedOnlineProvider {
                provider_id: provider_id.to_owned(),
            }
        })?;
        if mode == AsrMode::Streaming && provider.streaming.supported == Some(false) {
            return Err(SherpaError::StreamingNotSupported {
                provider_id: provider_id.to_owned(),
            });
        }
    }

    let capability = capabilities
        .iter()
        .find(|capability| capability.provider_id == provider_id)
        .ok_or_else(|| SherpaError::UnsupportedOnlineProvider {
            provider_id: provider_id.to_owned(),
        })?;

    if mode == AsrMode::Streaming && !capability.supports_streaming {
        return Err(SherpaError::StreamingNotSupported {
            provider_id: provider_id.to_owned(),
        });
    }

    Ok(capability.provider_id)
}
