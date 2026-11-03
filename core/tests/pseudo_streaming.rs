use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sona_core::ports::asr::{
    AsrAudioFrame, AsrPortError, AsrRuntimeObserver, AsrStreamBoundaryEvent, AsrStreamingSession,
    AsrTranscriptUpdateEvent, TranscriptNormalizationOptions, TranscriptPostprocessOptions,
};
use sona_core::ports::vad::{StreamingVadPort, VadStreamEvent};
use sona_core::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
use sona_core::transcription::pseudo_streaming::{
    DecodeStage, PseudoStreamDecodeResult, PseudoStreamDecoder, PseudoStreamingSession,
    PseudoStreamingSessionConfig,
};

struct TestStreamingVad {
    speech_detected: Arc<AtomicBool>,
    last_state: bool,
    sample_cursor: usize,
    speech_start_sample: usize,
    configured_end_sample: Option<usize>,
    max_speech_samples: Option<usize>,
}

impl TestStreamingVad {
    fn new(speech_detected: Arc<AtomicBool>) -> Self {
        Self {
            speech_detected,
            last_state: false,
            sample_cursor: 0,
            speech_start_sample: 0,
            configured_end_sample: None,
            max_speech_samples: None,
        }
    }
}

impl StreamingVadPort for TestStreamingVad {
    fn accept_samples(&mut self, samples: &[f32]) -> Vec<VadStreamEvent> {
        let current = self.speech_detected.load(Ordering::Acquire);
        let mut events = Vec::new();
        let frame_start = self.sample_cursor;
        self.sample_cursor += samples.len();

        if current && !self.last_state {
            self.last_state = true;
            self.speech_start_sample = frame_start;
            events.push(VadStreamEvent::SpeechStart {
                start_sample: frame_start,
            });
        } else if self.last_state {
            if let Some(max_samples) = self.max_speech_samples
                && self.sample_cursor.saturating_sub(self.speech_start_sample) >= max_samples
            {
                events.push(VadStreamEvent::SpeechEnd {
                    start_sample: self.speech_start_sample,
                    end_sample: self.sample_cursor,
                });
                self.speech_start_sample = self.sample_cursor;
                if current {
                    events.push(VadStreamEvent::SpeechStart {
                        start_sample: self.sample_cursor,
                    });
                } else {
                    self.last_state = false;
                }
            } else if !current {
                self.last_state = false;
                let end_sample = self.configured_end_sample.unwrap_or(frame_start);
                events.push(VadStreamEvent::SpeechEnd {
                    start_sample: self.speech_start_sample,
                    end_sample,
                });
            }
        }
        events
    }

    fn is_speech_detected(&self) -> bool {
        self.speech_detected.load(Ordering::Acquire)
    }

    fn reset(&mut self) {
        self.last_state = false;
        self.sample_cursor = 0;
        self.speech_start_sample = 0;
    }
}

struct TestDecoder;

impl PseudoStreamDecoder for TestDecoder {
    fn decode(
        &self,
        audio_samples: &[f32],
        stage: DecodeStage,
    ) -> Result<Option<PseudoStreamDecodeResult>, AsrPortError> {
        let label = match stage {
            DecodeStage::Partial => format!("hello partial ({})", audio_samples.len()),
            DecodeStage::Final => format!("hello final ({})", audio_samples.len()),
        };
        Ok(Some(PseudoStreamDecodeResult {
            text: label,
            tokens: None,
            timestamps: None,
            durations: None,
        }))
    }
}

#[derive(Default)]
struct TestObserver {
    transcript_updates: std::sync::Mutex<Vec<AsrTranscriptUpdateEvent>>,
    boundaries: std::sync::Mutex<Vec<AsrStreamBoundaryEvent>>,
    inferences: std::sync::Mutex<Vec<AsrInferenceMetric>>,
}

impl AsrRuntimeObserver for TestObserver {
    fn on_transcript_update(&self, event: &AsrTranscriptUpdateEvent) {
        self.transcript_updates.lock().unwrap().push(event.clone());
    }

    fn on_stream_boundary(&self, event: &AsrStreamBoundaryEvent) {
        self.boundaries.lock().unwrap().push(event.clone());
    }

    fn on_live_inference(&self, metric: &AsrInferenceMetric) {
        self.inferences.lock().unwrap().push(metric.clone());
    }

    fn on_model_load(&self, _metric: &AsrModelLoadMetric) {}
}

#[tokio::test]
async fn pseudo_streaming_session_end_to_end_flow() {
    let speech_flag = Arc::new(AtomicBool::new(false));
    let vad = TestStreamingVad::new(speech_flag.clone());
    let observer = Arc::new(TestObserver::default());

    let config = PseudoStreamingSessionConfig {
        instance_id: "test-pipeline".to_string(),
        decoder: Arc::new(TestDecoder),
        vad: Box::new(vad),
        punctuation: None,
        observer: observer.clone(),
        normalization_options: TranscriptNormalizationOptions::default(),
        postprocess_options: TranscriptPostprocessOptions::default(),
        initial_refresh_rate_ms: Some(50),
        enable_partial_decoding: None,
        vad_min_silence_duration_ms: None,
        vad_max_speech_duration_ms: None,
    };

    let session = PseudoStreamingSession::new(config).unwrap();
    session.start().await.unwrap();

    // 1. Feed silence (1600 samples = 100ms)
    session
        .feed_audio_frame(AsrAudioFrame::new(1, 0, vec![0.0f32; 1600]))
        .await
        .unwrap();
    assert_eq!(observer.transcript_updates.lock().unwrap().len(), 0);

    // 2. Speech starts (speech_flag = true)
    speech_flag.store(true, Ordering::Release);
    session
        .feed_audio_frame(AsrAudioFrame::new(2, 1600, vec![0.5f32; 1600]))
        .await
        .unwrap();

    // Wait 60ms to let backoff interval (50ms) expire for partial
    tokio::time::sleep(Duration::from_millis(60)).await;

    // Feed another chunk during speech -> triggers partial decode
    session
        .feed_audio_frame(AsrAudioFrame::new(3, 3200, vec![0.5f32; 1600]))
        .await
        .unwrap();

    // Wait a brief moment for the spawned inference task
    tokio::time::sleep(Duration::from_millis(100)).await;

    {
        let updates = observer.transcript_updates.lock().unwrap();
        assert!(
            !updates.is_empty(),
            "Should have emitted partial transcript"
        );
        assert_eq!(updates[0].stage, "partial");
        assert!(!updates[0].update.upsert_segments[0].is_final);
    }

    // 3. Speech ends (speech_flag = false)
    speech_flag.store(false, Ordering::Release);
    session
        .feed_audio_frame(AsrAudioFrame::new(4, 4800, vec![0.0f32; 1600]))
        .await
        .unwrap();

    // Wait for the final inference task to complete
    tokio::time::sleep(Duration::from_millis(100)).await;

    {
        let updates = observer.transcript_updates.lock().unwrap();
        let last = updates.last().unwrap();
        assert_eq!(last.stage, "final");
        assert!(last.update.upsert_segments[0].is_final);

        let boundaries = observer.boundaries.lock().unwrap();
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].sequence, 4);
    }

    session.stop().await.unwrap();
}

#[tokio::test]
async fn pseudo_streaming_session_sentence_only_flow() {
    let speech_flag = Arc::new(AtomicBool::new(false));
    let vad = TestStreamingVad::new(speech_flag.clone());
    let observer = Arc::new(TestObserver::default());

    let config = PseudoStreamingSessionConfig {
        instance_id: "test-sentence-only".to_string(),
        decoder: Arc::new(TestDecoder),
        vad: Box::new(vad),
        punctuation: None,
        observer: observer.clone(),
        normalization_options: TranscriptNormalizationOptions::default(),
        postprocess_options: TranscriptPostprocessOptions::default(),
        initial_refresh_rate_ms: Some(50),
        enable_partial_decoding: Some(false),
        vad_min_silence_duration_ms: None,
        vad_max_speech_duration_ms: None,
    };

    let session = PseudoStreamingSession::new(config).unwrap();
    session.start().await.unwrap();

    // 1. Feed silence (1600 samples = 100ms)
    session
        .feed_audio_frame(AsrAudioFrame::new(1, 0, vec![0.0f32; 1600]))
        .await
        .unwrap();
    assert_eq!(observer.transcript_updates.lock().unwrap().len(), 0);

    // 2. Speech starts (speech_flag = true)
    speech_flag.store(true, Ordering::Release);
    session
        .feed_audio_frame(AsrAudioFrame::new(2, 1600, vec![0.5f32; 1600]))
        .await
        .unwrap();

    // Wait 60ms to let refresh rate interval (50ms) expire
    tokio::time::sleep(Duration::from_millis(60)).await;

    // Feed another chunk during speech
    session
        .feed_audio_frame(AsrAudioFrame::new(3, 3200, vec![0.5f32; 1600]))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    // With partial decoding disabled, NO partial updates must be emitted during speech
    assert_eq!(
        observer.transcript_updates.lock().unwrap().len(),
        0,
        "Sentence-only mode must not emit any partial transcript during speech"
    );

    // 3. Speech ends (speech_flag = false) -> triggers single final whole-sentence inference
    speech_flag.store(false, Ordering::Release);
    session
        .feed_audio_frame(AsrAudioFrame::new(4, 4800, vec![0.0f32; 1600]))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    {
        let updates = observer.transcript_updates.lock().unwrap();
        assert_eq!(
            updates.len(),
            1,
            "Sentence-only mode must emit exactly one update at sentence boundary"
        );
        let final_update = &updates[0];
        assert_eq!(final_update.stage, "final");
        assert!(final_update.update.upsert_segments[0].is_final);

        let boundaries = observer.boundaries.lock().unwrap();
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].sequence, 4);
    }

    session.stop().await.unwrap();
}

struct DurationsTestDecoder {
    return_empty_durations: bool,
}

impl PseudoStreamDecoder for DurationsTestDecoder {
    fn decode(
        &self,
        _audio_samples: &[f32],
        _stage: DecodeStage,
    ) -> Result<Option<PseudoStreamDecodeResult>, AsrPortError> {
        Ok(Some(PseudoStreamDecodeResult {
            text: "word1 word2".to_string(),
            tokens: Some(vec!["word1".to_string(), "word2".to_string()]),
            timestamps: Some(vec![0.02, 0.08]),
            durations: if self.return_empty_durations {
                Some(Vec::new())
            } else {
                Some(vec![0.35, 0.45])
            },
        }))
    }
}

#[tokio::test]
async fn pseudo_streaming_empty_durations_falls_back_to_synthesize() {
    let speech_flag = Arc::new(AtomicBool::new(true));
    let vad = TestStreamingVad::new(speech_flag.clone());
    let observer = Arc::new(TestObserver::default());

    let config = PseudoStreamingSessionConfig {
        instance_id: "test-pipeline".to_string(),
        decoder: Arc::new(DurationsTestDecoder {
            return_empty_durations: true,
        }),
        vad: Box::new(vad),
        punctuation: None,
        observer: observer.clone(),
        normalization_options: TranscriptNormalizationOptions::default(),
        postprocess_options: TranscriptPostprocessOptions::default(),
        initial_refresh_rate_ms: Some(50),
        enable_partial_decoding: None,
        vad_min_silence_duration_ms: None,
        vad_max_speech_duration_ms: None,
    };

    let session = PseudoStreamingSession::new(config).unwrap();
    session.start().await.unwrap();

    session
        .feed_audio_frame(AsrAudioFrame::new(1, 0, vec![0.5f32; 1600]))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(60)).await;

    session
        .feed_audio_frame(AsrAudioFrame::new(2, 1600, vec![0.5f32; 1600]))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    {
        let updates = observer.transcript_updates.lock().unwrap();
        assert!(!updates.is_empty(), "Should have emitted transcript");
        let seg = &updates[0].update.upsert_segments[0];
        assert!(
            seg.durations.is_some(),
            "durations must be synthesized when decoder returned empty vector"
        );
        let durs = seg.durations.as_ref().unwrap();
        assert_eq!(durs.len(), 2);
        assert!(durs[0] > 0.0);
        assert!(durs[1] > 0.0);
    }

    session.stop().await.unwrap();
}

#[tokio::test]
async fn pseudo_streaming_explicit_durations_are_preserved() {
    let speech_flag = Arc::new(AtomicBool::new(true));
    let vad = TestStreamingVad::new(speech_flag.clone());
    let observer = Arc::new(TestObserver::default());

    let config = PseudoStreamingSessionConfig {
        instance_id: "test-pipeline".to_string(),
        decoder: Arc::new(DurationsTestDecoder {
            return_empty_durations: false,
        }),
        vad: Box::new(vad),
        punctuation: None,
        observer: observer.clone(),
        normalization_options: TranscriptNormalizationOptions::default(),
        postprocess_options: TranscriptPostprocessOptions::default(),
        initial_refresh_rate_ms: Some(50),
        enable_partial_decoding: None,
        vad_min_silence_duration_ms: None,
        vad_max_speech_duration_ms: None,
    };

    let session = PseudoStreamingSession::new(config).unwrap();
    session.start().await.unwrap();

    session
        .feed_audio_frame(AsrAudioFrame::new(1, 0, vec![0.5f32; 1600]))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(60)).await;

    session
        .feed_audio_frame(AsrAudioFrame::new(2, 1600, vec![0.5f32; 1600]))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    {
        let updates = observer.transcript_updates.lock().unwrap();
        assert!(!updates.is_empty(), "Should have emitted transcript");
        let seg = &updates[0].update.upsert_segments[0];
        assert_eq!(
            seg.durations,
            Some(vec![0.35, 0.45]),
            "matching explicit durations must be preserved"
        );
    }

    session.stop().await.unwrap();
}

struct MismatchedDurationsTestDecoder;

impl PseudoStreamDecoder for MismatchedDurationsTestDecoder {
    fn decode(
        &self,
        _audio_samples: &[f32],
        _stage: DecodeStage,
    ) -> Result<Option<PseudoStreamDecodeResult>, AsrPortError> {
        Ok(Some(PseudoStreamDecodeResult {
            text: "word1 word2".to_string(),
            tokens: Some(vec!["word1".to_string(), "word2".to_string()]),
            timestamps: Some(vec![0.02, 0.08]),
            durations: Some(vec![0.99]),
        }))
    }
}

#[tokio::test]
async fn pseudo_streaming_mismatched_durations_falls_back_to_synthesize() {
    let speech_flag = Arc::new(AtomicBool::new(true));
    let vad = TestStreamingVad::new(speech_flag.clone());
    let observer = Arc::new(TestObserver::default());

    let config = PseudoStreamingSessionConfig {
        instance_id: "test-pipeline".to_string(),
        decoder: Arc::new(MismatchedDurationsTestDecoder),
        vad: Box::new(vad),
        punctuation: None,
        observer: observer.clone(),
        normalization_options: TranscriptNormalizationOptions::default(),
        postprocess_options: TranscriptPostprocessOptions::default(),
        initial_refresh_rate_ms: Some(50),
        enable_partial_decoding: None,
        vad_min_silence_duration_ms: None,
        vad_max_speech_duration_ms: None,
    };

    let session = PseudoStreamingSession::new(config).unwrap();
    session.start().await.unwrap();

    session
        .feed_audio_frame(AsrAudioFrame::new(1, 0, vec![0.5f32; 1600]))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(60)).await;

    session
        .feed_audio_frame(AsrAudioFrame::new(2, 1600, vec![0.5f32; 1600]))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    {
        let updates = observer.transcript_updates.lock().unwrap();
        assert!(!updates.is_empty(), "Should have emitted transcript");
        let seg = &updates[0].update.upsert_segments[0];
        assert!(
            seg.durations.is_some(),
            "durations must be synthesized when decoder returned mismatched vector"
        );
        let durs = seg.durations.as_ref().unwrap();
        assert_eq!(durs.len(), 2);
        assert_ne!(durs, &vec![0.99]);
        assert!(durs[0] > 0.0);
        assert!(durs[1] > 0.0);
    }

    session.stop().await.unwrap();
}

#[tokio::test]
async fn natural_speech_offset_trims_confirmation_silence() {
    let speech_flag = Arc::new(AtomicBool::new(false));
    let mut vad = TestStreamingVad::new(speech_flag.clone());
    vad.configured_end_sample = Some(3200);
    let observer = Arc::new(TestObserver::default());

    let config = PseudoStreamingSessionConfig {
        instance_id: "test-trimming".to_string(),
        decoder: Arc::new(TestDecoder),
        vad: Box::new(vad),
        punctuation: None,
        observer: observer.clone(),
        normalization_options: TranscriptNormalizationOptions::default(),
        postprocess_options: TranscriptPostprocessOptions::default(),
        initial_refresh_rate_ms: Some(50),
        enable_partial_decoding: Some(false),
        vad_min_silence_duration_ms: Some(500),
        vad_max_speech_duration_ms: None,
    };

    let session = PseudoStreamingSession::new(config).unwrap();
    session.start().await.unwrap();

    // Speech starts: 2 frames of speech (3200 samples)
    speech_flag.store(true, Ordering::Release);
    session
        .feed_audio_frame(AsrAudioFrame::new(1, 0, vec![0.5f32; 1600]))
        .await
        .unwrap();
    session
        .feed_audio_frame(AsrAudioFrame::new(2, 1600, vec![0.5f32; 1600]))
        .await
        .unwrap();

    // VAD confirmation silence: 5 frames of silence (500ms = 8000 samples) while speech_detected is still true
    for i in 0..5 {
        let start = 3200 + i * 1600;
        session
            .feed_audio_frame(AsrAudioFrame::new(
                3 + i as u64,
                start as u64,
                vec![0.0f32; 1600],
            ))
            .await
            .unwrap();
    }

    // Now VAD signals speech offset (speech_detected = false)
    speech_flag.store(false, Ordering::Release);
    session
        .feed_audio_frame(AsrAudioFrame::new(8, 11200, vec![0.0f32; 1600]))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    {
        let updates = observer.transcript_updates.lock().unwrap();
        let last = updates.last().expect("should have emitted final update");
        assert_eq!(last.stage, "final");
        // Exact state-machine event trimming down to end_sample (3200)
        assert_eq!(last.update.upsert_segments[0].text, "hello final (3200)");
    }

    session.stop().await.unwrap();
}

#[tokio::test]
async fn flush_preserves_audio_without_tail_trimming() {
    let speech_flag = Arc::new(AtomicBool::new(false));
    let vad = TestStreamingVad::new(speech_flag.clone());
    let observer = Arc::new(TestObserver::default());

    let config = PseudoStreamingSessionConfig {
        instance_id: "test-flush-no-trim".to_string(),
        decoder: Arc::new(TestDecoder),
        vad: Box::new(vad),
        punctuation: None,
        observer: observer.clone(),
        normalization_options: TranscriptNormalizationOptions::default(),
        postprocess_options: TranscriptPostprocessOptions::default(),
        initial_refresh_rate_ms: Some(50),
        enable_partial_decoding: Some(false),
        vad_min_silence_duration_ms: Some(500),
        vad_max_speech_duration_ms: None,
    };

    let session = PseudoStreamingSession::new(config).unwrap();
    session.start().await.unwrap();

    // Speech starts: 2 frames of speech (3200 samples)
    speech_flag.store(true, Ordering::Release);
    session
        .feed_audio_frame(AsrAudioFrame::new(1, 0, vec![0.5f32; 1600]))
        .await
        .unwrap();
    session
        .feed_audio_frame(AsrAudioFrame::new(2, 1600, vec![0.5f32; 1600]))
        .await
        .unwrap();

    // Flush immediately during speech
    session.flush().await.unwrap();

    {
        let updates = observer.transcript_updates.lock().unwrap();
        let last = updates
            .last()
            .expect("should have emitted final update on flush");
        assert_eq!(last.stage, "final");
        // All 3200 samples preserved verbatim without trimming
        assert_eq!(last.update.upsert_segments[0].text, "hello final (3200)");
    }

    session.stop().await.unwrap();
}

#[tokio::test]
async fn continuous_speech_exceeding_max_duration_forces_utterance_finalization() {
    let speech_flag = Arc::new(AtomicBool::new(false));
    let mut vad = TestStreamingVad::new(speech_flag.clone());
    vad.max_speech_samples = Some(3200);
    let observer = Arc::new(TestObserver::default());

    // Set max speech duration to 200ms (3200 samples at 16kHz) for testing
    let config = PseudoStreamingSessionConfig {
        instance_id: "test-max-duration".to_string(),
        decoder: Arc::new(TestDecoder),
        vad: Box::new(vad),
        punctuation: None,
        observer: observer.clone(),
        normalization_options: TranscriptNormalizationOptions::default(),
        postprocess_options: TranscriptPostprocessOptions::default(),
        initial_refresh_rate_ms: Some(50),
        enable_partial_decoding: Some(false),
        vad_min_silence_duration_ms: Some(500),
        vad_max_speech_duration_ms: Some(200),
    };

    let session = PseudoStreamingSession::new(config).unwrap();
    session.start().await.unwrap();

    // Speech starts and stays true continuously (e.g. loud continuous noise or monologue)
    speech_flag.store(true, Ordering::Release);
    // Frame 1: 1600 samples (100ms)
    session
        .feed_audio_frame(AsrAudioFrame::new(1, 0, vec![0.5f32; 1600]))
        .await
        .unwrap();

    // Frame 2: 1600 samples (100ms) -> total reaches 3200 samples (200ms max speech limit)
    session
        .feed_audio_frame(AsrAudioFrame::new(2, 1600, vec![0.5f32; 1600]))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    let first_seg = {
        let updates = observer.transcript_updates.lock().unwrap();
        let last = updates
            .last()
            .expect("should have emitted forced final update at max speech duration");
        assert_eq!(last.stage, "final");
        assert_eq!(last.update.upsert_segments[0].text, "hello final (3200)");
        last.update.upsert_segments[0].clone()
    };

    // Frame 3: 1600 samples (100ms) -> speech continues in second segment
    session
        .feed_audio_frame(AsrAudioFrame::new(3, 3200, vec![0.5f32; 1600]))
        .await
        .unwrap();

    session.flush().await.unwrap();

    {
        let updates = observer.transcript_updates.lock().unwrap();
        let second = updates
            .last()
            .expect("should have emitted final update for continuing segment on flush");
        assert_eq!(second.stage, "final");
        let second_seg = &second.update.upsert_segments[0];
        assert_ne!(first_seg.id, second_seg.id);
        assert!(
            second_seg.start >= first_seg.end,
            "second segment start ({}) should be >= first segment end ({})",
            second_seg.start,
            first_seg.end
        );
    }

    session.stop().await.unwrap();
}
