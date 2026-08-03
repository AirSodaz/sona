use async_trait::async_trait;
use sona_application::live_transcription::{
    LiveInputTransform, LiveSourceEpoch, LiveTranscriptionCoordinator,
};
use sona_core::ports::asr::{
    AsrAudioFrame, AsrPortError, AsrRuntimeObserver, AsrStreamingErrorEvent, AsrStreamingSession,
    AsrTranscriptUpdateEvent, AsrTranscriptionRequest, NoopAsrRuntimeObserver,
    StreamingAsrFactoryPort, StreamingInferenceSpec, TranscriptNormalizationOptions,
    TranscriptPostprocessOptions, TranscriptTextReplacementRule, TranscriptTextReplacementRuleSet,
};
use sona_core::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
use sona_core::transcription::transcript::{TranscriptSegment, TranscriptUpdate};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct RecordingObserver {
    updates: Mutex<Vec<AsrTranscriptUpdateEvent>>,
    errors: Mutex<Vec<AsrStreamingErrorEvent>>,
}

struct PanickingObserver;

impl AsrRuntimeObserver for PanickingObserver {
    fn on_transcript_update(&self, _event: &AsrTranscriptUpdateEvent) {
        panic!("subscriber callback failed");
    }

    fn on_model_load(&self, _metric: &AsrModelLoadMetric) {}

    fn on_live_inference(&self, _metric: &AsrInferenceMetric) {}
}

impl AsrRuntimeObserver for RecordingObserver {
    fn on_transcript_update(&self, event: &AsrTranscriptUpdateEvent) {
        self.updates.lock().unwrap().push(event.clone());
    }

    fn on_model_load(&self, _metric: &AsrModelLoadMetric) {}

    fn on_live_inference(&self, _metric: &AsrInferenceMetric) {}

    fn on_streaming_error(&self, event: &AsrStreamingErrorEvent) {
        self.errors.lock().unwrap().push(event.clone());
    }
}

#[derive(Default)]
struct SessionState {
    starts: usize,
    stops: usize,
    flushes: usize,
    frames: Vec<AsrAudioFrame>,
    fail_start: bool,
    fail_feed: bool,
}

struct FakeSession {
    id: String,
    observer: Arc<dyn AsrRuntimeObserver>,
    state: Mutex<SessionState>,
}

impl FakeSession {
    fn emit(&self, text: &str, end: f64, is_final: bool) {
        self.observer
            .on_transcript_update(&AsrTranscriptUpdateEvent {
                instance_id: self.id.clone(),
                stage: if is_final { "final" } else { "partial" }.to_string(),
                update: TranscriptUpdate {
                    remove_ids: Vec::new(),
                    upsert_segments: vec![segment("segment-1", text, end, is_final)],
                },
            });
    }

    fn emit_boundary(&self, end_sample: u64) {
        self.observer
            .on_stream_boundary(&sona_core::ports::asr::AsrStreamBoundaryEvent {
                instance_id: self.id.clone(),
                sequence: 0,
                end_sample,
            });
    }
}

#[async_trait]
impl AsrStreamingSession for FakeSession {
    async fn start(&self) -> Result<(), AsrPortError> {
        let mut state = self.state.lock().unwrap();
        state.starts += 1;
        if state.fail_start {
            return Err(AsrPortError::runtime("injected start failure"));
        }
        Ok(())
    }

    async fn stop(&self) -> Result<(), AsrPortError> {
        self.state.lock().unwrap().stops += 1;
        Ok(())
    }

    async fn flush(&self) -> Result<(), AsrPortError> {
        self.state.lock().unwrap().flushes += 1;
        self.emit("flush final", 0.1, true);
        Ok(())
    }

    async fn feed_audio_frame(&self, frame: AsrAudioFrame) -> Result<(), AsrPortError> {
        let mut state = self.state.lock().unwrap();
        state.frames.push(frame);
        if state.fail_feed {
            return Err(AsrPortError::runtime("injected feed failure"));
        }
        Ok(())
    }
}

#[derive(Default)]
struct FakeFactory {
    prepare_count: Mutex<usize>,
    create_count: Mutex<usize>,
    fail_next_start: Mutex<bool>,
    sessions: Mutex<HashMap<String, Arc<FakeSession>>>,
}

impl FakeFactory {
    fn session(&self, id: &str) -> Arc<FakeSession> {
        Arc::clone(self.sessions.lock().unwrap().get(id).unwrap())
    }

    fn session_ids(&self) -> Vec<String> {
        let mut ids = self
            .sessions
            .lock()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        ids.sort();
        ids
    }
}

#[async_trait]
impl StreamingAsrFactoryPort for FakeFactory {
    async fn prepare(&self, _spec: &StreamingInferenceSpec) -> Result<(), AsrPortError> {
        *self.prepare_count.lock().unwrap() += 1;
        Ok(())
    }

    async fn create(
        &self,
        pipeline_id: &str,
        _spec: &StreamingInferenceSpec,
        observer: Arc<dyn AsrRuntimeObserver>,
    ) -> Result<Arc<dyn AsrStreamingSession>, AsrPortError> {
        *self.create_count.lock().unwrap() += 1;
        let fail_start = std::mem::take(&mut *self.fail_next_start.lock().unwrap());
        let session = Arc::new(FakeSession {
            id: pipeline_id.to_string(),
            observer,
            state: Mutex::new(SessionState {
                fail_start,
                ..SessionState::default()
            }),
        });
        self.sessions
            .lock()
            .unwrap()
            .insert(pipeline_id.to_string(), Arc::clone(&session));
        Ok(session)
    }
}

fn request() -> AsrTranscriptionRequest {
    AsrTranscriptionRequest::local_sherpa(
        sona_core::ports::asr::AsrMode::Streaming,
        "model".into(),
        4,
        true,
        "auto".into(),
        Some("punctuation".into()),
        Some("vad".into()),
        5.0,
        "sensevoice".into(),
        None,
        None,
        TranscriptNormalizationOptions::default(),
        TranscriptPostprocessOptions::default(),
        None,
        Some("cpu".into()),
    )
}

fn segment(id: &str, text: &str, end: f64, is_final: bool) -> TranscriptSegment {
    TranscriptSegment {
        id: id.into(),
        text: text.into(),
        start: 0.0,
        end,
        is_final,
        timing: None,
        tokens: None,
        timestamps: None,
        durations: None,
        translation: None,
        speaker: None,
        speaker_attribution: None,
    }
}

fn coordinator(factory: Arc<FakeFactory>) -> LiveTranscriptionCoordinator {
    LiveTranscriptionCoordinator::new(factory, Arc::new(NoopAsrRuntimeObserver))
}

async fn settle() {
    for _ in 0..8 {
        tokio::task::yield_now().await;
    }
}

#[tokio::test]
async fn shares_one_pipeline_and_applies_output_policy_per_consumer() {
    let factory = Arc::new(FakeFactory::default());
    let coordinator = coordinator(Arc::clone(&factory));
    let source = LiveSourceEpoch::new("microphone:default", 1);
    let record = Arc::new(RecordingObserver::default());
    let caption = Arc::new(RecordingObserver::default());
    let record_subscription = coordinator
        .acquire(
            "record",
            source.clone(),
            0,
            LiveInputTransform::default(),
            request(),
            record.clone(),
        )
        .await
        .unwrap();
    let mut caption_request = request();
    caption_request.postprocess_options = TranscriptPostprocessOptions {
        text_replacement_sets: vec![TranscriptTextReplacementRuleSet {
            enabled: true,
            ignore_case: false,
            rules: vec![TranscriptTextReplacementRule {
                from: "hello".into(),
                to: "caption".into(),
            }],
        }],
        drop_final_dot_segments: false,
    };
    let caption_subscription = coordinator
        .acquire(
            "caption",
            source.clone(),
            0,
            LiveInputTransform::default(),
            caption_request,
            caption.clone(),
        )
        .await
        .unwrap();

    assert_eq!(
        record_subscription.pipeline_id,
        caption_subscription.pipeline_id
    );
    assert!(caption_subscription.shared);
    coordinator
        .feed_source(&source, AsrAudioFrame::new(1, 0, vec![0.0; 1_600]))
        .await
        .unwrap();
    let session = factory.session(&record_subscription.pipeline_id);
    session.emit("hello", 0.1, false);
    settle().await;

    assert_eq!(*factory.create_count.lock().unwrap(), 1);
    assert_eq!(session.state.lock().unwrap().frames.len(), 1);
    assert_eq!(
        record.updates.lock().unwrap()[0].update.upsert_segments[0].text,
        "hello"
    );
    assert_eq!(
        caption.updates.lock().unwrap()[0].update.upsert_segments[0].text,
        "caption"
    );
    assert_eq!(coordinator.metrics().await.avoided_feed_count, 1);
}

#[tokio::test]
async fn late_join_uses_transient_pipeline_then_merges_at_matching_boundary() {
    let factory = Arc::new(FakeFactory::default());
    let coordinator = coordinator(Arc::clone(&factory));
    let source = LiveSourceEpoch::new("system:default", 7);
    let record = coordinator
        .acquire(
            "record",
            source.clone(),
            0,
            LiveInputTransform::default(),
            request(),
            Arc::new(RecordingObserver::default()),
        )
        .await
        .unwrap();
    coordinator
        .feed_source(&source, AsrAudioFrame::new(1, 0, vec![0.0; 1_600]))
        .await
        .unwrap();
    let caption = coordinator
        .acquire(
            "caption",
            source.clone(),
            1_600,
            LiveInputTransform::default(),
            request(),
            Arc::new(RecordingObserver::default()),
        )
        .await
        .unwrap();
    assert!(caption.transient);
    assert_ne!(record.pipeline_id, caption.pipeline_id);

    factory.session(&record.pipeline_id).emit("one", 0.2, true);
    factory.session(&caption.pipeline_id).emit("one", 0.1, true);
    settle().await;

    let metrics = coordinator.metrics().await;
    assert_eq!(metrics.active_pipelines, 1);
    assert_eq!(metrics.active_consumers, 2);
    assert_eq!(metrics.shared_pipelines, 1);
}

#[tokio::test]
async fn rejects_out_of_order_frames_and_keeps_different_transforms_separate() {
    let factory = Arc::new(FakeFactory::default());
    let coordinator = coordinator(Arc::clone(&factory));
    let source = LiveSourceEpoch::new("microphone:default", 1);
    let first = coordinator
        .acquire(
            "record",
            source.clone(),
            0,
            LiveInputTransform::default(),
            request(),
            Arc::new(RecordingObserver::default()),
        )
        .await
        .unwrap();
    let second = coordinator
        .acquire(
            "voice-typing",
            source.clone(),
            0,
            LiveInputTransform { gain: 1.5 },
            request(),
            Arc::new(RecordingObserver::default()),
        )
        .await
        .unwrap();
    assert_ne!(first.pipeline_id, second.pipeline_id);

    coordinator
        .feed_source(&source, AsrAudioFrame::new(3, 0, vec![0.8; 2]))
        .await
        .unwrap();
    let error = coordinator
        .feed_source(&source, AsrAudioFrame::new(3, 2, vec![0.0; 2]))
        .await
        .unwrap_err();
    assert!(error.message.contains("out-of-order"));
    assert_eq!(
        factory
            .session(&second.pipeline_id)
            .state
            .lock()
            .unwrap()
            .frames[0]
            .samples[0],
        1.0
    );
}

#[tokio::test]
async fn shared_release_replays_only_the_leaving_consumer_and_last_release_stops_main() {
    let factory = Arc::new(FakeFactory::default());
    let coordinator = coordinator(Arc::clone(&factory));
    let source = LiveSourceEpoch::new("microphone:default", 2);
    let record_observer = Arc::new(RecordingObserver::default());
    let voice_observer = Arc::new(RecordingObserver::default());
    let record = coordinator
        .acquire(
            "record",
            source.clone(),
            0,
            LiveInputTransform::default(),
            request(),
            record_observer,
        )
        .await
        .unwrap();
    coordinator
        .acquire(
            "voice-typing",
            source.clone(),
            0,
            LiveInputTransform::default(),
            request(),
            voice_observer.clone(),
        )
        .await
        .unwrap();
    coordinator
        .feed_source(&source, AsrAudioFrame::new(1, 0, vec![0.25; 1_600]))
        .await
        .unwrap();
    factory
        .session(&record.pipeline_id)
        .emit("partial", 0.1, false);
    settle().await;

    coordinator.release("voice-typing").await.unwrap();
    settle().await;
    let ids = factory.session_ids();
    assert_eq!(ids.len(), 2);
    let replay_id = ids
        .iter()
        .find(|id| id.starts_with("live-replay-"))
        .unwrap();
    let replay = factory.session(replay_id);
    assert_eq!(replay.state.lock().unwrap().frames.len(), 1);
    assert_eq!(replay.state.lock().unwrap().flushes, 1);
    assert_eq!(
        voice_observer.updates.lock().unwrap().last().unwrap().stage,
        "replay_final"
    );
    assert_eq!(
        factory
            .session(&record.pipeline_id)
            .state
            .lock()
            .unwrap()
            .stops,
        0
    );

    coordinator.release("record").await.unwrap();
    settle().await;
    let main = factory.session(&record.pipeline_id);
    assert_eq!(main.state.lock().unwrap().flushes, 1);
    assert_eq!(main.state.lock().unwrap().stops, 1);
    assert_eq!(coordinator.metrics().await.active_pipelines, 0);
}

#[tokio::test]
async fn last_release_drains_flush_final_before_removing_pipeline() {
    let factory = Arc::new(FakeFactory::default());
    let coordinator = coordinator(Arc::clone(&factory));
    let source = LiveSourceEpoch::new("microphone:default", 3);
    let observer = Arc::new(RecordingObserver::default());
    coordinator
        .acquire(
            "record",
            source,
            0,
            LiveInputTransform::default(),
            request(),
            observer.clone(),
        )
        .await
        .unwrap();

    coordinator.release("record").await.unwrap();

    let updates = observer.updates.lock().unwrap();
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].instance_id, "record");
    assert_eq!(updates[0].update.upsert_segments[0].text, "flush final");
}

#[tokio::test]
async fn retired_epoch_rejects_stale_work_without_affecting_next_epoch() {
    let factory = Arc::new(FakeFactory::default());
    let coordinator = coordinator(Arc::clone(&factory));
    let retired = LiveSourceEpoch::new("microphone:default", 4);
    let current = LiveSourceEpoch::new("microphone:default", 5);
    let retired_observer = Arc::new(RecordingObserver::default());
    coordinator
        .acquire(
            "record",
            retired.clone(),
            0,
            LiveInputTransform::default(),
            request(),
            retired_observer.clone(),
        )
        .await
        .unwrap();

    coordinator.retire_source(&retired).await.unwrap();
    assert_eq!(retired_observer.updates.lock().unwrap().len(), 1);
    assert!(
        coordinator
            .feed_source(&retired, AsrAudioFrame::new(1, 0, vec![0.0; 16]))
            .await
            .unwrap_err()
            .message
            .contains("retired")
    );
    assert!(
        coordinator
            .acquire(
                "stale",
                retired,
                0,
                LiveInputTransform::default(),
                request(),
                Arc::new(RecordingObserver::default()),
            )
            .await
            .unwrap_err()
            .message
            .contains("retired")
    );

    coordinator
        .acquire(
            "record",
            current.clone(),
            0,
            LiveInputTransform::default(),
            request(),
            Arc::new(RecordingObserver::default()),
        )
        .await
        .unwrap();
    coordinator
        .feed_source(&current, AsrAudioFrame::new(1, 0, vec![0.0; 16]))
        .await
        .unwrap();
    assert_eq!(coordinator.metrics().await.active_sources, 1);
}

#[tokio::test]
async fn panicking_subscriber_is_evicted_without_affecting_others() {
    let factory = Arc::new(FakeFactory::default());
    let coordinator = coordinator(Arc::clone(&factory));
    let source = LiveSourceEpoch::new("microphone:default", 6);
    let healthy = Arc::new(RecordingObserver::default());
    let subscription = coordinator
        .acquire(
            "record",
            source.clone(),
            0,
            LiveInputTransform::default(),
            request(),
            healthy.clone(),
        )
        .await
        .unwrap();
    coordinator
        .acquire(
            "caption",
            source.clone(),
            0,
            LiveInputTransform::default(),
            request(),
            Arc::new(PanickingObserver),
        )
        .await
        .unwrap();

    let session = factory.session(&subscription.pipeline_id);
    session.emit("first", 0.1, false);
    for _ in 0..32 {
        if coordinator.metrics().await.active_consumers == 1 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(coordinator.metrics().await.active_consumers, 1);

    coordinator
        .feed_source(&source, AsrAudioFrame::new(1, 0, vec![0.0; 16]))
        .await
        .unwrap();
    session.emit("second", 0.2, false);
    settle().await;
    assert_eq!(healthy.updates.lock().unwrap().len(), 2);
    assert_eq!(session.state.lock().unwrap().stops, 0);
}

#[tokio::test]
async fn one_pipeline_feed_failure_does_not_skip_other_pipelines() {
    let factory = Arc::new(FakeFactory::default());
    let coordinator = coordinator(Arc::clone(&factory));
    let source = LiveSourceEpoch::new("microphone:default", 7);
    let failing = coordinator
        .acquire(
            "record",
            source.clone(),
            0,
            LiveInputTransform::default(),
            request(),
            Arc::new(RecordingObserver::default()),
        )
        .await
        .unwrap();
    let healthy = coordinator
        .acquire(
            "voice-typing",
            source.clone(),
            0,
            LiveInputTransform { gain: 1.25 },
            request(),
            Arc::new(RecordingObserver::default()),
        )
        .await
        .unwrap();
    factory
        .session(&failing.pipeline_id)
        .state
        .lock()
        .unwrap()
        .fail_feed = true;

    let error = coordinator
        .feed_source(&source, AsrAudioFrame::new(1, 0, vec![0.2; 1_600]))
        .await
        .unwrap_err();

    assert_eq!(error.message, "injected feed failure");
    assert_eq!(
        factory
            .session(&healthy.pipeline_id)
            .state
            .lock()
            .unwrap()
            .frames
            .len(),
        1
    );
}

#[tokio::test]
async fn delayed_boundary_keeps_newer_audio_for_leaving_consumer_replay() {
    let factory = Arc::new(FakeFactory::default());
    let coordinator = coordinator(Arc::clone(&factory));
    let source = LiveSourceEpoch::new("microphone:default", 8);
    let record = coordinator
        .acquire(
            "record",
            source.clone(),
            0,
            LiveInputTransform::default(),
            request(),
            Arc::new(RecordingObserver::default()),
        )
        .await
        .unwrap();
    coordinator
        .acquire(
            "caption",
            source.clone(),
            0,
            LiveInputTransform::default(),
            request(),
            Arc::new(RecordingObserver::default()),
        )
        .await
        .unwrap();
    coordinator
        .feed_source(&source, AsrAudioFrame::new(1, 0, vec![0.1; 1_600]))
        .await
        .unwrap();
    coordinator
        .feed_source(&source, AsrAudioFrame::new(2, 1_600, vec![0.2; 1_600]))
        .await
        .unwrap();

    factory.session(&record.pipeline_id).emit_boundary(1_600);
    settle().await;
    coordinator.release("caption").await.unwrap();

    let replay_id = factory
        .session_ids()
        .into_iter()
        .find(|id| id.starts_with("live-replay-"))
        .unwrap();
    let replay = factory.session(&replay_id);
    let frames = &replay.state.lock().unwrap().frames;
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].sequence, 2);
    assert_eq!(frames[0].start_sample, 1_600);
}

#[tokio::test]
async fn invalid_gain_is_rejected_before_pipeline_creation() {
    let factory = Arc::new(FakeFactory::default());
    let coordinator = coordinator(Arc::clone(&factory));

    let error = coordinator
        .acquire(
            "record",
            LiveSourceEpoch::new("microphone:default", 9),
            0,
            LiveInputTransform { gain: f32::NAN },
            request(),
            Arc::new(RecordingObserver::default()),
        )
        .await
        .unwrap_err();

    assert!(error.message.contains("gain"));
    assert_eq!(*factory.create_count.lock().unwrap(), 0);
}

#[tokio::test]
async fn failed_pipeline_start_stops_session_and_clears_coordinator_state() {
    let factory = Arc::new(FakeFactory::default());
    *factory.fail_next_start.lock().unwrap() = true;
    let coordinator = coordinator(Arc::clone(&factory));

    let error = coordinator
        .acquire(
            "record",
            LiveSourceEpoch::new("microphone:default", 10),
            0,
            LiveInputTransform::default(),
            request(),
            Arc::new(RecordingObserver::default()),
        )
        .await
        .unwrap_err();

    assert_eq!(error.message, "injected start failure");
    let session = factory.session("live-pipeline-1");
    assert_eq!(session.state.lock().unwrap().stops, 1);
    assert_eq!(coordinator.metrics().await.active_pipelines, 0);
    assert_eq!(coordinator.metrics().await.active_consumers, 0);
}
