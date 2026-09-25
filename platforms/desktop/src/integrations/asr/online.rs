use super::metrics::{
    AsrInferenceMetric, current_time_millis, duration_to_ms, log_inference_metric,
    set_batch_inference_metric,
};
use super::transcript::apply_timeline_normalization;
use super::types::{AsrTranscriptionRequest, TranscriptSegment};
use super::{AsrBatchProcessor, AsrPortError, AsrProviderAdapter, AsrState};
use async_trait::async_trait;
use sona_core::ports::asr::{AsrPortErrorKind, OnlineBatchTranscriptionRequest};
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
    ) -> Result<Option<Arc<dyn AsrBatchProcessor>>, AsrPortError> {
        let provider_id = sona_online_asr::resolve_online_asr_provider_id(request)?;
        if provider_id != self.provider_id {
            return Err(AsrPortError::new(
                AsrPortErrorKind::Unsupported,
                format!("不支持的在线 ASR provider：{provider_id}"),
            )
            .with_code("UNSUPPORTED_ONLINE_PROVIDER"));
        }
        Ok(Some(Arc::new(OnlineBatchProcessor)))
    }
}

struct OnlineBatchProcessor;

#[async_trait]
impl AsrBatchProcessor for OnlineBatchProcessor {
    async fn process_file(
        &self,
        emitter: Arc<dyn crate::platform::event::EventEmitterPort>,
        state: &AsrState,
        file_path: std::path::PathBuf,
        _save_to_path: Option<std::path::PathBuf>,
        request: AsrTranscriptionRequest,
        speaker_processing: Option<sona_core::transcription::speaker::SpeakerProcessingConfig>,
        instance_id: Option<String>,
    ) -> Result<Vec<TranscriptSegment>, AsrPortError> {
        let cancel_rx = if let Some(id) = &instance_id {
            Some((id.clone(), state.batch_cancel.register(id).await))
        } else {
            None
        };
        let started = Instant::now();
        let transcribe_fut =
            sona_online_asr::OnlineAsrAdapter.transcribe_batch(OnlineBatchTranscriptionRequest {
                file_path: file_path.clone(),
                request: request.clone(),
            });

        let output = if let Some((id, mut rx)) = cancel_rx {
            let result = tokio::select! {
                result = transcribe_fut => result,
                _ = async {
                    loop {
                        if *rx.borrow() {
                            break;
                        }
                        if rx.changed().await.is_err() {
                            break;
                        }
                        if *rx.borrow() {
                            break;
                        }
                    }
                } => Err(AsrPortError::runtime("Task cancelled.")),
            };
            state.batch_cancel.remove(&id).await;
            result?
        } else {
            transcribe_fut.await?
        };
        let mut segments =
            apply_timeline_normalization(output.segments, request.normalization_options);
        segments = TranscriptPostprocessor::compile(request.postprocess_options.clone())
            .map_err(|error| AsrPortError::invalid_request(error.to_string()))?
            .process_segments(segments);

        let effective_speaker_processing =
            speaker_processing.or(request.speaker_processing.clone());
        if let Some(sp_config) = effective_speaker_processing.as_ref()
            && sp_config
                .speaker_embedding_model_path
                .as_deref()
                .is_some_and(|p| !p.trim().is_empty())
            && sona_online_asr::is_cloud_speaker_diarization_enabled(&request)
        {
            segments =
                sona_sherpa_onnx::speaker_processing::match_cloud_speaker_segments_from_file(
                    &file_path,
                    segments,
                    Some(sp_config),
                )
                .await?;
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use sona_core::ports::asr::{
        AsrEngineConfig, AsrMode, OnlineAsrProviderRequest, TranscriptNormalizationOptions,
        TranscriptPostprocessOptions, VOLCENGINE_DOUBAO_PROVIDER_ID,
    };

    fn make_online_request(provider_id: &str, diarization: bool) -> AsrTranscriptionRequest {
        AsrTranscriptionRequest {
            mode: AsrMode::Batch,
            language: "zh".into(),
            enable_itn: true,
            normalization_options: TranscriptNormalizationOptions::default(),
            postprocess_options: TranscriptPostprocessOptions::default(),
            hotwords: None,
            speaker_processing: None,
            engine_config: AsrEngineConfig::Online {
                provider: OnlineAsrProviderRequest {
                    provider_id: provider_id.into(),
                    profile_id: "test".into(),
                    config: json!({
                        "speakerDiarization": diarization,
                    }),
                },
            },
        }
    }

    #[test]
    fn creates_batch_processor_for_matching_provider() {
        let adapter = DesktopOnlineAsrAdapter::new(VOLCENGINE_DOUBAO_PROVIDER_ID);
        let request = make_online_request(VOLCENGINE_DOUBAO_PROVIDER_ID, true);
        assert!(adapter.create_batch_processor(&request).is_ok());

        let wrong_request = make_online_request("deepgram", true);
        assert!(adapter.create_batch_processor(&wrong_request).is_err());
    }

    #[test]
    fn cloud_speaker_diarization_flag_is_respected() {
        let req_enabled = make_online_request(VOLCENGINE_DOUBAO_PROVIDER_ID, true);
        assert!(sona_online_asr::is_cloud_speaker_diarization_enabled(
            &req_enabled
        ));

        let req_disabled = make_online_request(VOLCENGINE_DOUBAO_PROVIDER_ID, false);
        assert!(!sona_online_asr::is_cloud_speaker_diarization_enabled(
            &req_disabled
        ));
    }
}
