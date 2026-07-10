use axum::extract::ws::{Message, WebSocket};
use axum::extract::{ConnectInfo, Query, State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::Response;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio;

use crate::app::server::TauriStreamingContext;
use sona_api_server::{ServerState, authorize_streaming_request};
use sona_local_asr::audio::{load_vad, pcm_s16le_bytes_to_f32};

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
        segment: Box<crate::integrations::asr::TranscriptSegment>,
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
    let token = params.get("token").map(|s| s.as_str());
    let permit = authorize_streaming_request(&state, addr, token)?;

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

    if sona_core::ports::asr::find_online_asr_provider(&model_id).is_some() {
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
    let context = match tauri_streaming_context(&state) {
        Ok(context) => context,
        Err(message) => {
            let _ = socket
                .send(Message::Text(
                    serde_json::to_string(&ServerMessage::Error { message })
                        .unwrap()
                        .into(),
                ))
                .await;
            return;
        }
    };
    let app_handle = match context.app_handle() {
        Some(app) => app.clone(),
        None => {
            let _ = socket
                .send(Message::Text(
                    serde_json::to_string(&ServerMessage::Error {
                        message: "Online ASR streaming requires AppHandle".to_string(),
                    })
                    .unwrap()
                    .into(),
                ))
                .await;
            return;
        }
    };

    let provider = sona_core::ports::asr::find_online_asr_provider(&model_id).unwrap();
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

    let request = crate::integrations::asr::AsrTranscriptionRequest {
        engine_config: crate::integrations::asr::AsrEngineConfig::Online {
            provider: crate::integrations::asr::OnlineAsrProviderRequest {
                provider_id,
                profile_id: model_id.clone(),
                config,
            },
        },
        mode: crate::integrations::asr::AsrMode::Streaming,
        enable_itn: false,
        language: if language == "auto" {
            "".to_string()
        } else {
            language
        },
        hotwords,
        speaker_processing: None,
        normalization_options: Default::default(),
        postprocess_options: Default::default(),
    };

    let sherpa_state = app_handle.state::<crate::integrations::asr::AsrState>();

    if let Err(e) = crate::commands::asr::init_recognizer(
        app_handle.clone(),
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

    if let Err(e) =
        crate::commands::asr::start_recognizer(sherpa_state.clone(), session_id.clone()).await
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
    let (tx, mut rx) = mpsc::unbounded_channel::<crate::integrations::asr::TranscriptUpdate>();

    let handler_id = app_handle.listen(event_name, move |event| {
        if let Ok(update) =
            serde_json::from_str::<crate::integrations::asr::TranscriptUpdate>(event.payload())
        {
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
                        let samples = pcm_s16le_bytes_to_f32(&pcm);
                        if let Err(e) = crate::integrations::asr::feed_audio_samples(
                            sherpa_state.inner(),
                            &session_id,
                            &samples,
                        ).await {
                            log::error!("Error feeding audio samples: {}", e);
                        }
                    }
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(ClientMessage::Stop) = serde_json::from_str::<ClientMessage>(&text) {
                            let _ = crate::commands::asr::flush_recognizer(sherpa_state.clone(), session_id.clone()).await;
                            stopping = true;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
            Some(update) = rx.recv() => {
                for segment in update.upsert_segments {
                    let _ = socket.send(Message::Text(serde_json::to_string(&ServerMessage::Segment { segment: Box::new(segment) }).unwrap().into())).await;
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

    let _ = crate::commands::asr::stop_recognizer(sherpa_state.clone(), session_id.clone()).await;
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

    let vad_model_path = resolve_vad_model_path(&state.models_dir, &vad_model_id);
    let vad = match load_vad(Some(vad_model_path.to_string_lossy().to_string())) {
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
    let mut offline_state = sona_local_asr::runtime::OfflineState::default();
    let mut total_samples = 0;
    let mut current_segment_id: Option<String> = None;
    let mut last_inference_time = std::time::Instant::now();

    loop {
        tokio::select! {
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Binary(pcm))) => {
                        let samples = pcm_s16le_bytes_to_f32(&pcm);
                        total_samples += samples.len();

                        crate::integrations::asr::accept_vad_samples(&vad, &samples);
                        let currently_speaking = crate::integrations::asr::vad_detected(&vad);

                        if current_segment_id.is_none() {
                            current_segment_id = Some(uuid::Uuid::new_v4().to_string());
                        }

                        if currently_speaking && !offline_state.is_speech_active() {
                            let samples_to_keep = (16000.0 * 0.3) as usize;
                            offline_state.begin_speech(total_samples, samples_to_keep);
                        }

                        if currently_speaking {
                            offline_state.push_speech_chunk(samples);
                            let now = std::time::Instant::now();
                            if now.duration_since(last_inference_time).as_millis() > 200 {
                                let global_start = offline_state.utterance_start_seconds(16000.0);
                                if let Some(segment) = run_offline_inference_standalone(
                                    offline_state.speech_chunks(),
                                    &recognizer,
                                    current_segment_id.as_deref().unwrap(),
                                    global_start,
                                    false,
                                ) {
                                    let _ = socket.send(Message::Text(serde_json::to_string(&ServerMessage::Segment { segment: Box::new(segment) }).unwrap().into())).await;
                                }
                                last_inference_time = now;
                            }
                        } else {
                            if offline_state.is_speech_active() {
                                offline_state.finish_speech_with_chunk(samples.clone());
                                let global_start = offline_state.utterance_start_seconds(16000.0);
                                if let Some(segment) = run_offline_inference_standalone(
                                    offline_state.speech_chunks(),
                                    &recognizer,
                                    current_segment_id.as_deref().unwrap(),
                                    global_start,
                                    true,
                                ) {
                                    let _ = socket.send(Message::Text(serde_json::to_string(&ServerMessage::Segment { segment: Box::new(segment) }).unwrap().into())).await;
                                }
                                offline_state.clear_speech_buffer();
                                current_segment_id = None;
                            }
                            offline_state.push_ring_chunk(samples, 10);
                        }
                    }
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(ClientMessage::Stop) = serde_json::from_str::<ClientMessage>(&text) {
                            if offline_state.is_speech_active() {
                                let global_start = offline_state.utterance_start_seconds(16000.0);
                                if let Some(segment) = run_offline_inference_standalone(
                                    offline_state.speech_chunks(),
                                    &recognizer,
                                    current_segment_id.as_deref().unwrap(),
                                    global_start,
                                    true,
                                ) {
                                    let _ = socket.send(Message::Text(serde_json::to_string(&ServerMessage::Segment { segment: Box::new(segment) }).unwrap().into())).await;
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
) -> Result<Arc<crate::integrations::asr::Recognizer>, String> {
    let context = tauri_streaming_context(state)?;
    let preset =
        crate::platform::preset_models::find_preset_model(model_id).ok_or("Model not found")?;
    let model_path = preset.resolve_install_path(&state.models_dir);
    let config = crate::integrations::asr::build_model_config(
        &model_path,
        &preset.model_type,
        &preset.file_config,
        false, // enable_itn
        language,
        hotwords.clone(),
    )?;

    let gpu_plan = crate::platform::hardware::resolve_gpu_acceleration_plan(
        state.transcription_defaults.gpu_acceleration.as_deref(),
    )
    .await;
    let key = crate::integrations::asr::ModelConfigKey::new(
        model_path.to_string_lossy().to_string(),
        preset.model_type.clone(),
        2,
        false,
        language.to_string(),
        hotwords,
        None,
    );

    let primary_provider = gpu_plan.provider_options().first().cloned().flatten();
    let (cell, _is_new) = context
        .recognizer_pool()
        .recognizer_cell_for_gpu_plan(&key, gpu_plan.provider_options(), primary_provider.clone())
        .await;

    let recognizer = cell
        .get_or_try_init(|| async {
            let recognizer_result =
                crate::integrations::asr::create_recognizer_with_gpu_plan(config, 2, gpu_plan)?;
            if let Some(notice) = recognizer_result.fallback_notice.as_ref() {
                log::warn!(
                    "[streaming] {} recognizer creation failed, retrying with {}: {}",
                    notice.from_provider,
                    notice.to_provider,
                    notice.error
                );
            }
            let r = Arc::new(recognizer_result.recognizer);
            if !r.is_offline() {
                return Err("Only offline models are supported for streaming API".to_string());
            }

            let actual_provider = recognizer_result.provider.clone();
            if actual_provider != primary_provider {
                context
                    .recognizer_pool()
                    .register_recognizer_gpu_provider(&key, actual_provider, cell.clone())
                    .await;
            }

            Ok::<Arc<crate::integrations::asr::Recognizer>, String>(r)
        })
        .await?
        .clone();

    Ok(recognizer)
}

fn tauri_streaming_context(state: &ServerState) -> Result<Arc<TauriStreamingContext>, String> {
    let context = state
        .platform
        .streaming_context()
        .ok_or_else(|| "Tauri streaming context is not configured".to_string())?;
    Arc::downcast::<TauriStreamingContext>(context)
        .map_err(|_| "Tauri streaming context has unexpected type".to_string())
}

pub(crate) fn resolve_vad_model_path(models_dir: &Path, vad_model_id_or_path: &str) -> PathBuf {
    crate::platform::preset_models::find_preset_model(vad_model_id_or_path)
        .map(|model| model.resolve_install_path(models_dir))
        .unwrap_or_else(|| PathBuf::from(vad_model_id_or_path))
}

fn run_offline_inference_standalone(
    speech_buffer: &[Vec<f32>],
    recognizer: &crate::integrations::asr::Recognizer,
    segment_id: &str,
    global_start: f64,
    is_final: bool,
) -> Option<crate::integrations::asr::TranscriptSegment> {
    if speech_buffer.is_empty() {
        return None;
    }
    let Some(r) = recognizer.offline() else {
        return None;
    };

    let mut full_audio = Vec::new();
    for chunk in speech_buffer {
        full_audio.extend_from_slice(chunk);
    }

    if let Some(result) = crate::integrations::asr::decode_offline_samples(r, &full_audio) {
        let cleaned_text = crate::integrations::asr::normalize_recognizer_text(&result.text);
        if cleaned_text.is_empty() {
            return None;
        }

        let text = if is_final {
            crate::integrations::asr::finalize_transcript_text(&cleaned_text, None)
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
            .and_then(|ts| crate::integrations::asr::synthesize_durations(ts, global_end as f32));

        Some(crate::integrations::asr::TranscriptSegment {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_default_vad_id_to_installed_model_path() {
        let models_dir = Path::new("C:/models");

        let path = resolve_vad_model_path(models_dir, "silero-vad");

        assert_eq!(path, PathBuf::from("C:/models").join("silero_vad.onnx"));
    }

    #[test]
    fn preserves_explicit_vad_path() {
        let models_dir = Path::new("C:/models");

        let path = resolve_vad_model_path(models_dir, "D:/custom/vad.onnx");

        assert_eq!(path, PathBuf::from("D:/custom/vad.onnx"));
    }
}
