use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use log::{info, warn};
use sona_core::ports::asr::{
    AsrMode, AsrRuntimeObserver, AsrStreamingSession, AsrTranscriptionRequest, SherpaError,
};
use sona_core::transcription::postprocess::TranscriptPostprocessor;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
use tokio::sync::{Mutex, Notify};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::{HeaderName, HeaderValue};
use tokio_tungstenite::tungstenite::protocol::Message;

#[derive(Clone)]
struct VolcengineStreamingSession {
    instance_id: String,
    observer: Arc<dyn AsrRuntimeObserver>,
    request: AsrTranscriptionRequest,
    writer: Arc<Mutex<Option<VolcengineWriter>>>,
    flushing: Arc<AtomicBool>,
    final_response_received: Arc<Notify>,
}

type VolcengineWriter = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;

pub fn create_volcengine_streaming_session(
    instance_id: String,
    request: AsrTranscriptionRequest,
    observer: Arc<dyn AsrRuntimeObserver>,
) -> Result<Arc<dyn AsrStreamingSession>, SherpaError> {
    if request.mode != AsrMode::Streaming {
        return Err(SherpaError::VolcengineRealtimeOnlyForStreaming);
    }
    crate::resolve_volcengine_config_checked(&request, crate::VolcengineMode::Streaming)
        .map_err(SherpaError::from)?;
    Ok(Arc::new(VolcengineStreamingSession {
        instance_id,
        observer,
        request,
        writer: Arc::new(Mutex::new(None)),
        flushing: Arc::new(AtomicBool::new(false)),
        final_response_received: Arc::new(Notify::new()),
    }))
}

#[async_trait]
impl AsrStreamingSession for VolcengineStreamingSession {
    async fn start(&self) -> Result<(), SherpaError> {
        start_streaming_recognizer_impl(self.observer.clone(), self, &self.instance_id).await
    }

    async fn stop(&self) -> Result<(), SherpaError> {
        stop_streaming_recognizer_impl(self).await
    }

    async fn flush(&self) -> Result<(), SherpaError> {
        flush_streaming_recognizer_impl(self).await
    }

    async fn feed_audio_chunk(&self, samples: Vec<u8>) -> Result<(), SherpaError> {
        feed_audio_chunk_impl(self, samples).await
    }

    async fn feed_audio_samples(&self, samples: &[f32]) -> Result<(), SherpaError> {
        feed_audio_samples_impl(self, samples).await
    }
}

async fn start_streaming_recognizer_impl(
    observer: Arc<dyn AsrRuntimeObserver>,
    session: &VolcengineStreamingSession,
    instance_id: &str,
) -> Result<(), SherpaError> {
    let config = crate::resolve_volcengine_config_checked(
        &session.request,
        crate::VolcengineMode::Streaming,
    )
    .map_err(SherpaError::from)?;
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
    let init_frame = crate::build_volcengine_full_client_request_frame(
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
    let observer_for_task = observer.clone();
    tokio::spawn(async move {
        while let Some(message) = reader.next().await {
            match message {
                Ok(Message::Binary(frame)) => {
                    let flush_final = flushing.load(Ordering::SeqCst);
                    match crate::parse_volcengine_server_response_frame(&frame)
                        .map_err(SherpaError::from)
                        .and_then(|value| {
                            value
                                .map(|value| {
                                    crate::volcengine_streaming_segments_from_response(
                                        &value,
                                        flush_final,
                                    )
                                    .map_err(SherpaError::Generic)
                                })
                                .transpose()
                                .map(|value| value.unwrap_or_default())
                        }) {
                        Ok(segments) if !segments.is_empty() => {
                            let normalized = super::transcript::normalize_segments(
                                segments,
                                normalization_options,
                            );
                            let processed = postprocessor.process_segments(normalized);
                            for segment in processed {
                                let update = super::transcript::build_transcript_update(
                                    segment,
                                    normalization_options,
                                );
                                super::transcript::observe_transcript_update(
                                    observer_for_task.as_ref(),
                                    &instance_id_for_task,
                                    &update,
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
        .send(Message::Binary(crate::build_volcengine_audio_frame(
            &samples, false,
        )))
        .await
        .map_err(|error| SherpaError::VolcengineAudioSendFailed {
            error: error.to_string(),
        })?;
    Ok(())
}

/// Feed f32 audio samples from the hardware capture worker to a Volcengine
/// streaming session. Converts f32 -> i16 PCM bytes and sends via WebSocket.
async fn feed_audio_samples_impl(
    session: &VolcengineStreamingSession,
    samples: &[f32],
) -> Result<(), SherpaError> {
    let mut writer_guard = session.writer.lock().await;
    let Some(writer) = writer_guard.as_mut() else {
        // WebSocket not yet connected -- silently skip. The session may still
        // be in the "preparing" phase before start_recognizer connects.
        return Ok(());
    };
    let pcm_bytes = crate::f32_samples_to_i16_pcm_bytes(samples);
    writer
        .send(Message::Binary(crate::build_volcengine_audio_frame(
            &pcm_bytes, false,
        )))
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
        let notified = session.final_response_received.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        writer
            .send(Message::Binary(crate::build_volcengine_audio_frame(
                &[],
                true,
            )))
            .await
            .map_err(|error| SherpaError::VolcengineEndFrameSendFailed {
                error: error.to_string(),
            })?;
        let _ = tokio::time::timeout(Duration::from_millis(1500), notified).await;
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

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use sona_core::ports::asr::{
        AsrEngineConfig, AsrMode, AsrRuntimeObserver, AsrStreamingSession,
        AsrTranscriptUpdateEvent, AsrTranscriptionRequest, OnlineAsrProviderRequest, SherpaError,
        VOLCENGINE_DOUBAO_PROVIDER_ID,
    };
    use sona_core::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
    use sona_core::transcription::postprocess::{
        TranscriptNormalizationOptions, TranscriptPostprocessOptions,
    };
    use std::sync::{Arc, Mutex as StdMutex};
    use std::time::{Duration, Instant};
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_hdr_async;
    use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
    use tokio_tungstenite::tungstenite::protocol::Message;

    #[derive(Default)]
    struct RecordingObserver {
        events: StdMutex<Vec<AsrTranscriptUpdateEvent>>,
    }

    impl AsrRuntimeObserver for RecordingObserver {
        fn on_transcript_update(&self, event: &AsrTranscriptUpdateEvent) {
            self.events.lock().unwrap().push(event.clone());
        }

        fn on_model_load(&self, _metric: &AsrModelLoadMetric) {}

        fn on_live_inference(&self, _metric: &AsrInferenceMetric) {}
    }

    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    struct RecordedHeaders {
        api_key: String,
        resource_id: String,
        connect_id: String,
        sequence: String,
    }

    struct ServerObservation {
        frames: Vec<Vec<u8>>,
        close_observed: bool,
    }

    fn test_request(endpoint: &str, mode: AsrMode) -> AsrTranscriptionRequest {
        AsrTranscriptionRequest {
            mode,
            language: "auto".to_string(),
            enable_itn: true,
            normalization_options: TranscriptNormalizationOptions::default(),
            postprocess_options: TranscriptPostprocessOptions::default(),
            hotwords: None,
            speaker_processing: None,
            engine_config: AsrEngineConfig::Online {
                provider: OnlineAsrProviderRequest {
                    provider_id: VOLCENGINE_DOUBAO_PROVIDER_ID.to_string(),
                    profile_id: "volcengine-doubao-default".to_string(),
                    config: serde_json::json!({
                        "apiKey": "test-key",
                        "streamingEndpoint": endpoint,
                        "streamingResourceId": "test-resource",
                    }),
                },
            },
        }
    }

    fn server_result_frame() -> Vec<u8> {
        let payload = serde_json::to_vec(&serde_json::json!({
            "result": {
                "text": "hello from mock",
                "utterances": [{
                    "start_time": 0,
                    "end_time": 1000,
                    "text": "hello from mock",
                    "definite": true
                }]
            }
        }))
        .unwrap();
        let mut frame = vec![0x11, 0x90, 0x10, 0x00];
        frame.extend_from_slice(&1_u32.to_be_bytes());
        frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        frame.extend_from_slice(&payload);
        frame
    }

    fn binary_payload(frame: &[u8]) -> &[u8] {
        &frame[8..]
    }

    fn assert_streaming_session<T: AsrStreamingSession>() {}

    #[test]
    fn concrete_session_implements_the_core_streaming_port() {
        assert_streaming_session::<VolcengineStreamingSession>();
    }

    #[test]
    fn factory_rejects_batch_mode_before_network_access() {
        let request = test_request("ws://127.0.0.1:1", AsrMode::Batch);
        let result = create_volcengine_streaming_session(
            "live-1".to_string(),
            request,
            Arc::new(RecordingObserver::default()),
        );
        assert!(matches!(
            result.err(),
            Some(SherpaError::VolcengineRealtimeOnlyForStreaming)
        ));
    }

    #[tokio::test]
    async fn byte_feed_before_connect_returns_the_existing_error() {
        let session = create_volcengine_streaming_session(
            "live-1".to_string(),
            test_request("ws://127.0.0.1:1", AsrMode::Streaming),
            Arc::new(RecordingObserver::default()),
        )
        .unwrap();

        assert!(matches!(
            session.feed_audio_chunk(vec![1, 2]).await,
            Err(SherpaError::VolcengineWebSocketNotConnected)
        ));
    }

    #[tokio::test]
    async fn f32_feed_before_connect_is_a_silent_success() {
        let session = create_volcengine_streaming_session(
            "live-1".to_string(),
            test_request("ws://127.0.0.1:1", AsrMode::Streaming),
            Arc::new(RecordingObserver::default()),
        )
        .unwrap();

        session.feed_audio_samples(&[0.25, -0.25]).await.unwrap();
    }

    #[tokio::test]
    async fn local_websocket_covers_the_streaming_session_lifecycle() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let recorded_headers = Arc::new(StdMutex::new(RecordedHeaders::default()));
        let recorded_headers_for_server = recorded_headers.clone();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let websocket =
                accept_hdr_async(stream, move |request: &Request, response: Response| {
                    let header = |name| {
                        request
                            .headers()
                            .get(name)
                            .and_then(|value| value.to_str().ok())
                            .unwrap_or_default()
                            .to_string()
                    };
                    *recorded_headers_for_server.lock().unwrap() = RecordedHeaders {
                        api_key: header("X-Api-Key"),
                        resource_id: header("X-Api-Resource-Id"),
                        connect_id: header("X-Api-Connect-Id"),
                        sequence: header("X-Api-Sequence"),
                    };
                    Ok(response)
                })
                .await
                .unwrap();
            let (mut writer, mut reader) = websocket.split();
            let mut frames = Vec::new();
            for _ in 0..4 {
                match reader.next().await.unwrap().unwrap() {
                    Message::Binary(frame) => frames.push(frame),
                    message => panic!("expected binary frame, got {message:?}"),
                }
            }
            writer
                .send(Message::Binary(server_result_frame()))
                .await
                .unwrap();
            let close_observed = matches!(
                tokio::time::timeout(Duration::from_secs(2), reader.next()).await,
                Ok(Some(Ok(Message::Close(_))))
            );
            ServerObservation {
                frames,
                close_observed,
            }
        });

        let observer = Arc::new(RecordingObserver::default());
        let session: Arc<dyn AsrStreamingSession> = create_volcengine_streaming_session(
            "live-1".to_string(),
            test_request(&format!("ws://{address}"), AsrMode::Streaming),
            observer.clone(),
        )
        .unwrap();

        session.start().await.unwrap();
        session.feed_audio_chunk(vec![1, 2, 3, 4]).await.unwrap();
        session.feed_audio_samples(&[0.0, 1.0]).await.unwrap();
        let flush_started = Instant::now();
        session.flush().await.unwrap();
        assert!(flush_started.elapsed() < Duration::from_secs(1));
        session.stop().await.unwrap();

        let observation = tokio::time::timeout(Duration::from_secs(2), server)
            .await
            .unwrap()
            .unwrap();
        let headers = recorded_headers.lock().unwrap().clone();
        assert_eq!(headers.api_key, "test-key");
        assert_eq!(headers.resource_id, "test-resource");
        assert!(uuid::Uuid::parse_str(&headers.connect_id).is_ok());
        assert_eq!(headers.sequence, "-1");

        assert_eq!(observation.frames.len(), 4);
        assert_eq!(&observation.frames[0][..4], &[0x11, 0x10, 0x10, 0x00]);
        assert_eq!(&observation.frames[1][..4], &[0x11, 0x20, 0x00, 0x00]);
        assert_eq!(binary_payload(&observation.frames[1]), &[1, 2, 3, 4]);
        assert_eq!(&observation.frames[2][..4], &[0x11, 0x20, 0x00, 0x00]);
        assert_eq!(binary_payload(&observation.frames[2]), &[0, 0, 0xff, 0x7f]);
        assert_eq!(&observation.frames[3][..4], &[0x11, 0x22, 0x00, 0x00]);
        assert!(binary_payload(&observation.frames[3]).is_empty());
        assert!(observation.close_observed);

        let events = observer.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].instance_id, "live-1");
        assert_eq!(events[0].stage, "volcengine_streaming");
        assert_eq!(events[0].update.upsert_segments.len(), 1);
        assert_eq!(events[0].update.upsert_segments[0].text, "hello from mock");
        assert!(events[0].update.upsert_segments[0].is_final);
    }
}
