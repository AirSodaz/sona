use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sona_core::ports::asr::{
    AsrAudioFrame, AsrPortError, AsrRuntimeObserver, AsrStreamBoundaryEvent, AsrStreamingSession,
    AsrTranscriptUpdateEvent, TranscriptNormalizationOptions, TranscriptPostprocessOptions,
};
use sona_core::ports::vad::StreamingVadPort;
use sona_core::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
use sona_core::transcription::pseudo_streaming::{
    DecodeStage, PseudoStreamDecodeResult, PseudoStreamDecoder, PseudoStreamingSession,
    PseudoStreamingSessionConfig,
};

struct TestStreamingVad {
    speech_detected: Arc<AtomicBool>,
}

impl StreamingVadPort for TestStreamingVad {
    fn accept_samples(&mut self, _samples: &[f32]) {}

    fn is_speech_detected(&self) -> bool {
        self.speech_detected.load(Ordering::Acquire)
    }

    fn reset(&mut self) {}
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
    let vad = TestStreamingVad {
        speech_detected: speech_flag.clone(),
    };
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
