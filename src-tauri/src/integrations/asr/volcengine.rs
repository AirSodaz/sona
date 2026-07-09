use super::SherpaError;
use super::metrics::{
    AsrInferenceMetric, current_time_millis, duration_to_ms, log_inference_metric,
    set_batch_inference_metric,
};
use super::transcript::{
    apply_timeline_normalization, build_transcript_update, emit_transcript_update,
};
use super::types::{AsrMode, AsrTranscriptionRequest, TranscriptSegment};
use super::{AsrBatchProcessor, AsrProviderAdapter, AsrState, AsrStreamingSession};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use log::{info, warn};
use sona_core::ports::asr::{OnlineBatchTranscriber, OnlineBatchTranscriptionRequest};
use sona_core::transcription::postprocess::TranscriptPostprocessor;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Notify};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::{HeaderName, HeaderValue};
use tokio_tungstenite::tungstenite::protocol::Message;

#[derive(Clone)]
pub struct VolcengineStreamingSession {
    request: AsrTranscriptionRequest,
    writer: Arc<Mutex<Option<VolcengineWriter>>>,
    flushing: Arc<AtomicBool>,
    final_response_received: Arc<Notify>,
}

type VolcengineWriter = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;

fn config_from_request(
    request: &AsrTranscriptionRequest,
    mode: sona_online_asr::VolcengineMode,
) -> Result<sona_online_asr::VolcengineDoubaoConfigFields, SherpaError> {
    sona_online_asr::resolve_volcengine_config_checked(request, mode).map_err(map_config_error)
}

fn map_config_error(error: sona_online_asr::VolcengineConfigError) -> SherpaError {
    match error {
        sona_online_asr::VolcengineConfigError::ProviderConfigMissing => {
            SherpaError::VolcengineProviderConfigMissing
        }
        sona_online_asr::VolcengineConfigError::UnsupportedProvider { provider_id } => {
            SherpaError::UnsupportedVolcengineProvider { provider_id }
        }
        sona_online_asr::VolcengineConfigError::ApiKeyMissing => {
            SherpaError::VolcengineApiKeyMissing
        }
        sona_online_asr::VolcengineConfigError::StreamingConfigMissing => {
            SherpaError::VolcengineStreamingConfigMissing
        }
        sona_online_asr::VolcengineConfigError::BatchConfigMissing => {
            SherpaError::VolcengineBatchConfigMissing
        }
        sona_online_asr::VolcengineConfigError::LocalFileBatchUnsupported => {
            SherpaError::VolcengineLocalFileBatchUnsupported {
                message: error.to_string(),
            }
        }
        sona_online_asr::VolcengineConfigError::ManifestMissing
        | sona_online_asr::VolcengineConfigError::ManifestDefaultsInvalid => {
            SherpaError::Generic(error.to_string())
        }
    }
}

fn map_server_frame_error(error: sona_online_asr::VolcengineServerFrameError) -> SherpaError {
    match error {
        sona_online_asr::VolcengineServerFrameError::FrameTooShort => {
            SherpaError::VolcengineFrameTooShort
        }
        sona_online_asr::VolcengineServerFrameError::ErrorFrame => {
            SherpaError::VolcengineErrorFrame
        }
        sona_online_asr::VolcengineServerFrameError::ErrorCodeParseFailed => {
            SherpaError::VolcengineErrorCodeParseFailed
        }
        sona_online_asr::VolcengineServerFrameError::ErrorLengthParseFailed => {
            SherpaError::VolcengineErrorLengthParseFailed
        }
        sona_online_asr::VolcengineServerFrameError::ApiError { code, message } => {
            SherpaError::VolcengineApiError { code, message }
        }
        sona_online_asr::VolcengineServerFrameError::PayloadLengthMissing => {
            SherpaError::VolcenginePayloadLengthMissing
        }
        sona_online_asr::VolcengineServerFrameError::PayloadLengthParseFailed => {
            SherpaError::VolcenginePayloadLengthParseFailed
        }
        sona_online_asr::VolcengineServerFrameError::PayloadIncomplete => {
            SherpaError::VolcenginePayloadIncomplete
        }
        sona_online_asr::VolcengineServerFrameError::ResponseParseFailed { error } => {
            SherpaError::VolcengineResponseParseFailed { error }
        }
    }
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
        _instance_id: &str,
        request: &AsrTranscriptionRequest,
    ) -> Result<Option<std::sync::Arc<dyn AsrStreamingSession>>, SherpaError> {
        if request.mode != AsrMode::Streaming {
            return Err(SherpaError::VolcengineRealtimeOnlyForStreaming);
        }
        config_from_request(request, sona_online_asr::VolcengineMode::Streaming)?;
        Ok(Some(std::sync::Arc::new(VolcengineStreamingSession {
            request: request.clone(),
            writer: Arc::new(Mutex::new(None)),
            flushing: Arc::new(AtomicBool::new(false)),
            final_response_received: Arc::new(Notify::new()),
        })))
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

#[async_trait]
impl AsrStreamingSession for VolcengineStreamingSession {
    async fn start(
        &self,
        emitter: std::sync::Arc<dyn crate::platform::event::EventEmitter>,
        _state: &AsrState,
        instance_id: &str,
    ) -> Result<(), SherpaError> {
        start_streaming_recognizer_impl(emitter, self, instance_id).await
    }

    async fn stop(&self, _state: &AsrState, _instance_id: &str) -> Result<(), SherpaError> {
        stop_streaming_recognizer_impl(self).await
    }

    async fn flush(
        &self,
        _emitter: std::sync::Arc<dyn crate::platform::event::EventEmitter>,
        _state: &AsrState,
        _instance_id: &str,
    ) -> Result<(), SherpaError> {
        flush_streaming_recognizer_impl(self).await
    }

    async fn feed_audio_chunk(
        &self,
        _emitter: std::sync::Arc<dyn crate::platform::event::EventEmitter>,
        _state: &AsrState,
        _instance_id: &str,
        samples: Vec<u8>,
    ) -> Result<(), SherpaError> {
        feed_audio_chunk_impl(self, samples).await
    }

    async fn feed_audio_samples(
        &self,
        emitter: std::sync::Arc<dyn crate::platform::event::EventEmitter>,
        _state: &AsrState,
        _instance_id: &str,
        samples: &[f32],
    ) -> Result<(), SherpaError> {
        feed_audio_samples_impl(emitter, self, samples).await
    }
}

async fn start_streaming_recognizer_impl(
    emitter: std::sync::Arc<dyn crate::platform::event::EventEmitter>,
    session: &VolcengineStreamingSession,
    instance_id: &str,
) -> Result<(), SherpaError> {
    let config = config_from_request(&session.request, sona_online_asr::VolcengineMode::Streaming)?;
    let mut client_request = config
        .streaming_endpoint
        .as_str()
        .into_client_request()
        .map_err(|error| SherpaError::VolcengineEndpointInvalid {
            error: error.to_string(),
        })?;
    insert_header(client_request.headers_mut(), "X-Api-Key", &config.api_key)?;
    insert_header(
        client_request.headers_mut(),
        "X-Api-Resource-Id",
        &config.streaming_resource_id,
    )?;
    insert_header(
        client_request.headers_mut(),
        "X-Api-Connect-Id",
        &uuid::Uuid::new_v4().to_string(),
    )?;
    insert_header(client_request.headers_mut(), "X-Api-Sequence", "-1")?;

    let (ws, response) = tokio_tungstenite::connect_async(client_request)
        .await
        .map_err(|error| SherpaError::VolcengineConnectionFailed {
            error: error.to_string(),
        })?;
    if let Some(logid) = response
        .headers()
        .get("X-Tt-Logid")
        .and_then(|value| value.to_str().ok())
    {
        info!("[Volcengine ASR] websocket connected logid={logid}");
    }

    let (mut writer, mut reader) = ws.split();
    let init_frame = sona_online_asr::build_volcengine_full_client_request_frame(
        session.request.enable_itn,
        true,
        &session.request.language,
        session.request.hotwords.as_deref(),
    )
    .map_err(SherpaError::Generic)?;
    writer
        .send(Message::Binary(init_frame))
        .await
        .map_err(|error| SherpaError::VolcengineInitFrameSendFailed {
            error: error.to_string(),
        })?;

    {
        let mut writer_slot = session.writer.lock().await;
        *writer_slot = Some(writer);
    }

    let instance_id_for_task = instance_id.to_string();
    let normalization_options = session.request.normalization_options;
    let postprocessor =
        TranscriptPostprocessor::compile(session.request.postprocess_options.clone())?;
    let flushing = session.flushing.clone();
    let final_response_received = session.final_response_received.clone();
    let emitter_for_task = emitter.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(message) = reader.next().await {
            match message {
                Ok(Message::Binary(frame)) => {
                    let flush_final = flushing.load(Ordering::SeqCst);
                    match sona_online_asr::parse_volcengine_server_response_frame(&frame)
                        .map_err(map_server_frame_error)
                        .and_then(|value| {
                            value
                                .map(|value| {
                                    sona_online_asr::volcengine_streaming_segments_from_response(
                                        &value,
                                        flush_final,
                                    )
                                    .map_err(SherpaError::Generic)
                                })
                                .transpose()
                                .map(|value| value.unwrap_or_default())
                        }) {
                        Ok(segments) if !segments.is_empty() => {
                            let normalized =
                                apply_timeline_normalization(segments, normalization_options);
                            let processed = postprocessor.process_segments(normalized);
                            for segment in processed {
                                let update =
                                    build_transcript_update(segment, normalization_options);
                                emit_transcript_update(
                                    emitter_for_task.as_ref(),
                                    &instance_id_for_task,
                                    &update,
                                    "volcengine_streaming",
                                    None,
                                );
                            }
                        }
                        Ok(_) => {}
                        Err(error) => warn!("[Volcengine ASR] response parse failed: {error}"),
                    }
                    if flush_final {
                        final_response_received.notify_waiters();
                        break;
                    }
                }
                Ok(Message::Close(_)) => break,
                Ok(_) => {}
                Err(error) => {
                    warn!("[Volcengine ASR] websocket read failed: {error}");
                    break;
                }
            }
        }
    });

    Ok(())
}
fn insert_header(
    headers: &mut tokio_tungstenite::tungstenite::http::HeaderMap,
    name: &'static str,
    value: &str,
) -> Result<(), SherpaError> {
    let header_name = HeaderName::from_bytes(name.as_bytes()).map_err(|error| {
        SherpaError::VolcengineEndpointInvalid {
            error: error.to_string(),
        }
    })?;
    headers.insert(
        header_name,
        HeaderValue::from_str(value).map_err(|error| SherpaError::VolcengineEndpointInvalid {
            error: error.to_string(),
        })?,
    );
    Ok(())
}

async fn feed_audio_chunk_impl(
    session: &VolcengineStreamingSession,
    samples: Vec<u8>,
) -> Result<(), SherpaError> {
    let mut writer = session.writer.lock().await;
    let Some(writer) = writer.as_mut() else {
        return Err(SherpaError::VolcengineWebSocketNotConnected);
    };
    writer
        .send(Message::Binary(
            sona_online_asr::build_volcengine_audio_frame(&samples, false),
        ))
        .await
        .map_err(|error| SherpaError::VolcengineAudioSendFailed {
            error: error.to_string(),
        })?;
    Ok(())
}

/// Feed f32 audio samples from the hardware capture worker to a Volcengine
/// streaming session. Converts f32 → i16 PCM bytes and sends via WebSocket.
async fn feed_audio_samples_impl(
    _emitter: std::sync::Arc<dyn crate::platform::event::EventEmitter>,
    session: &VolcengineStreamingSession,
    samples: &[f32],
) -> Result<(), SherpaError> {
    let mut writer_guard = session.writer.lock().await;
    let Some(writer) = writer_guard.as_mut() else {
        // WebSocket not yet connected — silently skip. The session may still
        // be in the "preparing" phase before start_recognizer connects.
        return Ok(());
    };
    let pcm_bytes = sona_online_asr::f32_samples_to_i16_pcm_bytes(samples);
    writer
        .send(Message::Binary(
            sona_online_asr::build_volcengine_audio_frame(&pcm_bytes, false),
        ))
        .await
        .map_err(|error| SherpaError::VolcengineAudioSendFailed {
            error: error.to_string(),
        })?;
    Ok(())
}

async fn flush_streaming_recognizer_impl(
    session: &VolcengineStreamingSession,
) -> Result<(), SherpaError> {
    let mut writer = session.writer.lock().await;
    if let Some(writer) = writer.as_mut() {
        session.flushing.store(true, Ordering::SeqCst);
        writer
            .send(Message::Binary(
                sona_online_asr::build_volcengine_audio_frame(&[], true),
            ))
            .await
            .map_err(|error| SherpaError::VolcengineEndFrameSendFailed {
                error: error.to_string(),
            })?;
        let _ = tokio::time::timeout(
            Duration::from_millis(1500),
            session.final_response_received.notified(),
        )
        .await;
    }
    Ok(())
}

async fn stop_streaming_recognizer_impl(
    session: &VolcengineStreamingSession,
) -> Result<(), SherpaError> {
    let session = Some(session);
    if let Some(session) = session {
        let mut writer = session.writer.lock().await;
        if let Some(mut writer) = writer.take() {
            let _ = writer.send(Message::Close(None)).await;
        }
    }
    Ok(())
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
    use super::super::types::{TranscriptNormalizationOptions, TranscriptPostprocessOptions};
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
            engine_config: crate::integrations::asr::types::AsrEngineConfig::Online {
                provider: crate::integrations::asr::types::OnlineAsrProviderRequest {
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

    #[test]
    fn volcengine_streaming_frame_builders_use_expected_binary_header() {
        let request_frame =
            sona_online_asr::build_volcengine_full_client_request_frame(true, true, "auto", None)
                .expect("request frame");
        let audio_frame = sona_online_asr::build_volcengine_audio_frame(&[1, 2, 3, 4], false);
        let final_audio_frame = sona_online_asr::build_volcengine_audio_frame(&[], true);

        assert_eq!(&request_frame[0..4], &[0x11, 0x10, 0x10, 0x00]);
        assert_eq!(
            u32::from_be_bytes(request_frame[4..8].try_into().unwrap()) as usize,
            request_frame.len() - 8
        );
        assert!(
            String::from_utf8_lossy(&request_frame[8..]).contains("\"model_name\":\"bigmodel\"")
        );
        assert_eq!(&audio_frame[0..4], &[0x11, 0x20, 0x00, 0x00]);
        assert_eq!(u32::from_be_bytes(audio_frame[4..8].try_into().unwrap()), 4);
        assert_eq!(&audio_frame[8..], &[1, 2, 3, 4]);
        assert_eq!(&final_audio_frame[0..4], &[0x11, 0x22, 0x00, 0x00]);
    }

    #[test]
    fn volcengine_streaming_final_frame_defaults_missing_definite_to_final() {
        let response = serde_json::json!({
            "result": {
                "text": "tail sentence",
                "utterances": [
                    {
                        "end_time": 2400,
                        "start_time": 1200,
                        "text": "tail sentence"
                    }
                ]
            }
        });

        let segments =
            sona_online_asr::volcengine_streaming_segments_from_response(&response, true)
                .expect("final streaming frame should map");

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "tail sentence");
        assert!(segments[0].is_final);
    }

    #[test]
    fn adapter_pcm_bytes_convert_f32_samples_correctly() {
        let samples = [0.0_f32, 1.0, -1.0, 0.5];
        let bytes = sona_online_asr::f32_samples_to_i16_pcm_bytes(&samples);

        assert_eq!(bytes.len(), 8);
        let value_0 = i16::from_le_bytes([bytes[0], bytes[1]]);
        let value_1 = i16::from_le_bytes([bytes[2], bytes[3]]);
        let value_2 = i16::from_le_bytes([bytes[4], bytes[5]]);
        let value_3 = i16::from_le_bytes([bytes[6], bytes[7]]);

        assert_eq!(value_0, 0);
        assert_eq!(value_1, i16::MAX);
        assert_eq!(value_2, -i16::MAX);
        assert_eq!(value_3, (0.5 * i16::MAX as f32) as i16);
    }

    #[test]
    fn adapter_pcm_bytes_clamp_out_of_range_samples() {
        let samples = [2.0_f32, -3.0];
        let bytes = sona_online_asr::f32_samples_to_i16_pcm_bytes(&samples);

        let value_0 = i16::from_le_bytes([bytes[0], bytes[1]]);
        let value_1 = i16::from_le_bytes([bytes[2], bytes[3]]);

        assert_eq!(value_0, i16::MAX);
        assert_eq!(value_1, -i16::MAX);
    }
}
