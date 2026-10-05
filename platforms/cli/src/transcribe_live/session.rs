use std::io::Write;
use std::path::Path;
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
    stop_receiver: tokio::sync::oneshot::Receiver<LiveStopReason>,
    metadata: LiveSessionMetadata,
) -> CliResult<LiveStopReason> {
    run_live_session_with_save_audio(
        session,
        input,
        updates,
        renderer,
        output,
        stop_receiver,
        metadata,
        None,
    )
    .await
}
#[allow(clippy::too_many_arguments)]
pub async fn run_live_session_with_save_audio<W: Write + ?Sized + Send>(
    session: Arc<dyn AsrStreamingSession>,
    input: &mut RunningAudioInput,
    updates: &mut tokio::sync::mpsc::UnboundedReceiver<AsrTranscriptUpdateEvent>,
    renderer: &mut LiveOutputRenderer,
    output: &mut W,
    mut stop_receiver: tokio::sync::oneshot::Receiver<LiveStopReason>,
    metadata: LiveSessionMetadata,
    save_audio: Option<&Path>,
) -> CliResult<LiveStopReason> {
    let mut wav_writer = match save_audio {
        Some(path) => {
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: 16_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };
            match hound::WavWriter::create(path, spec) {
                Ok(writer) => Some(writer),
                Err(error) => {
                    input.request_stop();
                    return Err(CliError::Io(format!(
                        "Failed to create audio recording file {}: {error}",
                        path.display()
                    )));
                }
            }
        }
        None => None,
    };

    if let Err(error) = session.start().await {
        let error =
            finalize_wav_writer_on_error(wav_writer.take(), CliError::Model(error.to_string()));
        input.request_stop();
        let _ = session.stop().await;
        return Err(error);
    }
    if let Err(error) = renderer.write_started(
        output,
        &metadata.source,
        metadata.device_name.as_deref(),
        &metadata.model_id,
    ) {
        let error = finalize_wav_writer_on_error(wav_writer.take(), CliError::Io(error));
        input.request_stop();
        let _ = session.stop().await;
        return Err(error);
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
                        LiveAudioMessage::Chunk(chunk) => {
                            record_chunk(&mut wav_writer, &chunk)?;
                            feed_audio(session.as_ref(), &mut frame_cursor, chunk).await?;
                        }
                        LiveAudioMessage::Eof => break Ok(LiveStopReason::Eof),
                        LiveAudioMessage::Error(error) => break Err(CliError::Io(error)),
                    }
                }
                stop = &mut stop_receiver => {
                    let reason = stop.unwrap_or(LiveStopReason::CtrlC);
                    input.request_stop();
                    if input.should_drain_on_stop() {
                        drain_stopped_input(
                            session.as_ref(),
                            input,
                            &mut frame_cursor,
                            &mut wav_writer,
                        )
                        .await?;
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
            let error = finalize_wav_writer_on_error(wav_writer.take(), error);
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
        let error =
            finalize_wav_writer_on_error(wav_writer.take(), CliError::Model(error.to_string()));
        let _ = session.stop().await;
        renderer
            .write_error(output, &error.to_string())
            .map_err(CliError::Io)?;
        return Err(error);
    }
    if let Err(error) = session.stop().await {
        let error =
            finalize_wav_writer_on_error(wav_writer.take(), CliError::Model(error.to_string()));
        renderer
            .write_error(output, &error.to_string())
            .map_err(CliError::Io)?;
        return Err(error);
    }
    drop(session);
    if let Some(writer) = wav_writer.take() {
        writer.finalize().map_err(|error| {
            CliError::Io(format!(
                "Failed to finalize audio recording WAV file: {error}"
            ))
        })?;
    }
    while let Ok(Some(event)) = tokio::time::timeout(FINAL_DRAIN_TIMEOUT, updates.recv()).await {
        renderer
            .write_update(output, &event.stage, event.update)
            .map_err(CliError::Io)?;
    }
    Ok(reason)
}

fn record_chunk(
    writer: &mut Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>>,
    chunk: &LiveAudioChunk,
) -> CliResult<()> {
    if let Some(writer) = writer.as_mut() {
        match chunk {
            LiveAudioChunk::PcmS16Le(bytes) => {
                if bytes.len() % 2 != 0 {
                    return Err(CliError::Validation(format!(
                        "Invalid PCM chunk: byte length {} is not a multiple of 2",
                        bytes.len()
                    )));
                }
                let (chunks, _) = bytes.as_chunks::<2>();
                for sample_bytes in chunks {
                    let sample = i16::from_le_bytes(*sample_bytes);
                    writer.write_sample(sample).map_err(|error| {
                        CliError::Io(format!("Failed to write audio sample: {error}"))
                    })?;
                }
            }
            LiveAudioChunk::Samples(samples) => {
                for &sample in samples {
                    let sample_i16 = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
                    writer.write_sample(sample_i16).map_err(|error| {
                        CliError::Io(format!("Failed to write audio sample: {error}"))
                    })?;
                }
            }
        }
    }
    Ok(())
}

fn finalize_wav_writer_on_error(
    mut writer: Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>>,
    primary_err: CliError,
) -> CliError {
    if let Some(w) = writer.take() {
        let Err(finalize_err) = w.finalize() else {
            return primary_err;
        };
        log::warn!(
            "Failed to finalize audio recording WAV file during error shutdown: {finalize_err}"
        );
        return CliError::Io(format!(
            "{primary_err} (additional error: failed to finalize audio recording WAV file: {finalize_err})"
        ));
    }
    primary_err
}

pub async fn drain_stopped_input(
    session: &dyn AsrStreamingSession,
    input: &mut RunningAudioInput,
    frame_cursor: &mut sona_core::ports::asr::StreamingAudioFrameCursor,
    wav_writer: &mut Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>>,
) -> CliResult<()> {
    while let Some(message) = input.receiver.recv().await {
        match message {
            LiveAudioMessage::Chunk(chunk) => {
                record_chunk(wav_writer, &chunk)?;
                feed_audio(session, frame_cursor, chunk).await?;
            }
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
    write_final_transcript_with_mode(
        path,
        format,
        segments,
        sona_core::export::ExportMode::Original,
    )
}

pub fn write_final_transcript_with_mode(
    path: &std::path::Path,
    format: sona_core::export::ExportFormat,
    segments: &[TranscriptSegment],
    export_mode: sona_core::export::ExportMode,
) -> CliResult<String> {
    let exported = sona_core::export::export_segments_with_mode(segments, format, export_mode)
        .map_err(|error| CliError::Serialize(error.to_string()))?;
    sona_runtime_fs::write_transcript_output_file(path, &exported)
        .map_err(|error| CliError::Io(error.to_string()))?;
    Ok(format!("Wrote transcript to {}", path.display()))
}
