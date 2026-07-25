use std::any::Any;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use sona_core::ports::asr::{
    AsrEngineConfig, AsrMode, AsrTranscriptionRequest, OnlineAsrProviderRequest,
};
use sona_core::runtime::gpu::DEFAULT_GPU_ACCELERATION;
use sona_core::transcription::transcript::TranscriptSegment;

use crate::ApiServerPlatformError;

pub const ONLINE_ASR_BATCH_UNAVAILABLE: &str =
    "Online ASR batch is unavailable because no platform online ASR adapter is configured.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiServerTranscriptionDefaults {
    pub gpu_acceleration: Option<String>,
    pub vad_model_id: Option<String>,
    pub punctuation_model_id: Option<String>,
}

impl Default for ApiServerTranscriptionDefaults {
    fn default() -> Self {
        Self {
            gpu_acceleration: Some(DEFAULT_GPU_ACCELERATION.to_string()),
            vad_model_id: Some(
                sona_core::models::preset_models::DEFAULT_SILERO_VAD_MODEL_ID.to_string(),
            ),
            punctuation_model_id: Some(
                sona_core::models::preset_models::DEFAULT_PUNCTUATION_MODEL_ID.to_string(),
            ),
        }
    }
}

#[derive(Clone, Debug)]
pub struct OnlineBatchRequest {
    pub file_path: PathBuf,
    pub provider_id: String,
    pub profile_id: String,
    pub config: serde_json::Value,
    pub language: String,
    pub hotwords: Option<String>,
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

    fn streaming_context(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        None
    }
}

#[derive(Clone, Default)]
pub struct DefaultApiServerPlatform;

#[async_trait]
impl ApiServerPlatform for DefaultApiServerPlatform {}

pub fn online_batch_request_to_core_request(
    request: &OnlineBatchRequest,
) -> AsrTranscriptionRequest {
    AsrTranscriptionRequest {
        engine_config: AsrEngineConfig::Online {
            provider: OnlineAsrProviderRequest {
                provider_id: request.provider_id.clone(),
                profile_id: request.profile_id.clone(),
                config: request.config.clone(),
            },
        },
        mode: AsrMode::Batch,
        enable_itn: false,
        language: request.language.clone(),
        hotwords: request.hotwords.clone(),
        speaker_processing: None,
        normalization_options: Default::default(),
        postprocess_options: Default::default(),
    }
}
