use async_trait::async_trait;
use sona_core::ports::asr::{AsrRuntimeObserver, AsrStreamingSession};
use std::sync::Arc;

use super::{AsrState, AsrTranscriptionRequest, SherpaError, TranscriptSegment};

#[async_trait]
pub trait AsrBatchProcessor: Send + Sync {
    #[allow(clippy::too_many_arguments)]
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
        observer: Arc<dyn AsrRuntimeObserver>,
    ) -> Result<Option<Arc<dyn AsrStreamingSession>>, SherpaError>;
}
