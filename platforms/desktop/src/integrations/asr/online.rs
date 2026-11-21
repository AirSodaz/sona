use super::metrics::{
    AsrInferenceMetric, current_time_millis, duration_to_ms, log_inference_metric,
    set_batch_inference_metric,
};
use super::transcript::apply_timeline_normalization;
use super::types::{AsrTranscriptionRequest, TranscriptSegment};
use super::{AsrBatchProcessor, AsrPortError, AsrProviderAdapter, AsrState};
use async_trait::async_trait;
use sona_core::ports::asr::{AsrPortErrorKind, OnlineBatchTranscriptionRequest};
use sona_core::runtime::logging::diagnostic_label;
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
        Ok(Some(Arc::new(OnlineBatchProcessor {
            provider_id: self.provider_id,
        })))
    }
}

struct OnlineBatchProcessor {
    provider_id: &'static str,
}

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
        let cancel_registration = if let Some(id) = &instance_id {
            let (rx, guard) = state.batch_cancel.register(id).await;
            Some((id.clone(), rx, guard))
        } else {
            None
        };
        let started = Instant::now();
        let diagnostic_id = diagnostic_label(instance_id.as_deref());
        log::info!(
            "[OnlineASR] task_start instance_id={diagnostic_id} provider={}",
            self.provider_id
        );
        let transcribe_fut =
            sona_online_asr::OnlineAsrAdapter.transcribe_batch(OnlineBatchTranscriptionRequest {
                file_path: file_path.clone(),
                request: request.clone(),
            });

        let result = if let Some((_id, mut rx, guard)) = cancel_registration {
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
                } => Err(AsrPortError::runtime("Task cancelled.").with_code("TASK_CANCELLED")),
            };
            drop(guard);
            result
        } else {
            transcribe_fut.await
        };
        let output = match result {
            Ok(output) => output,
            Err(error) => {
                if error.code() == "TASK_CANCELLED" {
                    log::info!(
                        "[OnlineASR] cancelled instance_id={diagnostic_id} provider={}",
                        self.provider_id
                    );
                } else {
                    log::error!(
                        "[OnlineASR] task_failed instance_id={diagnostic_id} provider={} error={}",
                        self.provider_id,
                        sona_core::runtime::logging::sanitize_log_text(&error.to_string())
                    );
                }
                return Err(error);
            }
        };
        let raw_segment_count = output.segments.len();
        let mut segments =
            apply_timeline_normalization(output.segments, request.normalization_options);
        segments = TranscriptPostprocessor::compile(request.postprocess_options.clone())
            .map_err(|error| {
                log::error!(
                    "[OnlineASR] task_failed instance_id={diagnostic_id} stage=postprocess error={}",
                    sona_core::runtime::logging::sanitize_log_text(&error.to_string())
                );
                AsrPortError::invalid_request(error.to_string())
            })?
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
            if !sona_core::history::is_network_url(&file_path.to_string_lossy()) {
                segments =
                    sona_sherpa_onnx::speaker_processing::match_cloud_speaker_segments_from_file(
                        &file_path,
                        segments,
                        Some(sp_config),
                    )
                    .await
                    .inspect_err(|error| {
                        log::error!(
                            "[OnlineASR] task_failed instance_id={diagnostic_id} stage=speaker_match error={}",
                            sona_core::runtime::logging::sanitize_log_text(&error.to_string())
                        );
                    })?;
            } else {
                log::warn!(
                    "[OnlineASR] speaker_profile_match_skipped instance_id={diagnostic_id} reason=network_audio_source"
                );
            }
        }

        let elapsed_ms = duration_to_ms(started.elapsed());
        let metric = AsrInferenceMetric {
            occurred_at_ms: current_time_millis(),
            source: "batch".to_string(),
            instance_id: instance_id.clone(),
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
        log::info!(
            "[OnlineASR] task_complete instance_id={diagnostic_id} provider={} raw_segments={} final_segments={} elapsed_ms={elapsed_ms}",
            self.provider_id,
            raw_segment_count,
            segments.len()
        );

        let _ = emitter.emit(
            super::BATCH_PROGRESS_EVENT,
            serde_json::json!([
                file_path.to_string_lossy().as_ref(),
                100.0_f32,
                &instance_id
            ]),
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
