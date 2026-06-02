use async_trait::async_trait;
use serde_json::Value;
use tauri::AppHandle;

use crate::asr::TranscriptSegment;
use crate::asr::error::SherpaError;
use crate::asr::state::AsrState;
use crate::asr::types::AsrTranscriptionRequest;

#[async_trait]
pub trait AsrStreamingSession: Send + Sync {
    async fn start(
        &self,
        app: AppHandle,
        state: &AsrState,
        instance_id: &str,
    ) -> Result<(), SherpaError>;
    async fn stop(&self, state: &AsrState, instance_id: &str) -> Result<(), SherpaError>;
    async fn flush(
        &self,
        app: AppHandle,
        state: &AsrState,
        instance_id: &str,
    ) -> Result<(), SherpaError>;
    async fn feed_audio_chunk(
        &self,
        app: AppHandle,
        state: &AsrState,
        instance_id: &str,
        samples: Vec<u8>,
    ) -> Result<(), SherpaError>;
    async fn feed_audio_samples(
        &self,
        app: AppHandle,
        state: &AsrState,
        instance_id: &str,
        samples: &[f32],
    ) -> Result<(), SherpaError>;
}

#[async_trait]
pub trait AsrBatchProcessor: Send + Sync {
    async fn process_file(
        &self,
        app: AppHandle,
        state: &AsrState,
        file_path: String,
        request: AsrTranscriptionRequest,
    ) -> Result<Vec<TranscriptSegment>, SherpaError>;
}

pub trait AsrProviderAdapter: Send + Sync {
    fn provider_id(&self) -> &'static str;

    fn create_batch_processor(
        &self,
        config: &Value,
    ) -> Result<Option<std::sync::Arc<dyn AsrBatchProcessor>>, SherpaError>;

    fn create_streaming_session(
        &self,
        config: &Value,
        request: &AsrTranscriptionRequest,
    ) -> Result<Option<std::sync::Arc<dyn AsrStreamingSession>>, SherpaError>;
}
