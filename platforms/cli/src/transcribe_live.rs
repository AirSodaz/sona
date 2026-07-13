use crate::live_audio::{
    LiveAudioChunk, LiveAudioMessage, RunningAudioInput, microphone_device_names,
    spawn_stdin_reader, start_microphone_input,
};
use crate::live_output::{LiveOutputFormat, LiveOutputRenderer, LiveStopReason};
use crate::{CliError, CliIo, CliResult};
use clap::{Args, ValueEnum};
use sona_core::ports::asr::{AsrRuntimeObserver, AsrStreamingSession, AsrTranscriptUpdateEvent};
use sona_core::runtime::config::TranscribeLiveConfigSection;
use sona_core::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
use sona_core::transcription::runtime::{LiveTranscribeOptions, LiveTranscribePlan};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum LiveInputSource {
    Microphone,
    Stdin,
}

impl LiveInputSource {
    fn label(self) -> &'static str {
        match self {
            Self::Microphone => "microphone",
            Self::Stdin => "stdin",
        }
    }

    fn parse_config(value: &str) -> CliResult<Self> {
        match value {
            "microphone" => Ok(Self::Microphone),
            "stdin" => Ok(Self::Stdin),
            _ => Err(CliError::Validation(format!(
                "Invalid transcribe_live input '{value}'. Expected microphone or stdin."
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum LiveOutputFormatArg {
    Text,
    Ndjson,
}

impl LiveOutputFormatArg {
    fn parse_config(value: &str) -> CliResult<Self> {
        match value {
            "text" => Ok(Self::Text),
            "ndjson" => Ok(Self::Ndjson),
            _ => Err(CliError::Validation(format!(
                "Invalid transcribe_live output_format '{value}'. Expected text or ndjson."
            ))),
        }
    }
}

impl From<LiveOutputFormatArg> for LiveOutputFormat {
    fn from(value: LiveOutputFormatArg) -> Self {
        match value {
            LiveOutputFormatArg::Text => Self::Text,
            LiveOutputFormatArg::Ndjson => Self::Ndjson,
        }
    }
}

#[derive(Debug, Args)]
#[command(
    about = "Transcribe live audio with offline ASR",
    after_help = "Examples:\n  sona-cli transcribe-live --model-id sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17\n  ffmpeg -i sample.wav -f s16le -ac 1 -ar 16000 - | sona-cli transcribe-live --input stdin --model-id sherpa-onnx-streaming-paraformer-trilingual-zh-cantonese-en --output-format ndjson"
)]
pub struct TranscribeLiveArgs {
    /// Live input source.
    #[arg(long, value_enum)]
    input: Option<LiveInputSource>,
    /// Exact microphone device name.
    #[arg(long, value_name = "NAME")]
    device: Option<String>,
    /// List microphone input devices and exit.
    #[arg(long, default_value_t = false)]
    list_input_devices: bool,
    /// Stop after this many seconds.
    #[arg(long, value_name = "SECONDS")]
    duration: Option<f64>,
    /// Live stdout format.
    #[arg(long, value_enum, value_name = "FORMAT")]
    output_format: Option<LiveOutputFormatArg>,
    /// Optional final transcript file.
    #[arg(short, long, value_name = "PATH")]
    output: Option<PathBuf>,
    /// Final transcript format: json, txt, srt, vtt, or md.
    #[arg(short, long)]
    format: Option<String>,
    /// Optional config file, usually sona-cli.toml.
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,
    /// Override the language setting.
    #[arg(long)]
    language: Option<String>,
    /// Streaming preset model id to use.
    #[arg(long = "model-id")]
    model_id: Option<String>,
    /// Models directory containing installed presets.
    #[arg(long = "models-dir")]
    models_dir: Option<PathBuf>,
    /// VAD model id override.
    #[arg(long = "vad-model-id")]
    vad_model_id: Option<String>,
    /// Punctuation model id override.
    #[arg(long = "punctuation-model-id")]
    punctuation_model_id: Option<String>,
    /// Number of recognition threads.
    #[arg(long)]
    threads: Option<i32>,
    /// Enable inverse text normalization.
    #[arg(long, default_value_t = false)]
    enable_itn: bool,
    /// Optional hotwords string.
    #[arg(long)]
    hotwords: Option<String>,
    /// GPU acceleration mode.
    #[arg(long = "gpu-acceleration")]
    gpu_acceleration: Option<String>,
    /// VAD buffer size in seconds.
    #[arg(long = "vad-buffer")]
    vad_buffer: Option<f32>,
    /// Overwrite an existing final transcript file.
    #[arg(long, default_value_t = false)]
    force: bool,
}

struct ResolvedLiveCommand {
    input: LiveInputSource,
    device: Option<String>,
    duration: Option<Duration>,
    output_format: LiveOutputFormat,
    plan: LiveTranscribePlan,
}

struct CliStreamingObserver {
    sender: tokio::sync::mpsc::UnboundedSender<AsrTranscriptUpdateEvent>,
}

impl AsrRuntimeObserver for CliStreamingObserver {
    fn on_transcript_update(&self, event: &AsrTranscriptUpdateEvent) {
        let _ = self.sender.send(event.clone());
    }

    fn on_model_load(&self, _metric: &AsrModelLoadMetric) {}

    fn on_live_inference(&self, _metric: &AsrInferenceMetric) {}
}

pub fn run_transcribe_live(args: TranscribeLiveArgs, io: &mut dyn CliIo) -> CliResult<()> {
    if args.list_input_devices {
        let devices = microphone_device_names().map_err(CliError::Io)?;
        let output = if devices.is_empty() {
            String::new()
        } else {
            format!("{}\n", devices.join("\n"))
        };
        io.stdout()
            .write_all(output.as_bytes())
            .map_err(|error| CliError::Io(format!("Failed to write input devices: {error}")))?;
        io.stdout()
            .flush()
            .map_err(|error| CliError::Io(format!("Failed to flush input devices: {error}")))?;
        return Ok(());
    }

    validate_direct_input_options(args.input, args.device.as_deref(), args.duration)?;
    let config = load_config(args.config.as_ref())?;
    let resolved = resolve_live_command(args, config)?;
    let stdout_is_terminal = io.stdout_is_terminal();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| CliError::Io(format!("Failed to create async runtime: {error}")))?;
    let status = {
        let stdout = io.stdout();
        runtime.block_on(run_resolved_live_command(
            resolved,
            stdout_is_terminal,
            stdout,
        ))?
    };
    if let Some(status) = status {
        writeln!(io.stderr(), "{status}")
            .map_err(|error| CliError::Io(format!("Failed to write live status: {error}")))?;
    }
    Ok(())
}

fn validate_direct_input_options(
    input: Option<LiveInputSource>,
    device: Option<&str>,
    duration: Option<f64>,
) -> CliResult<()> {
    if duration.is_some_and(|seconds| !seconds.is_finite() || seconds <= 0.0) {
        return Err(CliError::Validation(
            "--duration must be greater than 0.".to_string(),
        ));
    }
    if input == Some(LiveInputSource::Stdin) && device.is_some() {
        return Err(CliError::Validation(
            "--device can only be used with microphone input.".to_string(),
        ));
    }
    Ok(())
}

fn load_config(path: Option<&PathBuf>) -> CliResult<Option<TranscribeLiveConfigSection>> {
    let Some(path) = path else {
        return Ok(None);
    };
    sona_runtime_fs::load_transcribe_live_config_file(path)
        .map(Some)
        .map_err(CliError::Validation)
}

fn resolve_live_command(
    args: TranscribeLiveArgs,
    config: Option<TranscribeLiveConfigSection>,
) -> CliResult<ResolvedLiveCommand> {
    let config = config.unwrap_or_default();
    let input = match args.input {
        Some(input) => input,
        None => config
            .input
            .as_deref()
            .map(LiveInputSource::parse_config)
            .transpose()?
            .unwrap_or(LiveInputSource::Microphone),
    };
    let device = args.device.or(config.device.clone());
    let duration_seconds = args.duration.or(config.duration_seconds);
    validate_direct_input_options(Some(input), device.as_deref(), duration_seconds)?;
    let duration = duration_seconds.map(Duration::from_secs_f64);
    let output_format = match args.output_format {
        Some(format) => format,
        None => config
            .output_format
            .as_deref()
            .map(LiveOutputFormatArg::parse_config)
            .transpose()?
            .unwrap_or(LiveOutputFormatArg::Text),
    };
    let plan = sona_runtime_fs::resolve_live_transcribe_plan_with_runtime_paths(
        LiveTranscribeOptions {
            output: args.output,
            format: args.format,
            model_id: args.model_id,
            models_dir: args.models_dir,
            default_models_dir: crate::desktop_paths::default_models_dir(),
            vad_model_id: args.vad_model_id,
            punctuation_model_id: args.punctuation_model_id,
            threads: args.threads,
            enable_itn: args.enable_itn.then_some(true),
            language: args.language,
            hotwords: args.hotwords,
            gpu_acceleration: args.gpu_acceleration,
            vad_buffer: args.vad_buffer,
            force: args.force,
        },
        Some(config),
    )
    .map_err(CliError::Validation)?;
    Ok(ResolvedLiveCommand {
        input,
        device,
        duration,
        output_format: output_format.into(),
        plan,
    })
}

async fn run_resolved_live_command(
    resolved: ResolvedLiveCommand,
    stdout_is_terminal: bool,
    stdout: &mut dyn Write,
) -> CliResult<Option<String>> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let (update_sender, mut update_receiver) = tokio::sync::mpsc::unbounded_channel();
    let observer: Arc<dyn AsrRuntimeObserver> = Arc::new(CliStreamingObserver {
        sender: update_sender,
    });
    let session =
        crate::asr_adapter::local_streaming_session(&resolved.plan, &session_id, observer)
            .await
            .map_err(CliError::Model)?;
    let mut input = match resolved.input {
        LiveInputSource::Microphone => {
            start_microphone_input(resolved.device.as_deref()).map_err(CliError::Io)?
        }
        LiveInputSource::Stdin => spawn_stdin_reader(std::io::stdin()),
    };
    let stop_receiver = spawn_stop_signal(resolved.duration);
    let metadata = LiveSessionMetadata {
        source: resolved.input.label().to_string(),
        device_name: input.device_name.clone(),
        model_id: resolved.plan.model_id.clone(),
    };
    let mut renderer =
        LiveOutputRenderer::new(resolved.output_format, stdout_is_terminal, &session_id);
    let reason = run_live_session(
        session,
        &mut input,
        &mut update_receiver,
        &mut renderer,
        stdout,
        stop_receiver,
        metadata,
    )
    .await?;

    let status = if let Some(path) = resolved.plan.output_path.as_ref() {
        let format = resolved
            .plan
            .export_format
            .expect("live plan with output path must include an export format");
        match write_final_transcript(path, format, renderer.segments()) {
            Ok(status) => Some(status),
            Err(error) => {
                let _ = renderer.write_error(stdout, &error.to_string());
                return Err(error);
            }
        }
    } else {
        None
    };
    renderer
        .write_stopped(stdout, reason)
        .map_err(CliError::Io)?;
    Ok(status)
}

fn write_final_transcript(
    path: &std::path::Path,
    format: sona_core::export::ExportFormat,
    segments: &[sona_core::transcription::transcript::TranscriptSegment],
) -> CliResult<String> {
    let exported = sona_core::export::export_segments_with_mode(
        segments,
        format,
        sona_core::export::ExportMode::Original,
    )
    .map_err(CliError::Serialize)?;
    sona_runtime_fs::write_transcript_output_file(path, &exported).map_err(CliError::Io)?;
    Ok(format!("Wrote transcript to {}", path.display()))
}

fn spawn_stop_signal(duration: Option<Duration>) -> tokio::sync::oneshot::Receiver<LiveStopReason> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let reason = match duration {
            Some(duration) => {
                tokio::select! {
                    _ = tokio::time::sleep(duration) => LiveStopReason::Duration,
                    _ = tokio::signal::ctrl_c() => LiveStopReason::CtrlC,
                }
            }
            None => {
                let _ = tokio::signal::ctrl_c().await;
                LiveStopReason::CtrlC
            }
        };
        let _ = sender.send(reason);
    });
    receiver
}

pub(crate) struct LiveSessionMetadata {
    pub(crate) source: String,
    pub(crate) device_name: Option<String>,
    pub(crate) model_id: String,
}

pub(crate) async fn run_live_session<W: Write + ?Sized>(
    session: Arc<dyn AsrStreamingSession>,
    input: &mut RunningAudioInput,
    updates: &mut tokio::sync::mpsc::UnboundedReceiver<AsrTranscriptUpdateEvent>,
    renderer: &mut LiveOutputRenderer,
    output: &mut W,
    mut stop_receiver: tokio::sync::oneshot::Receiver<LiveStopReason>,
    metadata: LiveSessionMetadata,
) -> CliResult<LiveStopReason> {
    if let Err(error) = session.start().await {
        input.request_stop();
        let _ = session.stop().await;
        return Err(CliError::Model(error.to_string()));
    }
    if let Err(error) = renderer.write_started(
        output,
        &metadata.source,
        metadata.device_name.as_deref(),
        &metadata.model_id,
    ) {
        input.request_stop();
        let _ = session.stop().await;
        return Err(CliError::Io(error));
    }

    let run_result: CliResult<LiveStopReason> = async {
        loop {
            tokio::select! {
                biased;
                Some(event) = updates.recv() => {
                    renderer
                        .write_update(output, &event.stage, event.update)
                        .map_err(CliError::Io)?;
                }
                message = input.receiver.recv() => {
                    match message.unwrap_or(LiveAudioMessage::Eof) {
                        LiveAudioMessage::Chunk(chunk) => feed_audio(session.as_ref(), chunk).await?,
                        LiveAudioMessage::Eof => break Ok(LiveStopReason::Eof),
                        LiveAudioMessage::Error(error) => break Err(CliError::Io(error)),
                    }
                }
                stop = &mut stop_receiver => {
                    let reason = stop.unwrap_or(LiveStopReason::CtrlC);
                    input.request_stop();
                    if input.should_drain_on_stop() {
                        drain_stopped_input(session.as_ref(), input).await?;
                    }
                    break Ok(reason);
                }
            }
        }
    }
    .await;
    let reason = match run_result {
        Ok(reason) => reason,
        Err(error) => {
            input.request_stop();
            let _ = session.stop().await;
            renderer
                .write_error(output, &error.to_string())
                .map_err(CliError::Io)?;
            return Err(error);
        }
    };

    input.request_stop();
    if let Err(error) = session.flush().await {
        let error = CliError::Model(error.to_string());
        let _ = session.stop().await;
        renderer
            .write_error(output, &error.to_string())
            .map_err(CliError::Io)?;
        return Err(error);
    }
    if let Err(error) = session.stop().await {
        let error = CliError::Model(error.to_string());
        renderer
            .write_error(output, &error.to_string())
            .map_err(CliError::Io)?;
        return Err(error);
    }
    drop(session);
    while let Some(event) = updates.recv().await {
        renderer
            .write_update(output, &event.stage, event.update)
            .map_err(CliError::Io)?;
    }
    Ok(reason)
}

async fn drain_stopped_input(
    session: &dyn AsrStreamingSession,
    input: &mut RunningAudioInput,
) -> CliResult<()> {
    while let Some(message) = input.receiver.recv().await {
        match message {
            LiveAudioMessage::Chunk(chunk) => feed_audio(session, chunk).await?,
            LiveAudioMessage::Eof => return Ok(()),
            LiveAudioMessage::Error(error) => return Err(CliError::Io(error)),
        }
    }
    Ok(())
}

async fn feed_audio(session: &dyn AsrStreamingSession, chunk: LiveAudioChunk) -> CliResult<()> {
    match chunk {
        LiveAudioChunk::PcmS16Le(bytes) => session.feed_audio_chunk(bytes).await,
        LiveAudioChunk::Samples(samples) => session.feed_audio_samples(&samples).await,
    }
    .map_err(|error| CliError::Model(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use sona_core::ports::asr::{AsrStreamingSession, AsrTranscriptUpdateEvent, SherpaError};
    use sona_core::transcription::transcript::{TranscriptSegment, TranscriptUpdate};
    use std::sync::{Arc, Mutex};

    struct RecordingSession {
        calls: Arc<Mutex<Vec<&'static str>>>,
        updates: tokio::sync::mpsc::UnboundedSender<AsrTranscriptUpdateEvent>,
        fail_start: bool,
        fail_feed: bool,
    }

    #[async_trait]
    impl AsrStreamingSession for RecordingSession {
        async fn start(&self) -> Result<(), SherpaError> {
            self.calls.lock().unwrap().push("start");
            if self.fail_start {
                return Err(SherpaError::Generic("model start failed".to_string()));
            }
            Ok(())
        }

        async fn stop(&self) -> Result<(), SherpaError> {
            self.calls.lock().unwrap().push("stop");
            Ok(())
        }

        async fn flush(&self) -> Result<(), SherpaError> {
            self.calls.lock().unwrap().push("flush");
            self.updates
                .send(update_event("final", true))
                .map_err(|error| SherpaError::Generic(error.to_string()))?;
            Ok(())
        }

        async fn feed_audio_chunk(&self, _samples: Vec<u8>) -> Result<(), SherpaError> {
            self.calls.lock().unwrap().push("feed-bytes");
            if self.fail_feed {
                return Err(SherpaError::Generic("decode failed".to_string()));
            }
            self.updates
                .send(update_event("partial", false))
                .map_err(|error| SherpaError::Generic(error.to_string()))?;
            Ok(())
        }

        async fn feed_audio_samples(&self, _samples: &[f32]) -> Result<(), SherpaError> {
            self.calls.lock().unwrap().push("feed-samples");
            Ok(())
        }
    }

    struct DelayedFinalSession {
        updates: Mutex<Option<tokio::sync::mpsc::UnboundedSender<AsrTranscriptUpdateEvent>>>,
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

    #[async_trait]
    impl AsrStreamingSession for DelayedFinalSession {
        async fn start(&self) -> Result<(), SherpaError> {
            Ok(())
        }

        async fn stop(&self) -> Result<(), SherpaError> {
            Ok(())
        }

        async fn flush(&self) -> Result<(), SherpaError> {
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

        async fn feed_audio_chunk(&self, _samples: Vec<u8>) -> Result<(), SherpaError> {
            Ok(())
        }

        async fn feed_audio_samples(&self, _samples: &[f32]) -> Result<(), SherpaError> {
            Ok(())
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
            .send(crate::live_audio::LiveAudioMessage::Chunk(
                crate::live_audio::LiveAudioChunk::PcmS16Le(vec![0, 0]),
            ))
            .await
            .unwrap();
        input_sender
            .send(crate::live_audio::LiveAudioMessage::Eof)
            .await
            .unwrap();
        drop(input_sender);
        let mut input =
            crate::live_audio::RunningAudioInput::from_parts(input_receiver, None, None, false);
        let (update_sender, mut update_receiver) = tokio::sync::mpsc::unbounded_channel();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let session: Arc<dyn AsrStreamingSession> = Arc::new(RecordingSession {
            calls: calls.clone(),
            updates: update_sender,
            fail_start: false,
            fail_feed: false,
        });
        let (_stop_sender, stop_receiver) = tokio::sync::oneshot::channel();
        let mut renderer = crate::live_output::LiveOutputRenderer::new(
            crate::live_output::LiveOutputFormat::Text,
            false,
            "session-1",
        );
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
            &["start", "feed-bytes", "flush", "stop"]
        );
        assert_eq!(renderer.segments().len(), 1);
        assert!(renderer.segments()[0].is_final);
        assert_eq!(String::from_utf8(output).unwrap(), "hello\n");
    }

    #[tokio::test]
    async fn live_runtime_waits_for_delayed_final_update_before_returning() {
        let (input_sender, input_receiver) = tokio::sync::mpsc::channel(1);
        input_sender
            .send(crate::live_audio::LiveAudioMessage::Eof)
            .await
            .unwrap();
        drop(input_sender);
        let mut input =
            crate::live_audio::RunningAudioInput::from_parts(input_receiver, None, None, false);
        let (update_sender, mut update_receiver) = tokio::sync::mpsc::unbounded_channel();
        let session: Arc<dyn AsrStreamingSession> = Arc::new(DelayedFinalSession {
            updates: Mutex::new(Some(update_sender)),
        });
        let (_stop_sender, stop_receiver) = tokio::sync::oneshot::channel();
        let mut renderer = crate::live_output::LiveOutputRenderer::new(
            crate::live_output::LiveOutputFormat::Text,
            false,
            "session-1",
        );
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
    async fn live_runtime_stops_session_and_emits_ndjson_error_on_feed_failure() {
        let (input_sender, input_receiver) = tokio::sync::mpsc::channel(2);
        input_sender
            .send(crate::live_audio::LiveAudioMessage::Chunk(
                crate::live_audio::LiveAudioChunk::PcmS16Le(vec![0, 0]),
            ))
            .await
            .unwrap();
        drop(input_sender);
        let mut input =
            crate::live_audio::RunningAudioInput::from_parts(input_receiver, None, None, false);
        let (update_sender, mut update_receiver) = tokio::sync::mpsc::unbounded_channel();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let session: Arc<dyn AsrStreamingSession> = Arc::new(RecordingSession {
            calls: calls.clone(),
            updates: update_sender,
            fail_start: false,
            fail_feed: true,
        });
        let (_stop_sender, stop_receiver) = tokio::sync::oneshot::channel();
        let mut renderer = crate::live_output::LiveOutputRenderer::new(
            crate::live_output::LiveOutputFormat::Ndjson,
            false,
            "session-1",
        );
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
            &["start", "feed-bytes", "stop"]
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
        let mut input = crate::live_audio::RunningAudioInput::from_parts(
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
        let mut renderer = crate::live_output::LiveOutputRenderer::new(
            crate::live_output::LiveOutputFormat::Ndjson,
            false,
            "session-1",
        );
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
        let mut input = crate::live_audio::RunningAudioInput::from_parts(
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
        let mut renderer = crate::live_output::LiveOutputRenderer::new(
            crate::live_output::LiveOutputFormat::Ndjson,
            false,
            "session-1",
        );
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
        let mut input = crate::live_audio::RunningAudioInput::from_parts(
            audio_receiver,
            Some(input_stop_sender),
            Some("Studio Mic".to_string()),
            true,
        );
        std::thread::spawn(move || {
            input_stop_receiver.recv().unwrap();
            audio_sender
                .blocking_send(crate::live_audio::LiveAudioMessage::Chunk(
                    crate::live_audio::LiveAudioChunk::Samples(vec![0.25; 16]),
                ))
                .unwrap();
            audio_sender
                .blocking_send(crate::live_audio::LiveAudioMessage::Eof)
                .unwrap();
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
        let mut renderer = crate::live_output::LiveOutputRenderer::new(
            crate::live_output::LiveOutputFormat::Text,
            false,
            "session-1",
        );
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
            &["start", "feed-samples", "flush", "stop"]
        );
    }

    #[test]
    fn final_transcript_is_exported_only_when_the_target_write_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("live.txt");
        let segments = update_event("final", true).update.upsert_segments;

        let status =
            write_final_transcript(&output, sona_core::export::ExportFormat::Txt, &segments)
                .unwrap();

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
}
