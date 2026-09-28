use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

use sona_core::ports::asr::{AsrRuntimeObserver, AsrStreamingSession, AsrTranscriptUpdateEvent};
use sona_core::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
use sona_core::transcription::transcript::TranscriptSegment;

use crate::live_audio::{LiveAudioChunk, LiveAudioMessage, RunningAudioInput};
use crate::live_output::{LiveOutputRenderer, LiveStopReason};
use crate::{CliError, CliResult};

const FINAL_DRAIN_TIMEOUT: Duration = Duration::from_millis(250);

pub struct CliStreamingObserver {
    pub sender: tokio::sync::mpsc::UnboundedSender<AsrTranscriptUpdateEvent>,
}

impl AsrRuntimeObserver for CliStreamingObserver {
    fn on_transcript_update(&self, event: &AsrTranscriptUpdateEvent) {
        let _ = self.sender.send(event.clone());
    }

    fn on_model_load(&self, _metric: &AsrModelLoadMetric) {}

    fn on_live_inference(&self, _metric: &AsrInferenceMetric) {}
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveSessionMetadata {
    pub source: String,
    pub device_name: Option<String>,
    pub model_id: String,
}

pub fn spawn_stop_signal(
    duration: Option<Duration>,
) -> tokio::sync::oneshot::Receiver<LiveStopReason> {
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

pub async fn run_live_session<W: Write + ?Sized + Send>(
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

    let mut frame_cursor = sona_core::ports::asr::StreamingAudioFrameCursor::default();
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
                        LiveAudioMessage::Chunk(chunk) => feed_audio(session.as_ref(), &mut frame_cursor, chunk).await?,
                        LiveAudioMessage::Eof => break Ok(LiveStopReason::Eof),
                        LiveAudioMessage::Error(error) => break Err(CliError::Io(error)),
                    }
                }
                stop = &mut stop_receiver => {
                    let reason = stop.unwrap_or(LiveStopReason::CtrlC);
                    input.request_stop();
                    if input.should_drain_on_stop() {
                        drain_stopped_input(session.as_ref(), input, &mut frame_cursor).await?;
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
    while let Ok(Some(event)) = tokio::time::timeout(FINAL_DRAIN_TIMEOUT, updates.recv()).await {
        renderer
            .write_update(output, &event.stage, event.update)
            .map_err(CliError::Io)?;
    }
    Ok(reason)
}

pub async fn drain_stopped_input(
    session: &dyn AsrStreamingSession,
    input: &mut RunningAudioInput,
    frame_cursor: &mut sona_core::ports::asr::StreamingAudioFrameCursor,
) -> CliResult<()> {
    while let Some(message) = input.receiver.recv().await {
        match message {
            LiveAudioMessage::Chunk(chunk) => feed_audio(session, frame_cursor, chunk).await?,
            LiveAudioMessage::Eof => return Ok(()),
            LiveAudioMessage::Error(error) => return Err(CliError::Io(error)),
        }
    }
    Ok(())
}

pub async fn feed_audio(
    session: &dyn AsrStreamingSession,
    frame_cursor: &mut sona_core::ports::asr::StreamingAudioFrameCursor,
    chunk: LiveAudioChunk,
) -> CliResult<()> {
    let frame = match chunk {
        LiveAudioChunk::PcmS16Le(bytes) => frame_cursor.next_pcm_s16le(&bytes),
        LiveAudioChunk::Samples(samples) => Ok(frame_cursor.next_samples(samples)),
    }
    .map_err(|error| CliError::Model(error.to_string()))?;
    session
        .feed_audio_frame(frame)
        .await
        .map_err(|error| CliError::Model(error.to_string()))
}

pub fn write_final_transcript(
    path: &std::path::Path,
    format: sona_core::export::ExportFormat,
    segments: &[TranscriptSegment],
) -> CliResult<String> {
    let exported = sona_core::export::export_segments_with_mode(
        segments,
        format,
        sona_core::export::ExportMode::Original,
    )
    .map_err(|error| CliError::Serialize(error.to_string()))?;
    sona_runtime_fs::write_transcript_output_file(path, &exported)
        .map_err(|error| CliError::Io(error.to_string()))?;
    Ok(format!("Wrote transcript to {}", path.display()))
}
