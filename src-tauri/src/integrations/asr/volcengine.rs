use super::SherpaError;
use super::metrics::{
    AsrInferenceMetric, current_time_millis, duration_to_ms, log_inference_metric,
    set_batch_inference_metric,
};
use super::transcript::apply_timeline_normalization;
use super::types::{AsrMode, AsrTranscriptionRequest, TranscriptSegment};
use super::{AsrBatchProcessor, AsrProviderAdapter, AsrState};
use async_trait::async_trait;
use sona_core::ports::asr::{
    AsrRuntimeObserver, AsrStreamingSession, OnlineBatchTranscriber,
    OnlineBatchTranscriptionRequest,
};
use sona_core::transcription::postprocess::TranscriptPostprocessor;
use std::sync::Arc;
use std::time::Instant;

fn config_from_request(
    request: &AsrTranscriptionRequest,
    mode: sona_online_asr::VolcengineMode,
) -> Result<sona_online_asr::VolcengineDoubaoConfigFields, SherpaError> {
    sona_online_asr::resolve_volcengine_config_checked(request, mode).map_err(SherpaError::from)
}

pub struct VolcengineAdapter;

#[async_trait]
impl AsrProviderAdapter for VolcengineAdapter {
    fn provider_id(&self) -> &'static str {
        sona_core::ports::asr::VOLCENGINE_DOUBAO_PROVIDER_ID
    }

    fn create_batch_processor(
        &self,
        _request: &AsrTranscriptionRequest,
    ) -> Result<Option<std::sync::Arc<dyn AsrBatchProcessor>>, SherpaError> {
        Ok(Some(std::sync::Arc::new(VolcengineBatchProcessor)))
    }

    async fn create_streaming_session(
        &self,
        _state: &AsrState,
        instance_id: &str,
        request: &AsrTranscriptionRequest,
        observer: Arc<dyn AsrRuntimeObserver>,
    ) -> Result<Option<Arc<dyn AsrStreamingSession>>, SherpaError> {
        let session = sona_online_asr::create_volcengine_streaming_session(
            instance_id.to_string(),
            request.clone(),
            observer,
        )?;
        Ok(Some(session))
    }
}

pub struct VolcengineBatchProcessor;

#[async_trait]
impl AsrBatchProcessor for VolcengineBatchProcessor {
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
        process_batch_file_impl(emitter, state, file_path, request).await
    }
}

pub async fn process_batch_file_impl(
    emitter: std::sync::Arc<dyn crate::platform::event::EventEmitter>,
    state: &AsrState,
    file_path: std::path::PathBuf,
    request: AsrTranscriptionRequest,
) -> Result<Vec<TranscriptSegment>, SherpaError> {
    if request.mode != AsrMode::Batch {
        return Err(SherpaError::VolcengineBatchModeMismatch);
    }
    config_from_request(&request, sona_online_asr::VolcengineMode::Batch)?;
    let started = Instant::now();
    let output = sona_online_asr::VolcengineDoubaoBatchTranscriber::default()
        .transcribe(OnlineBatchTranscriptionRequest {
            file_path: file_path.clone(),
            request: request.clone(),
        })
        .await
        .map_err(|error| SherpaError::VolcengineBatchRequestFailed { error })?;

    let mut segments = output.segments;
    segments = apply_timeline_normalization(segments, request.normalization_options);
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
        super::BATCH_PROGRESS_EVENT,
        serde_json::json!([file_path.to_string_lossy().as_ref(), 100.0_f32]),
    );
    Ok(segments)
}

#[cfg(test)]
mod tests {
    use super::super::{
        AsrEngineConfig, OnlineAsrProviderRequest, TranscriptNormalizationOptions,
        TranscriptPostprocessOptions,
    };
    use super::*;

    fn config(api_key: &str) -> serde_json::Value {
        serde_json::json!({
            "apiKey": api_key.to_string(),
            "streamingEndpoint": "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel".to_string(),
            "streamingResourceId": "volc.seedasr.sauc.duration".to_string(),
            "batchEndpoint": "https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash".to_string(),
            "batchResourceId": "volc.bigasr.auc_turbo".to_string(),
        })
    }

    fn online_request(config: serde_json::Value) -> AsrTranscriptionRequest {
        AsrTranscriptionRequest {
            mode: AsrMode::Batch,
            language: "auto".to_string(),
            enable_itn: true,
            normalization_options: TranscriptNormalizationOptions::default(),
            postprocess_options: TranscriptPostprocessOptions::default(),
            hotwords: None,
            speaker_processing: None,
            engine_config: AsrEngineConfig::Online {
                provider: OnlineAsrProviderRequest {
                    provider_id: sona_core::ports::asr::VOLCENGINE_DOUBAO_PROVIDER_ID.to_string(),
                    profile_id: "volcengine-doubao-default".to_string(),
                    config,
                },
            },
        }
    }

    #[test]
    fn volcengine_config_errors_map_to_sherpa_errors() {
        let missing_key = online_request(config("  "));
        let error = config_from_request(&missing_key, sona_online_asr::VolcengineMode::Batch)
            .expect_err("missing API key should fail before network access");
        assert!(matches!(error, SherpaError::VolcengineApiKeyMissing));

        let mut async_endpoint = config("volc-test-key");
        async_endpoint["batchEndpoint"] =
            serde_json::json!("https://openspeech.bytedance.com/api/v3/auc/bigmodel/submit");
        async_endpoint["batchResourceId"] = serde_json::json!("volc.seedasr.auc");
        let error = config_from_request(
            &online_request(async_endpoint),
            sona_online_asr::VolcengineMode::Batch,
        )
        .expect_err("standard async endpoint requires a public audio URL");
        assert!(matches!(
            error,
            SherpaError::VolcengineLocalFileBatchUnsupported { .. }
        ));
        assert!(error.to_string().contains("recognize/flash"));
    }
}
