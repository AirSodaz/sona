use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use log::{info, warn};
use serde_json::Value;
use sona_core::ports::asr::{
    AsrAudioFrame, AsrMode, AsrPortError, AsrRuntimeObserver, AsrStreamBoundaryEvent,
    AsrStreamingSession, AsrTranscriptionRequest, MISTRAL_VOXTRAL_PROVIDER_ID,
};
use sona_core::transcription::postprocess::TranscriptPostprocessor;
use sona_core::transcription::transcript::TranscriptSegment;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::Duration;
use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::Message;

use crate::aimux_adapter::resolve_online_provider_config;
use crate::error::SherpaError;
use crate::f32_samples_to_i16_pcm_bytes;
use crate::streaming_common::{
    build_transcript_update, insert_header, normalize_segments, observe_streaming_error,
    observe_transcript_update, publish_reader_outcome,
};

const DEFAULT_MISTRAL_STREAMING_ENDPOINT: &str =
    "wss://api.mistral.ai/v1/audio/transcriptions/realtime";
const DEFAULT_MISTRAL_STREAMING_MODEL: &str = "voxtral-mini-transcribe-realtime-2602";

type MistralWriter = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;

#[derive(Clone)]
pub struct MistralStreamingSession {
    instance_id: String,
    observer: Arc<dyn AsrRuntimeObserver>,
    request: AsrTranscriptionRequest,
    writer: Arc<Mutex<Option<MistralWriter>>>,
    stopping: Arc<AtomicBool>,
    final_response_received: Arc<Notify>,
    final_response_outcome: Arc<Mutex<Option<Result<(), SherpaError>>>>,
    reader_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    last_frame_sequence: Arc<AtomicU64>,
    last_frame_end_sample: Arc<AtomicU64>,
}

pub fn create_mistral_streaming_session(
    instance_id: String,
    request: AsrTranscriptionRequest,
    observer: Arc<dyn AsrRuntimeObserver>,
) -> Result<Arc<dyn AsrStreamingSession>, AsrPortError> {
    if request.mode != AsrMode::Streaming {
        return Err(AsrPortError::invalid_request(
            "Mistral provider can only be used in streaming mode.",
        ));
    }
    let _ = resolve_online_provider_config(&request, MISTRAL_VOXTRAL_PROVIDER_ID)?;
    Ok(Arc::new(MistralStreamingSession {
        instance_id,
        observer,
        request,
        writer: Arc::new(Mutex::new(None)),
        stopping: Arc::new(AtomicBool::new(false)),
        final_response_received: Arc::new(Notify::new()),
        final_response_outcome: Arc::new(Mutex::new(None)),
        reader_task: Arc::new(Mutex::new(None)),
        last_frame_sequence: Arc::new(AtomicU64::new(0)),
        last_frame_end_sample: Arc::new(AtomicU64::new(0)),
    }))
}

#[async_trait]
impl AsrStreamingSession for MistralStreamingSession {
    async fn start(&self) -> Result<(), AsrPortError> {
        self.last_frame_sequence.store(0, Ordering::Release);
        self.last_frame_end_sample.store(0, Ordering::Release);
        start_streaming_recognizer_impl(self.observer.clone(), self, &self.instance_id)
            .await
            .map_err(AsrPortError::from)
    }

    async fn stop(&self) -> Result<(), AsrPortError> {
        stop_streaming_recognizer_impl(self)
            .await
            .map_err(AsrPortError::from)
    }

    async fn flush(&self) -> Result<(), AsrPortError> {
        flush_streaming_recognizer_impl(self)
            .await
            .map_err(AsrPortError::from)
    }

    async fn feed_audio_frame(&self, frame: AsrAudioFrame) -> Result<(), AsrPortError> {
        self.last_frame_sequence
            .store(frame.sequence, Ordering::Release);
        self.last_frame_end_sample
            .store(frame.end_sample(), Ordering::Release);
        feed_audio_samples_impl(self, &frame.samples)
            .await
            .map_err(AsrPortError::from)
    }
}

pub fn build_mistral_streaming_url(base_endpoint: &str, model: &str, language: &str) -> String {
    let base = base_endpoint.trim_end_matches('/');
    let mut base_url = base.to_string();
    if let Some(scheme_end) = base.find("://") {
        let after_scheme = &base[scheme_end + 3..];
        if !after_scheme.contains('/') {
            base_url.push('/');
        }
    }
    let sep = if base_url.contains('?') { '&' } else { '?' };
    let model_id = if model.trim().is_empty() || model.trim() == "mistral-small-latest" {
        DEFAULT_MISTRAL_STREAMING_MODEL
    } else {
        model.trim()
    };
    let mut url = format!("{base_url}{sep}model={model_id}");
    let lang = language.trim();
    if !lang.is_empty() && lang != "auto" {
        url.push_str(&format!("&language={lang}"));
    }
    url
}

pub fn mistral_streaming_segments_from_response(
    body: &Value,
    accumulated_text: &mut String,
    id_prefix: &str,
    segment_index: usize,
) -> Option<(Vec<TranscriptSegment>, bool)> {
    let event_type = body
        .get("type")
        .or_else(|| body.get("event"))
        .and_then(Value::as_str)
        .unwrap_or("");

    if event_type == "TranscriptionStreamDone" || event_type == "done" {
        if accumulated_text.trim().is_empty() {
            return None;
        }
        let segment = TranscriptSegment {
            id: format!("{id_prefix}-{segment_index}"),
            text: accumulated_text.trim().to_string(),
            start: 0.0,
            end: 0.0,
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            speaker: None,
            speaker_attribution: None,
        };
        return Some((vec![segment], true));
    }

    if event_type == "TranscriptionStreamTextDelta" || event_type == "text_delta" {
        let delta = body
            .get("text")
            .or_else(|| body.get("delta"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        if delta.is_empty() {
            return None;
        }
        accumulated_text.push_str(delta);
        let segment = TranscriptSegment {
            id: format!("{id_prefix}-{segment_index}"),
            text: accumulated_text.trim().to_string(),
            start: 0.0,
            end: 0.0,
            is_final: false,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            speaker: None,
            speaker_attribution: None,
        };
        return Some((vec![segment], false));
    }

    None
}

async fn start_streaming_recognizer_impl(
    observer: Arc<dyn AsrRuntimeObserver>,
    session: &MistralStreamingSession,
    instance_id: &str,
) -> Result<(), SherpaError> {
    session.stopping.store(false, Ordering::SeqCst);
    *session.final_response_outcome.lock().await = None;

    let config = resolve_online_provider_config(&session.request, MISTRAL_VOXTRAL_PROVIDER_ID)
        .map_err(|e| SherpaError::Generic(e.to_string()))?;

    let base_endpoint = config
        .streaming_endpoint
        .as_deref()
        .unwrap_or(DEFAULT_MISTRAL_STREAMING_ENDPOINT);

    let full_url =
        build_mistral_streaming_url(base_endpoint, &config.model, &session.request.language);

    let mut client_request = full_url.as_str().into_client_request().map_err(|error| {
        SherpaError::StreamingEndpointInvalid {
            provider: "Mistral",
            error: error.to_string(),
        }
    })?;

    insert_header(
        client_request.headers_mut(),
        "Authorization",
        &format!("Bearer {}", config.api_key),
        "Mistral",
    )?;

    let (ws, _response) = tokio_tungstenite::connect_async(client_request)
        .await
        .map_err(|error| SherpaError::StreamingConnectionFailed {
            provider: "Mistral",
            error: error.to_string(),
        })?;

    info!("[Mistral ASR] websocket connected to {base_endpoint}");

    let (writer, mut reader) = ws.split();
    {
        let mut writer_slot = session.writer.lock().await;
        *writer_slot = Some(writer);
    }

    let instance_id_for_task = instance_id.to_string();
    let normalization_options = session.request.normalization_options;
    let postprocessor =
        TranscriptPostprocessor::compile(session.request.postprocess_options.clone())
            .map_err(|error| SherpaError::Generic(error.to_string()))?;
    let stopping = session.stopping.clone();
    let final_response_received = session.final_response_received.clone();
    let final_response_outcome = session.final_response_outcome.clone();
    let observer_for_task = observer.clone();
    let last_frame_sequence = Arc::clone(&session.last_frame_sequence);
    let last_frame_end_sample = Arc::clone(&session.last_frame_end_sample);

    let reader_task = tokio::spawn(async move {
        let mut accumulated_text = String::new();
        let mut segment_index = 0;

        while let Some(message) = reader.next().await {
            match message {
                Ok(Message::Text(text)) => match serde_json::from_str::<Value>(&text) {
                    Ok(body) => {
                        let event_type = body
                            .get("type")
                            .or_else(|| body.get("event"))
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        if event_type == "RealtimeTranscriptionError" {
                            let err_msg = body
                                .get("message")
                                .or_else(|| body.get("error"))
                                .and_then(Value::as_str)
                                .unwrap_or("Unknown error");
                            let error = SherpaError::StreamingResponseParseFailed {
                                provider: "Mistral",
                                error: err_msg.to_string(),
                            };
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

                        if let Some((segments, is_final)) = mistral_streaming_segments_from_response(
                            &body,
                            &mut accumulated_text,
                            "mistral-live",
                            segment_index,
                        ) {
                            let normalized = normalize_segments(segments, normalization_options);
                            let processed = postprocessor.process_segments(normalized);
                            let has_final_segment =
                                processed.iter().any(|segment| segment.is_final);
                            for segment in processed {
                                let update =
                                    build_transcript_update(segment, normalization_options);
                                observe_transcript_update(
                                    observer_for_task.as_ref(),
                                    &instance_id_for_task,
                                    "mistral_streaming",
                                    &update,
                                );
                            }
                            if has_final_segment || is_final {
                                observer_for_task.on_stream_boundary(&AsrStreamBoundaryEvent {
                                    instance_id: instance_id_for_task.clone(),
                                    sequence: last_frame_sequence.load(Ordering::Acquire),
                                    end_sample: last_frame_end_sample.load(Ordering::Acquire),
                                });
                            }
                            if is_final {
                                accumulated_text.clear();
                                segment_index += 1;
                                publish_reader_outcome(
                                    &final_response_outcome,
                                    &final_response_received,
                                    Ok(()),
                                )
                                .await;
                            }
                        }
                    }
                    Err(error) => {
                        warn!("[Mistral ASR] json parse failed: {error}");
                    }
                },
                Ok(Message::Close(_)) => {
                    let error = SherpaError::StreamingWebSocketClosed {
                        provider: "Mistral",
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
                        if stopping.load(Ordering::SeqCst) {
                            Ok(())
                        } else {
                            Err(error)
                        },
                    )
                    .await;
                    return;
                }
                Ok(_) => {}
                Err(error) => {
                    warn!("[Mistral ASR] websocket read failed: {error}");
                    let error = SherpaError::StreamingWebSocketReadFailed {
                        provider: "Mistral",
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

        publish_reader_outcome(&final_response_outcome, &final_response_received, Ok(())).await;
    });

    *session.reader_task.lock().await = Some(reader_task);
    Ok(())
}

async fn feed_audio_samples_impl(
    session: &MistralStreamingSession,
    samples: &[f32],
) -> Result<(), SherpaError> {
    let mut writer_guard = session.writer.lock().await;
    let Some(writer) = writer_guard.as_mut() else {
        return Ok(());
    };
    let pcm_bytes = f32_samples_to_i16_pcm_bytes(samples);
    if pcm_bytes.is_empty() {
        return Ok(());
    }
    writer
        .send(Message::Binary(pcm_bytes.into()))
        .await
        .map_err(|error| SherpaError::StreamingAudioSendFailed {
            provider: "Mistral",
            error: error.to_string(),
        })?;
    Ok(())
}

async fn flush_streaming_recognizer_impl(
    session: &MistralStreamingSession,
) -> Result<(), SherpaError> {
    let mut writer_guard = session.writer.lock().await;
    let Some(writer) = writer_guard.as_mut() else {
        return Err(SherpaError::StreamingWebSocketNotConnected {
            provider: "Mistral",
        });
    };
    let notified = session.final_response_received.notified();
    tokio::pin!(notified);
    notified.as_mut().enable();

    let _ = writer
        .send(Message::Text(r#"{"type":"close"}"#.into()))
        .await;
    drop(writer_guard);

    if tokio::time::timeout(Duration::from_millis(1500), notified)
        .await
        .is_ok()
    {
        return session
            .final_response_outcome
            .lock()
            .await
            .clone()
            .unwrap_or(Ok(()));
    }

    Ok(())
}

async fn stop_streaming_recognizer_impl(
    session: &MistralStreamingSession,
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
    use parking_lot::Mutex;
    use sona_core::ports::asr::{
        AsrAudioFrame, AsrEngineConfig, AsrMode, AsrRuntimeObserver, AsrStreamBoundaryEvent,
        AsrStreamingErrorEvent, AsrTranscriptUpdateEvent, OnlineAsrProviderRequest,
    };
    use sona_core::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
    use sona_core::transcription::postprocess::{
        TranscriptNormalizationOptions, TranscriptPostprocessOptions,
    };

    #[derive(Default)]
    struct RecordingObserver {
        transcript_events: Mutex<Vec<AsrTranscriptUpdateEvent>>,
        boundary_events: Mutex<Vec<AsrStreamBoundaryEvent>>,
        error_events: Mutex<Vec<AsrStreamingErrorEvent>>,
    }

    impl AsrRuntimeObserver for RecordingObserver {
        fn on_transcript_update(&self, event: &AsrTranscriptUpdateEvent) {
            self.transcript_events.lock().push(event.clone());
        }
        fn on_model_load(&self, _metric: &AsrModelLoadMetric) {}
        fn on_live_inference(&self, _metric: &AsrInferenceMetric) {}
        fn on_stream_boundary(&self, event: &AsrStreamBoundaryEvent) {
            self.boundary_events.lock().push(event.clone());
        }
        fn on_streaming_error(&self, event: &AsrStreamingErrorEvent) {
            self.error_events.lock().push(event.clone());
        }
    }

    fn test_request(endpoint: &str, mode: AsrMode) -> AsrTranscriptionRequest {
        let mut config = serde_json::Map::new();
        config.insert(
            "apiKey".to_string(),
            Value::String("mistral-test-key".to_string()),
        );
        config.insert(
            "streamingEndpoint".to_string(),
            Value::String(endpoint.to_string()),
        );
        config.insert(
            "model".to_string(),
            Value::String("voxtral-mini-transcribe-realtime-2602".to_string()),
        );
        AsrTranscriptionRequest {
            mode,
            language: "en".to_string(),
            enable_itn: true,
            normalization_options: TranscriptNormalizationOptions::default(),
            postprocess_options: TranscriptPostprocessOptions::default(),
            hotwords: None,
            speaker_processing: None,
            engine_config: AsrEngineConfig::Online {
                provider: OnlineAsrProviderRequest {
                    provider_id: MISTRAL_VOXTRAL_PROVIDER_ID.to_string(),
                    profile_id: "mistral-voxtral-default".to_string(),
                    config: Value::Object(config),
                },
            },
        }
    }

    #[test]
    fn parse_mistral_streaming_response() {
        let mut text_buf = String::new();
        let delta1 = serde_json::json!({
            "type": "TranscriptionStreamTextDelta",
            "text": "Hello "
        });
        let res1 = mistral_streaming_segments_from_response(&delta1, &mut text_buf, "m-test", 0);
        assert!(res1.is_some());
        let (segments1, is_final1) = res1.unwrap();
        assert!(!is_final1);
        assert_eq!(segments1[0].text, "Hello");

        let delta2 = serde_json::json!({
            "type": "TranscriptionStreamTextDelta",
            "text": "world!"
        });
        let res2 = mistral_streaming_segments_from_response(&delta2, &mut text_buf, "m-test", 0);
        assert!(res2.is_some());
        let (segments2, is_final2) = res2.unwrap();
        assert!(!is_final2);
        assert_eq!(segments2[0].text, "Hello world!");

        let done = serde_json::json!({
            "type": "TranscriptionStreamDone"
        });
        let res3 = mistral_streaming_segments_from_response(&done, &mut text_buf, "m-test", 0);
        assert!(res3.is_some());
        let (segments3, is_final3) = res3.unwrap();
        assert!(is_final3);
        assert_eq!(segments3[0].text, "Hello world!");
    }

    #[tokio::test]
    async fn mistral_mock_websocket_flow() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();

            // Receive binary audio frame
            if let Some(Ok(Message::Binary(_))) = ws.next().await {
                // Send text delta
                let delta = serde_json::json!({
                    "type": "TranscriptionStreamTextDelta",
                    "text": "Bonjour le monde"
                });
                ws.send(Message::Text(delta.to_string().into()))
                    .await
                    .unwrap();
            }

            // Wait for close message or end
            if let Some(Ok(Message::Text(_))) = ws.next().await {
                let done = serde_json::json!({
                    "type": "TranscriptionStreamDone"
                });
                ws.send(Message::Text(done.to_string().into()))
                    .await
                    .unwrap();
                let _ = ws.close(None).await;
            }
        });

        let observer = Arc::new(RecordingObserver::default());
        let request = test_request(&format!("ws://{address}"), AsrMode::Streaming);
        let session =
            create_mistral_streaming_session("m-sess-1".to_string(), request, observer.clone())
                .unwrap();

        session.start().await.unwrap();
        session
            .feed_audio_frame(AsrAudioFrame {
                samples: vec![0.1; 1600].into(),
                sequence: 1,
                start_sample: 0,
            })
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;

        session.flush().await.unwrap();
        session.stop().await.unwrap();

        let events = observer.transcript_events.lock();
        assert!(!events.is_empty());
        let has_final = events.iter().any(|e| {
            e.update
                .upsert_segments
                .iter()
                .any(|s| s.text.contains("Bonjour le monde"))
        });
        assert!(has_final);
    }
}
