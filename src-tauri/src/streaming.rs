use axum::extract::ws::{Message, WebSocket};
use axum::extract::{ConnectInfo, Query, State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::Response;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio;

use crate::server::ServerState;

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ClientMessage {
    Start {
        model_id: String,
        #[serde(default = "default_language")]
        language: String,
        hotwords: Option<String>,
        #[serde(default = "default_vad_model_id")]
        vad_model_id: String,
    },
    Stop,
}

fn default_language() -> String {
    "auto".to_string()
}
fn default_vad_model_id() -> String {
    "silero-vad".to_string()
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ServerMessage {
    Started {
        session_id: String,
    },
    Segment {
        segment: crate::asr::TranscriptSegment,
    },
    Stopped,
    Error {
        message: String,
    },
}

pub async fn handle_streaming(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<ServerState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, StatusCode> {
    // 1. Check IP whitelist
    let ip = addr.ip();
    let is_whitelisted = state.ip_whitelist.iter().any(|net| net.contains(&ip));
    if !is_whitelisted {
        return Err(StatusCode::FORBIDDEN);
    }

    // 2. Check token if api_key is configured
    if !state.api_key.is_empty() {
        let token = params.get("token").map(|s| s.as_str()).unwrap_or("");
        if token != state.api_key {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }

    // 3. Acquire semaphore for concurrency limit
    let permit = state
        .streaming_semaphore
        .clone()
        .try_acquire_owned()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    Ok(ws.on_upgrade(move |socket| async move {
        handle_streaming_socket(socket, state, permit).await;
    }))
}

use tauri::{Listener, Manager};
use tokio::sync::mpsc;

async fn handle_streaming_socket(
    mut socket: WebSocket,
    state: ServerState,
    _permit: tokio::sync::OwnedSemaphorePermit,
) {
    let session_id = uuid::Uuid::new_v4().to_string();

    // Phase 1: Wait for Start message
    let start_msg = match tokio::time::timeout(std::time::Duration::from_secs(10), socket.recv())
        .await
    {
        Ok(Some(Ok(Message::Text(text)))) => match serde_json::from_str::<ClientMessage>(&text) {
            Ok(ClientMessage::Start {
                model_id,
                language,
                hotwords,
                vad_model_id,
            }) => (model_id, language, hotwords, vad_model_id),
            _ => {
                let _ = socket
                    .send(Message::Text(
                        serde_json::to_string(&ServerMessage::Error {
                            message: "Expected start message".to_string(),
                        })
                        .unwrap()
                        .into(),
                    ))
                    .await;
                return;
            }
        },
        _ => return,
    };

    let (model_id, language, hotwords, vad_model_id) = start_msg;

    if crate::asr_providers::find_online_asr_provider(&model_id).is_some() {
        handle_online_streaming_socket(socket, state, session_id, model_id, language, hotwords)
            .await;
    } else {
        handle_local_streaming_socket(
            socket,
            state,
            session_id,
            model_id,
            language,
            hotwords,
            vad_model_id,
        )
        .await;
    }
}

async fn handle_online_streaming_socket(
    mut socket: WebSocket,
    state: ServerState,
    session_id: String,
    model_id: String,
    language: String,
    hotwords: Option<String>,
) {
    let app_handle = match &state.app {
        Some(app) => app.clone(),
        None => {
            let _ = socket
                .send(Message::Text(
                    serde_json::to_string(&ServerMessage::Error {
                        message: "Cloud ASR streaming requires AppHandle".to_string(),
                    })
                    .unwrap()
                    .into(),
                ))
                .await;
            return;
        }
    };

    let provider = crate::asr_providers::find_online_asr_provider(&model_id).unwrap();
    if !provider.streaming.supported.unwrap_or(true) {
        let _ = socket
            .send(Message::Text(
                serde_json::to_string(&ServerMessage::Error {
                    message: format!("Provider {} does not support streaming", model_id),
                })
                .unwrap()
                .into(),
            ))
            .await;
        return;
    }
    let provider_id = provider.id.clone();

    let config = {
        let configs = state.online_asr_config.read().await;
        configs.get(&provider_id).cloned().unwrap_or_default()
    };

    let request = crate::asr::AsrTranscriptionRequest {
        engine_config: crate::asr::AsrEngineConfig::Online {
            provider: crate::asr::OnlineAsrProviderRequest {
                provider_id,
                profile_id: model_id.clone(),
                config,
            },
        },
        mode: crate::asr::AsrMode::Streaming,
        enable_itn: false,
        language: if language == "auto" {
            "".to_string()
        } else {
            language
        },
        hotwords,
        normalization_options: Default::default(),
        postprocess_options: Default::default(),
    };

    let sherpa_state = app_handle.state::<crate::asr::AsrState>();

    if let Err(e) = crate::asr::online::init_streaming_recognizer_impl(
        sherpa_state.clone(),
        session_id.clone(),
        request,
    )
    .await
    {
        let _ = socket
            .send(Message::Text(
                serde_json::to_string(&ServerMessage::Error {
                    message: e.to_string(),
                })
                .unwrap()
                .into(),
            ))
            .await;
        return;
    }

    if let Err(e) = crate::asr::online::start_streaming_recognizer_impl(
        app_handle.clone(),
        sherpa_state.clone(),
        session_id.clone(),
    )
    .await
    {
        let _ = socket
            .send(Message::Text(
                serde_json::to_string(&ServerMessage::Error {
                    message: e.to_string(),
                })
                .unwrap()
                .into(),
            ))
            .await;
        return;
    }

    let _ = socket
        .send(Message::Text(
            serde_json::to_string(&ServerMessage::Started {
                session_id: session_id.clone(),
            })
            .unwrap()
            .into(),
        ))
        .await;

    let event_name = format!("recognizer-output-{}", session_id);
    let (tx, mut rx) = mpsc::unbounded_channel::<crate::asr::TranscriptUpdate>();

    let handler_id = app_handle.listen(event_name, move |event| {
        if let Ok(update) = serde_json::from_str::<crate::asr::TranscriptUpdate>(event.payload()) {
            let _ = tx.send(update);
        }
    });

    struct CleanupGuard<F: FnMut()> {
        f: F,
    }
    impl<F: FnMut()> Drop for CleanupGuard<F> {
        fn drop(&mut self) {
            (self.f)();
        }
    }
    let _cleanup_guard = CleanupGuard {
        f: {
            let app_handle = app_handle.clone();
            move || {
                app_handle.unlisten(handler_id);
            }
        },
    };

    let mut stopping = false;
    loop {
        tokio::select! {
            msg = socket.recv(), if !stopping => {
                match msg {
                    Some(Ok(Message::Binary(pcm))) => {
                        let samples = pcm.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0).collect::<Vec<f32>>();
                        if let Err(e) = crate::asr::online::feed_audio_samples_impl(
                            app_handle.clone(),
                            sherpa_state.inner(),
                            &session_id,
                            &samples,
                        ).await {
                            log::error!("Error feeding audio samples: {}", e);
                        }
                    }
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(ClientMessage::Stop) = serde_json::from_str::<ClientMessage>(&text) {
                            let _ = crate::asr::online::flush_streaming_recognizer_impl(app_handle.clone(), sherpa_state.clone(), session_id.clone()).await;
                            stopping = true;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
            Some(update) = rx.recv() => {
                for segment in update.upsert_segments {
                    let _ = socket.send(Message::Text(serde_json::to_string(&ServerMessage::Segment { segment }).unwrap().into())).await;
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(500)), if stopping => {
                let _ = socket.send(Message::Text(serde_json::to_string(&ServerMessage::Stopped).unwrap().into())).await;
                break;
            }
            _ = tokio::time::sleep(std::time::Duration::from_secs(60)), if !stopping => {
                let _ = socket.send(Message::Text(serde_json::to_string(&ServerMessage::Error { message: "Idle timeout".to_string() }).unwrap().into())).await;
                break;
            }
        }
    }

    let _ = crate::asr::online::stop_streaming_recognizer_impl(
        sherpa_state.clone(),
        session_id.clone(),
    )
    .await;
}

async fn handle_local_streaming_socket(
    mut socket: WebSocket,
    state: ServerState,
    session_id: String,
    model_id: String,
    language: String,
    hotwords: Option<String>,
    vad_model_id: String,
) {
    // Load models
    let recognizer = match load_recognizer(&state, &model_id, &language, hotwords).await {
        Ok(r) => r,
        Err(e) => {
            let _ = socket
                .send(Message::Text(
                    serde_json::to_string(&ServerMessage::Error { message: e })
                        .unwrap()
                        .into(),
                ))
                .await;
            return;
        }
    };

    let vad = match crate::asr::load_vad(Some(vad_model_id)) {
        Some(v) => v,
        None => {
            let _ = socket
                .send(Message::Text(
                    serde_json::to_string(&ServerMessage::Error {
                        message: "VAD model required but not found".to_string(),
                    })
                    .unwrap()
                    .into(),
                ))
                .await;
            return;
        }
    };

    let _ = socket
        .send(Message::Text(
            serde_json::to_string(&ServerMessage::Started {
                session_id: session_id.clone(),
            })
            .unwrap()
            .into(),
        ))
        .await;

    // Phase 2: Audio streaming
    let mut offline_state = crate::asr::sherpa_onnx::OfflineState::default();
    let mut total_samples = 0;
    let mut current_segment_id: Option<String> = None;
    let mut last_inference_time = std::time::Instant::now();

    loop {
        tokio::select! {
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Binary(pcm))) => {
                        let samples = pcm.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0).collect::<Vec<f32>>();
                        total_samples += samples.len();

                        vad.0.accept_waveform(&samples);
                        let currently_speaking = vad.0.detected();

                        if current_segment_id.is_none() {
                            current_segment_id = Some(uuid::Uuid::new_v4().to_string());
                        }

                        if currently_speaking && !offline_state.is_speaking {
                            offline_state.is_speaking = true;
                            let samples_to_keep = (16000.0 * 0.3) as usize;
                            let mut context_len = 0;
                            if !offline_state.ring_buffer.is_empty() {
                                let ring_flat: Vec<f32> = offline_state.ring_buffer.iter().flatten().copied().collect();
                                let keep_start = ring_flat.len().saturating_sub(samples_to_keep);
                                let context = ring_flat[keep_start..].to_vec();
                                context_len = context.len();
                                offline_state.speech_buffer.push(context);
                            }
                            offline_state.utterance_start_sample = total_samples - context_len;
                            offline_state.ring_buffer.clear();
                        }

                        if currently_speaking {
                            offline_state.speech_buffer.push(samples.to_vec());
                            let now = std::time::Instant::now();
                            if now.duration_since(last_inference_time).as_millis() > 200 {
                                let global_start = offline_state.utterance_start_sample as f64 / 16000.0;
                                if let Some(segment) = run_offline_inference_standalone(
                                    &offline_state.speech_buffer,
                                    &recognizer,
                                    current_segment_id.as_deref().unwrap(),
                                    global_start,
                                    false,
                                ) {
                                    let _ = socket.send(Message::Text(serde_json::to_string(&ServerMessage::Segment { segment }).unwrap().into())).await;
                                }
                                last_inference_time = now;
                            }
                        } else {
                            if offline_state.is_speaking {
                                offline_state.is_speaking = false;
                                offline_state.speech_buffer.push(samples.to_vec());
                                let global_start = offline_state.utterance_start_sample as f64 / 16000.0;
                                if let Some(segment) = run_offline_inference_standalone(
                                    &offline_state.speech_buffer,
                                    &recognizer,
                                    current_segment_id.as_deref().unwrap(),
                                    global_start,
                                    true,
                                ) {
                                    let _ = socket.send(Message::Text(serde_json::to_string(&ServerMessage::Segment { segment }).unwrap().into())).await;
                                }
                                offline_state.speech_buffer.clear();
                                current_segment_id = None;
                            }
                            offline_state.ring_buffer.push_back(samples);
                            if offline_state.ring_buffer.len() > 10 {
                                offline_state.ring_buffer.pop_front();
                            }
                        }
                    }
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(ClientMessage::Stop) = serde_json::from_str::<ClientMessage>(&text) {
                            if offline_state.is_speaking {
                                let global_start = offline_state.utterance_start_sample as f64 / 16000.0;
                                if let Some(segment) = run_offline_inference_standalone(
                                    &offline_state.speech_buffer,
                                    &recognizer,
                                    current_segment_id.as_deref().unwrap(),
                                    global_start,
                                    true,
                                ) {
                                    let _ = socket.send(Message::Text(serde_json::to_string(&ServerMessage::Segment { segment }).unwrap().into())).await;
                                }
                            }
                            let _ = socket.send(Message::Text(serde_json::to_string(&ServerMessage::Stopped).unwrap().into())).await;
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {
                let _ = socket.send(Message::Text(serde_json::to_string(&ServerMessage::Error { message: "Idle timeout".to_string() }).unwrap().into())).await;
                break;
            }
        }
    }
}

async fn load_recognizer(
    state: &ServerState,
    model_id: &str,
    language: &str,
    hotwords: Option<String>,
) -> Result<Arc<crate::asr::Recognizer>, String> {
    let preset = crate::preset_models::find_preset_model(model_id).ok_or("Model not found")?;
    let model_path = preset.resolve_install_path(&state.models_dir);
    let config = crate::asr::build_model_config(
        &model_path,
        &preset.model_type,
        &preset.file_config,
        false, // enable_itn
        language,
        hotwords.clone(),
    )?;

    let key = crate::asr::ModelConfigKey {
        model_path: model_path.to_string_lossy().to_string(),
        model_type: preset.model_type.clone(),
        num_threads: 2,
        enable_itn: false,
        language: language.to_string(),
        hotwords,
    };

    let mut pool = state.recognizer_pool.lock().await;
    if let Some(r) = pool.get(&key) {
        return Ok(r.clone());
    }

    let recognizer = Arc::new(crate::asr::Recognizer::new(config, 2, None)?);
    if !matches!(recognizer.inner, crate::asr::RecognizerInner::Offline(_)) {
        return Err("Only offline models are supported for streaming API".to_string());
    }

    pool.insert(key, recognizer.clone());
    Ok(recognizer)
}

fn run_offline_inference_standalone(
    speech_buffer: &[Vec<f32>],
    recognizer: &crate::asr::Recognizer,
    segment_id: &str,
    global_start: f64,
    is_final: bool,
) -> Option<crate::asr::TranscriptSegment> {
    if speech_buffer.is_empty() {
        return None;
    }
    let crate::asr::RecognizerInner::Offline(r) = &recognizer.inner else {
        return None;
    };

    let mut full_audio = Vec::new();
    for chunk in speech_buffer {
        full_audio.extend_from_slice(chunk);
    }

    let stream = r.0.create_stream();
    stream.accept_waveform(16000, &full_audio);
    r.0.decode(&stream);

    if let Some(result) = stream.get_result() {
        let cleaned_text = crate::asr::normalize_recognizer_text(&result.text);
        if cleaned_text.is_empty() {
            return None;
        }

        let text = if is_final {
            crate::asr::finalize_transcript_text(&cleaned_text, None)
        } else {
            cleaned_text
        };
        if text.is_empty() {
            return None;
        }

        let global_end = global_start + (full_audio.len() as f64 / 16000.0);
        let timestamps_abs: Option<Vec<f32>> = result
            .timestamps
            .as_ref()
            .map(|ts| ts.iter().map(|t| *t + global_start as f32).collect());
        let durations = timestamps_abs
            .as_ref()
            .and_then(|ts| crate::asr::synthesize_durations(ts, global_end as f32));

        Some(crate::asr::TranscriptSegment {
            id: segment_id.to_string(),
            text,
            start: global_start,
            end: global_end,
            is_final,
            timing: None,
            tokens: Some(result.tokens),
            timestamps: timestamps_abs,
            durations,
            translation: None,
            speaker: None,
            speaker_attribution: None,
        })
    } else {
        None
    }
}
