use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use log::{info, warn};
use serde_json::Value;
use sona_core::ports::asr::{
    ASSEMBLYAI_PROVIDER_ID, AsrAudioFrame, AsrMode, AsrPortError, AsrRuntimeObserver,
    AsrStreamBoundaryEvent, AsrStreamingSession, AsrTranscriptionRequest,
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

use crate::aimux_adapter::resolve_online_provider_config;
use crate::create_cloud_speaker;
use crate::error::SherpaError;
use crate::f32_samples_to_i16_pcm_bytes;
use crate::streaming_common::{
    build_transcript_update, insert_header, normalize_segments, observe_streaming_error,
    observe_transcript_update, publish_reader_outcome,
};

const DEFAULT_ASSEMBLYAI_STREAMING_ENDPOINT: &str = "wss://streaming.assemblyai.com/v3/ws";

type AssemblyAIWriter = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;

#[derive(Clone)]
pub struct AssemblyAIStreamingSession {
    instance_id: String,
    observer: Arc<dyn AsrRuntimeObserver>,
    request: AsrTranscriptionRequest,
    writer: Arc<Mutex<Option<AssemblyAIWriter>>>,
    stopping: Arc<AtomicBool>,
    final_response_received: Arc<Notify>,
    final_response_outcome: Arc<Mutex<Option<Result<(), SherpaError>>>>,
    reader_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    last_frame_sequence: Arc<AtomicU64>,
    last_frame_end_sample: Arc<AtomicU64>,
}

pub fn create_assemblyai_streaming_session(
    instance_id: String,
    request: AsrTranscriptionRequest,
    observer: Arc<dyn AsrRuntimeObserver>,
) -> Result<Arc<dyn AsrStreamingSession>, AsrPortError> {
    if request.mode != AsrMode::Streaming {
        return Err(AsrPortError::invalid_request(
            "AssemblyAI provider can only be used in streaming mode.",
        ));
    }
    let _ = resolve_online_provider_config(&request, ASSEMBLYAI_PROVIDER_ID)?;
    Ok(Arc::new(AssemblyAIStreamingSession {
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
impl AsrStreamingSession for AssemblyAIStreamingSession {
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

pub fn build_assemblyai_streaming_url(base_endpoint: &str, model: &str) -> String {
    let base = base_endpoint.trim_end_matches('/');
    let mut base_url = base.to_string();
    if let Some(scheme_end) = base.find("://") {
        let after_scheme = &base[scheme_end + 3..];
        if !after_scheme.contains('/') {
            base_url.push('/');
        }
    }
    let sep = if base_url.contains('?') { '&' } else { '?' };
    let model_id = if model.trim().is_empty() || model.trim() == "best" {
        "universal-3-5-pro"
    } else {
        model.trim()
    };
    format!("{base_url}{sep}sample_rate=16000&format_turns=true&speech_model={model_id}")
}

pub fn assemblyai_streaming_segments_from_response(
    body: &Value,
    id_prefix: &str,
) -> Option<(Vec<TranscriptSegment>, bool)> {
    let msg_type = body.get("type").and_then(Value::as_str).unwrap_or("");
    if msg_type != "Turn" {
        return None;
    }

    let end_of_turn = body
        .get("end_of_turn")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let transcript = body
        .get("transcript")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();

    let words = body.get("words").and_then(Value::as_array);

    if transcript.is_empty() && words.is_none_or(|w| w.is_empty()) {
        return None;
    }

    let turn_order = body.get("turn_order").and_then(Value::as_u64).unwrap_or(0);

    let mut timing_units = Vec::new();
    let mut tokens = Vec::new();
    let mut timestamps = Vec::new();
    let mut durations = Vec::new();
    let mut speaker_id: Option<String> = None;
    let mut min_start: Option<f64> = None;
    let mut max_end: Option<f64> = None;

    if let Some(words) = words {
        for w in words {
            let w_text = w
                .get("text")
                .or_else(|| w.get("word"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim();
            if w_text.is_empty() {
                continue;
            }
            let w_start = w.get("start").and_then(Value::as_f64).unwrap_or(0.0) / 1000.0;
            let w_end = w
                .get("end")
                .and_then(Value::as_f64)
                .unwrap_or(w_start * 1000.0)
                / 1000.0;

            if min_start.is_none() || Some(w_start) < min_start {
                min_start = Some(w_start);
            }
            if max_end.is_none() || Some(w_end) > max_end {
                max_end = Some(w_end);
            }

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

    let start = min_start.unwrap_or(0.0);
    let end = max_end.unwrap_or(start);

    let (speaker, speaker_attribution) = match speaker_id {
        Some(id) => {
            let (tag, attr) = create_cloud_speaker(&id);
            (Some(tag), Some(attr))
        }
        None => (None, None),
    };

    let segment = TranscriptSegment {
        id: format!("{id_prefix}-{turn_order}"),
        text: transcript.to_string(),
        start,
        end: end.max(start),
        is_final: end_of_turn,
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

    Some((vec![segment], end_of_turn))
}

async fn start_streaming_recognizer_impl(
    observer: Arc<dyn AsrRuntimeObserver>,
    session: &AssemblyAIStreamingSession,
    instance_id: &str,
) -> Result<(), SherpaError> {
    session.stopping.store(false, Ordering::SeqCst);
    *session.final_response_outcome.lock().await = None;

    let config = resolve_online_provider_config(&session.request, ASSEMBLYAI_PROVIDER_ID)
        .map_err(|e| SherpaError::Generic(e.to_string()))?;

    let base_endpoint = config
        .streaming_endpoint
        .as_deref()
        .unwrap_or(DEFAULT_ASSEMBLYAI_STREAMING_ENDPOINT);

    let full_url = build_assemblyai_streaming_url(base_endpoint, &config.model);

    let mut client_request = full_url.as_str().into_client_request().map_err(|error| {
        SherpaError::StreamingEndpointInvalid {
            provider: "AssemblyAI",
            error: error.to_string(),
        }
    })?;

    insert_header(
        client_request.headers_mut(),
        "Authorization",
        &config.api_key,
        "AssemblyAI",
    )?;

    let (ws, _response) = tokio_tungstenite::connect_async(client_request)
        .await
        .map_err(|error| SherpaError::StreamingConnectionFailed {
            provider: "AssemblyAI",
            error: error.to_string(),
        })?;

    info!("[AssemblyAI ASR] websocket connected to {base_endpoint}");

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
                        let msg_type = body.get("type").and_then(Value::as_str).unwrap_or("");
                        if msg_type == "Termination" {
                            publish_reader_outcome(
                                &final_response_outcome,
                                &final_response_received,
                                Ok(()),
                            )
                            .await;
                            return;
                        }

                        if let Some((segments, is_final)) =
                            assemblyai_streaming_segments_from_response(&body, "aai-live")
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
                                    "assemblyai_streaming",
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
                        warn!("[AssemblyAI ASR] json parse failed: {error}");
                    }
                },
                Ok(Message::Close(_)) => {
                    let error = SherpaError::StreamingWebSocketClosed {
                        provider: "AssemblyAI",
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
                    warn!("[AssemblyAI ASR] websocket read failed: {error}");
                    let error = SherpaError::StreamingWebSocketReadFailed {
                        provider: "AssemblyAI",
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
    session: &AssemblyAIStreamingSession,
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
            provider: "AssemblyAI",
            error: error.to_string(),
        })?;
    Ok(())
}

async fn flush_streaming_recognizer_impl(
    session: &AssemblyAIStreamingSession,
) -> Result<(), SherpaError> {
    let mut writer_guard = session.writer.lock().await;
    let Some(writer) = writer_guard.as_mut() else {
        return Err(SherpaError::StreamingWebSocketNotConnected {
            provider: "AssemblyAI",
        });
    };
    let notified = session.final_response_received.notified();
    tokio::pin!(notified);
    notified.as_mut().enable();

    writer
        .send(Message::Text(r#"{"type":"Terminate"}"#.into()))
        .await
        .map_err(|error| SherpaError::StreamingEndFrameSendFailed {
            provider: "AssemblyAI",
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
    session: &AssemblyAIStreamingSession,
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
            Value::String("aai-test-key".to_string()),
        );
        config.insert(
            "streamingEndpoint".to_string(),
            Value::String(endpoint.to_string()),
        );
        config.insert(
            "model".to_string(),
            Value::String("universal-3-5-pro".to_string()),
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
                    provider_id: ASSEMBLYAI_PROVIDER_ID.to_string(),
                    profile_id: "assemblyai-default".to_string(),
                    config: Value::Object(config),
                },
            },
        }
    }

    #[test]
    fn parse_assemblyai_streaming_turn_response() {
        let json: Value = serde_json::json!({
            "type": "Turn",
            "turn_order": 1,
            "turn_is_formatted": true,
            "end_of_turn": true,
            "transcript": "Good morning world",
            "words": [
                {
                    "text": "Good",
                    "start": 100,
                    "end": 350,
                    "confidence": 0.98,
                    "speaker": "A"
                },
                {
                    "text": "morning",
                    "start": 360,
                    "end": 600,
                    "confidence": 0.99,
                    "speaker": "A"
                },
                {
                    "text": "world",
                    "start": 610,
                    "end": 950,
                    "confidence": 0.97,
                    "speaker": "A"
                }
            ]
        });

        let parsed = assemblyai_streaming_segments_from_response(&json, "aai-test");
        assert!(parsed.is_some());
        let (segments, is_final) = parsed.unwrap();
        assert!(is_final);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "Good morning world");
        assert_eq!(segments[0].start, 0.1);
        assert_eq!(segments[0].end, 0.95);
        assert_eq!(segments[0].tokens.as_ref().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn assemblyai_mock_websocket_flow() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();

            // Send Begin event
            let begin = serde_json::json!({
                "type": "Begin",
                "id": "aai-sess-123",
                "expires_at": 1800000000.0
            });
            ws.send(Message::Text(begin.to_string().into()))
                .await
                .unwrap();

            // Receive binary audio frame
            if let Some(Ok(Message::Binary(_))) = ws.next().await {
                // Send partial turn
                let partial = serde_json::json!({
                    "type": "Turn",
                    "turn_order": 1,
                    "end_of_turn": false,
                    "transcript": "Good morning",
                    "words": [{
                        "text": "Good",
                        "start": 100,
                        "end": 300,
                        "confidence": 0.95
                    }]
                });
                ws.send(Message::Text(partial.to_string().into()))
                    .await
                    .unwrap();
            }

            // Wait for Terminate message
            if let Some(Ok(Message::Text(text))) = ws.next().await
                && text.contains("Terminate")
            {
                let final_msg = serde_json::json!({
                    "type": "Turn",
                    "turn_order": 1,
                    "end_of_turn": true,
                    "transcript": "Good morning world",
                    "words": [
                        {"text": "Good", "start": 100, "end": 300, "confidence": 0.95},
                        {"text": "morning", "start": 310, "end": 600, "confidence": 0.98},
                        {"text": "world", "start": 610, "end": 900, "confidence": 0.99}
                    ]
                });
                ws.send(Message::Text(final_msg.to_string().into()))
                    .await
                    .unwrap();

                let termination = serde_json::json!({
                    "type": "Termination",
                    "audio_duration_seconds": 1.0,
                    "session_duration_seconds": 1.5
                });
                ws.send(Message::Text(termination.to_string().into()))
                    .await
                    .unwrap();
                let _ = ws.close(None).await;
            }
        });

        let observer = Arc::new(RecordingObserver::default());
        let request = test_request(&format!("ws://{address}"), AsrMode::Streaming);
        let session = create_assemblyai_streaming_session(
            "aai-sess-1".to_string(),
            request,
            observer.clone(),
        )
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
                .any(|s| s.text.contains("Good morning world"))
        });
        assert!(has_final);
    }
}
