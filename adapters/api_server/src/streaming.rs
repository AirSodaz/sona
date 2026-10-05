use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{ConnectInfo, Query, State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::Response;
use axum::{Router, routing::get};
use serde::{Deserialize, Serialize};

use sona_core::models::preset_models::{DEFAULT_PUNCTUATION_MODEL_ID, find_preset_model};
use sona_core::ports::asr::{
    AsrEngineConfig, AsrMode, AsrRuntimeObserver, AsrStreamingErrorEvent, AsrTranscriptUpdateEvent,
    AsrTranscriptionRequest, LocalAsrEngine, OnlineAsrProviderRequest, StreamingAudioFrameCursor,
    StreamingInferenceSpec, find_online_asr_provider, pcm_s16le_bytes_to_f32,
};
use sona_core::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
use sona_core::transcription::transcript::TranscriptSegment;

use crate::state::ServerState;

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ClientMessage {
    Start {
        model_id: String,
        #[serde(default = "default_language")]
        language: String,
        hotwords: Option<String>,
        #[serde(default = "default_vad_model_id")]
        vad_model_id: String,
        #[serde(default)]
        punctuation_model_id: Option<String>,
    },
    Stop,
}

fn default_language() -> String {
    "auto".to_string()
}

fn default_vad_model_id() -> String {
    "silero-vad".to_string()
}

#[derive(Serialize, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ServerMessage {
    Started { session_id: String },
    Segment { segment: Box<TranscriptSegment> },
    Stopped,
    Error { message: String },
}

pub fn serialize_server_message(msg: &ServerMessage) -> String {
    serde_json::to_string(msg).unwrap_or_else(|e| {
        log::error!("[Streaming] Failed to serialize ServerMessage: {e}");
        r#"{"type":"error","message":"Internal serialization error"}"#.to_string()
    })
}

pub fn build_streaming_router<H, T>(handler: H) -> Router<ServerState>
where
    H: axum::handler::Handler<T, ServerState>,
    T: 'static,
{
    Router::new().route("/v1/streaming", get(handler))
}

pub fn authorize_streaming_request(
    state: &ServerState,
    addr: SocketAddr,
    token: Option<&str>,
) -> Result<tokio::sync::OwnedSemaphorePermit, StatusCode> {
    let ip = addr.ip().to_canonical();

    // 1. Enforce network perimeter (IP whitelist) first
    if !state.ip_whitelist.iter().any(|net| net.contains(&ip)) {
        log::warn!(
            "[ApiServer] Streaming request from {} rejected: IP not in whitelist ({:?})",
            ip,
            state.ip_whitelist
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // 2. Enforce API key authentication if configured (Defense in Depth)
    if !state.api_key.is_empty() {
        let has_valid_api_key =
            token.is_some_and(|t| crate::handlers::constant_time_eq_str(t, &state.api_key));
        if !has_valid_api_key {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }
    state
        .streaming_semaphore
        .clone()
        .try_acquire_owned()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
}

pub async fn handle_streaming_websocket(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<ServerState>,
    Query(params): Query<HashMap<String, String>>,
    headers: axum::http::HeaderMap,
) -> Result<Response, StatusCode> {
    let token_from_header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|val| val.to_str().ok())
        .and_then(|auth_str| {
            let trimmed = auth_str.trim();
            if trimmed.len() >= 7
                && trimmed[..6].eq_ignore_ascii_case("bearer")
                && trimmed.as_bytes()[6] == b' '
            {
                Some(trimmed[7..].trim())
            } else {
                None
            }
        });
    let token = params
        .get("token")
        .or_else(|| params.get("api_key"))
        .map(|s| s.as_str())
        .or(token_from_header);
    let permit = authorize_streaming_request(&state, addr, token)?;

    Ok(ws.on_upgrade(move |socket| async move {
        handle_streaming_socket(socket, state, permit).await;
    }))
}

enum StreamingWorkerEvent {
    Update(AsrTranscriptUpdateEvent),
    Error(String),
}

struct StreamingTranscriptObserver {
    tx: tokio::sync::mpsc::UnboundedSender<StreamingWorkerEvent>,
}

impl AsrRuntimeObserver for StreamingTranscriptObserver {
    fn on_transcript_update(&self, event: &AsrTranscriptUpdateEvent) {
        let _ = self.tx.send(StreamingWorkerEvent::Update(event.clone()));
    }

    fn on_streaming_error(&self, event: &AsrStreamingErrorEvent) {
        let _ = self
            .tx
            .send(StreamingWorkerEvent::Error(event.message.clone()));
    }

    fn on_model_load(&self, _metric: &AsrModelLoadMetric) {}

    fn on_live_inference(&self, _metric: &AsrInferenceMetric) {}
}

async fn handle_streaming_socket(
    mut socket: WebSocket,
    state: ServerState,
    _permit: tokio::sync::OwnedSemaphorePermit,
) {
    let session_id = uuid::Uuid::new_v4().to_string();

    let start_msg = match tokio::time::timeout(std::time::Duration::from_secs(10), socket.recv())
        .await
    {
        Ok(Some(Ok(Message::Text(text)))) => match serde_json::from_str::<ClientMessage>(&text) {
            Ok(ClientMessage::Start {
                model_id,
                language,
                hotwords,
                vad_model_id,
                punctuation_model_id,
            }) => (
                model_id,
                language,
                hotwords,
                vad_model_id,
                punctuation_model_id,
            ),
            _ => {
                let _ = socket
                    .send(Message::Text(
                        serialize_server_message(&ServerMessage::Error {
                            message: "Expected start message".to_string(),
                        })
                        .into(),
                    ))
                    .await;
                return;
            }
        },
        Ok(Some(Ok(Message::Close(_)))) | Ok(Some(Err(_))) | Ok(None) | Err(_) => return,
        _ => {
            let _ = socket
                .send(Message::Text(
                    serialize_server_message(&ServerMessage::Error {
                        message: "Expected start message".to_string(),
                    })
                    .into(),
                ))
                .await;
            return;
        }
    };

    let (model_id, language, hotwords, vad_model_id, punctuation_model_id) = start_msg;

    let Some(factory) = state.streaming_transcriber.as_ref() else {
        let _ = socket
            .send(Message::Text(
                serialize_server_message(&ServerMessage::Error {
                    message: "Streaming transcription is not available on this server".to_string(),
                })
                .into(),
            ))
            .await;
        return;
    };

    let asr_request = if let Some(provider) = find_online_asr_provider(&model_id) {
        if !provider.streaming.supported.unwrap_or(true) {
            let _ = socket
                .send(Message::Text(
                    serialize_server_message(&ServerMessage::Error {
                        message: format!("Provider {} does not support streaming", model_id),
                    })
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
        AsrTranscriptionRequest {
            engine_config: AsrEngineConfig::Online {
                provider: OnlineAsrProviderRequest {
                    provider_id,
                    profile_id: model_id.clone(),
                    config,
                },
            },
            mode: AsrMode::Streaming,
            enable_itn: false,
            language: if language == "auto" {
                String::new()
            } else {
                language
            },
            hotwords,
            speaker_processing: None,
            normalization_options: Default::default(),
            postprocess_options: Default::default(),
        }
    } else {
        let Some(preset) = find_preset_model(&model_id) else {
            let _ = socket
                .send(Message::Text(
                    serialize_server_message(&ServerMessage::Error {
                        message: format!("Model not found: {model_id}"),
                    })
                    .into(),
                ))
                .await;
            return;
        };
        let model_path = preset.resolve_install_path(&state.models_dir);
        let vad_model_path = resolve_vad_model_path(&state.models_dir, &vad_model_id);
        let punct_model_id = punctuation_model_id
            .or_else(|| state.transcription_defaults.punctuation_model_id.clone())
            .unwrap_or_else(|| DEFAULT_PUNCTUATION_MODEL_ID.to_string());
        let punct_model_path = resolve_punctuation_model_path(&state.models_dir, &punct_model_id);
        let engine_name = preset.engine.as_deref().unwrap_or("sherpa-onnx");
        let local_engine =
            LocalAsrEngine::from_name(engine_name).unwrap_or(LocalAsrEngine::SherpaOnnx);

        AsrTranscriptionRequest {
            engine_config: AsrEngineConfig::Local {
                local_engine,
                model_id: Some(model_id.clone()),
                model_path: model_path.to_string_lossy().to_string(),
                num_threads: 2,
                punctuation_model: Some(punct_model_path.to_string_lossy().to_string()),
                alignment_model: None,
                vad_model: Some(vad_model_path.to_string_lossy().to_string()),
                vad_buffer: 0.0,
                batch_segmentation_mode: sona_core::ports::asr::BatchSegmentationMode::default(),
                ffmpeg_path: None,
                model_type: preset.model_type.clone(),
                file_config: Box::new(preset.file_config.clone()),
                gpu_acceleration: state.transcription_defaults.gpu_acceleration.clone(),
                initial_refresh_rate_ms: preset.resolved_rules().initial_refresh_rate_ms,
                enable_partial_decoding: None,
            },
            mode: AsrMode::Streaming,
            enable_itn: false,
            language: if language == "auto" {
                String::new()
            } else {
                language
            },
            hotwords,
            speaker_processing: None,
            normalization_options: Default::default(),
            postprocess_options: Default::default(),
        }
    };

    let spec = match StreamingInferenceSpec::from_request(&asr_request) {
        Ok(s) => s,
        Err(e) => {
            let _ = socket
                .send(Message::Text(
                    serialize_server_message(&ServerMessage::Error {
                        message: e.to_string(),
                    })
                    .into(),
                ))
                .await;
            return;
        }
    };

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<StreamingWorkerEvent>();
    let observer: Arc<dyn AsrRuntimeObserver> = Arc::new(StreamingTranscriptObserver { tx });

    let session = match factory.create(&session_id, &spec, observer).await {
        Ok(session) => session,
        Err(error) => {
            let _ = socket
                .send(Message::Text(
                    serialize_server_message(&ServerMessage::Error {
                        message: error.to_string(),
                    })
                    .into(),
                ))
                .await;
            return;
        }
    };

    if let Err(e) = session.start().await {
        let _ = socket
            .send(Message::Text(
                serialize_server_message(&ServerMessage::Error {
                    message: e.to_string(),
                })
                .into(),
            ))
            .await;
        return;
    }

    let _ = socket
        .send(Message::Text(
            serialize_server_message(&ServerMessage::Started {
                session_id: session_id.clone(),
            })
            .into(),
        ))
        .await;

    let mut stopping = false;
    let mut frame_cursor = StreamingAudioFrameCursor::default();
    loop {
        tokio::select! {
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Binary(pcm))) if !stopping => {
                        let samples = pcm_s16le_bytes_to_f32(&pcm);
                        let frame = frame_cursor.next_samples(samples);
                        if let Err(e) = session.feed_audio_frame(frame).await {
                            log::error!("[Streaming] Error feeding audio samples: {e}");
                        }
                    }
                    Some(Ok(Message::Text(text))) if !stopping => {
                        if let Ok(ClientMessage::Stop) = serde_json::from_str::<ClientMessage>(&text) {
                            let _ = session.flush().await;
                            stopping = true;
                        }
                    }
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                    _ => {}
                }
            }
            Some(event) = rx.recv() => {
                match event {
                    StreamingWorkerEvent::Update(update) => {
                        let mut send_failed = false;
                        for segment in update.update.upsert_segments {
                            if socket
                                .send(Message::Text(
                                    serialize_server_message(&ServerMessage::Segment {
                                        segment: Box::new(segment),
                                    })
                                    .into(),
                                ))
                                .await
                                .is_err()
                            {
                                send_failed = true;
                                break;
                            }
                        }
                        if send_failed {
                            break;
                        }
                    }
                    StreamingWorkerEvent::Error(err_msg) => {
                        let _ = socket
                            .send(Message::Text(
                                serialize_server_message(&ServerMessage::Error {
                                    message: err_msg,
                                })
                                .into(),
                            ))
                            .await;
                        break;
                    }
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(2000)), if stopping => {
                let _ = socket
                    .send(Message::Text(
                        serialize_server_message(&ServerMessage::Stopped).into(),
                    ))
                    .await;
                break;
            }
            _ = tokio::time::sleep(std::time::Duration::from_secs(60)), if !stopping => {
                let _ = socket
                    .send(Message::Text(
                        serialize_server_message(&ServerMessage::Error {
                            message: "Idle timeout".to_string(),
                        })
                        .into(),
                    ))
                    .await;
                break;
            }
        }
    }

    let _ = session.stop().await;
}

pub fn resolve_vad_model_path(models_dir: &Path, vad_model_id_or_path: &str) -> PathBuf {
    find_preset_model(vad_model_id_or_path)
        .map(|model| model.resolve_install_path(models_dir))
        .unwrap_or_else(|| PathBuf::from(vad_model_id_or_path))
}

pub fn resolve_punctuation_model_path(models_dir: &Path, punct_model_id_or_path: &str) -> PathBuf {
    find_preset_model(punct_model_id_or_path)
        .map(|model| model.resolve_install_path(models_dir))
        .unwrap_or_else(|| PathBuf::from(punct_model_id_or_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sona_core::ports::asr::{AsrAudioFrame, AsrPortError, StreamingAsrFactoryPort};

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

    #[test]
    fn serializes_server_messages() {
        let started = ServerMessage::Started {
            session_id: "test-session".to_string(),
        };
        assert_eq!(
            serialize_server_message(&started),
            r#"{"type":"started","session_id":"test-session"}"#
        );

        let stopped = ServerMessage::Stopped;
        assert_eq!(serialize_server_message(&stopped), r#"{"type":"stopped"}"#);

        let error = ServerMessage::Error {
            message: "Something failed".to_string(),
        };
        assert_eq!(
            serialize_server_message(&error),
            r#"{"type":"error","message":"Something failed"}"#
        );
    }

    #[test]
    fn parses_client_start_message_with_defaults() {
        let json = r#"{"type":"start","model_id":"sherpa-onnx-sense-voice"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        match msg {
            ClientMessage::Start {
                model_id,
                language,
                hotwords,
                vad_model_id,
                punctuation_model_id,
            } => {
                assert_eq!(model_id, "sherpa-onnx-sense-voice");
                assert_eq!(language, "auto");
                assert_eq!(hotwords, None);
                assert_eq!(vad_model_id, "silero-vad");
                assert_eq!(punctuation_model_id, None);
            }
            _ => panic!("Expected ClientMessage::Start"),
        }
    }

    #[test]
    fn parses_client_start_message_with_custom_fields() {
        let json = r#"{"type":"start","model_id":"my-model","language":"zh","hotwords":"AI","vad_model_id":"custom-vad","punctuation_model_id":"custom-punct"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        match msg {
            ClientMessage::Start {
                model_id,
                language,
                hotwords,
                vad_model_id,
                punctuation_model_id,
            } => {
                assert_eq!(model_id, "my-model");
                assert_eq!(language, "zh");
                assert_eq!(hotwords.as_deref(), Some("AI"));
                assert_eq!(vad_model_id, "custom-vad");
                assert_eq!(punctuation_model_id.as_deref(), Some("custom-punct"));
            }
            _ => panic!("Expected ClientMessage::Start"),
        }
    }

    #[test]
    fn parses_client_stop_message() {
        let json = r#"{"type":"stop"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ClientMessage::Stop));
    }

    #[test]
    fn resolves_default_punct_id_to_installed_model_path() {
        let models_dir = Path::new("C:/models");
        let path = resolve_punctuation_model_path(models_dir, DEFAULT_PUNCTUATION_MODEL_ID);
        assert_eq!(
            path,
            PathBuf::from("C:/models").join(DEFAULT_PUNCTUATION_MODEL_ID)
        );
    }

    #[test]
    fn preserves_explicit_punct_path() {
        let models_dir = Path::new("C:/models");
        let path = resolve_punctuation_model_path(models_dir, "D:/custom/punct.onnx");
        assert_eq!(path, PathBuf::from("D:/custom/punct.onnx"));
    }

    struct MockStreamingSession {
        started: Arc<std::sync::atomic::AtomicBool>,
        flushed: Arc<std::sync::atomic::AtomicBool>,
        stopped: Arc<std::sync::atomic::AtomicBool>,
        frames: Arc<std::sync::atomic::AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl sona_core::ports::asr::AsrStreamingSession for MockStreamingSession {
        async fn start(&self) -> Result<(), AsrPortError> {
            self.started
                .store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }

        async fn feed_audio_frame(&self, _frame: AsrAudioFrame) -> Result<(), AsrPortError> {
            self.frames
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }

        async fn flush(&self) -> Result<(), AsrPortError> {
            self.flushed
                .store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }

        async fn stop(&self) -> Result<(), AsrPortError> {
            self.stopped
                .store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
    }

    struct MockStreamingFactory {
        session: Arc<MockStreamingSession>,
    }

    #[async_trait::async_trait]
    impl sona_core::ports::asr::StreamingAsrFactoryPort for MockStreamingFactory {
        async fn prepare(&self, _spec: &StreamingInferenceSpec) -> Result<(), AsrPortError> {
            Ok(())
        }

        async fn create(
            &self,
            _pipeline_id: &str,
            _spec: &StreamingInferenceSpec,
            _observer: Arc<dyn AsrRuntimeObserver>,
        ) -> Result<Arc<dyn sona_core::ports::asr::AsrStreamingSession>, AsrPortError> {
            Ok(self.session.clone())
        }
    }

    #[tokio::test]
    async fn mock_streaming_factory_session_lifecycle() {
        let session = Arc::new(MockStreamingSession {
            started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            flushed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            stopped: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            frames: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        });
        let factory = MockStreamingFactory {
            session: session.clone(),
        };

        let req = AsrTranscriptionRequest {
            engine_config: AsrEngineConfig::Online {
                provider: OnlineAsrProviderRequest {
                    provider_id: "test".to_string(),
                    profile_id: "test-model".to_string(),
                    config: serde_json::json!({}),
                },
            },
            mode: AsrMode::Streaming,
            enable_itn: false,
            language: "auto".to_string(),
            hotwords: None,
            speaker_processing: None,
            normalization_options: Default::default(),
            postprocess_options: Default::default(),
        };
        let spec = StreamingInferenceSpec::from_request(&req).unwrap();
        let observer = Arc::new(sona_core::ports::asr::NoopAsrRuntimeObserver);

        let active_session = factory.create("pipeline-1", &spec, observer).await.unwrap();
        active_session.start().await.unwrap();
        assert!(session.started.load(std::sync::atomic::Ordering::SeqCst));

        let frame = AsrAudioFrame::new(1, 0, vec![0.0; 160]);
        active_session.feed_audio_frame(frame).await.unwrap();
        assert_eq!(session.frames.load(std::sync::atomic::Ordering::SeqCst), 1);

        active_session.flush().await.unwrap();
        assert!(session.flushed.load(std::sync::atomic::Ordering::SeqCst));

        active_session.stop().await.unwrap();
        assert!(session.stopped.load(std::sync::atomic::Ordering::SeqCst));
    }
}
