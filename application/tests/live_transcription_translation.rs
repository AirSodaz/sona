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
    call_count: Mutex<usize>,
    delay: Mutex<Option<Duration>>,
    last_languages: Mutex<Option<(String, String)>>,
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
            call_count: Mutex::new(0),
            delay: Mutex::new(None),
            last_languages: Mutex::new(None),
        }
    }

    fn set_fail_on(&self, text: &str) {
        *self.fail_on.lock().unwrap() = Some(text.to_string());
    }

    fn set_delay(&self, delay: Duration) {
        *self.delay.lock().unwrap() = Some(delay);
    }

    fn call_count(&self) -> usize {
        *self.call_count.lock().unwrap()
    }

    fn last_languages(&self) -> Option<(String, String)> {
        self.last_languages.lock().unwrap().clone()
    }
}

#[async_trait]
impl TranslationEnginePort for MockTranslationEngine {
    async fn translate(
        &self,
        text: &str,
        source_lang: &str,
        target_lang: &str,
    ) -> Result<String, TranslationPortError> {
        *self.call_count.lock().unwrap() += 1;
        *self.last_languages.lock().unwrap() =
            Some((source_lang.to_string(), target_lang.to_string()));
        let delay = *self.delay.lock().unwrap();
        if let Some(d) = delay {
            tokio::time::sleep(d).await;
        }
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
    fn emit_update(&self, update: TranscriptUpdate, is_final: bool) {
        self.observer
            .on_transcript_update(&AsrTranscriptUpdateEvent {
                instance_id: self.id.clone(),
                stage: if is_final { "final" } else { "partial" }.to_string(),
                update,
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
    assert!(
        updates.len() >= 5,
        "expected at least 5 updates (immediate + enriched + final)"
    );

    // Partial 1 (immediate transcript broadcast with 0 latency)
    let p1_immediate = &updates[0].update.upsert_segments[0];
    assert_eq!(p1_immediate.text, "I like to");
    assert_eq!(p1_immediate.translation, None);

    // Partial 1 (translation enriched in background)
    let p1_enriched = &updates[1].update.upsert_segments[0];
    assert_eq!(p1_enriched.text, "I like to");
    assert_eq!(p1_enriched.draft_translation, Some("我喜欢".into()));
    assert_eq!(p1_enriched.translation, None);

    // Partial 2 (immediate transcript broadcast with cached draft translation)
    let p2_immediate = &updates[2].update.upsert_segments[0];
    assert_eq!(p2_immediate.text, "I like to read books");
    assert_eq!(p2_immediate.draft_translation, Some("我喜欢".into()));

    // Partial 2 (translation enriched in background)
    let p2_enriched = &updates[3].update.upsert_segments[0];
    assert_eq!(p2_enriched.text, "I like to read books");
    assert_eq!(p2_enriched.translation, Some("我喜欢".into()));
    assert_eq!(p2_enriched.draft_translation, Some("看书".into()));

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

#[tokio::test]
async fn test_stale_partial_after_finalization_does_not_overwrite() {
    let factory = Arc::new(TestFactory::default());
    let translation_engine = Arc::new(MockTranslationEngine::new());
    let translation_coord = Arc::new(
        StreamingTranslationCoordinator::new(Arc::clone(&translation_engine) as _, "en", "zh")
            .with_debounce(10, 1),
    );

    let coordinator = LiveTranscriptionCoordinator::with_optional_translation(
        Arc::clone(&factory) as Arc<dyn StreamingAsrFactoryPort>,
        Arc::new(NoopAsrRuntimeObserver),
        Some(translation_coord),
    );

    let observer = Arc::new(RecordingObserver::default());
    let sub = coordinator
        .acquire(
            "consumer-stale",
            LiveSourceEpoch::new("src-stale", 1),
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

    // Make partial translation take 80ms
    translation_engine.set_delay(Duration::from_millis(80));

    // Emit partial 1 - starts background translation task with 80ms delay
    session.emit("I like to", 1.0, false);
    tokio::time::sleep(Duration::from_millis(15)).await;

    // Remove delay so final translation completes before partial
    *translation_engine.delay.lock().unwrap() = None;

    // Emit final segment for seg-1
    session.emit("I like to read books every day.", 2.0, true);

    // Wait for the slow 80ms partial task to complete
    tokio::time::sleep(Duration::from_millis(150)).await;

    let updates = observer.updates.lock().unwrap().clone();
    let last = updates.last().expect("must have updates");
    let seg = &last.update.upsert_segments[0];
    assert!(
        seg.is_final,
        "Last received segment must be final, got is_final={}",
        seg.is_final
    );
    assert_eq!(seg.text, "I like to read books every day.");
    assert_eq!(seg.translation, Some("我喜欢每天看书。".into()));
}

#[tokio::test]
async fn test_concurrent_partial_updates_do_not_stampede() {
    let translation_engine = Arc::new(MockTranslationEngine::new());
    translation_engine.set_delay(Duration::from_millis(50));
    let coord = Arc::new(
        StreamingTranslationCoordinator::new(Arc::clone(&translation_engine) as _, "en", "zh")
            .with_debounce(500, 10),
    );

    let mut update1 = TranscriptUpdate {
        upsert_segments: vec![make_segment("seg-conc", "Hello", false)],
        remove_ids: vec![],
    };
    let mut update2 = TranscriptUpdate {
        upsert_segments: vec![make_segment("seg-conc", "Hello world", false)],
        remove_ids: vec![],
    };

    let c1 = Arc::clone(&coord);
    let c2 = Arc::clone(&coord);

    tokio::join!(
        async move { c1.enrich_update(&mut update1).await },
        async move { c2.enrich_update(&mut update2).await }
    );

    assert_eq!(
        translation_engine.call_count(),
        1,
        "Concurrent partials must not stampede translation engine"
    );
}

#[tokio::test]
async fn test_mixed_update_invalidates_stale_partial() {
    let factory = Arc::new(TestFactory::default());
    let translation_engine = Arc::new(MockTranslationEngine::new());
    let translation_coord = Arc::new(
        StreamingTranslationCoordinator::new(Arc::clone(&translation_engine) as _, "en", "zh")
            .with_debounce(10, 1),
    );
    let coordinator = LiveTranscriptionCoordinator::with_optional_translation(
        Arc::clone(&factory) as Arc<dyn StreamingAsrFactoryPort>,
        Arc::new(NoopAsrRuntimeObserver),
        Some(translation_coord),
    );

    let observer = Arc::new(RecordingObserver::default());
    let sub = coordinator
        .acquire(
            "consumer-mixed",
            LiveSourceEpoch::new("src-mixed", 1),
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

    // Make partial translation take 80ms
    translation_engine.set_delay(Duration::from_millis(80));

    // 1. Emit partial for seg-2: "Hello" (triggers background worker with 80ms delay)
    session.emit_update(
        TranscriptUpdate {
            remove_ids: Vec::new(),
            upsert_segments: vec![make_segment("seg-2", "Hello", false)],
        },
        false,
    );
    tokio::time::sleep(Duration::from_millis(15)).await;

    // 2. Remove delay so subsequent translations are fast
    *translation_engine.delay.lock().unwrap() = None;

    // 3. Emit mixed update: seg-1 (final) and seg-2 (new partial "Hello world")
    session.emit_update(
        TranscriptUpdate {
            remove_ids: Vec::new(),
            upsert_segments: vec![
                make_segment("seg-1", "I like to read books every day.", true),
                make_segment("seg-2", "Hello world", false),
            ],
        },
        true,
    );

    // 4. Wait for the initial 80ms partial task to finish
    tokio::time::sleep(Duration::from_millis(150)).await;

    let updates = observer.updates.lock().unwrap().clone();
    // Find any updates emitted after the mixed update
    // seg-2 must NOT have reverted to "Hello"
    let last_seg2 = updates
        .iter()
        .rev()
        .find_map(|u| u.update.upsert_segments.iter().find(|s| s.id == "seg-2"))
        .expect("seg-2 must exist");

    assert_eq!(
        last_seg2.text, "Hello world",
        "seg-2 must not be reverted to stale partial 'Hello'"
    );
}
#[tokio::test]
async fn test_cjk_char_delta_one_allows_translation_after_debounce() {
    let engine = Arc::new(MockTranslationEngine::new());
    let coordinator = StreamingTranslationCoordinator::new(
        Arc::clone(&engine) as Arc<dyn TranslationEnginePort>,
        "zh",
        "en",
    )
    .with_debounce(50, 3);

    // Initial partial: "我"
    let mut update1 = TranscriptUpdate {
        remove_ids: Vec::new(),
        upsert_segments: vec![make_segment("seg-cjk", "我", false)],
    };
    coordinator.enrich_update(&mut update1).await;
    assert_eq!(engine.call_count(), 1);

    // Wait for debounce interval to pass
    tokio::time::sleep(Duration::from_millis(60)).await;

    // Emit second partial with 1 CJK char added: "我看" (char_delta = 1 < min_char_delta 3)
    let mut update2 = TranscriptUpdate {
        remove_ids: Vec::new(),
        upsert_segments: vec![make_segment("seg-cjk", "我看", false)],
    };
    coordinator.enrich_update(&mut update2).await;
    // Because CJK threshold is 1, it should translate!
    assert_eq!(engine.call_count(), 2);
}

#[tokio::test]
async fn test_word_correction_zero_char_delta_translates_after_debounce() {
    let engine = Arc::new(MockTranslationEngine::new());
    let coordinator = StreamingTranslationCoordinator::new(
        Arc::clone(&engine) as Arc<dyn TranslationEnginePort>,
        "en",
        "zh",
    )
    .with_debounce(50, 3);

    // Initial partial: "cat"
    let mut update1 = TranscriptUpdate {
        remove_ids: Vec::new(),
        upsert_segments: vec![make_segment("seg-correction", "cat", false)],
    };
    coordinator.enrich_update(&mut update1).await;
    assert_eq!(engine.call_count(), 1);

    // Wait for debounce interval to pass
    tokio::time::sleep(Duration::from_millis(60)).await;

    // Emit word correction: "dog" (same length 3, char_delta = 0)
    let mut update2 = TranscriptUpdate {
        remove_ids: Vec::new(),
        upsert_segments: vec![make_segment("seg-correction", "dog", false)],
    };
    coordinator.enrich_update(&mut update2).await;
    // Word correction with char_delta == 0 should translate after debounce!
    assert_eq!(engine.call_count(), 2);
}

#[tokio::test]
async fn test_slow_final_translation_does_not_block_release_or_hang() {
    let factory = Arc::new(TestFactory::default());
    let translation_engine = Arc::new(MockTranslationEngine::new());
    // Simulate slow translation (500ms delay)
    translation_engine.set_delay(Duration::from_millis(500));

    let streaming_coord = Arc::new(
        StreamingTranslationCoordinator::new(
            Arc::clone(&translation_engine) as Arc<dyn TranslationEnginePort>,
            "en",
            "zh",
        )
        .with_debounce(100, 3),
    );
    let coordinator = Arc::new(LiveTranscriptionCoordinator::with_optional_translation(
        Arc::clone(&factory) as Arc<dyn StreamingAsrFactoryPort>,
        Arc::new(NoopAsrRuntimeObserver),
        Some(streaming_coord),
    ));
    let observer = Arc::new(RecordingObserver::default());
    let sub = coordinator
        .acquire(
            "consumer-slow-final",
            LiveSourceEpoch::new("src-slow-final", 1),
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

    // Emit final segment
    session.emit("I like to read books every day.", 3.0, true);

    // The un-translated final segment must be broadcast with zero latency (< 60ms)
    tokio::time::sleep(Duration::from_millis(30)).await;
    {
        let updates = observer.updates.lock().unwrap().clone();
        assert!(
            !updates.is_empty(),
            "Immediate final update should have been broadcast"
        );
        let immediate_final = &updates[0].update.upsert_segments[0];
        assert_eq!(immediate_final.text, "I like to read books every day.");
        assert_eq!(immediate_final.translation, None);
    }

    // Call release immediately while translation is still pending in background
    let start_release = std::time::Instant::now();
    let release_res = coordinator.release("consumer-slow-final").await;
    assert!(release_res.is_ok());
    assert!(
        start_release.elapsed() < Duration::from_millis(150),
        "Release took {:?}, should not block on slow translation",
        start_release.elapsed()
    );

    // Wait for the slow translation background task to settle cleanly without panic
    tokio::time::sleep(Duration::from_millis(600)).await;
}

#[tokio::test]
async fn test_language_detection_and_same_language_skip() {
    let engine = Arc::new(MockTranslationEngine::new());
    let coordinator = StreamingTranslationCoordinator::new(
        Arc::clone(&engine) as Arc<dyn TranslationEnginePort>,
        "auto",
        "en",
    );

    // 1. Japanese text with "auto" source -> detects "ja"
    let mut ja_update = TranscriptUpdate {
        remove_ids: Vec::new(),
        upsert_segments: vec![make_segment("seg-ja", "こんにちは世界", true)],
    };
    coordinator.enrich_update(&mut ja_update).await;
    assert_eq!(
        engine.last_languages(),
        Some(("ja".to_string(), "en".to_string()))
    );
    assert!(ja_update.upsert_segments[0].translation.is_some());

    // 2. Chinese text with "auto" source -> detects "zh"
    let mut zh_update = TranscriptUpdate {
        remove_ids: Vec::new(),
        upsert_segments: vec![make_segment("seg-zh", "你好世界", true)],
    };
    coordinator.enrich_update(&mut zh_update).await;
    assert_eq!(
        engine.last_languages(),
        Some(("zh".to_string(), "en".to_string()))
    );
    assert!(zh_update.upsert_segments[0].translation.is_some());

    // 3. English text with target "en" -> detected "en" equals target "en", skips translation
    let initial_calls = engine.call_count();
    let mut en_update = TranscriptUpdate {
        remove_ids: Vec::new(),
        upsert_segments: vec![make_segment("seg-en", "Hello world", true)],
    };
    coordinator.enrich_update(&mut en_update).await;
    // Skipped!
    assert_eq!(engine.call_count(), initial_calls);
    assert_eq!(en_update.upsert_segments[0].translation, None);

    // 4. Explicit same language ("zh" to "zh-CN") skips translation
    let same_lang_coord = StreamingTranslationCoordinator::new(
        Arc::clone(&engine) as Arc<dyn TranslationEnginePort>,
        "zh",
        "zh-CN",
    );
    let mut zh_same_update = TranscriptUpdate {
        remove_ids: Vec::new(),
        upsert_segments: vec![make_segment("seg-zh-same", "测试内容", true)],
    };
    same_lang_coord.enrich_update(&mut zh_same_update).await;
    assert_eq!(engine.call_count(), initial_calls);
    assert_eq!(zh_same_update.upsert_segments[0].translation, None);
}

#[tokio::test]
async fn test_final_translation_timeout_falls_back_gracefully() {
    let engine = Arc::new(MockTranslationEngine::new());
    // Delay 5200ms (> 5000ms timeout)
    engine.set_delay(Duration::from_millis(5200));
    let coordinator = StreamingTranslationCoordinator::new(
        Arc::clone(&engine) as Arc<dyn TranslationEnginePort>,
        "en",
        "zh",
    );

    let mut update = TranscriptUpdate {
        remove_ids: Vec::new(),
        upsert_segments: vec![make_segment("seg-timeout", "Timeout test text", true)],
    };
    let start = std::time::Instant::now();
    coordinator.enrich_update(&mut update).await;
    let elapsed = start.elapsed();
    assert!(
        elapsed >= Duration::from_millis(4900) && elapsed < Duration::from_millis(5600),
        "enrich_update took {:?}",
        elapsed
    );
    assert_eq!(update.upsert_segments[0].draft_translation, None);
}
