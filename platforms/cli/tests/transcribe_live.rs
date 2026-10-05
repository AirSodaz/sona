use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use sona_cli::live_audio::{LiveAudioChunk, LiveAudioMessage, RunningAudioInput};
use sona_cli::live_output::{LiveOutputFormat, LiveOutputRenderer, LiveStopReason};
use sona_cli::transcribe_live::{LiveSessionMetadata, run_live_session, write_final_transcript};
use sona_core::ports::asr::{
    AsrAudioFrame, AsrPortError, AsrStreamingSession, AsrTranscriptUpdateEvent,
};
use sona_core::transcription::transcript::{TranscriptSegment, TranscriptUpdate};

#[test]
fn transcribe_live_command_exposes_the_public_input_and_output_flags() {
    let output = sona_cli::run_cli_from_args(["sona-cli", "transcribe-live", "--help"])
        .expect("clap help should succeed with exit code 0");
    assert_eq!(output.stderr, "");
    let help = output.stdout;

    assert!(help.contains("Transcribe live audio"));
    assert!(help.contains("--input"));
    assert!(help.contains("--device"));
    assert!(!help.contains("--list-input-devices"));
    assert!(!help.contains("--list-providers"));
    assert!(help.contains("Input/Output"));
    assert!(help.contains("Model Options"));
    assert!(help.contains("Audio & Performance"));
    assert!(help.contains("--duration"));
    assert!(!help.contains("--output-format"));
    assert!(help.contains("--export-format"));
    assert!(help.contains("--output"));
    assert!(help.contains("--force"));
    assert!(help.contains("--online-provider"));
    assert!(help.contains("--api-key-env"));
    assert!(help.contains("--online-config"));
}

#[test]
fn transcribe_live_legacy_list_input_devices_flag_still_works() {
    let output =
        sona_cli::run_cli_from_args(["sona-cli", "transcribe-live", "--list-input-devices"])
            .expect("should succeed");
    assert_eq!(output.stderr, "");
}

#[test]
fn transcribe_live_rejects_zero_duration_before_model_resolution() {
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe-live",
        "--input",
        "stdin",
        "--duration",
        "0",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert_eq!(error.to_string(), "--duration must be greater than 0.");
}

#[test]
fn transcribe_live_rejects_device_for_stdin_before_model_resolution() {
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe-live",
        "--input",
        "stdin",
        "--device",
        "Studio Mic",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert_eq!(
        error.to_string(),
        "--device can only be used with microphone input."
    );
}

#[test]
fn transcribe_live_requires_a_streaming_model_before_opening_input() {
    let error = sona_cli::run_cli_from_args(["sona-cli", "transcribe-live", "--input", "stdin"])
        .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert!(
        error
            .to_string()
            .contains("Missing required streaming model. Pass -m/--model-id, set model_id in --config, or use --online-provider")
    );
    assert!(
        error
            .to_string()
            .contains("sona-cli models download sensevoice")
    );
}

struct RecordingSession {
    calls: Arc<Mutex<Vec<&'static str>>>,
    updates: tokio::sync::mpsc::UnboundedSender<AsrTranscriptUpdateEvent>,
    fail_start: bool,
    fail_feed: bool,
}

#[async_trait]
impl AsrStreamingSession for RecordingSession {
    async fn start(&self) -> Result<(), AsrPortError> {
        self.calls.lock().unwrap().push("start");
        if self.fail_start {
            return Err(AsrPortError::runtime("model start failed"));
        }
        Ok(())
    }

    async fn stop(&self) -> Result<(), AsrPortError> {
        self.calls.lock().unwrap().push("stop");
        Ok(())
    }

    async fn flush(&self) -> Result<(), AsrPortError> {
        self.calls.lock().unwrap().push("flush");
        self.updates
            .send(update_event("final", true))
            .map_err(|error| AsrPortError::runtime(error.to_string()))?;
        Ok(())
    }

    async fn feed_audio_frame(&self, _frame: AsrAudioFrame) -> Result<(), AsrPortError> {
        self.calls.lock().unwrap().push("feed-frame");
        if self.fail_feed {
            return Err(AsrPortError::runtime("decode failed"));
        }
        self.updates
            .send(update_event("partial", false))
            .map_err(|error| AsrPortError::runtime(error.to_string()))?;
        Ok(())
    }
}

struct DelayedFinalSession {
    updates: Mutex<Option<tokio::sync::mpsc::UnboundedSender<AsrTranscriptUpdateEvent>>>,
}

#[async_trait]
impl AsrStreamingSession for DelayedFinalSession {
    async fn start(&self) -> Result<(), AsrPortError> {
        Ok(())
    }

    async fn stop(&self) -> Result<(), AsrPortError> {
        Ok(())
    }

    async fn flush(&self) -> Result<(), AsrPortError> {
        let sender = self
            .updates
            .lock()
            .unwrap()
            .take()
            .expect("flush should run once");
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            let _ = sender.send(update_event("final", true));
        });
        Ok(())
    }

    async fn feed_audio_frame(&self, _frame: AsrAudioFrame) -> Result<(), AsrPortError> {
        Ok(())
    }
}

struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _buffer: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "output closed",
        ))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "output closed",
        ))
    }
}

fn update_event(stage: &str, is_final: bool) -> AsrTranscriptUpdateEvent {
    AsrTranscriptUpdateEvent {
        instance_id: "session-1".to_string(),
        stage: stage.to_string(),
        update: TranscriptUpdate {
            remove_ids: Vec::new(),
            upsert_segments: vec![TranscriptSegment {
                id: "segment-1".to_string(),
                text: "hello".to_string(),
                start: 0.0,
                end: 1.0,
                is_final,
                timing: None,
                tokens: None,
                timestamps: None,
                durations: None,
                translation: None,
                speaker: None,
                speaker_attribution: None,
            }],
        },
    }
}

#[tokio::test]
async fn live_runtime_feeds_audio_flushes_stops_and_drains_final_update() {
    let (input_sender, input_receiver) = tokio::sync::mpsc::channel(4);
    input_sender
        .send(LiveAudioMessage::Chunk(LiveAudioChunk::PcmS16Le(vec![
            0, 0,
        ])))
        .await
        .unwrap();
    input_sender.send(LiveAudioMessage::Eof).await.unwrap();
    drop(input_sender);
    let mut input = RunningAudioInput::from_parts(input_receiver, None, None, false);
    let (update_sender, mut update_receiver) = tokio::sync::mpsc::unbounded_channel();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let session: Arc<dyn AsrStreamingSession> = Arc::new(RecordingSession {
        calls: calls.clone(),
        updates: update_sender,
        fail_start: false,
        fail_feed: false,
    });
    let (_stop_sender, stop_receiver) = tokio::sync::oneshot::channel();
    let mut renderer = LiveOutputRenderer::new(LiveOutputFormat::Text, false, "session-1");
    let mut output = Vec::new();

    let reason = run_live_session(
        session,
        &mut input,
        &mut update_receiver,
        &mut renderer,
        &mut output,
        stop_receiver,
        LiveSessionMetadata {
            source: "stdin".to_string(),
            device_name: None,
            model_id: "streaming-model".to_string(),
        },
    )
    .await
    .unwrap();
    renderer.write_stopped(&mut output, reason).unwrap();

    assert_eq!(
        calls.lock().unwrap().as_slice(),
        &["start", "feed-frame", "flush", "stop"]
    );
    assert_eq!(renderer.segments().len(), 1);
    assert!(renderer.segments()[0].is_final);
    assert_eq!(String::from_utf8(output).unwrap(), "hello\n");
}

#[tokio::test]
async fn live_runtime_waits_for_delayed_final_update_before_returning() {
    let (input_sender, input_receiver) = tokio::sync::mpsc::channel(1);
    input_sender.send(LiveAudioMessage::Eof).await.unwrap();
    drop(input_sender);
    let mut input = RunningAudioInput::from_parts(input_receiver, None, None, false);
    let (update_sender, mut update_receiver) = tokio::sync::mpsc::unbounded_channel();
    let session: Arc<dyn AsrStreamingSession> = Arc::new(DelayedFinalSession {
        updates: Mutex::new(Some(update_sender)),
    });
    let (_stop_sender, stop_receiver) = tokio::sync::oneshot::channel();
    let mut renderer = LiveOutputRenderer::new(LiveOutputFormat::Text, false, "session-1");
    let mut output = Vec::new();

    run_live_session(
        session,
        &mut input,
        &mut update_receiver,
        &mut renderer,
        &mut output,
        stop_receiver,
        LiveSessionMetadata {
            source: "stdin".to_string(),
            device_name: None,
            model_id: "streaming-model".to_string(),
        },
    )
    .await
    .unwrap();

    assert_eq!(renderer.segments().len(), 1);
    assert!(renderer.segments()[0].is_final);
}

#[tokio::test]
async fn live_runtime_does_not_hang_if_update_sender_is_retained() {
    let (input_sender, input_receiver) = tokio::sync::mpsc::channel(1);
    input_sender.send(LiveAudioMessage::Eof).await.unwrap();
    drop(input_sender);
    let mut input = RunningAudioInput::from_parts(input_receiver, None, None, false);
    let (update_sender, mut update_receiver) = tokio::sync::mpsc::unbounded_channel();
    let _retained_sender = update_sender.clone();
    let session: Arc<dyn AsrStreamingSession> = Arc::new(RecordingSession {
        calls: Arc::new(Mutex::new(Vec::new())),
        updates: update_sender,
        fail_start: false,
        fail_feed: false,
    });
    let (_stop_sender, stop_receiver) = tokio::sync::oneshot::channel();
    let mut renderer = LiveOutputRenderer::new(LiveOutputFormat::Text, false, "session-1");
    let mut output = Vec::new();

    let started = std::time::Instant::now();
    let reason = run_live_session(
        session,
        &mut input,
        &mut update_receiver,
        &mut renderer,
        &mut output,
        stop_receiver,
        LiveSessionMetadata {
            source: "stdin".to_string(),
            device_name: None,
            model_id: "streaming-model".to_string(),
        },
    )
    .await
    .unwrap();

    assert_eq!(reason, LiveStopReason::Eof);
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[tokio::test]
async fn live_runtime_stops_session_and_emits_ndjson_error_on_feed_failure() {
    let (input_sender, input_receiver) = tokio::sync::mpsc::channel(2);
    input_sender
        .send(LiveAudioMessage::Chunk(LiveAudioChunk::PcmS16Le(vec![
            0, 0,
        ])))
        .await
        .unwrap();
    drop(input_sender);
    let mut input = RunningAudioInput::from_parts(input_receiver, None, None, false);
    let (update_sender, mut update_receiver) = tokio::sync::mpsc::unbounded_channel();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let session: Arc<dyn AsrStreamingSession> = Arc::new(RecordingSession {
        calls: calls.clone(),
        updates: update_sender,
        fail_start: false,
        fail_feed: true,
    });
    let (_stop_sender, stop_receiver) = tokio::sync::oneshot::channel();
    let mut renderer = LiveOutputRenderer::new(LiveOutputFormat::Ndjson, false, "session-1");
    let mut output = Vec::new();

    let error = run_live_session(
        session,
        &mut input,
        &mut update_receiver,
        &mut renderer,
        &mut output,
        stop_receiver,
        LiveSessionMetadata {
            source: "stdin".to_string(),
            device_name: None,
            model_id: "streaming-model".to_string(),
        },
    )
    .await
    .unwrap_err();

    assert_eq!(error.to_string(), "decode failed");
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        &["start", "feed-frame", "stop"]
    );
    let events = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["type"], "started");
    assert_eq!(events[1]["type"], "error");
    assert_eq!(events[1]["message"], "decode failed");
}

#[tokio::test]
async fn live_runtime_stops_input_and_session_when_start_fails() {
    let (_input_sender, input_receiver) = tokio::sync::mpsc::channel(1);
    let (input_stop_sender, input_stop_receiver) = std::sync::mpsc::channel();
    let mut input = RunningAudioInput::from_parts(
        input_receiver,
        Some(input_stop_sender),
        Some("Studio Mic".to_string()),
        true,
    );
    let (update_sender, mut update_receiver) = tokio::sync::mpsc::unbounded_channel();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let session: Arc<dyn AsrStreamingSession> = Arc::new(RecordingSession {
        calls: calls.clone(),
        updates: update_sender,
        fail_start: true,
        fail_feed: false,
    });
    let (_stop_sender, stop_receiver) = tokio::sync::oneshot::channel();
    let mut renderer = LiveOutputRenderer::new(LiveOutputFormat::Ndjson, false, "session-1");
    let mut output = Vec::new();

    let error = run_live_session(
        session,
        &mut input,
        &mut update_receiver,
        &mut renderer,
        &mut output,
        stop_receiver,
        LiveSessionMetadata {
            source: "microphone".to_string(),
            device_name: Some("Studio Mic".to_string()),
            model_id: "streaming-model".to_string(),
        },
    )
    .await
    .unwrap_err();

    assert_eq!(error.to_string(), "model start failed");
    assert!(input_stop_receiver.try_recv().is_ok());
    assert_eq!(calls.lock().unwrap().as_slice(), &["start", "stop"]);
    assert!(output.is_empty());
}

#[tokio::test]
async fn live_runtime_cleans_up_when_started_event_cannot_be_written() {
    let (_input_sender, input_receiver) = tokio::sync::mpsc::channel(1);
    let (input_stop_sender, input_stop_receiver) = std::sync::mpsc::channel();
    let mut input = RunningAudioInput::from_parts(
        input_receiver,
        Some(input_stop_sender),
        Some("Studio Mic".to_string()),
        true,
    );
    let (update_sender, mut update_receiver) = tokio::sync::mpsc::unbounded_channel();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let session: Arc<dyn AsrStreamingSession> = Arc::new(RecordingSession {
        calls: calls.clone(),
        updates: update_sender,
        fail_start: false,
        fail_feed: false,
    });
    let (_stop_sender, stop_receiver) = tokio::sync::oneshot::channel();
    let mut renderer = LiveOutputRenderer::new(LiveOutputFormat::Ndjson, false, "session-1");
    let mut output = FailingWriter;

    let error = run_live_session(
        session,
        &mut input,
        &mut update_receiver,
        &mut renderer,
        &mut output,
        stop_receiver,
        LiveSessionMetadata {
            source: "microphone".to_string(),
            device_name: Some("Studio Mic".to_string()),
            model_id: "streaming-model".to_string(),
        },
    )
    .await
    .unwrap_err();

    assert!(error.to_string().contains("output closed"));
    assert!(input_stop_receiver.try_recv().is_ok());
    assert_eq!(calls.lock().unwrap().as_slice(), &["start", "stop"]);
}

#[tokio::test]
async fn duration_stop_drains_microphone_tail_before_flushing_session() {
    let (audio_sender, audio_receiver) = tokio::sync::mpsc::channel(2);
    let (input_stop_sender, input_stop_receiver) = std::sync::mpsc::channel();
    let mut input = RunningAudioInput::from_parts(
        audio_receiver,
        Some(input_stop_sender),
        Some("Studio Mic".to_string()),
        true,
    );
    std::thread::spawn(move || {
        input_stop_receiver.recv().unwrap();
        audio_sender
            .blocking_send(LiveAudioMessage::Chunk(LiveAudioChunk::Samples(vec![
                0.25;
                16
            ])))
            .unwrap();
        audio_sender.blocking_send(LiveAudioMessage::Eof).unwrap();
    });
    let (update_sender, mut update_receiver) = tokio::sync::mpsc::unbounded_channel();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let session: Arc<dyn AsrStreamingSession> = Arc::new(RecordingSession {
        calls: calls.clone(),
        updates: update_sender,
        fail_start: false,
        fail_feed: false,
    });
    let (stop_sender, stop_receiver) = tokio::sync::oneshot::channel();
    stop_sender.send(LiveStopReason::Duration).unwrap();
    let mut renderer = LiveOutputRenderer::new(LiveOutputFormat::Text, false, "session-1");
    let mut output = Vec::new();

    let reason = run_live_session(
        session,
        &mut input,
        &mut update_receiver,
        &mut renderer,
        &mut output,
        stop_receiver,
        LiveSessionMetadata {
            source: "microphone".to_string(),
            device_name: Some("Studio Mic".to_string()),
            model_id: "streaming-model".to_string(),
        },
    )
    .await
    .unwrap();

    assert_eq!(reason, LiveStopReason::Duration);
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        &["start", "feed-frame", "flush", "stop"]
    );
}

#[test]
fn final_transcript_is_exported_only_when_the_target_write_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("live.txt");
    let segments = update_event("final", true).update.upsert_segments;

    let status =
        write_final_transcript(&output, sona_core::export::ExportFormat::Txt, &segments).unwrap();

    assert_eq!(status, format!("Wrote transcript to {}", output.display()));
    assert!(std::fs::read_to_string(&output).unwrap().contains("hello"));

    let blocked_parent = dir.path().join("blocked");
    std::fs::write(&blocked_parent, "not a directory").unwrap();
    let failed_output = blocked_parent.join("live.txt");
    assert!(
        write_final_transcript(
            &failed_output,
            sona_core::export::ExportFormat::Txt,
            &segments,
        )
        .is_err()
    );
    assert!(!failed_output.exists());
}

#[test]
fn final_transcript_respects_bilingual_and_translation_mode() {
    let dir = tempfile::tempdir().unwrap();
    let segments = vec![TranscriptSegment {
        id: "segment-1".to_string(),
        text: "hello".to_string(),
        start: 0.0,
        end: 1.0,
        is_final: true,
        timing: None,
        tokens: None,
        timestamps: None,
        durations: None,
        translation: Some("bonjour".to_string()),
        speaker: None,
        speaker_attribution: None,
    }];

    let srt_bilingual = dir.path().join("bilingual.srt");
    sona_cli::transcribe_live::write_final_transcript_with_mode(
        &srt_bilingual,
        sona_core::export::ExportFormat::Srt,
        &segments,
        sona_core::export::ExportMode::Bilingual,
    )
    .unwrap();
    let content = std::fs::read_to_string(&srt_bilingual).unwrap();
    assert!(content.contains("bonjour\nhello"));

    let srt_translation = dir.path().join("translation.srt");
    sona_cli::transcribe_live::write_final_transcript_with_mode(
        &srt_translation,
        sona_core::export::ExportFormat::Srt,
        &segments,
        sona_core::export::ExportMode::Translation,
    )
    .unwrap();
    let content_tr = std::fs::read_to_string(&srt_translation).unwrap();
    assert!(content_tr.contains("bonjour"));
    assert!(!content_tr.contains("hello"));
}

#[test]
fn transcribe_live_rejects_invalid_mode() {
    let error =
        sona_cli::run_cli_from_args(["sona-cli", "transcribe-live", "--mode", "invalid-mode"])
            .unwrap_err();
    assert!(matches!(error, sona_cli::CliError::Usage(_)));
}

#[test]
fn transcribe_live_stream_aliases_parse_correctly() {
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe-live",
        "--stream",
        "ndjson",
        "--duration",
        "0",
    ])
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("--duration must be greater than 0")
    );

    let error2 = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe-live",
        "--stream-format",
        "text",
        "--duration",
        "0",
    ])
    .unwrap_err();
    assert!(
        error2
            .to_string()
            .contains("--duration must be greater than 0")
    );

    let rejected = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe-live",
        "-o",
        "out.srt",
        "--output-format",
        "srt",
    ])
    .unwrap_err();
    assert!(
        rejected
            .to_string()
            .contains("unexpected argument '--output-format'")
            || rejected
                .to_string()
                .contains("unrecognized option '--output-format'")
    );
    let error4 = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe-live",
        "-o",
        "out.srt",
        "--export-format",
        "srt",
        "--duration",
        "0",
    ])
    .unwrap_err();
    assert!(
        error4
            .to_string()
            .contains("--duration must be greater than 0")
    );
}

#[test]
fn write_final_transcript_with_mode_creates_file_even_when_segments_empty() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("empty_live.srt");
    let status = sona_cli::transcribe_live::write_final_transcript_with_mode(
        &out,
        sona_core::export::ExportFormat::Srt,
        &[],
        sona_core::export::ExportMode::Original,
    )
    .unwrap();

    assert!(out.exists());
    assert!(status.contains("empty_live.srt"));
}

#[test]
fn transcribe_live_validates_stream_format_and_format_from_config() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("sona-cli.toml");

    // 1. Invalid stream_format in config triggers validation error
    std::fs::write(
        &config_path,
        r#"
[transcribe_live]
stream_format = "invalid_stream"
"#,
    )
    .unwrap();

    let err = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe-live",
        "-c",
        config_path.to_string_lossy().as_ref(),
    ])
    .unwrap_err();
    assert!(
        err.to_string()
            .contains("Invalid transcribe_live stream_format 'invalid_stream'")
    );

    // 2. stream_format takes precedence over legacy output_format
    std::fs::write(
        &config_path,
        r#"
[transcribe_live]
stream_format = "invalid_preferred"
output_format = "text"
"#,
    )
    .unwrap();

    let err2 = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe-live",
        "-c",
        config_path.to_string_lossy().as_ref(),
    ])
    .unwrap_err();
    assert!(
        err2.to_string()
            .contains("Invalid transcribe_live stream_format 'invalid_preferred'")
    );

    // 3. Invalid format with output file triggers validation error
    std::fs::write(
        &config_path,
        r#"
[transcribe_live]
format = "invalid_export"
"#,
    )
    .unwrap();

    let err3 = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe-live",
        "-o",
        "out.bin",
        "-c",
        config_path.to_string_lossy().as_ref(),
    ])
    .unwrap_err();
    assert!(
        err3.to_string()
            .contains("Unsupported export format: invalid_export")
    );
}

#[test]
fn live_command_primary_syntax_and_unified_model() {
    let output = sona_cli::run_cli_from_args(["sona-cli", "live", "--help"])
        .expect("clap help should succeed for live");
    assert!(output.stdout.contains("--model"));

    // Verify -m is accepted for online provider in live mode
    let err = sona_cli::run_cli_from_args([
        "sona-cli",
        "live",
        "--online-provider",
        "volcengine-doubao",
        "-m",
        "doubao-streaming",
        "--api-key-env",
        "SONA_CLI_TEST_MISSING_LIVE_KEY",
    ])
    .unwrap_err();
    assert_eq!(err.exit_code(), 2);
    assert!(err.to_string().contains("SONA_CLI_TEST_MISSING_LIVE_KEY"));

    // Verify conflict check
    let conflict_err = sona_cli::run_cli_from_args([
        "sona-cli",
        "live",
        "--online-provider",
        "volcengine-doubao",
        "-m",
        "model-a",
        "--online-model",
        "model-b",
    ])
    .unwrap_err();
    assert_eq!(conflict_err.exit_code(), 2);
    assert!(conflict_err.to_string().contains("Conflicting model names"));
}
