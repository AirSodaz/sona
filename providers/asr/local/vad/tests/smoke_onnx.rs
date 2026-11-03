use sona_core::ports::vad::{VadDetectionOptions, VadEngineKind};
use sona_vad::built_in_engines;
use std::f32::consts::PI;
use std::path::{Path, PathBuf};

fn workspace_model_path(relative: &str) -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(4)
        .unwrap_or(manifest_dir)
        .join(relative)
}

fn generate_speech_like_audio(duration_secs: f32) -> Vec<f32> {
    let num_samples = (duration_secs * 16_000.0) as usize;
    let mut audio = vec![0.0f32; num_samples];

    // Mix 220Hz, 440Hz, and 880Hz harmonics with modulation to simulate speech formant energy
    for (i, sample) in audio.iter_mut().enumerate() {
        let t = i as f32 / 16_000.0;
        let carrier = (2.0 * PI * 220.0 * t).sin() * 0.4
            + (2.0 * PI * 440.0 * t).sin() * 0.3
            + (2.0 * PI * 880.0 * t).sin() * 0.2;
        let envelope = (2.0 * PI * 3.0 * t).sin().abs(); // 3 Hz syllable rate
        *sample = carrier * envelope * 0.6;
    }
    audio
}

#[test]
fn silero_onnx_real_model_smoke_test() {
    let model_path = workspace_model_path("target/test-models/silero_vad.onnx");
    if !model_path.is_file() {
        eprintln!(
            "Skipping silero_onnx_real_model_smoke_test: model not found at {}",
            model_path.display()
        );
        return;
    }

    let engine_set = built_in_engines();
    let engine = engine_set
        .resolve(Some(&model_path))
        .expect("resolves engine");
    assert_eq!(engine.engine_kind(), VadEngineKind::SileroOnnx);

    let opts = VadDetectionOptions::batch_defaults(&model_path);

    // 1. Pure silence: should detect 0 speech spans
    let silence = vec![0.0f32; 32_000]; // 2 seconds
    let silence_spans = engine
        .detect(&silence, 16_000, &opts)
        .expect("detects on silence");
    assert!(silence_spans.is_empty(), "Expected 0 spans on pure silence");

    // 2. Speech-like audio with silence lead and trail:
    // 1.0s silence + 2.0s speech + 1.0s silence = 4.0s (64,000 samples)
    let mut audio = vec![0.0f32; 16_000];
    audio.extend(generate_speech_like_audio(2.0));
    audio.extend(vec![0.0f32; 16_000]);

    let spans = engine
        .detect(&audio, 16_000, &opts)
        .expect("detects on audio");
    assert!(
        !spans.is_empty(),
        "Expected speech spans on synthetic audio"
    );
    for span in &spans {
        assert_eq!(span.sample_rate, 16_000);
        assert!(
            span.start_sample >= 14_000,
            "Speech span should not trigger during initial silence"
        );
        assert!(
            span.end_sample <= 52_000,
            "Speech span should not extend through trailing silence"
        );
    }
    println!("Silero detected spans: {spans:?}");

    // 3. Streaming test
    let streaming_opts = VadDetectionOptions::streaming_defaults(&model_path);
    let mut stream = engine
        .create_stream_detector(&streaming_opts)
        .expect("creates stream");
    assert!(!stream.is_speech_detected());

    // Feed silence: verify no speech detected and no events
    let silence_events = stream.accept_samples(&[0.0; 16_000]);
    assert!(silence_events.is_empty());
    assert!(!stream.is_speech_detected());

    // Feed speech to verify positive activation and SpeechStart event
    let speech_events = stream.accept_samples(&audio[16_000..48_000]);
    assert!(
        stream.is_speech_detected(),
        "Streaming should detect active speech"
    );
    assert!(
        speech_events
            .iter()
            .any(|e| matches!(e, sona_core::ports::vad::VadStreamEvent::SpeechStart { .. })),
        "Expected SpeechStart event on speech segment"
    );

    // Feed trailing silence to verify deactivation at silence boundary
    let end_events = stream.accept_samples(&audio[48_000..64_000]);
    assert!(
        !stream.is_speech_detected(),
        "Streaming should deactivate after trailing silence"
    );
    assert!(
        end_events
            .iter()
            .any(|e| matches!(e, sona_core::ports::vad::VadStreamEvent::SpeechEnd { .. })),
        "Expected SpeechEnd event after trailing silence"
    );

    stream.reset();
    assert!(!stream.is_speech_detected());
}

#[test]
fn fsmn_onnx_real_model_smoke_test() {
    let model_path = workspace_model_path("target/test-models/fsmn_vad.onnx");
    if !model_path.is_file() {
        eprintln!(
            "Skipping fsmn_onnx_real_model_smoke_test: model not found at {}",
            model_path.display()
        );
        return;
    }

    let engine_set = built_in_engines();
    let engine = engine_set
        .resolve(Some(&model_path))
        .expect("resolves engine");
    assert_eq!(engine.engine_kind(), VadEngineKind::FsmnVadOnnx);

    let opts = VadDetectionOptions::batch_defaults(&model_path);

    // 1. Pure silence
    let silence = vec![0.0f32; 32_000];
    let silence_spans = engine
        .detect(&silence, 16_000, &opts)
        .expect("detects on silence");
    assert!(silence_spans.is_empty(), "Expected 0 spans on pure silence");

    // 2. Speech-like audio with lead and trail
    let mut audio = vec![0.0f32; 16_000];
    audio.extend(generate_speech_like_audio(2.0));
    audio.extend(vec![0.0f32; 16_000]);

    let spans = engine
        .detect(&audio, 16_000, &opts)
        .expect("detects on audio");
    assert!(
        !spans.is_empty(),
        "Expected speech spans on synthetic audio"
    );
    for span in &spans {
        assert_eq!(span.sample_rate, 16_000);
        assert!(
            span.start_sample >= 14_000,
            "Speech span should not trigger during initial silence"
        );
        assert!(
            span.end_sample <= 52_000,
            "Speech span should not extend through trailing silence"
        );
    }
    println!("FSMN detected spans: {spans:?}");

    // 3. Streaming test
    let streaming_opts = VadDetectionOptions::streaming_defaults(&model_path);
    let mut stream = engine
        .create_stream_detector(&streaming_opts)
        .expect("creates stream");
    assert!(!stream.is_speech_detected());

    // Feed silence: verify no speech detected and no events
    let silence_events = stream.accept_samples(&[0.0; 16_000]);
    assert!(silence_events.is_empty());
    assert!(!stream.is_speech_detected());

    // Feed speech to verify positive activation and SpeechStart event
    let speech_events = stream.accept_samples(&audio[16_000..48_000]);
    assert!(
        stream.is_speech_detected(),
        "Streaming should detect active speech"
    );
    assert!(
        speech_events
            .iter()
            .any(|e| matches!(e, sona_core::ports::vad::VadStreamEvent::SpeechStart { .. })),
        "Expected SpeechStart event on speech segment"
    );

    // Feed trailing silence to verify deactivation at silence boundary
    let end_events = stream.accept_samples(&audio[48_000..64_000]);
    assert!(
        !stream.is_speech_detected(),
        "Streaming should deactivate after trailing silence"
    );
    assert!(
        end_events
            .iter()
            .any(|e| matches!(e, sona_core::ports::vad::VadStreamEvent::SpeechEnd { .. })),
        "Expected SpeechEnd event after trailing silence"
    );

    stream.reset();
    assert!(!stream.is_speech_detected());
}

#[test]
fn real_speech_audio_file_detection_test() {
    let wav_path = std::env::var_os("SONA_TEST_SAMPLE_WAV")
        .map(PathBuf::from)
        .or_else(|| {
            let p = workspace_model_path("target/test-models/asr_example.wav");
            p.is_file().then_some(p)
        })
        .or_else(|| {
            let p = workspace_model_path("platforms/desktop/sample.wav");
            p.is_file().then_some(p)
        });
    let Some(wav_path) = wav_path else {
        eprintln!("Skipping real_speech_audio_file_detection_test: wav not found");
        return;
    };

    let samples =
        sona_core::audio::decoder::decode_audio_file(&wav_path, 16_000).expect("decodes test wav");
    assert!(!samples.is_empty());

    let engine_set = built_in_engines();

    // 1. Silero VAD on real speech
    let silero_path = workspace_model_path("target/test-models/silero_vad.onnx");
    let fsmn_path = workspace_model_path("target/test-models/fsmn_vad.onnx");
    if !silero_path.is_file() && !fsmn_path.is_file() {
        eprintln!(
            "Skipping real_speech_audio_file_detection_test: neither Silero nor FSMN model found"
        );
        return;
    }

    if silero_path.is_file() {
        let engine = engine_set.resolve(Some(&silero_path)).unwrap();
        let opts = VadDetectionOptions::batch_defaults(&silero_path);
        let spans = engine.detect(&samples, 16_000, &opts).unwrap();
        println!(
            "Real speech - Silero spans count: {}, spans: {:?}",
            spans.len(),
            spans
        );
        assert!(
            !spans.is_empty(),
            "Silero should detect speech in asr_example.wav"
        );
        for span in &spans {
            assert!(
                span.duration() > 0.1,
                "Each span should be meaningful duration"
            );
        }
    }

    // 2. FSMN VAD on real speech
    if fsmn_path.is_file() {
        let engine = engine_set.resolve(Some(&fsmn_path)).unwrap();
        let opts = VadDetectionOptions::batch_defaults(&fsmn_path);
        let spans = engine.detect(&samples, 16_000, &opts).unwrap();
        println!(
            "Real speech - FSMN spans count: {}, spans: {:?}",
            spans.len(),
            spans
        );
        assert!(
            !spans.is_empty(),
            "FSMN should detect speech in asr_example.wav"
        );
        for span in &spans {
            assert!(
                span.duration() > 0.1,
                "Each span should be meaningful duration"
            );
        }
    }
}
