use super::metrics::{
    AsrInferenceMetric, current_time_millis, duration_to_ms, log_inference_metric,
    set_batch_inference_metric,
};
use super::transcript::apply_timeline_normalization;
use super::types::{AsrTranscriptionRequest, TranscriptSegment};
use super::{AsrBatchProcessor, AsrProviderAdapter, AsrState, SherpaError};
use async_trait::async_trait;
use sona_core::ports::asr::{
    AsrRuntimeObserver, AsrStreamingSession, OnlineBatchTranscriptionRequest,
};
use sona_core::transcription::postprocess::TranscriptPostprocessor;
use std::sync::Arc;
use std::time::Instant;

pub struct DesktopOnlineAsrAdapter {
    provider_id: &'static str,
}

impl DesktopOnlineAsrAdapter {
    pub const fn new(provider_id: &'static str) -> Self {
        Self { provider_id }
    }
}

#[async_trait]
impl AsrProviderAdapter for DesktopOnlineAsrAdapter {
    fn provider_id(&self) -> &'static str {
        self.provider_id
    }

    fn create_batch_processor(
        &self,
        request: &AsrTranscriptionRequest,
    ) -> Result<Option<Arc<dyn AsrBatchProcessor>>, SherpaError> {
        let provider_id = sona_online_asr::resolve_online_asr_provider_id(request)?;
        if provider_id != self.provider_id {
            return Err(SherpaError::UnsupportedOnlineProvider {
                provider_id: provider_id.to_string(),
            });
        }
        Ok(Some(Arc::new(OnlineBatchProcessor)))
    }

    async fn create_streaming_session(
        &self,
        _state: &AsrState,
        instance_id: &str,
        request: &AsrTranscriptionRequest,
        observer: Arc<dyn AsrRuntimeObserver>,
    ) -> Result<Option<Arc<dyn AsrStreamingSession>>, SherpaError> {
        let provider_id = sona_online_asr::resolve_online_asr_provider_id(request)?;
        if provider_id != self.provider_id {
            return Err(SherpaError::UnsupportedOnlineProvider {
                provider_id: provider_id.to_string(),
            });
        }
        let session = sona_online_asr::OnlineAsrAdapter.create_streaming_session(
            instance_id.to_string(),
            request.clone(),
            observer,
        )?;
        Ok(Some(session))
    }
}

struct OnlineBatchProcessor;

#[async_trait]
impl AsrBatchProcessor for OnlineBatchProcessor {
    async fn process_file(
        &self,
        emitter: Arc<dyn crate::platform::event::EventEmitter>,
        state: &AsrState,
        file_path: std::path::PathBuf,
        _save_to_path: Option<std::path::PathBuf>,
        request: AsrTranscriptionRequest,
        _speaker_processing: Option<sona_core::transcription::speaker::SpeakerProcessingConfig>,
        _instance_id: Option<String>,
    ) -> Result<Vec<TranscriptSegment>, SherpaError> {
        let started = Instant::now();
        let output = sona_online_asr::OnlineAsrAdapter
            .transcribe_batch(OnlineBatchTranscriptionRequest {
                file_path: file_path.clone(),
                request: request.clone(),
            })
            .await?;

        let mut segments =
            apply_timeline_normalization(output.segments, request.normalization_options);
        segments = TranscriptPostprocessor::compile(request.postprocess_options)
            .map_err(|error| SherpaError::Generic(error.to_string()))?
            .process_segments(segments);

        let elapsed_ms = duration_to_ms(started.elapsed());
        let metric = AsrInferenceMetric {
            occurred_at_ms: current_time_millis(),
            source: "batch".to_string(),
            instance_id: None,
            stage: output.stage,
            is_final: true,
            audio_duration_ms: output.audio_duration_ms,
            buffered_samples: output.buffered_samples,
            audio_extract_ms: None,
            decode_ms: elapsed_ms,
            emit_latency_ms: None,
            total_ms: Some(elapsed_ms),
            rtf: None,
            segment_count: Some(segments.len()),
            process_rss_mb: None,
        };
        set_batch_inference_metric(&state.metrics, metric.clone());
        log_inference_metric(&metric);

        let _ = emitter.emit(
            super::BATCH_PROGRESS_EVENT,
            serde_json::json!([file_path.to_string_lossy().as_ref(), 100.0_f32]),
        );

        Ok(segments)
    }
}
