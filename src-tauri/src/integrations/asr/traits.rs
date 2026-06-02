use async_trait::async_trait;
use tauri::AppHandle;

use crate::integrations::asr::TranscriptSegment;
use crate::integrations::asr::error::SherpaError;
use crate::integrations::asr::state::AsrState;
use crate::integrations::asr::types::AsrTranscriptionRequest;

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
        save_to_path: Option<String>,
        request: AsrTranscriptionRequest,
        speaker_processing: Option<crate::integrations::speaker::SpeakerProcessingConfig>,
    ) -> Result<Vec<TranscriptSegment>, SherpaError>;
}

#[async_trait]
pub trait AsrProviderAdapter: Send + Sync {
    fn provider_id(&self) -> &'static str;

    fn create_batch_processor(
        &self,
        request: &AsrTranscriptionRequest,
    ) -> Result<Option<std::sync::Arc<dyn AsrBatchProcessor>>, SherpaError>;

    async fn create_streaming_session(
        &self,
        state: &AsrState,
        instance_id: &str,
        request: &AsrTranscriptionRequest,
    ) -> Result<Option<std::sync::Arc<dyn AsrStreamingSession>>, SherpaError>;
}
