use async_trait::async_trait;
use serde_json::json;
use sona_core::ports::asr::{
    AsrAudioFrame, AsrEngineConfig, AsrMode, AsrPortError, AsrRuntimeObserver,
    AsrStreamBoundaryEvent, AsrStreamingErrorEvent, AsrStreamingSession, AsrTranscriptUpdateEvent,
    AsrTranscriptionRequest, NoopAsrRuntimeObserver, OnlineAsrProviderRequest,
    StreamingAudioFrameCursor, StreamingInferenceSpec, StreamingOutputPolicy,
    TranscriptNormalizationOptions, TranscriptPostprocessOptions, TranscriptTextReplacementRule,
    TranscriptTextReplacementRuleSet,
};
use sona_core::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
use sona_core::transcription::transcript::{TranscriptSegment, TranscriptUpdate};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct RecordingObserver {
    updates: Mutex<Vec<AsrTranscriptUpdateEvent>>,
    model_loads: Mutex<Vec<AsrModelLoadMetric>>,
    live_inferences: Mutex<Vec<AsrInferenceMetric>>,
    streaming_errors: Mutex<Vec<AsrStreamingErrorEvent>>,
    boundaries: Mutex<Vec<AsrStreamBoundaryEvent>>,
}

impl AsrRuntimeObserver for RecordingObserver {
    fn on_transcript_update(&self, event: &AsrTranscriptUpdateEvent) {
        self.updates.lock().unwrap().push(event.clone());
    }

    fn on_model_load(&self, metric: &AsrModelLoadMetric) {
        self.model_loads.lock().unwrap().push(metric.clone());
    }

    fn on_live_inference(&self, metric: &AsrInferenceMetric) {
        self.live_inferences.lock().unwrap().push(metric.clone());
    }

    fn on_streaming_error(&self, event: &AsrStreamingErrorEvent) {
        self.streaming_errors.lock().unwrap().push(event.clone());
    }

    fn on_stream_boundary(&self, event: &AsrStreamBoundaryEvent) {
        self.boundaries.lock().unwrap().push(event.clone());
    }
}

struct DummySession;

#[async_trait]
impl AsrStreamingSession for DummySession {
    async fn start(&self) -> Result<(), AsrPortError> {
        Ok(())
    }

    async fn stop(&self) -> Result<(), AsrPortError> {
        Ok(())
    }

    async fn flush(&self) -> Result<(), AsrPortError> {
        Ok(())
    }

    async fn feed_audio_frame(&self, _frame: AsrAudioFrame) -> Result<(), AsrPortError> {
        Ok(())
    }
}

fn model_load_metric() -> AsrModelLoadMetric {
    AsrModelLoadMetric {
        occurred_at_ms: 1,
        instance_id: "live-1".to_string(),
        model_path: "model".to_string(),
        model_type: "sensevoice".to_string(),
        recognizer_kind: "offline".to_string(),
        num_threads: 4,
        reused_from_pool: false,
        load_ms: 12.0,
        rss_before_mb: None,
        rss_after_mb: None,
        rss_delta_mb: None,
        process_rss_mb: None,
    }
}

fn live_metric() -> AsrInferenceMetric {
    AsrInferenceMetric {
        occurred_at_ms: 2,
        source: "live".to_string(),
        instance_id: Some("live-1".to_string()),
        stage: "partial".to_string(),
        is_final: false,
        audio_duration_ms: 100.0,
        buffered_samples: 1_600,
        audio_extract_ms: None,
        decode_ms: 10.0,
        emit_latency_ms: None,
        total_ms: None,
        rtf: Some(0.1),
        segment_count: Some(1),
        process_rss_mb: None,
    }
}

#[test]
fn observer_accepts_typed_streaming_outputs() {
    let observer = RecordingObserver::default();
    let event = AsrTranscriptUpdateEvent {
        instance_id: "live-1".to_string(),
        stage: "partial".to_string(),
        update: TranscriptUpdate {
            remove_ids: Vec::new(),
            upsert_segments: Vec::new(),
        },
    };
    let model_load = model_load_metric();
    let live = live_metric();
    let streaming_error = AsrStreamingErrorEvent {
        instance_id: "live-1".to_string(),
        code: "VOLCENGINE_WEB_SOCKET_CLOSED".to_string(),
        message: "closed".to_string(),
    };
    let boundary = AsrStreamBoundaryEvent {
        instance_id: "live-1".to_string(),
        sequence: 4,
        end_sample: 6_400,
    };

    observer.on_transcript_update(&event);
    observer.on_model_load(&model_load);
    observer.on_live_inference(&live);
    observer.on_streaming_error(&streaming_error);
    observer.on_stream_boundary(&boundary);

    assert_eq!(*observer.updates.lock().unwrap(), vec![event]);
    assert_eq!(*observer.model_loads.lock().unwrap(), vec![model_load]);
    assert_eq!(*observer.live_inferences.lock().unwrap(), vec![live]);
    assert_eq!(
        *observer.streaming_errors.lock().unwrap(),
        vec![streaming_error]
    );
    assert_eq!(*observer.boundaries.lock().unwrap(), vec![boundary]);
}

#[test]
fn noop_observer_accepts_all_outputs() {
    let observer = NoopAsrRuntimeObserver;
    let event = AsrTranscriptUpdateEvent {
        instance_id: "live-1".to_string(),
        stage: "partial".to_string(),
        update: TranscriptUpdate {
            remove_ids: Vec::new(),
            upsert_segments: Vec::new(),
        },
    };

    observer.on_transcript_update(&event);
    observer.on_model_load(&model_load_metric());
    observer.on_live_inference(&live_metric());
    observer.on_streaming_error(&AsrStreamingErrorEvent {
        instance_id: "live-1".to_string(),
        code: "VOLCENGINE_WEB_SOCKET_CLOSED".to_string(),
        message: "closed".to_string(),
    });
}

#[tokio::test]
async fn streaming_session_is_object_safe() {
    let session: Arc<dyn AsrStreamingSession> = Arc::new(DummySession);

    session.start().await.unwrap();
    session
        .feed_audio_frame(AsrAudioFrame::new(1, 0, vec![0.0, 0.25]))
        .await
        .unwrap();
    session.flush().await.unwrap();
    session.stop().await.unwrap();
}

#[test]
fn audio_frame_cursor_assigns_monotonic_sequence_and_sample_boundaries() {
    let mut cursor = StreamingAudioFrameCursor::default();
    let first = cursor.next_pcm_s16le(&[0, 0, 0, 64]).unwrap();
    let second = cursor.next_samples(vec![0.5; 3]);

    assert_eq!(first.sequence, 0);
    assert_eq!(first.start_sample, 0);
    assert_eq!(first.end_sample(), 2);
    assert_eq!(second.sequence, 1);
    assert_eq!(second.start_sample, 2);
    assert_eq!(second.end_sample(), 5);

    let mut cursor = StreamingAudioFrameCursor::default();
    let explicit = AsrAudioFrame::new(7, 100, vec![0.0; 4]);
    cursor.observe(&explicit);
    let next = cursor.next_samples(vec![0.0]);
    assert_eq!((next.sequence, next.start_sample), (8, 104));
}

fn local_request() -> AsrTranscriptionRequest {
    AsrTranscriptionRequest::local_sherpa(
        AsrMode::Streaming,
        "model".into(),
        4,
        true,
        "auto".into(),
        Some("punctuation".into()),
        Some("vad".into()),
        5.0,
        "sensevoice".into(),
        None,
        Some("sona".into()),
        TranscriptNormalizationOptions::default(),
        TranscriptPostprocessOptions::default(),
        None,
        Some("auto".into()),
    )
}

#[test]
fn inference_spec_excludes_consumer_output_policy() {
    let left = local_request();
    let mut right = left.clone();
    right.normalization_options.enable_timeline = true;
    right.postprocess_options = TranscriptPostprocessOptions {
        text_replacement_sets: vec![TranscriptTextReplacementRuleSet {
            enabled: true,
            ignore_case: false,
            rules: vec![TranscriptTextReplacementRule {
                from: "hello".into(),
                to: "hi".into(),
            }],
        }],
        drop_final_dot_segments: false,
    };

    assert_eq!(
        StreamingInferenceSpec::from_request(&left).unwrap(),
        StreamingInferenceSpec::from_request(&right).unwrap()
    );

    right.language = "zh".into();
    assert_ne!(
        StreamingInferenceSpec::from_request(&left).unwrap(),
        StreamingInferenceSpec::from_request(&right).unwrap()
    );
}

#[test]
fn inference_spec_debug_redacts_online_configuration() {
    let request = AsrTranscriptionRequest {
        mode: AsrMode::Streaming,
        language: "auto".into(),
        enable_itn: true,
        normalization_options: Default::default(),
        postprocess_options: Default::default(),
        hotwords: None,
        speaker_processing: None,
        engine_config: AsrEngineConfig::Online {
            provider: OnlineAsrProviderRequest {
                provider_id: "volcengine-doubao".into(),
                profile_id: "private-profile".into(),
                config: json!({ "apiKey": "super-secret-value" }),
            },
        },
    };

    let debug = format!(
        "{:?}",
        StreamingInferenceSpec::from_request(&request).unwrap()
    );
    assert!(!debug.contains("super-secret-value"));
    assert!(!debug.contains("private-profile"));
    assert!(debug.contains("volcengine-doubao"));
}

#[test]
fn output_policy_applies_timeline_and_replacements_per_consumer() {
    let mut request = local_request();
    request.normalization_options.enable_timeline = true;
    request.postprocess_options = TranscriptPostprocessOptions {
        text_replacement_sets: vec![TranscriptTextReplacementRuleSet {
            enabled: true,
            ignore_case: false,
            rules: vec![TranscriptTextReplacementRule {
                from: "World".into(),
                to: "Sona".into(),
            }],
        }],
        drop_final_dot_segments: false,
    };
    let policy = StreamingOutputPolicy::from_request(&request).unwrap();
    let update = policy.process_update(TranscriptUpdate {
        remove_ids: Vec::new(),
        upsert_segments: vec![TranscriptSegment {
            id: "raw-1".into(),
            text: "Hello. World.".into(),
            start: 0.0,
            end: 2.0,
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            speaker: None,
            speaker_attribution: None,
        }],
    });

    assert_eq!(update.remove_ids, vec!["raw-1"]);
    assert_eq!(update.upsert_segments.len(), 2);
    assert_eq!(update.upsert_segments[1].text, "Sona.");
}
