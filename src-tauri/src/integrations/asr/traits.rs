use async_trait::async_trait;

use crate::integrations::asr::SherpaError;
use crate::integrations::asr::TranscriptSegment;
use crate::integrations::asr::state::AsrState;
use crate::integrations::asr::types::AsrTranscriptionRequest;

#[async_trait]
pub trait AsrStreamingSession: Send + Sync {
    async fn start(
        &self,
        emitter: std::sync::Arc<dyn crate::platform::event::EventEmitter>,
        state: &AsrState,
        instance_id: &str,
    ) -> Result<(), SherpaError>;
    async fn stop(&self, state: &AsrState, instance_id: &str) -> Result<(), SherpaError>;
    async fn flush(
        &self,
        emitter: std::sync::Arc<dyn crate::platform::event::EventEmitter>,
        state: &AsrState,
        instance_id: &str,
    ) -> Result<(), SherpaError>;
    async fn feed_audio_chunk(
        &self,
        emitter: std::sync::Arc<dyn crate::platform::event::EventEmitter>,
        state: &AsrState,
        instance_id: &str,
        samples: Vec<u8>,
    ) -> Result<(), SherpaError>;
    async fn feed_audio_samples(
        &self,
        emitter: std::sync::Arc<dyn crate::platform::event::EventEmitter>,
        state: &AsrState,
        instance_id: &str,
        samples: &[f32],
    ) -> Result<(), SherpaError>;
}

#[async_trait]
pub trait AsrBatchProcessor: Send + Sync {
    async fn process_file(
        &self,
        emitter: std::sync::Arc<dyn crate::platform::event::EventEmitter>,
        state: &AsrState,
        file_path: std::path::PathBuf,
        save_to_path: Option<std::path::PathBuf>,
        request: AsrTranscriptionRequest,
        speaker_processing: Option<sona_core::transcription::speaker::SpeakerProcessingConfig>,
        instance_id: Option<String>,
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
