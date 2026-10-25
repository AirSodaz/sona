use async_trait::async_trait;
use sona_application::live_transcription::{
    LiveInputTransform, LiveSourceEpoch, LiveTranscriptionCoordinator,
    StreamingTranslationCoordinator,
};
use sona_core::ports::asr::{
    AsrAudioFrame, AsrPortError, AsrRuntimeObserver, AsrStreamingErrorEvent, AsrStreamingSession,
    AsrTranscriptUpdateEvent, AsrTranscriptionRequest, NoopAsrRuntimeObserver,
    StreamingAsrFactoryPort, StreamingInferenceSpec, TranscriptNormalizationOptions,
    TranscriptPostprocessOptions,
};
use sona_core::ports::translation::{
    TranslationEngineKind, TranslationEnginePort, TranslationPortError,
};
use sona_core::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
use sona_core::transcription::transcript::{TranscriptSegment, TranscriptUpdate};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct MockTranslationEngine {
    translations: Mutex<HashMap<String, String>>,
    fail_on: Mutex<Option<String>>,
}

impl MockTranslationEngine {
    fn new() -> Self {
        let mut t = HashMap::new();
        t.insert("I like to".into(), "我喜欢".into());
        t.insert("I like to read books".into(), "我喜欢看书".into());
        t.insert(
            "I like to read books every day.".into(),
            "我喜欢每天看书。".into(),
        );
        Self {
            translations: Mutex::new(t),
            fail_on: Mutex::new(None),
        }
    }

    fn set_fail_on(&self, text: &str) {
        *self.fail_on.lock().unwrap() = Some(text.to_string());
    }
}

#[async_trait]
impl TranslationEnginePort for MockTranslationEngine {
    async fn translate(
        &self,
        text: &str,
        _source_lang: &str,
        _target_lang: &str,
    ) -> Result<String, TranslationPortError> {
        if self.fail_on.lock().unwrap().as_deref() == Some(text) {
            return Err(TranslationPortError::inference_failed(
                "mock translation error",
            ));
        }
        let map = self.translations.lock().unwrap();
        if let Some(trans) = map.get(text) {
            Ok(trans.clone())
        } else {
            Ok(format!("译:[{text}]"))
        }
    }

    fn engine_kind(&self) -> TranslationEngineKind {
        TranslationEngineKind::OnnxNmt
    }
}

#[derive(Default)]
struct RecordingObserver {
    updates: Mutex<Vec<AsrTranscriptUpdateEvent>>,
}

impl AsrRuntimeObserver for RecordingObserver {
    fn on_transcript_update(&self, event: &AsrTranscriptUpdateEvent) {
        self.updates.lock().unwrap().push(event.clone());
    }

    fn on_model_load(&self, _metric: &AsrModelLoadMetric) {}
    fn on_live_inference(&self, _metric: &AsrInferenceMetric) {}
    fn on_streaming_error(&self, _event: &AsrStreamingErrorEvent) {}
}

struct TestSession {
    id: String,
    observer: Arc<dyn AsrRuntimeObserver>,
}

impl TestSession {
    fn emit(&self, text: &str, end: f64, is_final: bool) {
        self.observer
            .on_transcript_update(&AsrTranscriptUpdateEvent {
                instance_id: self.id.clone(),
                stage: if is_final { "final" } else { "partial" }.to_string(),
                update: TranscriptUpdate {
                    remove_ids: Vec::new(),
                    upsert_segments: vec![TranscriptSegment {
                        id: "seg-1".to_string(),
                        text: text.to_string(),
                        start: 0.0,
                        end,
                        is_final,
                        timing: None,
                        tokens: None,
                        timestamps: None,
                        durations: None,
                        translation: None,
                        draft_translation: None,
                        speaker: None,
                        speaker_attribution: None,
                    }],
                },
            });
    }
}

#[async_trait]
impl AsrStreamingSession for TestSession {
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

#[derive(Default)]
struct TestFactory {
    sessions: Mutex<HashMap<String, Arc<TestSession>>>,
}

#[async_trait]
impl StreamingAsrFactoryPort for TestFactory {
    async fn prepare(&self, _spec: &StreamingInferenceSpec) -> Result<(), AsrPortError> {
        Ok(())
    }

    async fn create(
        &self,
        id: &str,
        _spec: &StreamingInferenceSpec,
        observer: Arc<dyn AsrRuntimeObserver>,
    ) -> Result<Arc<dyn AsrStreamingSession>, AsrPortError> {
        let session = Arc::new(TestSession {
            id: id.to_string(),
            observer,
        });
        self.sessions
            .lock()
            .unwrap()
            .insert(id.to_string(), Arc::clone(&session));
        Ok(session)
    }
}
fn sample_request() -> AsrTranscriptionRequest {
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

#[tokio::test]
async fn test_live_transcription_translation_enrichment() {
    let factory = Arc::new(TestFactory::default());
    let translation_engine = Arc::new(MockTranslationEngine::new());
    let translation_coord = Arc::new(
        StreamingTranslationCoordinator::new(translation_engine, "en", "zh").with_debounce(50, 2),
    );

    let coordinator = LiveTranscriptionCoordinator::with_optional_translation(
        Arc::clone(&factory) as Arc<dyn StreamingAsrFactoryPort>,
        Arc::new(NoopAsrRuntimeObserver),
        Some(translation_coord),
    );

    let observer = Arc::new(RecordingObserver::default());
    let sub = coordinator
        .acquire(
            "consumer-1",
            LiveSourceEpoch::new("src-1", 1),
            0,
            LiveInputTransform::default(),
            sample_request(),
            Arc::clone(&observer) as Arc<dyn AsrRuntimeObserver>,
        )
        .await
        .expect("subscription should succeed");

    let session = {
        let sessions = factory.sessions.lock().unwrap();
        Arc::clone(sessions.get(&sub.pipeline_id).expect("session exists"))
    };

    // Emit partial 1
    session.emit("I like to", 1.0, false);
    tokio::time::sleep(Duration::from_millis(80)).await;

    // Emit partial 2 extending prefix
    session.emit("I like to read books", 2.0, false);
    tokio::time::sleep(Duration::from_millis(80)).await;

    // Emit final segment
    session.emit("I like to read books every day.", 3.0, true);
    tokio::time::sleep(Duration::from_millis(80)).await;

    let updates = observer.updates.lock().unwrap().clone();
    assert!(updates.len() >= 3, "expected at least 3 updates");

    // Partial 1
    let p1 = &updates[0].update.upsert_segments[0];
    assert_eq!(p1.text, "I like to");
    assert_eq!(p1.draft_translation, Some("我喜欢".into()));
    assert_eq!(p1.translation, None);

    // Partial 2
    let p2 = &updates[1].update.upsert_segments[0];
    assert_eq!(p2.text, "I like to read books");
    assert_eq!(p2.translation, Some("我喜欢".into()));
    assert_eq!(p2.draft_translation, Some("看书".into()));

    // Final
    let f = updates
        .last()
        .unwrap()
        .update
        .upsert_segments
        .first()
        .unwrap();
    assert_eq!(f.text, "I like to read books every day.");
    assert_eq!(f.translation, Some("我喜欢每天看书。".into()));
    assert_eq!(f.draft_translation, None);
}
fn make_segment(id: &str, text: &str, is_final: bool) -> TranscriptSegment {
    TranscriptSegment {
        id: id.to_string(),
        text: text.to_string(),
        start: 0.0,
        end: 1.0,
        is_final,
        timing: None,
        tokens: None,
        timestamps: None,
        durations: None,
        translation: None,
        draft_translation: None,
        speaker: None,
        speaker_attribution: None,
    }
}

#[tokio::test]
async fn test_live_transcription_translation_debounce_preserves_draft() {
    let translation_engine = Arc::new(MockTranslationEngine::new());
    let coord = StreamingTranslationCoordinator::new(translation_engine, "en", "zh")
        .with_debounce(1000, 10);

    // Frame 1: initial text triggers translation
    let mut update1 = TranscriptUpdate {
        upsert_segments: vec![make_segment("seg-1", "Hello", false)],
        remove_ids: vec![],
    };
    coord.enrich_update(&mut update1).await;
    assert_eq!(
        update1.upsert_segments[0].draft_translation,
        Some("译:[Hello]".into())
    );

    // Frame 2: minor increment (delta < 10 chars, elapsed < 1000ms) - should debounce
    let mut update2 = TranscriptUpdate {
        upsert_segments: vec![make_segment("seg-1", "Hello ", false)],
        remove_ids: vec![],
    };
    coord.enrich_update(&mut update2).await;
    // Draft translation MUST be preserved from state, not cleared to None
    assert_eq!(
        update2.upsert_segments[0].draft_translation,
        Some("译:[Hello]".into()),
        "Debounced frame must preserve previous draft translation"
    );
}

#[tokio::test]
async fn test_live_transcription_final_failure_falls_back_to_frozen_prefix() {
    let translation_engine = Arc::new(MockTranslationEngine::new());
    let coord =
        StreamingTranslationCoordinator::new(Arc::clone(&translation_engine) as _, "en", "zh")
            .with_debounce(10, 1);

    // Frame 1: partial 1
    let mut update1 = TranscriptUpdate {
        upsert_segments: vec![make_segment("seg-2", "I like to", false)],
        remove_ids: vec![],
    };
    coord.enrich_update(&mut update1).await;

    tokio::time::sleep(Duration::from_millis(20)).await;

    // Frame 2: partial 2, locks frozen_prefix = "我喜欢"
    let mut update2 = TranscriptUpdate {
        upsert_segments: vec![make_segment("seg-2", "I like to read books", false)],
        remove_ids: vec![],
    };
    coord.enrich_update(&mut update2).await;
    assert_eq!(
        update2.upsert_segments[0].translation,
        Some("我喜欢".into())
    );

    // Simulate translation engine failure on final text
    translation_engine.set_fail_on("I like to read books every day.");

    // Frame 3: final segment fails to translate
    let mut update3 = TranscriptUpdate {
        upsert_segments: vec![make_segment(
            "seg-2",
            "I like to read books every day.",
            true,
        )],
        remove_ids: vec![],
    };
    coord.enrich_update(&mut update3).await;

    // Should gracefully fall back to the previously stabilized frozen_prefix
    assert_eq!(
        update3.upsert_segments[0].translation,
        Some("我喜欢".into()),
        "Final translation failure must fall back to stabilized frozen prefix"
    );
    assert_eq!(update3.upsert_segments[0].draft_translation, None);
}
