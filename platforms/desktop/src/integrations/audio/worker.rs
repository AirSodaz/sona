use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use log::error;
use sona_application::live_transcription::LiveSourceEpoch;
use sona_audio_capture::CaptureEvent;
use sona_core::ports::asr::AsrAudioFrame;
use sona_sherpa_onnx::audio::LiveWavRecorder;

use super::AudioState;
use super::recorder::{CaptureWriterState, RecorderCommand};
use super::types::{AUDIO_CAPTURE_ERROR_EVENT, AudioStopSignal, CaptureKey};

pub(crate) struct CaptureWorkerSource<'a> {
    pub emitter: &'a dyn crate::platform::event::EventEmitterPort,
    pub audio_state: &'a AudioState,
    pub asr_state: &'a crate::integrations::asr::AsrState,
    pub key: &'a CaptureKey,
    pub source: &'a LiveSourceEpoch,
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_capture_worker_task(
    asr_state: Arc<crate::integrations::asr::AsrState>,
    audio_state: Arc<AudioState>,
    emitter: Arc<dyn crate::platform::event::EventEmitterPort>,
    key: CaptureKey,
    source: LiveSourceEpoch,
    sample_cursor: Arc<AtomicU64>,
    stop_signal: AudioStopSignal,
    mut event_rx: tokio::sync::mpsc::Receiver<CaptureEvent>,
    mut recorder_rx: tokio::sync::mpsc::Receiver<RecorderCommand>,
) {
    tokio::spawn(async move {
        let mut writers = HashMap::<String, CaptureWriterState>::new();
        let mut sequence = 0_u64;
        let mut last_peak_emit = std::time::Instant::now();
        let worker_source = CaptureWorkerSource {
            emitter: emitter.as_ref(),
            audio_state: audio_state.as_ref(),
            asr_state: asr_state.as_ref(),
            key: &key,
            source: &source,
        };
        loop {
            tokio::select! {
                biased;
                cmd = recorder_rx.recv() => {
                    match cmd {
                        Some(RecorderCommand::Start { owner, path, completed }) => {
                            if let Some(previous) = writers.remove(&owner) {
                                let _ = previous.writer.finalize();
                            }
                            match LiveWavRecorder::create(std::path::Path::new(&path), 16000) {
                                Ok(w) => {
                                    writers.insert(owner, CaptureWriterState {
                                        writer: w,
                                        path,
                                        paused: false,
                                    });
                                    let _ = completed.send(Ok(()));
                                }
                                Err(error) => {
                                    let _ = completed.send(Err(format!(
                                        "Failed to create {} WAV writer: {}",
                                        key.kind.log_name(),
                                        error
                                    )));
                                }
                            }
                        }
                        Some(RecorderCommand::Stop { owner, completed }) => {
                            let path = if let Some(writer) = writers.remove(&owner) {
                                let path = writer.path;
                                let _ = writer.writer.finalize();
                                path
                            } else {
                                String::new()
                            };
                            let _ = completed.send(path);
                        }
                        Some(RecorderCommand::SetPaused { owner, paused }) => {
                            if let Some(writer) = writers.get_mut(&owner) {
                                writer.paused = paused;
                            }
                        }
                        None => break,
                    }
                }
                event = event_rx.recv() => {
                    match event {
                        Some(CaptureEvent::Chunk(chunk)) => {
                            drain_capture_worker_chunk(
                                &worker_source,
                                &chunk,
                                &mut writers,
                                &mut sequence,
                                sample_cursor.as_ref(),
                                &mut last_peak_emit,
                            )
                            .await;
                        }
                        Some(CaptureEvent::Error(err)) => {
                            error!(
                                "[Audio] {} capture stream error occurred: {err}",
                                key.kind.label()
                            );
                            let _ = emitter.emit(
                                AUDIO_CAPTURE_ERROR_EVENT,
                                serde_json::json!(format!("{}: {err}", key.kind.label())),
                            );
                            break;
                        }
                        Some(CaptureEvent::Eof) | None => break,
                    }
                }
            }
        }

        for writer in writers.into_values() {
            let _ = writer.writer.finalize();
        }
        stop_signal.stop();
        super::coordinator::evict_defunct_capture(&audio_state, &key, &source);
    });
}

pub(crate) async fn drain_capture_worker_chunk(
    worker_source: &CaptureWorkerSource<'_>,
    chunk: &[f32],
    writers: &mut HashMap<String, CaptureWriterState>,
    sequence: &mut u64,
    sample_cursor: &AtomicU64,
    last_peak_emit: &mut std::time::Instant,
) -> bool {
    if chunk.is_empty() {
        return false;
    }
    let mut max_abs = 0.0_f32;
    for &sample in chunk {
        let abs_val = sample.abs();
        if abs_val > max_abs {
            max_abs = abs_val;
        }
    }
    if last_peak_emit.elapsed() >= std::time::Duration::from_millis(50) {
        let peak_i16 = (max_abs.clamp(0.0, 1.0) * 32767.0) as i16;
        let _ = worker_source.emitter.emit(
            worker_source.key.kind.peak_event(),
            serde_json::json!(peak_i16),
        );
        *last_peak_emit = std::time::Instant::now();
    }

    for writer in writers.values_mut() {
        if !writer.paused
            && let Err(error) = writer.writer.write_samples(chunk)
        {
            log::error!("[Audio] Failed to write WAV samples: {error}");
        }
    }

    *sequence = sequence.saturating_add(1);
    let start_sample = sample_cursor.fetch_add(chunk.len() as u64, Ordering::AcqRel);
    let frame = AsrAudioFrame::new(*sequence, start_sample, chunk.to_vec());
    feed_capture_audio(
        worker_source.audio_state,
        worker_source.asr_state,
        worker_source.key,
        worker_source.source,
        frame,
    )
    .await;
    true
}

pub(crate) async fn feed_capture_audio(
    audio_state: &AudioState,
    asr_state: &crate::integrations::asr::AsrState,
    key: &CaptureKey,
    source: &LiveSourceEpoch,
    frame: AsrAudioFrame,
) {
    let instance_ids = {
        let registry = match audio_state.registry.lock() {
            Ok(registry) => registry,
            Err(_) => return,
        };
        let Some(capture) = registry.captures.get(key) else {
            return;
        };
        if &capture.source != source {
            return;
        }
        capture.active_instances()
    };

    if instance_ids.is_empty() {
        return;
    }

    if let Err(error) = asr_state
        .live_coordinator()
        .feed_source(source, frame.clone())
        .await
        && !error.message.contains("retired")
    {
        error!(
            "[Audio] Failed to feed live source {}: {error}",
            source.source_id
        );
    }
}
