use std::path::PathBuf;

use async_trait::async_trait;
use sona_core::ports::asr::{
    AsrEngineConfig, AsrMode, AsrTranscriptionRequest, OnlineAsrProviderRequest,
};
use sona_core::runtime::serve::ServeTranscriptionDefaults;
use sona_core::transcription::transcript::TranscriptSegment;

use crate::ApiServerPlatformError;
use sona_core::llm::requests::LlmConfig;

pub const ONLINE_ASR_BATCH_UNAVAILABLE: &str =
    "Online ASR batch is unavailable because no platform online ASR adapter is configured.";
pub const LLM_POLISH_UNAVAILABLE: &str =
    "LLM polish is unavailable because no platform LLM adapter is configured.";
pub const LLM_TRANSLATE_UNAVAILABLE: &str =
    "LLM translation is unavailable because no platform LLM adapter is configured.";

pub type ApiServerTranscriptionDefaults = ServeTranscriptionDefaults;

#[derive(Clone, Debug)]
pub struct OnlineBatchRequest {
    pub file_path: PathBuf,
    pub provider_id: String,
    pub profile_id: String,
    pub config: serde_json::Value,
    pub language: String,
    pub hotwords: Option<String>,
}

impl OnlineBatchRequest {
    pub fn to_core_request(&self) -> AsrTranscriptionRequest {
        AsrTranscriptionRequest {
            engine_config: AsrEngineConfig::Online {
                provider: OnlineAsrProviderRequest {
                    provider_id: self.provider_id.clone(),
                    profile_id: self.profile_id.clone(),
                    config: self.config.clone(),
                },
            },
            mode: AsrMode::Batch,
            enable_itn: false,
            language: self.language.clone(),
            hotwords: self.hotwords.clone(),
            speaker_processing: None,
            normalization_options: Default::default(),
            postprocess_options: Default::default(),
        }
    }

    pub fn to_core_batch_request(&self) -> sona_core::ports::asr::OnlineBatchTranscriptionRequest {
        sona_core::ports::asr::OnlineBatchTranscriptionRequest {
            file_path: self.file_path.clone(),
            request: self.to_core_request(),
        }
    }
}

#[async_trait]
pub trait ApiServerPlatform: Send + Sync {
    async fn transcribe_online_batch(
        &self,
        _request: OnlineBatchRequest,
    ) -> Result<Vec<TranscriptSegment>, ApiServerPlatformError> {
        Err(ApiServerPlatformError::unavailable(
            ONLINE_ASR_BATCH_UNAVAILABLE,
        ))
    }

    async fn polish_segments(
        &self,
        _segments: Vec<TranscriptSegment>,
        _config: Option<LlmConfig>,
    ) -> Result<Vec<TranscriptSegment>, ApiServerPlatformError> {
        Err(ApiServerPlatformError::unavailable(LLM_POLISH_UNAVAILABLE))
    }

    async fn translate_segments(
        &self,
        _segments: Vec<TranscriptSegment>,
        _target_language: String,
        _config: Option<LlmConfig>,
    ) -> Result<Vec<TranscriptSegment>, ApiServerPlatformError> {
        Err(ApiServerPlatformError::unavailable(
            LLM_TRANSLATE_UNAVAILABLE,
        ))
    }
}

#[derive(Clone, Default)]
pub struct DefaultApiServerPlatform;

#[async_trait]
impl ApiServerPlatform for DefaultApiServerPlatform {}

pub fn online_batch_request_to_core_request(
    request: &OnlineBatchRequest,
) -> AsrTranscriptionRequest {
    request.to_core_request()
}
