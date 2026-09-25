use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use log::{info, warn};
use serde_json::Value;
use sona_core::ports::asr::{
    AsrAudioFrame, AsrMode, AsrPortError, AsrRuntimeObserver, AsrStreamBoundaryEvent,
    AsrStreamingSession, AsrTranscriptionRequest, DEEPGRAM_PROVIDER_ID,
};
use sona_core::transcription::postprocess::TranscriptPostprocessor;
use sona_core::transcription::transcript::{
    TranscriptSegment, TranscriptTiming, TranscriptTimingLevel, TranscriptTimingSource,
    TranscriptTimingUnit,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::Duration;
use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::Message;

use crate::aimux_adapter::{is_cloud_speaker_diarization_enabled, resolve_online_provider_config};
use crate::create_cloud_speaker;
use crate::error::SherpaError;
use crate::f32_samples_to_i16_pcm_bytes;
use crate::streaming_common::{
    build_transcript_update, insert_header, normalize_segments, observe_streaming_error,
    observe_transcript_update, publish_reader_outcome,
};

const DEFAULT_DEEPGRAM_STREAMING_ENDPOINT: &str = "wss://api.deepgram.com/v1/listen";

type DeepgramWriter = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;

#[derive(Clone)]
pub struct DeepgramStreamingSession {
    instance_id: String,
    observer: Arc<dyn AsrRuntimeObserver>,
    request: AsrTranscriptionRequest,
    writer: Arc<Mutex<Option<DeepgramWriter>>>,
    stopping: Arc<AtomicBool>,
    final_response_received: Arc<Notify>,
    final_response_outcome: Arc<Mutex<Option<Result<(), SherpaError>>>>,
    reader_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    last_frame_sequence: Arc<AtomicU64>,
    last_frame_end_sample: Arc<AtomicU64>,
}

pub fn create_deepgram_streaming_session(
    instance_id: String,
    request: AsrTranscriptionRequest,
    observer: Arc<dyn AsrRuntimeObserver>,
) -> Result<Arc<dyn AsrStreamingSession>, AsrPortError> {
    if request.mode != AsrMode::Streaming {
        return Err(AsrPortError::invalid_request(
            "Deepgram provider can only be used in streaming mode.",
        ));
    }
    let _ = resolve_online_provider_config(&request, DEEPGRAM_PROVIDER_ID)?;
    Ok(Arc::new(DeepgramStreamingSession {
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
impl AsrStreamingSession for DeepgramStreamingSession {
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

pub fn build_deepgram_streaming_url(
    base_endpoint: &str,
    model: &str,
    language: &str,
    diarization_enabled: bool,
) -> String {
    let base = base_endpoint.trim_end_matches('/');
    let mut base_url = base.to_string();
    if let Some(scheme_end) = base.find("://") {
        let after_scheme = &base[scheme_end + 3..];
        if !after_scheme.contains('/') {
            base_url.push('/');
        }
    }
    let sep = if base_url.contains('?') { '&' } else { '?' };
    let mut url = format!(
        "{base_url}{sep}encoding=linear16&sample_rate=16000&channels=1&smart_format=true&interim_results=true"
    );
    let model_id = if model.trim().is_empty() {
        "nova-2"
    } else {
        model.trim()
    };
    url.push_str(&format!("&model={model_id}"));

    if diarization_enabled {
        url.push_str("&diarize_model=latest");
    }

    let lang = language.trim();
    if !lang.is_empty() && lang != "auto" {
        url.push_str(&format!("&language={lang}"));
    }

    url
}

pub fn deepgram_streaming_segments_from_response(
    body: &Value,
    id_prefix: &str,
) -> Option<(Vec<TranscriptSegment>, bool)> {
    let msg_type = body.get("type").and_then(Value::as_str).unwrap_or("");
    if msg_type == "Metadata" || msg_type == "UtteranceEnd" {
        return None;
    }

    let is_final = body
        .get("is_final")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let alt = body
        .get("channel")
        .and_then(|c| c.get("alternatives"))
        .and_then(Value::as_array)
        .and_then(|a| a.first())?;

    let transcript = alt
        .get("transcript")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();

    let words = alt.get("words").and_then(Value::as_array);

    if transcript.is_empty() && words.is_none_or(|w| w.is_empty()) {
        return None;
    }

    let start = body.get("start").and_then(Value::as_f64).unwrap_or(0.0);
    let duration = body.get("duration").and_then(Value::as_f64).unwrap_or(0.0);
    let end = start + duration;

    let mut timing_units = Vec::new();
    let mut tokens = Vec::new();
    let mut timestamps = Vec::new();
    let mut durations = Vec::new();
    let mut speaker_id: Option<String> = None;

    if let Some(words) = words {
        for w in words {
            let w_text = w
                .get("punctuated_word")
                .or_else(|| w.get("word"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim();
            if w_text.is_empty() {
                continue;
            }
            let w_start = w.get("start").and_then(Value::as_f64).unwrap_or(start);
            let w_end = w.get("end").and_then(Value::as_f64).unwrap_or(w_start);

            if speaker_id.is_none() {
                speaker_id = w.get("speaker").map(|s| s.to_string());
            }

            tokens.push(w_text.to_string());
            timestamps.push(w_start as f32);
            durations.push((w_end.max(w_start) - w_start) as f32);
            timing_units.push(TranscriptTimingUnit {
                text: w_text.to_string(),
                start: w_start,
                end: w_end.max(w_start),
            });
        }
    }

    let (speaker, speaker_attribution) = match speaker_id {
        Some(id) => {
            let (tag, attr) = create_cloud_speaker(&id);
            (Some(tag), Some(attr))
        }
        None => (None, None),
    };

    let segment = TranscriptSegment {
        id: format!("{id_prefix}-0"),
        text: transcript.to_string(),
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
        speaker,
        speaker_attribution,
    };

    Some((vec![segment], is_final))
}

async fn start_streaming_recognizer_impl(
    observer: Arc<dyn AsrRuntimeObserver>,
    session: &DeepgramStreamingSession,
    instance_id: &str,
) -> Result<(), SherpaError> {
    session.stopping.store(false, Ordering::SeqCst);
    *session.final_response_outcome.lock().await = None;

    let config = resolve_online_provider_config(&session.request, DEEPGRAM_PROVIDER_ID)
        .map_err(|e| SherpaError::Generic(e.to_string()))?;

    let base_endpoint = config
        .streaming_endpoint
        .as_deref()
        .unwrap_or(DEFAULT_DEEPGRAM_STREAMING_ENDPOINT);

    let diarization_enabled = is_cloud_speaker_diarization_enabled(&session.request);
    let full_url = build_deepgram_streaming_url(
        base_endpoint,
        &config.model,
        &session.request.language,
        diarization_enabled,
    );

    let mut client_request = full_url.as_str().into_client_request().map_err(|error| {
        SherpaError::StreamingEndpointInvalid {
            provider: "Deepgram",
            error: error.to_string(),
        }
    })?;

    insert_header(
        client_request.headers_mut(),
        "Authorization",
        &format!("Token {}", config.api_key),
        "Deepgram",
    )?;

    let (ws, _response) = tokio_tungstenite::connect_async(client_request)
        .await
        .map_err(|error| SherpaError::StreamingConnectionFailed {
            provider: "Deepgram",
            error: error.to_string(),
        })?;

    info!("[Deepgram ASR] websocket connected to {base_endpoint}");

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
        while let Some(message) = reader.next().await {
            match message {
                Ok(Message::Text(text)) => match serde_json::from_str::<Value>(&text) {
                    Ok(body) => {
                        if let Some((segments, is_final)) =
                            deepgram_streaming_segments_from_response(&body, "dg-live")
                        {
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
                                    "deepgram_streaming",
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
                        warn!("[Deepgram ASR] json parse failed: {error}");
                    }
                },
                Ok(Message::Close(_)) => {
                    let error = SherpaError::StreamingWebSocketClosed {
                        provider: "Deepgram",
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
                    warn!("[Deepgram ASR] websocket read failed: {error}");
                    let error = SherpaError::StreamingWebSocketReadFailed {
                        provider: "Deepgram",
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
    session: &DeepgramStreamingSession,
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
            provider: "Deepgram",
            error: error.to_string(),
        })?;
    Ok(())
}

async fn flush_streaming_recognizer_impl(
    session: &DeepgramStreamingSession,
) -> Result<(), SherpaError> {
    let mut writer_guard = session.writer.lock().await;
    let Some(writer) = writer_guard.as_mut() else {
        return Err(SherpaError::StreamingWebSocketNotConnected {
            provider: "Deepgram",
        });
    };
    let notified = session.final_response_received.notified();
    tokio::pin!(notified);
    notified.as_mut().enable();

    writer
        .send(Message::Text(r#"{"type":"CloseStream"}"#.into()))
        .await
        .map_err(|error| SherpaError::StreamingEndFrameSendFailed {
            provider: "Deepgram",
            error: error.to_string(),
        })?;
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
    session: &DeepgramStreamingSession,
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
            Value::String("dg-test-key".to_string()),
        );
        config.insert(
            "streamingEndpoint".to_string(),
            Value::String(endpoint.to_string()),
        );
        config.insert("model".to_string(), Value::String("nova-2".to_string()));
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
                    provider_id: DEEPGRAM_PROVIDER_ID.to_string(),
                    profile_id: "deepgram-default".to_string(),
                    config: Value::Object(config),
                },
            },
        }
    }

    #[test]
    fn parse_deepgram_streaming_response() {
        let json: Value = serde_json::json!({
            "type": "Results",
            "start": 0.0,
            "duration": 1.5,
            "is_final": true,
            "channel": {
                "alternatives": [{
                    "transcript": "Hello world",
                    "confidence": 0.99,
                    "words": [
                        {
                            "word": "hello",
                            "start": 0.1,
                            "end": 0.5,
                            "confidence": 0.99,
                            "speaker": 0,
                            "punctuated_word": "Hello"
                        },
                        {
                            "word": "world",
                            "start": 0.6,
                            "end": 1.2,
                            "confidence": 0.98,
                            "speaker": 0,
                            "punctuated_word": "world"
                        }
                    ]
                }]
            }
        });

        let parsed = deepgram_streaming_segments_from_response(&json, "dg-test");
        assert!(parsed.is_some());
        let (segments, is_final) = parsed.unwrap();
        assert!(is_final);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "Hello world");
        assert_eq!(segments[0].tokens.as_ref().unwrap().len(), 2);
    }
    #[tokio::test]
    async fn deepgram_mock_websocket_flow() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();

            // Receive binary audio frame
            if let Some(Ok(Message::Binary(_))) = ws.next().await {
                // Send partial transcript
                let partial = serde_json::json!({
                    "type": "Results",
                    "start": 0.0,
                    "duration": 0.5,
                    "is_final": false,
                    "channel": {
                        "alternatives": [{
                            "transcript": "Hello",
                            "confidence": 0.9,
                            "words": [{
                                "word": "hello",
                                "start": 0.0,
                                "end": 0.4,
                                "confidence": 0.9,
                                "punctuated_word": "Hello"
                            }]
                        }]
                    }
                });
                ws.send(Message::Text(partial.to_string().into()))
                    .await
                    .unwrap();
            }

            // Wait for CloseStream message
            if let Some(Ok(Message::Text(text))) = ws.next().await {
                if text.contains("CloseStream") {
                    let final_msg = serde_json::json!({
                        "type": "Results",
                        "start": 0.0,
                        "duration": 1.0,
                        "is_final": true,
                        "channel": {
                            "alternatives": [{
                                "transcript": "Hello world",
                                "confidence": 0.99,
                                "words": [{
                                    "word": "world",
                                    "start": 0.5,
                                    "end": 1.0,
                                    "confidence": 0.99,
                                    "punctuated_word": "world"
                                }]
                            }]
                        }
                    });
                    ws.send(Message::Text(final_msg.to_string().into()))
                        .await
                        .unwrap();
                }
                let _ = ws.close(None).await;
            }
        });

        let observer = Arc::new(RecordingObserver::default());
        let request = test_request(&format!("ws://{address}"), AsrMode::Streaming);
        let session =
            create_deepgram_streaming_session("dg-sess-1".to_string(), request, observer.clone())
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

        // Give reader loop time to process partial
        tokio::time::sleep(Duration::from_millis(50)).await;

        session.flush().await.unwrap();
        session.stop().await.unwrap();

        let events = observer.transcript_events.lock();
        assert!(!events.is_empty());
        let has_final = events.iter().any(|e| {
            e.update
                .upsert_segments
                .iter()
                .any(|s| s.text.contains("Hello world"))
        });
        assert!(has_final);
    }
}
