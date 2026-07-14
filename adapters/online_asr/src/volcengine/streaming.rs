use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use log::{info, warn};
use sona_core::ports::asr::{
    AsrMode, AsrRuntimeObserver, AsrStreamingErrorEvent, AsrStreamingSession,
    AsrTranscriptionRequest, SherpaError,
};
use sona_core::transcription::postprocess::TranscriptPostprocessor;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::{HeaderName, HeaderValue};
use tokio_tungstenite::tungstenite::protocol::Message;

#[derive(Clone)]
struct VolcengineStreamingSession {
    instance_id: String,
    observer: Arc<dyn AsrRuntimeObserver>,
    request: AsrTranscriptionRequest,
    writer: Arc<Mutex<Option<VolcengineWriter>>>,
    stopping: Arc<AtomicBool>,
    final_response_received: Arc<Notify>,
    final_response_outcome: Arc<Mutex<Option<Result<(), SherpaError>>>>,
    reader_task: Arc<Mutex<Option<JoinHandle<()>>>>,
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
        stopping: Arc::new(AtomicBool::new(false)),
        final_response_received: Arc::new(Notify::new()),
        final_response_outcome: Arc::new(Mutex::new(None)),
        reader_task: Arc::new(Mutex::new(None)),
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
    session.stopping.store(false, Ordering::SeqCst);
    *session.final_response_outcome.lock().await = None;
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
    let stopping = session.stopping.clone();
    let final_response_received = session.final_response_received.clone();
    let final_response_outcome = session.final_response_outcome.clone();
    let observer_for_task = observer.clone();
    let reader_task = tokio::spawn(async move {
        while let Some(message) = reader.next().await {
            match message {
                Ok(Message::Binary(frame)) => {
                    match crate::parse_volcengine_server_response_frame(&frame)
                        .map_err(SherpaError::from)
                        .and_then(|frame| {
                            frame
                                .payload
                                .map(|value| {
                                    crate::volcengine_streaming_segments_from_response(
                                        &value,
                                        frame.is_final,
                                    )
                                    .map_err(SherpaError::Generic)
                                })
                                .transpose()
                                .map(|value| value.unwrap_or_default())
                                .map(|segments| (segments, frame.is_final))
                        }) {
                        Ok((segments, is_final)) => {
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
                            if is_final {
                                publish_reader_outcome(
                                    &final_response_outcome,
                                    &final_response_received,
                                    Ok(()),
                                )
                                .await;
                                return;
                            }
                        }
                        Err(error) => {
                            warn!("[Volcengine ASR] response parse failed: {error}");
                            observe_streaming_error(
                                observer_for_task.as_ref(),
                                &instance_id_for_task,
                                &error,
                            );
                            publish_reader_outcome(
                                &final_response_outcome,
                                &final_response_received,
                                Err(error),
                            )
                            .await;
                            return;
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    let error = SherpaError::VolcengineWebSocketClosed;
                    if !stopping.load(Ordering::SeqCst) {
                        observe_streaming_error(
                            observer_for_task.as_ref(),
                            &instance_id_for_task,
                            &error,
                        );
                    }
                    publish_reader_outcome(
                        &final_response_outcome,
                        &final_response_received,
                        Err(error),
                    )
                    .await;
                    return;
                }
                Ok(_) => {}
                Err(error) => {
                    warn!("[Volcengine ASR] websocket read failed: {error}");
                    let error = SherpaError::VolcengineWebSocketReadFailed {
                        error: error.to_string(),
                    };
                    if !stopping.load(Ordering::SeqCst) {
                        observe_streaming_error(
                            observer_for_task.as_ref(),
                            &instance_id_for_task,
                            &error,
                        );
                    }
                    publish_reader_outcome(
                        &final_response_outcome,
                        &final_response_received,
                        Err(error),
                    )
                    .await;
                    return;
                }
            }
        }

        let error = SherpaError::VolcengineWebSocketClosed;
        if !stopping.load(Ordering::SeqCst) {
            observe_streaming_error(observer_for_task.as_ref(), &instance_id_for_task, &error);
        }
        publish_reader_outcome(
            &final_response_outcome,
            &final_response_received,
            Err(error),
        )
        .await;
    });
    *session.reader_task.lock().await = Some(reader_task);

    Ok(())
}

fn observe_streaming_error(
    observer: &dyn AsrRuntimeObserver,
    instance_id: &str,
    error: &SherpaError,
) {
    observer.on_streaming_error(&AsrStreamingErrorEvent {
        instance_id: instance_id.to_string(),
        code: error.code().to_string(),
        message: error.to_string(),
    });
}

async fn publish_reader_outcome(
    outcome: &Mutex<Option<Result<(), SherpaError>>>,
    notification: &Notify,
    value: Result<(), SherpaError>,
) {
    let mut outcome = outcome.lock().await;
    if outcome.is_none() {
        *outcome = Some(value);
        notification.notify_waiters();
    }
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
    let mut writer_guard = session.writer.lock().await;
    let Some(writer) = writer_guard.as_mut() else {
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
    let mut writer_guard = session.writer.lock().await;
    let Some(writer) = writer_guard.as_mut() else {
        return Err(SherpaError::VolcengineWebSocketNotConnected);
    };
    let notified = session.final_response_received.notified();
    tokio::pin!(notified);
    notified.as_mut().enable();
    if let Some(outcome) = session.final_response_outcome.lock().await.clone() {
        return outcome;
    }
    writer
        .send(Message::Binary(crate::build_volcengine_audio_frame(
            &[],
            true,
        )))
        .await
        .map_err(|error| SherpaError::VolcengineEndFrameSendFailed {
            error: error.to_string(),
        })?;
    drop(writer_guard);

    if let Some(outcome) = session.final_response_outcome.lock().await.clone() {
        return outcome;
    }
    if tokio::time::timeout(Duration::from_millis(1500), notified)
        .await
        .is_ok()
    {
        return session
            .final_response_outcome
            .lock()
            .await
            .clone()
            .unwrap_or(Err(SherpaError::VolcengineWebSocketClosed));
    }

    let error = SherpaError::VolcengineFinalResponseTimeout;
    observe_streaming_error(session.observer.as_ref(), &session.instance_id, &error);
    publish_reader_outcome(
        &session.final_response_outcome,
        &session.final_response_received,
        Err(error.clone()),
    )
    .await;
    Err(error)
}

async fn stop_streaming_recognizer_impl(
    session: &VolcengineStreamingSession,
) -> Result<(), SherpaError> {
    session.stopping.store(true, Ordering::SeqCst);
    let mut writer = session.writer.lock().await;
    if let Some(mut writer) = writer.take() {
        let _ = writer.send(Message::Close(None)).await;
    }
    drop(writer);

    if let Some(mut reader_task) = session.reader_task.lock().await.take()
        && tokio::time::timeout(Duration::from_millis(1500), &mut reader_task)
            .await
            .is_err()
    {
        reader_task.abort();
        let _ = reader_task.await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use sona_core::ports::asr::{
        AsrEngineConfig, AsrMode, AsrRuntimeObserver, AsrStreamingErrorEvent, AsrStreamingSession,
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
        errors: StdMutex<Vec<AsrStreamingErrorEvent>>,
    }

    impl AsrRuntimeObserver for RecordingObserver {
        fn on_transcript_update(&self, event: &AsrTranscriptUpdateEvent) {
            self.events.lock().unwrap().push(event.clone());
        }

        fn on_model_load(&self, _metric: &AsrModelLoadMetric) {}

        fn on_live_inference(&self, _metric: &AsrInferenceMetric) {}

        fn on_streaming_error(&self, event: &AsrStreamingErrorEvent) {
            self.errors.lock().unwrap().push(event.clone());
        }
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

    fn server_result_frame(sequence: i32, text: &str, definite: bool) -> Vec<u8> {
        let payload = serde_json::to_vec(&serde_json::json!({
            "result": {
                "text": text,
                "utterances": [{
                    "start_time": 0,
                    "end_time": 1000,
                    "text": text,
                    "definite": definite
                }]
            }
        }))
        .unwrap();
        let flags = if sequence < 0 { 0x93 } else { 0x91 };
        let mut frame = vec![0x11, flags, 0x10, 0x00];
        frame.extend_from_slice(&sequence.to_be_bytes());
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

    async fn streaming_error_from_server_message(message: Message) -> AsrStreamingErrorEvent {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut websocket = tokio_tungstenite::accept_async(stream).await.unwrap();
            websocket.send(message).await.unwrap();
        });
        let observer = Arc::new(RecordingObserver::default());
        let session = create_volcengine_streaming_session(
            "live-error".to_string(),
            test_request(&format!("ws://{address}"), AsrMode::Streaming),
            observer.clone(),
        )
        .unwrap();

        session.start().await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if !observer.errors.lock().unwrap().is_empty() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        session.stop().await.unwrap();
        server.await.unwrap();

        observer.errors.lock().unwrap()[0].clone()
    }

    #[tokio::test]
    async fn reader_failures_are_reported_to_the_runtime_observer() {
        for (message, expected_code) in [
            (Message::Close(None), "VOLCENGINE_WEB_SOCKET_CLOSED"),
            (Message::Binary(vec![0]), "VOLCENGINE_FRAME_TOO_SHORT"),
        ] {
            let error = streaming_error_from_server_message(message).await;
            assert_eq!(
                (error.instance_id.as_str(), error.code.as_str()),
                ("live-error", expected_code),
            );
        }
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
                .send(Message::Binary(server_result_frame(
                    -1,
                    "hello from mock",
                    true,
                )))
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
        assert!(observer.errors.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn flush_waits_for_the_protocol_final_response() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let partial_sent = Arc::new(Notify::new());
        let release_final = Arc::new(Notify::new());
        let partial_sent_for_server = partial_sent.clone();
        let release_final_for_server = release_final.clone();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let websocket = tokio_tungstenite::accept_async(stream).await.unwrap();
            let (mut writer, mut reader) = websocket.split();
            for _ in 0..2 {
                assert!(matches!(reader.next().await, Some(Ok(Message::Binary(_)))));
            }
            writer
                .send(Message::Binary(server_result_frame(1, "partial", false)))
                .await
                .unwrap();
            partial_sent_for_server.notify_one();
            release_final_for_server.notified().await;
            writer
                .send(Message::Binary(server_result_frame(-2, "final", true)))
                .await
                .unwrap();
            let _ = reader.next().await;
        });
        let observer = Arc::new(RecordingObserver::default());
        let session = create_volcengine_streaming_session(
            "live-final".to_string(),
            test_request(&format!("ws://{address}"), AsrMode::Streaming),
            observer.clone(),
        )
        .unwrap();

        session.start().await.unwrap();
        let session_for_flush = session.clone();
        let flush = tokio::spawn(async move { session_for_flush.flush().await });
        partial_sent.notified().await;
        tokio::task::yield_now().await;
        assert!(!flush.is_finished());
        release_final.notify_one();
        flush.await.unwrap().unwrap();
        session.stop().await.unwrap();
        server.await.unwrap();

        let events = observer.events.lock().unwrap();
        assert_eq!(
            events
                .iter()
                .map(|event| {
                    let segment = &event.update.upsert_segments[0];
                    (segment.text.as_str(), segment.is_final)
                })
                .collect::<Vec<_>>(),
            vec![("partial", false), ("final", true)],
        );
    }

    #[tokio::test]
    async fn flush_without_a_final_response_returns_a_structured_timeout() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let websocket = tokio_tungstenite::accept_async(stream).await.unwrap();
            let (_writer, mut reader) = websocket.split();
            for _ in 0..2 {
                assert!(matches!(reader.next().await, Some(Ok(Message::Binary(_)))));
            }
            let _ = reader.next().await;
        });
        let session = create_volcengine_streaming_session(
            "live-timeout".to_string(),
            test_request(&format!("ws://{address}"), AsrMode::Streaming),
            Arc::new(RecordingObserver::default()),
        )
        .unwrap();

        session.start().await.unwrap();
        let error = session.flush().await.unwrap_err();
        assert_eq!(error.code(), "VOLCENGINE_FINAL_RESPONSE_TIMEOUT");
        session.stop().await.unwrap();
        server.await.unwrap();
    }
}
