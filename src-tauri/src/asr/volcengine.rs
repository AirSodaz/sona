use super::error::SherpaError;
use super::metrics::{
    AsrInferenceMetric, current_time_millis, duration_to_ms, log_inference_metric,
    set_batch_inference_metric,
};
use super::state::AsrState;
use super::traits::{AsrBatchProcessor, AsrProviderAdapter, AsrStreamingSession};
use super::transcript::{
    apply_timeline_normalization, build_transcript_update, emit_transcript_update,
};
use super::types::{
    AsrMode, AsrTranscriptionRequest, TranscriptSegment, TranscriptTiming, TranscriptTimingLevel,
    TranscriptTimingSource, TranscriptTimingUnit,
};
use crate::asr::postprocess::TranscriptPostprocessor;
use async_trait::async_trait;
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use log::{info, warn};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::sync::{Mutex, Notify};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::{HeaderName, HeaderValue};
use tokio_tungstenite::tungstenite::protocol::Message;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VolcengineMode {
    Streaming,
    Batch,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolcengineDoubaoConfigFields {
    pub api_key: String,
    pub streaming_endpoint: String,
    pub streaming_resource_id: String,
    pub batch_endpoint: String,
    pub batch_resource_id: String,
}

fn get_string(config: &Value, key: &str, default_val: &str) -> String {
    config
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| default_val.to_string())
}

fn config_fields(request_config: &Value) -> VolcengineDoubaoConfigFields {
    let manifest = crate::asr_providers::find_online_asr_provider(
        crate::asr_providers::VOLCENGINE_DOUBAO_PROVIDER_ID,
    )
    .expect("Volcengine Doubao provider not found in manifest");
    let defaults = manifest.defaults.as_object().unwrap();

    VolcengineDoubaoConfigFields {
        api_key: get_string(
            request_config,
            "apiKey",
            defaults.get("apiKey").and_then(Value::as_str).unwrap_or(""),
        ),
        streaming_endpoint: get_string(
            request_config,
            "streamingEndpoint",
            defaults
                .get("streamingEndpoint")
                .and_then(Value::as_str)
                .unwrap_or(""),
        ),
        streaming_resource_id: get_string(
            request_config,
            "streamingResourceId",
            defaults
                .get("streamingResourceId")
                .and_then(Value::as_str)
                .unwrap_or(""),
        ),
        batch_endpoint: get_string(
            request_config,
            "batchEndpoint",
            defaults
                .get("batchEndpoint")
                .and_then(Value::as_str)
                .unwrap_or(""),
        ),
        batch_resource_id: get_string(
            request_config,
            "batchResourceId",
            defaults
                .get("batchResourceId")
                .and_then(Value::as_str)
                .unwrap_or(""),
        ),
    }
}

fn detect_audio_format(file_path: &str) -> &'static str {
    let path = std::path::Path::new(file_path);
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("mp3") => "mp3",
        Some("wav") => "wav",
        Some("pcm") => "pcm",
        Some("ogg") | Some("oga") => "ogg",
        Some("m4a") => "m4a",
        Some("aac") => "aac",
        Some("flac") => "flac",
        Some("wma") => "wma",
        Some("amr") => "amr",
        Some("opus") => "opus",
        Some("webm") => "webm",
        // Default to wav for unknown extensions; the API will attempt
        // auto-detection from the file header bytes.
        _ => "wav",
    }
}

pub fn validate_config(
    request_config: &Value,
    mode: VolcengineMode,
) -> Result<VolcengineDoubaoConfigFields, SherpaError> {
    let fields = config_fields(request_config);
    if fields.api_key.is_empty() {
        return Err(SherpaError::VolcengineApiKeyMissing);
    }
    match mode {
        VolcengineMode::Streaming => {
            if fields.streaming_endpoint.is_empty() || fields.streaming_resource_id.is_empty() {
                return Err(SherpaError::VolcengineStreamingConfigMissing);
            }
        }
        VolcengineMode::Batch => {
            if fields.batch_endpoint.is_empty() || fields.batch_resource_id.is_empty() {
                return Err(SherpaError::VolcengineBatchConfigMissing);
            }
            if !fields.batch_endpoint.starts_with("https://")
                && !fields.batch_endpoint.starts_with("http://")
            {
                return Err(SherpaError::VolcengineLocalFileBatchUnsupported {
                    message: "火山本地批量导入仅支持极速版 recognize/flash；标准/闲时异步接口需要公网音频 URL，当前不可用于本地文件导入。".to_string(),
                });
            }
            if fields.batch_endpoint.contains("idle/submit")
                || fields.batch_endpoint.ends_with("/submit")
            {
                return Err(SherpaError::VolcengineLocalFileBatchUnsupported {
                    message: "火山本地批量导入仅支持极速版 recognize/flash；标准/闲时异步接口需要公网音频 URL，当前不可用于本地文件导入。".to_string(),
                });
            }
        }
    }
    Ok(fields)
}

fn config_from_request(
    request: &AsrTranscriptionRequest,
    mode: VolcengineMode,
) -> Result<VolcengineDoubaoConfigFields, SherpaError> {
    let provider = if let crate::asr::types::AsrEngineConfig::Online { provider } = &request.engine_config {
        provider
    } else {
        return Err(SherpaError::VolcengineProviderConfigMissing);
    };
    if provider.provider_id != crate::asr_providers::VOLCENGINE_DOUBAO_PROVIDER_ID {
        return Err(SherpaError::UnsupportedVolcengineProvider {
            provider_id: provider.provider_id.clone(),
        });
    }
    validate_config(&provider.config, mode)
}

pub fn build_full_client_request_frame(
    enable_itn: bool,
    enable_punc: bool,
    language: &str,
    hotwords: Option<&str>,
) -> Result<Vec<u8>, SherpaError> {
    let mut request = json!({
        "user": {
            "uid": "sona"
        },
        "audio": {
            "format": "pcm",
            "codec": "raw",
            "rate": 16000,
            "bits": 16,
            "channel": 1
        },
        "request": {
            "model_name": "bigmodel",
            "enable_itn": enable_itn,
            "enable_punc": enable_punc,
            "show_utterances": true,
            "result_type": "full"
        }
    });

    if language != "auto" {
        request["audio"]["language"] = json!(language);
    }
    if let Some(hotwords) = hotwords.filter(|value| !value.trim().is_empty()) {
        request["request"]["corpus"] = json!({
            "context": serde_json::to_string(&json!({
                "hotwords": hotwords
                    .split(',')
                    .map(str::trim)
                    .filter(|word| !word.is_empty())
                    .map(|word| json!({ "word": word }))
                    .collect::<Vec<_>>()
            })).unwrap_or_default()
        });
    }

    let payload = serde_json::to_vec(&request).map_err(|e| SherpaError::Generic(e.to_string()))?;
    Ok(build_frame(0x10, 0x10, &payload))
}

pub fn build_audio_frame(samples: &[u8], is_final: bool) -> Vec<u8> {
    let flags = if is_final { 0x22 } else { 0x20 };
    build_frame(flags, 0x00, samples)
}

fn build_frame(
    message_type_and_flags: u8,
    serialization_and_compression: u8,
    payload: &[u8],
) -> Vec<u8> {
    let mut frame = Vec::with_capacity(8 + payload.len());
    frame.extend_from_slice(&[
        0x11,
        message_type_and_flags,
        serialization_and_compression,
        0x00,
    ]);
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(payload);
    frame
}

fn build_flash_batch_request_body(
    file_path: &str,
    audio_data: String,
    request: &AsrTranscriptionRequest,
) -> Value {
    let audio_format = detect_audio_format(file_path);
    json!({
        "user": { "uid": "sona" },
        "audio": { "format": audio_format, "data": audio_data },
        "request": {
            "model_name": "bigmodel",
            "enable_itn": request.enable_itn,
            "enable_punc": true,
            "show_utterances": true
        }
    })
}

fn parse_server_response_frame(frame: &[u8]) -> Result<Option<Value>, SherpaError> {
    if frame.len() < 8 {
        return Err(SherpaError::VolcengineFrameTooShort);
    }
    let message_type = frame[1] >> 4;
    if message_type == 0x0f {
        if frame.len() < 12 {
            return Err(SherpaError::VolcengineErrorFrame);
        }
        let code = u32::from_be_bytes(
            frame[4..8]
                .try_into()
                .map_err(|_| SherpaError::VolcengineErrorCodeParseFailed)?,
        );
        let size = u32::from_be_bytes(
            frame[8..12]
                .try_into()
                .map_err(|_| SherpaError::VolcengineErrorLengthParseFailed)?,
        ) as usize;
        let message = frame
            .get(12..12 + size)
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .unwrap_or("未知错误");
        return Err(SherpaError::VolcengineApiError {
            code,
            message: message.to_string(),
        });
    }
    if message_type != 0x09 {
        return Ok(None);
    }

    let header_size = ((frame[0] & 0x0f) as usize) * 4;
    let offset = header_size + 4;
    if frame.len() < offset + 4 {
        return Err(SherpaError::VolcenginePayloadLengthMissing);
    }
    let payload_size = u32::from_be_bytes(
        frame[offset..offset + 4]
            .try_into()
            .map_err(|_| SherpaError::VolcenginePayloadLengthParseFailed)?,
    ) as usize;
    let payload_start = offset + 4;
    let Some(payload) = frame.get(payload_start..payload_start + payload_size) else {
        return Err(SherpaError::VolcenginePayloadIncomplete);
    };
    serde_json::from_slice(payload).map(Some).map_err(|error| {
        SherpaError::VolcengineResponseParseFailed {
            error: error.to_string(),
        }
    })
}

pub fn segments_from_response_value(
    response: &Value,
    default_final: bool,
    id_prefix: &str,
) -> Result<Vec<TranscriptSegment>, SherpaError> {
    let result = response.get("result").unwrap_or(response);
    let utterances = result.get("utterances").and_then(Value::as_array);
    if let Some(utterances) = utterances {
        let mut segments = Vec::new();
        for (index, utterance) in utterances.iter().enumerate() {
            if let Some(segment) =
                segment_from_utterance(utterance, index, default_final, id_prefix)
            {
                segments.push(segment);
            }
        }
        if !segments.is_empty() {
            return Ok(segments);
        }
    }

    let text = result
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let duration = response
        .get("audio_info")
        .and_then(|value| value.get("duration"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
        / 1000.0;
    Ok(vec![TranscriptSegment {
        id: format!("{id_prefix}-0"),
        text: text.to_string(),
        start: 0.0,
        end: duration.max(0.0),
        is_final: default_final,
        timing: None,
        tokens: None,
        timestamps: None,
        durations: None,
        translation: None,
        speaker: None,
        speaker_attribution: None,
    }])
}

fn segments_from_streaming_response_value(
    response: &Value,
    flush_final: bool,
) -> Result<Vec<TranscriptSegment>, SherpaError> {
    segments_from_response_value(response, flush_final, "volc-live")
}

fn segment_from_utterance(
    utterance: &Value,
    index: usize,
    default_final: bool,
    id_prefix: &str,
) -> Option<TranscriptSegment> {
    let text = utterance.get("text").and_then(Value::as_str)?.trim();
    if text.is_empty() {
        return None;
    }
    let start = ms_value(utterance.get("start_time")).unwrap_or(0.0);
    let end = ms_value(utterance.get("end_time")).unwrap_or(start);
    let is_final = utterance
        .get("definite")
        .and_then(Value::as_bool)
        .unwrap_or(default_final);

    let words = utterance.get("words").and_then(Value::as_array);
    let mut tokens = Vec::new();
    let mut timestamps = Vec::new();
    let mut durations = Vec::new();
    let mut timing_units = Vec::new();
    if let Some(words) = words {
        for word in words {
            let word_text = word.get("text").and_then(Value::as_str).unwrap_or_default();
            if word_text.is_empty() {
                continue;
            }
            let word_start = ms_value(word.get("start_time")).unwrap_or(start);
            let word_end = ms_value(word.get("end_time"))
                .unwrap_or(word_start)
                .max(word_start);
            tokens.push(word_text.to_string());
            timestamps.push(word_start as f32);
            durations.push((word_end - word_start) as f32);
            timing_units.push(TranscriptTimingUnit {
                text: word_text.to_string(),
                start: word_start,
                end: word_end,
            });
        }
    }

    Some(TranscriptSegment {
        id: format!("{id_prefix}-{index}"),
        text: text.to_string(),
        start,
        end: end.max(start),
        is_final,
        timing: (!timing_units.is_empty()).then_some(TranscriptTiming {
            level: TranscriptTimingLevel::Token,
            source: TranscriptTimingSource::Model,
            units: timing_units,
        }),
        tokens: (!tokens.is_empty()).then_some(tokens),
        timestamps: (!timestamps.is_empty()).then_some(timestamps),
        durations: (!durations.is_empty()).then_some(durations),
        translation: None,
        speaker: None,
        speaker_attribution: None,
    })
}

fn ms_value(value: Option<&Value>) -> Option<f64> {
    value.and_then(Value::as_f64).map(|value| value / 1000.0)
}

pub fn map_status_error(status: u16, api_code: Option<&str>, api_message: Option<&str>) -> String {
    let code = api_code.unwrap_or_default();
    let message = api_message.unwrap_or_default();
    let category = match (status, code) {
        (401 | 403, _) => "鉴权失败",
        (429, _) | (_, "55000031") => "配额或限流",
        (400, _) | (_, "45000001" | "45000002" | "45000151") => "请求配置错误",
        (500..=599, _) => "火山服务错误",
        _ => "火山 ASR 请求失败",
    };
    if code.is_empty() && message.is_empty() {
        format!("{category}（HTTP {status}）")
    } else {
        format!("{category}（HTTP {status}, code {code}: {message}）")
    }
}

pub struct VolcengineAdapter;

#[async_trait]
impl AsrProviderAdapter for VolcengineAdapter {
    fn provider_id(&self) -> &'static str {
        crate::asr_providers::VOLCENGINE_DOUBAO_PROVIDER_ID
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
        config_from_request(request, VolcengineMode::Streaming)?;
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
        app: AppHandle,
        state: &AsrState,
        file_path: String,
        _save_to_path: Option<String>,
        request: AsrTranscriptionRequest,
        _speaker_processing: Option<crate::speaker::SpeakerProcessingConfig>,
    ) -> Result<Vec<TranscriptSegment>, SherpaError> {
        process_batch_file_impl(app, state, file_path, request).await
    }
}

#[async_trait]
impl AsrStreamingSession for VolcengineStreamingSession {
    async fn start(
        &self,
        app: AppHandle,
        _state: &AsrState,
        instance_id: &str,
    ) -> Result<(), SherpaError> {
        start_streaming_recognizer_impl(app, self, instance_id).await
    }

    async fn stop(&self, _state: &AsrState, _instance_id: &str) -> Result<(), SherpaError> {
        stop_streaming_recognizer_impl(self).await
    }

    async fn flush(
        &self,
        _app: AppHandle,
        _state: &AsrState,
        _instance_id: &str,
    ) -> Result<(), SherpaError> {
        flush_streaming_recognizer_impl(self).await
    }

    async fn feed_audio_chunk(
        &self,
        _app: AppHandle,
        _state: &AsrState,
        _instance_id: &str,
        samples: Vec<u8>,
    ) -> Result<(), SherpaError> {
        feed_audio_chunk_impl(self, samples).await
    }

    async fn feed_audio_samples(
        &self,
        app: AppHandle,
        _state: &AsrState,
        _instance_id: &str,
        samples: &[f32],
    ) -> Result<(), SherpaError> {
        feed_audio_samples_impl(app, self, samples).await
    }
}

async fn start_streaming_recognizer_impl(
    app: AppHandle,
    session: &VolcengineStreamingSession,
    instance_id: &str,
) -> Result<(), SherpaError> {
    let config = config_from_request(&session.request, VolcengineMode::Streaming)?;
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
    let init_frame = build_full_client_request_frame(
        session.request.enable_itn,
        true,
        &session.request.language,
        session.request.hotwords.as_deref(),
    )?;
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
    tauri::async_runtime::spawn(async move {
        while let Some(message) = reader.next().await {
            match message {
                Ok(Message::Binary(frame)) => {
                    let flush_final = flushing.load(Ordering::SeqCst);
                    match parse_server_response_frame(&frame).and_then(|value| {
                        value
                            .map(|value| {
                                segments_from_streaming_response_value(&value, flush_final)
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
                                    &app,
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
        .send(Message::Binary(build_audio_frame(&samples, false)))
        .await
        .map_err(|error| SherpaError::VolcengineAudioSendFailed {
            error: error.to_string(),
        })?;
    Ok(())
}

/// Feed f32 audio samples from the hardware capture worker to a Volcengine
/// streaming session. Converts f32 → i16 PCM bytes and sends via WebSocket.
async fn feed_audio_samples_impl(
    _app: AppHandle,
    session: &VolcengineStreamingSession,
    samples: &[f32],
) -> Result<(), SherpaError> {
    let mut writer_guard = session.writer.lock().await;
    let Some(writer) = writer_guard.as_mut() else {
        // WebSocket not yet connected — silently skip. The session may still
        // be in the "preparing" phase before start_recognizer connects.
        return Ok(());
    };
    let pcm_bytes = f32_samples_to_i16_pcm_bytes(samples);
    writer
        .send(Message::Binary(build_audio_frame(&pcm_bytes, false)))
        .await
        .map_err(|error| SherpaError::VolcengineAudioSendFailed {
            error: error.to_string(),
        })?;
    Ok(())
}

fn f32_samples_to_i16_pcm_bytes(samples: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let value = (clamped * i16::MAX as f32) as i16;
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

async fn flush_streaming_recognizer_impl(
    session: &VolcengineStreamingSession,
) -> Result<(), SherpaError> {
    let mut writer = session.writer.lock().await;
    if let Some(writer) = writer.as_mut() {
        session.flushing.store(true, Ordering::SeqCst);
        writer
            .send(Message::Binary(build_audio_frame(&[], true)))
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
    app: AppHandle,
    state: &AsrState,
    file_path: String,
    request: AsrTranscriptionRequest,
) -> Result<Vec<TranscriptSegment>, SherpaError> {
    if request.mode != AsrMode::Offline {
        return Err(SherpaError::VolcengineBatchOnlyForOffline);
    }
    let config = config_from_request(&request, VolcengineMode::Batch)?;
    let started = Instant::now();
    let bytes =
        tokio::fs::read(&file_path)
            .await
            .map_err(|error| SherpaError::AudioFileReadFailed {
                error: error.to_string(),
            })?;
    let audio_data = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let request_id = uuid::Uuid::new_v4().to_string();
    let body = build_flash_batch_request_body(&file_path, audio_data, &request);

    let client = reqwest::Client::new();
    let response = client
        .post(&config.batch_endpoint)
        .header("X-Api-Key", &config.api_key)
        .header("X-Api-Resource-Id", &config.batch_resource_id)
        .header("X-Api-Request-Id", request_id)
        .header("X-Api-Sequence", "-1")
        .json(&body)
        .send()
        .await
        .map_err(|error| SherpaError::VolcengineBatchRequestFailed {
            error: error.to_string(),
        })?;
    let status = response.status();
    let headers = response.headers().clone();
    let api_code = headers
        .get("X-Api-Status-Code")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let api_message = headers
        .get("X-Api-Message")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    if !status.is_success() || api_code.as_deref().is_some_and(|code| code != "20000000") {
        return Err(SherpaError::VolcengineBatchRequestFailed {
            error: map_status_error(status.as_u16(), api_code.as_deref(), api_message.as_deref())
                .to_string(),
        });
    }

    let response_value = response.json::<Value>().await.map_err(|error| {
        SherpaError::VolcengineBatchResponseParseFailed {
            error: error.to_string(),
        }
    })?;
    let mut segments = segments_from_response_value(&response_value, true, "volc-batch")?;
    segments = apply_timeline_normalization(segments, request.normalization_options);
    segments =
        TranscriptPostprocessor::compile(request.postprocess_options)?.process_segments(segments);

    let metric = AsrInferenceMetric {
        occurred_at_ms: current_time_millis(),
        source: "batch".to_string(),
        instance_id: None,
        stage: "volcengine_batch_complete".to_string(),
        is_final: true,
        audio_duration_ms: response_value
            .get("audio_info")
            .and_then(|value| value.get("duration"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0),
        buffered_samples: bytes.len() / 2,
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
    let _ = app.emit(
        super::BATCH_PROGRESS_EVENT,
        &(file_path.as_str(), 100.0_f32),
    );
    Ok(segments)
}

#[cfg(test)]
mod tests {
    use super::super::types::{
        TranscriptNormalizationOptions,
        TranscriptPostprocessOptions,
    };
    use super::*;

    fn config(api_key: &str) -> Value {
        serde_json::json!({
            "apiKey": api_key.to_string(),
            "streamingEndpoint": "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel".to_string(),
            "streamingResourceId": "volc.seedasr.sauc.duration".to_string(),
            "batchEndpoint": "https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash".to_string(),
            "batchResourceId": "volc.bigasr.auc_turbo".to_string(),
        })
    }

    #[test]
    fn volcengine_config_requires_api_key() {
        let error = validate_config(&config("  "), VolcengineMode::Batch)
            .expect_err("missing API key should fail before network access");

        assert!(error.to_string().contains("API Key"));
    }

    #[test]
    fn volcengine_local_batch_rejects_async_recording_file_endpoints() {
        let mut standard = config("volc-test-key");
        standard["batchEndpoint"] =
            serde_json::json!("https://openspeech.bytedance.com/api/v3/auc/bigmodel/submit");
        standard["batchResourceId"] = serde_json::json!("volc.seedasr.auc");

        let mut offpeak = config("volc-test-key");
        offpeak["batchEndpoint"] =
            serde_json::json!("https://openspeech.bytedance.com/api/v3/auc/bigmodel/idle/submit");
        offpeak["batchResourceId"] = serde_json::json!("volc.bigasr.auc_idle");

        let standard_error = validate_config(&standard, VolcengineMode::Batch)
            .expect_err("standard async endpoint requires a public audio URL");
        let offpeak_error = validate_config(&offpeak, VolcengineMode::Batch)
            .expect_err("off-peak async endpoint requires a public audio URL");

        assert!(standard_error.to_string().contains("本地批量导入"));
        assert!(standard_error.to_string().contains("极速版"));
        assert!(offpeak_error.to_string().contains("公网音频 URL"));
    }

    #[test]
    fn volcengine_flash_batch_request_body_uses_local_audio_data_and_existing_options() {
        let request = AsrTranscriptionRequest {
            mode: AsrMode::Offline,
            language: "auto".to_string(),
            enable_itn: true,
            normalization_options: TranscriptNormalizationOptions::default(),
            postprocess_options: TranscriptPostprocessOptions::default(),
            hotwords: None,
            engine_config: crate::asr::types::AsrEngineConfig::Online {
                provider: crate::asr::types::OnlineAsrProviderRequest {
                    provider_id: crate::asr_providers::VOLCENGINE_DOUBAO_PROVIDER_ID.to_string(),
                    profile_id: "volcengine-doubao-default".to_string(),
                    config: serde_json::to_value(config("volc-test-key")).expect("config json"),
                }
            },
        };

        let body = build_flash_batch_request_body(
            "C:/recordings/meeting.mp3",
            "bG9jYWwtYXVkaW8=".to_string(),
            &request,
        );

        assert_eq!(body["audio"]["format"], "mp3");
        assert_eq!(body["audio"]["data"], "bG9jYWwtYXVkaW8=");
        assert!(body["audio"].get("url").is_none());
        assert_eq!(body["request"]["enable_itn"], true);
        assert_eq!(body["request"]["enable_punc"], true);
        assert_eq!(body["request"]["show_utterances"], true);
    }

    #[test]
    fn volcengine_streaming_frame_builders_use_expected_binary_header() {
        let request_frame =
            build_full_client_request_frame(true, true, "auto", None).expect("request frame");
        let audio_frame = build_audio_frame(&[1, 2, 3, 4], false);
        let final_audio_frame = build_audio_frame(&[], true);

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
    fn volcengine_response_maps_utterances_and_words_to_transcript_segments() {
        let response = serde_json::json!({
            "audio_info": { "duration": 2499 },
            "result": {
                "text": "关闭透传。",
                "utterances": [
                    {
                        "end_time": 1530,
                        "start_time": 450,
                        "text": "关闭透传。",
                        "definite": true,
                        "words": [
                            { "confidence": 0, "end_time": 770, "start_time": 450, "text": "关" },
                            { "confidence": 0, "end_time": 970, "start_time": 770, "text": "闭" },
                            { "confidence": 0, "end_time": 1210, "start_time": 1130, "text": "透" },
                            { "confidence": 0, "end_time": 1530, "start_time": 1490, "text": "传" }
                        ]
                    }
                ]
            }
        });

        let segments =
            segments_from_response_value(&response, true, "volc").expect("segments should map");

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].id, "volc-0");
        assert_eq!(segments[0].text, "关闭透传。");
        assert_eq!(segments[0].start, 0.45);
        assert_eq!(segments[0].end, 1.53);
        assert!(segments[0].is_final);
        assert_eq!(
            segments[0].tokens.as_ref().unwrap(),
            &vec!["关", "闭", "透", "传"]
        );
        assert_eq!(
            segments[0].timestamps.as_ref().unwrap(),
            &vec![0.45, 0.77, 1.13, 1.49]
        );
        assert_eq!(
            segments[0].durations.as_ref().unwrap(),
            &vec![0.32, 0.2, 0.08, 0.04]
        );
        let timing = segments[0].timing.as_ref().unwrap();
        assert_eq!(timing.level, TranscriptTimingLevel::Token);
        assert_eq!(timing.source, TranscriptTimingSource::Model);
        assert_eq!(timing.units[0].text, "关");
    }

    #[test]
    fn volcengine_streaming_final_frame_defaults_missing_definite_to_final() {
        let response = serde_json::json!({
            "result": {
                "text": "尾句保存。",
                "utterances": [
                    {
                        "end_time": 2400,
                        "start_time": 1200,
                        "text": "尾句保存。"
                    }
                ]
            }
        });

        let segments = segments_from_streaming_response_value(&response, true)
            .expect("final streaming frame should map");

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "尾句保存。");
        assert!(segments[0].is_final);
    }

    #[test]
    fn volcengine_error_codes_are_mapped_to_user_readable_messages() {
        let auth = map_status_error(401, Some("45000001"), Some("auth failed"));
        let quota = map_status_error(429, Some("55000031"), Some("busy"));

        assert!(auth.contains("鉴权"));
        assert!(quota.contains("限流"));
    }

    #[test]
    fn detect_audio_format_maps_known_extensions() {
        assert_eq!(detect_audio_format("recording.mp3"), "mp3");
        assert_eq!(detect_audio_format("recording.MP3"), "mp3");
        assert_eq!(detect_audio_format("recording.wav"), "wav");
        assert_eq!(detect_audio_format("recording.flac"), "flac");
        assert_eq!(detect_audio_format("recording.m4a"), "m4a");
        assert_eq!(detect_audio_format("recording.ogg"), "ogg");
        assert_eq!(detect_audio_format("recording.oga"), "ogg");
        assert_eq!(detect_audio_format("recording.aac"), "aac");
        assert_eq!(detect_audio_format("recording.opus"), "opus");
        assert_eq!(detect_audio_format("recording.webm"), "webm");
        assert_eq!(detect_audio_format("recording.pcm"), "pcm");
        assert_eq!(detect_audio_format("recording.amr"), "amr");
        assert_eq!(detect_audio_format("recording.wma"), "wma");
    }

    #[test]
    fn detect_audio_format_defaults_to_wav_for_unknown() {
        assert_eq!(detect_audio_format("recording.xyz"), "wav");
        assert_eq!(detect_audio_format("recording"), "wav");
        assert_eq!(detect_audio_format(""), "wav");
    }

    #[test]
    fn f32_samples_to_i16_pcm_bytes_converts_correctly() {
        let samples = [0.0_f32, 1.0, -1.0, 0.5];
        let bytes = f32_samples_to_i16_pcm_bytes(&samples);

        assert_eq!(bytes.len(), 8); // 4 samples × 2 bytes each
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
    fn f32_samples_to_i16_pcm_bytes_clamps_out_of_range() {
        let samples = [2.0_f32, -3.0];
        let bytes = f32_samples_to_i16_pcm_bytes(&samples);

        let value_0 = i16::from_le_bytes([bytes[0], bytes[1]]);
        let value_1 = i16::from_le_bytes([bytes[2], bytes[3]]);

        assert_eq!(value_0, i16::MAX);
        assert_eq!(value_1, -i16::MAX);
    }
}
