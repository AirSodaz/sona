use super::SherpaError;
use super::apply_timeline_normalization;
use super::metrics::{
    AsrInferenceMetric, current_time_millis, duration_to_ms, log_inference_metric,
    set_batch_inference_metric,
};
use super::types::{AsrTranscriptionRequest, TranscriptSegment};
use super::{AsrBatchProcessor, AsrProviderAdapter, AsrState};
use async_trait::async_trait;
use sona_core::ports::asr::{
    AsrRuntimeObserver, AsrStreamingSession, OnlineBatchTranscriber,
    OnlineBatchTranscriptionRequest,
};
use sona_core::transcription::postprocess::TranscriptPostprocessor;
use std::sync::Arc;
use std::time::Instant;

pub struct MistralVoxtralAdapter;

#[async_trait]
impl AsrProviderAdapter for MistralVoxtralAdapter {
    fn provider_id(&self) -> &'static str {
        sona_core::ports::asr::MISTRAL_VOXTRAL_PROVIDER_ID
    }

    fn create_batch_processor(
        &self,
        _request: &AsrTranscriptionRequest,
    ) -> Result<Option<std::sync::Arc<dyn AsrBatchProcessor>>, SherpaError> {
        Ok(Some(std::sync::Arc::new(MistralVoxtralBatchProcessor)))
    }

    async fn create_streaming_session(
        &self,
        _state: &AsrState,
        _instance_id: &str,
        _request: &AsrTranscriptionRequest,
        _observer: Arc<dyn AsrRuntimeObserver>,
    ) -> Result<Option<Arc<dyn AsrStreamingSession>>, SherpaError> {
        Ok(None)
    }
}

pub struct MistralVoxtralBatchProcessor;

#[async_trait]
impl AsrBatchProcessor for MistralVoxtralBatchProcessor {
    async fn process_file(
        &self,
        emitter: std::sync::Arc<dyn crate::platform::event::EventEmitter>,
        state: &AsrState,
        file_path: std::path::PathBuf,
        _save_to_path: Option<std::path::PathBuf>,
        request: AsrTranscriptionRequest,
        _speaker_processing: Option<sona_core::transcription::speaker::SpeakerProcessingConfig>,
        _instance_id: Option<String>,
    ) -> Result<Vec<TranscriptSegment>, SherpaError> {
        process_batch_file_impl(emitter, state, file_path, request)
            .await
            .map_err(SherpaError::Generic)
    }
}

pub async fn process_batch_file_impl(
    emitter: std::sync::Arc<dyn crate::platform::event::EventEmitter>,
    state: &AsrState,
    file_path: std::path::PathBuf,
    request: AsrTranscriptionRequest,
) -> Result<Vec<TranscriptSegment>, String> {
    let started = Instant::now();
    let output = sona_online_asr::MistralVoxtralBatchTranscriber::default()
        .transcribe(OnlineBatchTranscriptionRequest {
            file_path: file_path.clone(),
            request: request.clone(),
        })
        .await?;

    let mut segments = apply_timeline_normalization(output.segments, request.normalization_options);
    segments =
        TranscriptPostprocessor::compile(request.postprocess_options)?.process_segments(segments);

    let metric = AsrInferenceMetric {
        occurred_at_ms: current_time_millis(),
        source: "batch".to_string(),
        instance_id: None,
        stage: output.stage,
        is_final: true,
        audio_duration_ms: output.audio_duration_ms,
        buffered_samples: output.buffered_samples,
        audio_extract_ms: None,
        decode_ms: duration_to_ms(started.elapsed()),
        emit_latency_ms: None,
        total_ms: Some(duration_to_ms(started.elapsed())),
        rtf: None,
        segment_count: Some(segments.len()),
        process_rss_mb: None,
    };
    set_batch_inference_metric(&state.metrics, metric.clone());
    log_inference_metric(&metric);

    let _ = emitter.emit(
        crate::integrations::asr::BATCH_PROGRESS_EVENT,
        serde_json::json!([file_path.to_string_lossy().as_ref(), 100.0_f32]),
    );

    Ok(segments)
}
