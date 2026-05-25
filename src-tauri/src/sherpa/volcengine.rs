use super::metrics::{
    current_time_millis, duration_to_ms, log_inference_metric, set_batch_inference_metric,
    AsrInferenceMetric,
};
use super::state::SherpaState;
use super::transcript::{apply_timeline_normalization, build_transcript_update, emit_transcript_update};
use super::types::{
    AsrMode, AsrTranscriptionRequest, TranscriptSegment, TranscriptTiming, TranscriptTimingLevel,
    TranscriptTimingSource, TranscriptTimingUnit, VolcengineDoubaoAsrConfig,
};
use super::TranscriptPostprocessor;
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use log::{info, warn};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::{HeaderName, HeaderValue};
use tokio_tungstenite::tungstenite::protocol::Message;

const STREAMING_ENDPOINT_DEFAULT: &str =
    "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async";
const STREAMING_RESOURCE_DEFAULT: &str = "volc.seedasr.sauc.duration";
const BATCH_ENDPOINT_DEFAULT: &str =
    "https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash";
const BATCH_RESOURCE_DEFAULT: &str = "volc.bigasr.auc_turbo";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VolcengineMode {
    Streaming,
    Batch,
}

#[derive(Clone)]
pub struct VolcengineStreamingSession {
    request: AsrTranscriptionRequest,
    writer: Arc<Mutex<Option<VolcengineWriter>>>,
}

type VolcengineWriter = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;

fn normalized_config(config: &VolcengineDoubaoAsrConfig) -> VolcengineDoubaoAsrConfig {
    VolcengineDoubaoAsrConfig {
        api_key: config.api_key.trim().to_string(),
        streaming_endpoint: non_empty_or_default(&config.streaming_endpoint, STREAMING_ENDPOINT_DEFAULT),
        streaming_resource_id: non_empty_or_default(&config.streaming_resource_id, STREAMING_RESOURCE_DEFAULT),
        batch_endpoint: non_empty_or_default(&config.batch_endpoint, BATCH_ENDPOINT_DEFAULT),
        batch_resource_id: non_empty_or_default(&config.batch_resource_id, BATCH_RESOURCE_DEFAULT),
    }
}

fn non_empty_or_default(value: &str, default: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        default.to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn validate_config(
    config: &VolcengineDoubaoAsrConfig,
    mode: VolcengineMode,
) -> Result<VolcengineDoubaoAsrConfig, String> {
    let config = normalized_config(config);
    if config.api_key.is_empty() {
        return Err("火山 ASR API Key 未配置。".to_string());
    }
    match mode {
        VolcengineMode::Streaming => {
            if config.streaming_endpoint.is_empty() || config.streaming_resource_id.is_empty() {
                return Err("火山实时 ASR endpoint 或 Resource ID 未配置。".to_string());
            }
        }
        VolcengineMode::Batch => {
            if config.batch_endpoint.is_empty() || config.batch_resource_id.is_empty() {
                return Err("火山批量 ASR endpoint 或 Resource ID 未配置。".to_string());
            }
        }
    }
    Ok(config)
}

fn config_from_request(
    request: &AsrTranscriptionRequest,
    mode: VolcengineMode,
) -> Result<VolcengineDoubaoAsrConfig, String> {
    let Some(config) = request.volcengine.as_ref() else {
        return Err("火山 ASR provider 配置缺失。".to_string());
    };
    validate_config(config, mode)
}

pub fn build_full_client_request_frame(
    enable_itn: bool,
    enable_punc: bool,
    language: &str,
    hotwords: Option<&str>,
) -> Result<Vec<u8>, String> {
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

    let payload = serde_json::to_vec(&request).map_err(|error| error.to_string())?;
    Ok(build_frame(0x10, 0x10, &payload))
}

pub fn build_audio_frame(samples: &[u8], is_final: bool) -> Vec<u8> {
    let flags = if is_final { 0x22 } else { 0x20 };
    build_frame(flags, 0x00, samples)
}

fn build_frame(message_type_and_flags: u8, serialization_and_compression: u8, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(8 + payload.len());
    frame.extend_from_slice(&[0x11, message_type_and_flags, serialization_and_compression, 0x00]);
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(payload);
    frame
}

fn parse_server_response_frame(frame: &[u8]) -> Result<Option<Value>, String> {
    if frame.len() < 8 {
        return Err("火山 ASR 响应帧过短。".to_string());
    }
    let message_type = frame[1] >> 4;
    if message_type == 0x0f {
        if frame.len() < 12 {
            return Err("火山 ASR 返回错误帧。".to_string());
        }
        let code = u32::from_be_bytes(frame[4..8].try_into().map_err(|_| "火山错误码解析失败。")?);
        let size = u32::from_be_bytes(frame[8..12].try_into().map_err(|_| "火山错误长度解析失败。")?) as usize;
        let message = frame
            .get(12..12 + size)
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .unwrap_or("未知错误");
        return Err(format!("火山 ASR 返回错误：{code} {message}"));
    }
    if message_type != 0x09 {
        return Ok(None);
    }

    let header_size = ((frame[0] & 0x0f) as usize) * 4;
    let offset = header_size + 4;
    if frame.len() < offset + 4 {
        return Err("火山 ASR 响应 payload 长度缺失。".to_string());
    }
    let payload_size = u32::from_be_bytes(
        frame[offset..offset + 4]
            .try_into()
            .map_err(|_| "火山 ASR 响应 payload 长度解析失败。")?,
    ) as usize;
    let payload_start = offset + 4;
    let Some(payload) = frame.get(payload_start..payload_start + payload_size) else {
        return Err("火山 ASR 响应 payload 不完整。".to_string());
    };
    serde_json::from_slice(payload)
        .map(Some)
        .map_err(|error| format!("火山 ASR 响应解析失败：{error}"))
}

pub fn segments_from_response_value(
    response: &Value,
    default_final: bool,
    id_prefix: &str,
) -> Result<Vec<TranscriptSegment>, String> {
    let result = response.get("result").unwrap_or(response);
    let utterances = result.get("utterances").and_then(Value::as_array);
    if let Some(utterances) = utterances {
        let mut segments = Vec::new();
        for (index, utterance) in utterances.iter().enumerate() {
            if let Some(segment) = segment_from_utterance(utterance, index, default_final, id_prefix) {
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
            let word_end = ms_value(word.get("end_time")).unwrap_or(word_start).max(word_start);
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

pub fn map_status_error(
    status: u16,
    api_code: Option<&str>,
    api_message: Option<&str>,
) -> String {
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

pub async fn init_streaming_recognizer_impl(
    state: State<'_, SherpaState>,
    instance_id: String,
    request: AsrTranscriptionRequest,
) -> Result<(), String> {
    if request.mode != AsrMode::Streaming {
        return Err("火山实时 ASR 只能用于 streaming 槽位。".to_string());
    }
    config_from_request(&request, VolcengineMode::Streaming)?;
    let mut sessions = state.volcengine_sessions.lock().await;
    sessions.insert(
        instance_id,
        VolcengineStreamingSession {
            request,
            writer: Arc::new(Mutex::new(None)),
        },
    );
    Ok(())
}

pub async fn start_streaming_recognizer_impl<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, SherpaState>,
    instance_id: String,
) -> Result<(), String> {
    let session = {
        let sessions = state.volcengine_sessions.lock().await;
        sessions
            .get(&instance_id)
            .cloned()
            .ok_or_else(|| "火山 ASR session 未初始化。".to_string())?
    };
    let config = config_from_request(&session.request, VolcengineMode::Streaming)?;
    let mut client_request = config
        .streaming_endpoint
        .as_str()
        .into_client_request()
        .map_err(|error| format!("火山 ASR WebSocket endpoint 无效：{error}"))?;
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
        .map_err(|error| format!("火山 ASR WebSocket 连接失败：{error}"))?;
    if let Some(logid) = response.headers().get("X-Tt-Logid").and_then(|value| value.to_str().ok()) {
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
        .map_err(|error| format!("火山 ASR 初始化帧发送失败：{error}"))?;

    {
        let mut writer_slot = session.writer.lock().await;
        *writer_slot = Some(writer);
    }

    let instance_id_for_task = instance_id.clone();
    let normalization_options = session.request.normalization_options;
    let postprocessor = TranscriptPostprocessor::compile(session.request.postprocess_options.clone())?;
    tauri::async_runtime::spawn(async move {
        while let Some(message) = reader.next().await {
            match message {
                Ok(Message::Binary(frame)) => {
                    match parse_server_response_frame(&frame)
                        .and_then(|value| {
                            value
                                .map(|value| segments_from_response_value(&value, false, "volc-live"))
                                .transpose()
                                .map(|value| value.unwrap_or_default())
                        }) {
                        Ok(segments) if !segments.is_empty() => {
                            let normalized = apply_timeline_normalization(segments, normalization_options);
                            let processed = postprocessor.process_segments(normalized);
                            for segment in processed {
                                let update = build_transcript_update(segment, normalization_options);
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
) -> Result<(), String> {
    let header_name = HeaderName::from_bytes(name.as_bytes())
        .map_err(|error| format!("火山 ASR header 名称无效：{error}"))?;
    headers.insert(
        header_name,
        HeaderValue::from_str(value).map_err(|error| format!("火山 ASR header 无效：{error}"))?,
    );
    Ok(())
}

pub async fn feed_audio_chunk_impl<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, SherpaState>,
    instance_id: String,
    samples: Vec<u8>,
) -> Result<(), String> {
    let session = {
        let sessions = state.volcengine_sessions.lock().await;
        sessions
            .get(&instance_id)
            .cloned()
            .ok_or_else(|| "火山 ASR session 未初始化。".to_string())?
    };
    let mut writer = session.writer.lock().await;
    let Some(writer) = writer.as_mut() else {
        return Err("火山 ASR WebSocket 尚未连接。".to_string());
    };
    writer
        .send(Message::Binary(build_audio_frame(&samples, false)))
        .await
        .map_err(|error| format!("火山 ASR 音频发送失败：{error}"))?;
    let _ = app;
    Ok(())
}

pub async fn flush_streaming_recognizer_impl<R: tauri::Runtime>(
    _app: AppHandle<R>,
    state: State<'_, SherpaState>,
    instance_id: String,
) -> Result<(), String> {
    let session = {
        let sessions = state.volcengine_sessions.lock().await;
        sessions
            .get(&instance_id)
            .cloned()
            .ok_or_else(|| "火山 ASR session 未初始化。".to_string())?
    };
    let mut writer = session.writer.lock().await;
    if let Some(writer) = writer.as_mut() {
        writer
            .send(Message::Binary(build_audio_frame(&[], true)))
            .await
            .map_err(|error| format!("火山 ASR 结束帧发送失败：{error}"))?;
    }
    Ok(())
}

pub async fn stop_streaming_recognizer_impl(
    state: State<'_, SherpaState>,
    instance_id: String,
) -> Result<(), String> {
    let session = {
        let sessions = state.volcengine_sessions.lock().await;
        sessions.get(&instance_id).cloned()
    };
    if let Some(session) = session {
        let mut writer = session.writer.lock().await;
        if let Some(mut writer) = writer.take() {
            let _ = writer.send(Message::Close(None)).await;
        }
    }
    Ok(())
}

pub async fn process_batch_file_impl<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: &SherpaState,
    file_path: String,
    request: AsrTranscriptionRequest,
) -> Result<Vec<TranscriptSegment>, String> {
    if request.mode != AsrMode::Offline {
        return Err("火山批量 ASR 只能用于 offline/batch 槽位。".to_string());
    }
    let config = config_from_request(&request, VolcengineMode::Batch)?;
    let started = Instant::now();
    let bytes = tokio::fs::read(&file_path)
        .await
        .map_err(|error| format!("读取音频文件失败：{error}"))?;
    let audio_data = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let request_id = uuid::Uuid::new_v4().to_string();
    let body = json!({
        "user": { "uid": "sona" },
        "audio": { "data": audio_data },
        "request": {
            "model_name": "bigmodel",
            "enable_itn": request.enable_itn,
            "enable_punc": true,
            "show_utterances": true
        }
    });

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
        .map_err(|error| format!("火山批量 ASR 网络请求失败：{error}"))?;
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
        return Err(map_status_error(
            status.as_u16(),
            api_code.as_deref(),
            api_message.as_deref(),
        ));
    }

    let response_value = response
        .json::<Value>()
        .await
        .map_err(|error| format!("火山批量 ASR 响应解析失败：{error}"))?;
    let mut segments = segments_from_response_value(&response_value, true, "volc-batch")?;
    segments = apply_timeline_normalization(segments, request.normalization_options);
    segments = TranscriptPostprocessor::compile(request.postprocess_options)?.process_segments(segments);

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
    let _ = app.emit(super::BATCH_PROGRESS_EVENT, &(file_path.as_str(), 100.0_f32));
    Ok(segments)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(api_key: &str) -> VolcengineDoubaoAsrConfig {
        VolcengineDoubaoAsrConfig {
            api_key: api_key.to_string(),
            streaming_endpoint: "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async"
                .to_string(),
            streaming_resource_id: "volc.seedasr.sauc.duration".to_string(),
            batch_endpoint:
                "https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash".to_string(),
            batch_resource_id: "volc.bigasr.auc_turbo".to_string(),
        }
    }

    #[test]
    fn volcengine_config_requires_api_key() {
        let error = validate_config(&config("  "), VolcengineMode::Batch)
            .expect_err("missing API key should fail before network access");

        assert!(error.contains("API Key"));
    }

    #[test]
    fn volcengine_streaming_frame_builders_use_expected_binary_header() {
        let request_frame = build_full_client_request_frame(true, true, "auto", None)
            .expect("request frame");
        let audio_frame = build_audio_frame(&[1, 2, 3, 4], false);
        let final_audio_frame = build_audio_frame(&[], true);

        assert_eq!(&request_frame[0..4], &[0x11, 0x10, 0x10, 0x00]);
        assert_eq!(u32::from_be_bytes(request_frame[4..8].try_into().unwrap()) as usize, request_frame.len() - 8);
        assert!(String::from_utf8_lossy(&request_frame[8..]).contains("\"model_name\":\"bigmodel\""));
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

        let segments = segments_from_response_value(&response, true, "volc")
            .expect("segments should map");

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].id, "volc-0");
        assert_eq!(segments[0].text, "关闭透传。");
        assert_eq!(segments[0].start, 0.45);
        assert_eq!(segments[0].end, 1.53);
        assert!(segments[0].is_final);
        assert_eq!(segments[0].tokens.as_ref().unwrap(), &vec!["关", "闭", "透", "传"]);
        assert_eq!(segments[0].timestamps.as_ref().unwrap(), &vec![0.45, 0.77, 1.13, 1.49]);
        assert_eq!(segments[0].durations.as_ref().unwrap(), &vec![0.32, 0.2, 0.08, 0.04]);
        let timing = segments[0].timing.as_ref().unwrap();
        assert_eq!(timing.level, TranscriptTimingLevel::Token);
        assert_eq!(timing.source, TranscriptTimingSource::Model);
        assert_eq!(timing.units[0].text, "关");
    }

    #[test]
    fn volcengine_error_codes_are_mapped_to_user_readable_messages() {
        let auth = map_status_error(401, Some("45000001"), Some("auth failed"));
        let quota = map_status_error(429, Some("55000031"), Some("busy"));

        assert!(auth.contains("鉴权"));
        assert!(quota.contains("限流"));
    }
}
