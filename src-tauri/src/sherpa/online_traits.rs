use async_trait::async_trait;
use serde_json::Value;
use tauri::AppHandle;

use crate::sherpa::error::SherpaError;
use crate::sherpa::state::SherpaState;
use crate::sherpa::types::AsrTranscriptionRequest;
use crate::sherpa::TranscriptSegment;

#[async_trait]
pub trait OnlineStreamingSession: Send + Sync {
    async fn start(&self, app: AppHandle, instance_id: &str) -> Result<(), SherpaError>;
    async fn stop(&self, state: &SherpaState, instance_id: &str) -> Result<(), SherpaError>;
    async fn flush(&self, app: AppHandle, state: &SherpaState, instance_id: &str) -> Result<(), SherpaError>;
    async fn feed_audio_chunk(
        &self,
        app: AppHandle,
        state: &SherpaState,
        instance_id: &str,
        samples: Vec<u8>,
    ) -> Result<(), SherpaError>;
    async fn feed_audio_samples(
        &self,
        state: &SherpaState,
        instance_id: &str,
        samples: &[f32],
    ) -> Result<(), SherpaError>;
}

#[async_trait]
pub trait OnlineBatchProcessor: Send + Sync {
    async fn process_file(
        &self,
        app: AppHandle,
        state: &SherpaState,
        file_path: String,
        request: AsrTranscriptionRequest,
    ) -> Result<Vec<TranscriptSegment>, SherpaError>;
}

pub trait OnlineAsrProviderAdapter: Send + Sync {
    fn provider_id(&self) -> &'static str;

    fn create_batch_processor(
        &self,
        config: &Value,
    ) -> Result<Option<Box<dyn OnlineBatchProcessor>>, SherpaError>;

    fn create_streaming_session(
        &self,
        config: &Value,
        request: &AsrTranscriptionRequest,
    ) -> Result<Option<Box<dyn OnlineStreamingSession>>, SherpaError>;
}
