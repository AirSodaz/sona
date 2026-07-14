use async_trait::async_trait;
use sona_core::ports::asr::{
    AsrRuntimeObserver, AsrStreamingErrorEvent, AsrStreamingSession, AsrTranscriptUpdateEvent,
    NoopAsrRuntimeObserver, SherpaError,
};
use sona_core::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
use sona_core::transcription::transcript::TranscriptUpdate;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct RecordingObserver {
    updates: Mutex<Vec<AsrTranscriptUpdateEvent>>,
    model_loads: Mutex<Vec<AsrModelLoadMetric>>,
    live_inferences: Mutex<Vec<AsrInferenceMetric>>,
    streaming_errors: Mutex<Vec<AsrStreamingErrorEvent>>,
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
}

struct DummySession;

#[async_trait]
impl AsrStreamingSession for DummySession {
    async fn start(&self) -> Result<(), SherpaError> {
        Ok(())
    }

    async fn stop(&self) -> Result<(), SherpaError> {
        Ok(())
    }

    async fn flush(&self) -> Result<(), SherpaError> {
        Ok(())
    }

    async fn feed_audio_chunk(&self, _samples: Vec<u8>) -> Result<(), SherpaError> {
        Ok(())
    }

    async fn feed_audio_samples(&self, _samples: &[f32]) -> Result<(), SherpaError> {
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

    observer.on_transcript_update(&event);
    observer.on_model_load(&model_load);
    observer.on_live_inference(&live);
    observer.on_streaming_error(&streaming_error);

    assert_eq!(*observer.updates.lock().unwrap(), vec![event]);
    assert_eq!(*observer.model_loads.lock().unwrap(), vec![model_load]);
    assert_eq!(*observer.live_inferences.lock().unwrap(), vec![live]);
    assert_eq!(
        *observer.streaming_errors.lock().unwrap(),
        vec![streaming_error]
    );
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
    session.feed_audio_chunk(vec![0, 1]).await.unwrap();
    session.feed_audio_samples(&[0.0, 0.25]).await.unwrap();
    session.flush().await.unwrap();
    session.stop().await.unwrap();
}
