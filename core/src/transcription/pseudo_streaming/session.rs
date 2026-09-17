use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

use async_trait::async_trait;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::ports::asr::{
    AsrAudioFrame, AsrPortError, AsrRuntimeObserver, AsrStreamBoundaryEvent, AsrStreamingSession,
    AsrTranscriptUpdateEvent, TranscriptNormalizationOptions, TranscriptPostprocessOptions,
};
use crate::ports::punctuation::{PunctuationModel, apply_optional_punctuation};
use crate::ports::vad::StreamingVadPort;
use crate::transcription::asr_metrics::{AsrInferenceMetric, duration_to_ms};
use crate::transcription::postprocess::TranscriptPostprocessor;
use crate::transcription::transcript::{
    TranscriptSegment, build_transcript_update_with_id_generator, normalize_recognizer_text,
    synthesize_durations,
};

use super::buffer::PseudoStreamAudioBuffer;
use super::decoder::{DecodeStage, PseudoStreamDecoder};

const SAMPLE_RATE: f64 = 16_000.0;
const PRE_ROLL_DURATION_SECONDS: f64 = 0.3;
const PRE_ROLL_SAMPLES: usize = (SAMPLE_RATE * PRE_ROLL_DURATION_SECONDS) as usize;
const RING_BUFFER_TRIM_SLACK_SAMPLES: usize = 4_000;

type PendingInferenceTask = JoinHandle<Result<(), AsrPortError>>;

/// Configuration parameters to initialize a [`PseudoStreamingSession`].
pub struct PseudoStreamingSessionConfig {
    pub instance_id: String,
    pub decoder: Arc<dyn PseudoStreamDecoder>,
    pub vad: Box<dyn StreamingVadPort>,
    pub punctuation: Option<Arc<dyn PunctuationModel>>,
    pub observer: Arc<dyn AsrRuntimeObserver>,
    pub normalization_options: TranscriptNormalizationOptions,
    pub postprocess_options: TranscriptPostprocessOptions,
    pub initial_refresh_rate_ms: Option<u32>,
}

/// Generic pseudo-streaming session coordinating VAD, pre-roll ring buffers,
/// dynamic refresh rate backoff, and asynchronous inference decoding.
///
/// Implements [`AsrStreamingSession`]. Any ASR engine providing an implementation
/// of [`PseudoStreamDecoder`] can be plugged directly into this session.
pub struct PseudoStreamingSession {
    instance_id: String,
    decoder: Arc<dyn PseudoStreamDecoder>,
    vad: Mutex<Box<dyn StreamingVadPort>>,
    punctuation: Option<Arc<dyn PunctuationModel>>,
    observer: Arc<dyn AsrRuntimeObserver>,
    normalization_options: TranscriptNormalizationOptions,
    postprocessor: TranscriptPostprocessor,
    buffer: Mutex<PseudoStreamAudioBuffer>,
    current_segment_id: Mutex<Option<String>>,
    pending_inference: Mutex<Option<PendingInferenceTask>>,
    last_partial_decode_ms: Arc<AtomicU64>,
    last_frame_sequence: AtomicU64,
    last_frame_end_sample: AtomicU64,
    is_running: AtomicBool,
}

impl PseudoStreamingSession {
    pub fn new(config: PseudoStreamingSessionConfig) -> Result<Self, AsrPortError> {
        let initial_refresh = config.initial_refresh_rate_ms.unwrap_or(200) as u64;
        let postprocessor = TranscriptPostprocessor::compile(config.postprocess_options)
            .map_err(|error| AsrPortError::invalid_request(error.to_string()))?;

        Ok(Self {
            instance_id: config.instance_id,
            decoder: config.decoder,
            vad: Mutex::new(config.vad),
            punctuation: config.punctuation,
            observer: config.observer,
            normalization_options: config.normalization_options,
            postprocessor,
            buffer: Mutex::new(PseudoStreamAudioBuffer::with_initial_refresh_rate(
                initial_refresh,
            )),
            current_segment_id: Mutex::new(None),
            pending_inference: Mutex::new(None),
            last_partial_decode_ms: Arc::new(AtomicU64::new(0)),
            last_frame_sequence: AtomicU64::new(0),
            last_frame_end_sample: AtomicU64::new(0),
            is_running: AtomicBool::new(false),
        })
    }

    fn current_boundary(&self) -> AsrStreamBoundaryEvent {
        AsrStreamBoundaryEvent {
            instance_id: self.instance_id.clone(),
            sequence: self.last_frame_sequence.load(Ordering::Acquire),
            end_sample: self.last_frame_end_sample.load(Ordering::Acquire),
        }
    }
}

async fn wait_for_inference_task(
    pending: &mut Option<PendingInferenceTask>,
) -> Result<(), AsrPortError> {
    if let Some(task) = pending.take() {
        task.await
            .map_err(|error| AsrPortError::runtime(format!("Inference task failed: {error}")))?
    } else {
        Ok(())
    }
}

async fn prepare_partial_inference_slot(
    pending: &mut Option<PendingInferenceTask>,
) -> Result<bool, AsrPortError> {
    if pending.as_ref().is_some_and(|task| task.is_finished()) {
        wait_for_inference_task(pending).await?;
    }
    Ok(pending.is_none())
}

fn queue_inference_task(
    previous: Option<PendingInferenceTask>,
    task: impl FnOnce() -> Result<(), AsrPortError> + Send + 'static,
) -> PendingInferenceTask {
    tokio::spawn(async move {
        if let Some(previous) = previous {
            previous.await.map_err(|error| {
                AsrPortError::runtime(format!("Inference task failed: {error}"))
            })??;
        }
        tokio::task::spawn_blocking(task)
            .await
            .map_err(|error| AsrPortError::runtime(format!("Inference task failed: {error}")))?
    })
}

#[allow(clippy::too_many_arguments)]
fn execute_inference_pass(
    decoder: &Arc<dyn PseudoStreamDecoder>,
    audio_samples: &[f32],
    stage: DecodeStage,
    punctuation: Option<&Arc<dyn PunctuationModel>>,
    segment_id: &str,
    global_start: f64,
    instance_id: &str,
    normalization_options: TranscriptNormalizationOptions,
    postprocessor: &TranscriptPostprocessor,
    observer: &Arc<dyn AsrRuntimeObserver>,
    triggered_at: Instant,
    last_partial_decode_target: Option<&Arc<AtomicU64>>,
) -> Result<(), AsrPortError> {
    if audio_samples.is_empty() {
        return Ok(());
    }

    let decode_started = Instant::now();
    let decode_result = decoder.decode(audio_samples, stage)?;
    let decode_ms = duration_to_ms(decode_started.elapsed());

    if stage == DecodeStage::Partial
        && let Some(target) = last_partial_decode_target
    {
        target.store(decode_ms.round() as u64, Ordering::Release);
    }

    let Some(result) = decode_result else {
        return Ok(());
    };

    let raw_text = result.text.trim();
    if raw_text.is_empty() {
        return Ok(());
    }

    let cleaned_text = normalize_recognizer_text(raw_text);
    if cleaned_text.is_empty() {
        return Ok(());
    }

    let is_final = stage.is_final();
    let text = if is_final {
        apply_optional_punctuation(
            punctuation.map(|p| &**p as &dyn PunctuationModel),
            &cleaned_text,
        )
    } else {
        cleaned_text
    };

    if text.is_empty() {
        return Ok(());
    }

    let global_end = global_start + (audio_samples.len() as f64 / SAMPLE_RATE);
    let timestamps_abs: Option<Vec<f32>> = result
        .timestamps
        .as_ref()
        .map(|ts| ts.iter().map(|t| *t + global_start as f32).collect());
    let durations = timestamps_abs
        .as_ref()
        .and_then(|ts| synthesize_durations(ts, global_end as f32));

    let segment = TranscriptSegment {
        id: segment_id.to_string(),
        text,
        start: global_start,
        end: global_end,
        is_final,
        timing: None,
        tokens: result.tokens,
        timestamps: timestamps_abs,
        durations,
        translation: None,
        speaker: None,
        speaker_attribution: None,
    };

    let raw_update =
        build_transcript_update_with_id_generator(segment, normalization_options, || {
            uuid::Uuid::new_v4().to_string()
        });
    let update = postprocessor.process_update(raw_update);

    observer.on_transcript_update(&AsrTranscriptUpdateEvent {
        instance_id: instance_id.to_string(),
        stage: stage.as_str().to_string(),
        update,
    });

    let total_elapsed = duration_to_ms(triggered_at.elapsed());
    let audio_duration_ms = (audio_samples.len() as f64 / SAMPLE_RATE) * 1000.0;
    let rtf = if audio_duration_ms > 0.0 {
        Some(decode_ms / audio_duration_ms)
    } else {
        None
    };

    observer.on_live_inference(&AsrInferenceMetric {
        occurred_at_ms: chrono::Utc::now().timestamp_millis() as u64,
        source: "pseudo_streaming".to_string(),
        instance_id: Some(instance_id.to_string()),
        stage: stage.as_str().to_string(),
        is_final,
        audio_duration_ms,
        buffered_samples: audio_samples.len(),
        audio_extract_ms: None,
        decode_ms,
        emit_latency_ms: Some(total_elapsed),
        total_ms: Some(total_elapsed),
        rtf,
        segment_count: Some(1),
        process_rss_mb: None,
    });

    Ok(())
}

#[async_trait]
impl AsrStreamingSession for PseudoStreamingSession {
    async fn start(&self) -> Result<(), AsrPortError> {
        self.is_running.store(true, Ordering::Release);
        self.vad.lock().await.reset();
        self.buffer.lock().await.reset();
        *self.current_segment_id.lock().await = None;
        Ok(())
    }

    async fn stop(&self) -> Result<(), AsrPortError> {
        self.is_running.store(false, Ordering::Release);
        let mut pending = self.pending_inference.lock().await;
        wait_for_inference_task(&mut pending).await?;
        Ok(())
    }

    async fn flush(&self) -> Result<(), AsrPortError> {
        let mut pending = self.pending_inference.lock().await;
        wait_for_inference_task(&mut pending).await?;

        let mut buffer = self.buffer.lock().await;
        if buffer.buffered_speech_chunk_count() > 0 {
            let audio_samples = buffer.flatten_speech_buffer();
            let global_start = buffer.utterance_start_seconds(SAMPLE_RATE);
            let seg_id = self
                .current_segment_id
                .lock()
                .await
                .clone()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

            let decoder = self.decoder.clone();
            let punctuation = self.punctuation.clone();
            let observer = self.observer.clone();
            let postprocessor = self.postprocessor.clone();
            let normalization_options = self.normalization_options;
            let instance_id = self.instance_id.clone();
            let triggered_at = Instant::now();

            tokio::task::spawn_blocking(move || {
                execute_inference_pass(
                    &decoder,
                    &audio_samples,
                    DecodeStage::Final,
                    punctuation.as_ref(),
                    &seg_id,
                    global_start,
                    &instance_id,
                    normalization_options,
                    &postprocessor,
                    &observer,
                    triggered_at,
                    None,
                )
            })
            .await
            .map_err(|e| AsrPortError::runtime(format!("Flush task failed: {e}")))??;

            buffer.clear_speech_buffer();
        }

        *self.current_segment_id.lock().await = None;
        buffer.reset();
        self.last_partial_decode_ms.store(0, Ordering::Release);

        self.observer.on_stream_boundary(&self.current_boundary());
        Ok(())
    }

    async fn feed_audio_frame(&self, frame: AsrAudioFrame) -> Result<(), AsrPortError> {
        self.last_frame_sequence
            .store(frame.sequence, Ordering::Release);
        self.last_frame_end_sample
            .store(frame.end_sample(), Ordering::Release);

        if !self.is_running.load(Ordering::Acquire) {
            return Ok(());
        }

        let samples = &frame.samples;
        let mut vad = self.vad.lock().await;
        vad.accept_samples(samples);
        let currently_speaking = vad.is_speech_detected();
        drop(vad);

        let mut current_id_guard = self.current_segment_id.lock().await;
        if current_id_guard.is_none() {
            *current_id_guard = Some(uuid::Uuid::new_v4().to_string());
        }
        let seg_id = current_id_guard.as_ref().unwrap().clone();
        drop(current_id_guard);

        let mut buffer = self.buffer.lock().await;
        let mut pending = self.pending_inference.lock().await;

        if currently_speaking && !buffer.is_speech_active() {
            // Speech onset: capture pre-roll context
            buffer.begin_speech(PRE_ROLL_SAMPLES);
        }

        if currently_speaking {
            buffer.push_speech_chunk(samples.to_vec());

            let prev_decode_ms = self.last_partial_decode_ms.swap(0, Ordering::AcqRel);
            if prev_decode_ms > 0 {
                buffer.record_decode_duration(prev_decode_ms);
            }

            let now = Instant::now();
            if buffer.should_run_partial(now) {
                let slot_available = prepare_partial_inference_slot(&mut pending).await?;
                if !slot_available {
                    buffer.record_overrun();
                } else {
                    let audio_samples = buffer.flatten_speech_buffer();
                    let global_start = buffer.utterance_start_seconds(SAMPLE_RATE);
                    let decoder = self.decoder.clone();
                    let punctuation = self.punctuation.clone();
                    let observer = self.observer.clone();
                    let postprocessor = self.postprocessor.clone();
                    let normalization_options = self.normalization_options;
                    let instance_id = self.instance_id.clone();
                    let triggered_at = Instant::now();
                    let seg_id_copy = seg_id.clone();
                    let decode_target = self.last_partial_decode_ms.clone();

                    let task = move || {
                        execute_inference_pass(
                            &decoder,
                            &audio_samples,
                            DecodeStage::Partial,
                            punctuation.as_ref(),
                            &seg_id_copy,
                            global_start,
                            &instance_id,
                            normalization_options,
                            &postprocessor,
                            &observer,
                            triggered_at,
                            Some(&decode_target),
                        )
                    };

                    *pending = Some(queue_inference_task(None, task));
                    buffer.mark_inference_time(now);
                }
            }
        } else {
            let prev_decode_ms = self.last_partial_decode_ms.swap(0, Ordering::AcqRel);
            if prev_decode_ms > 0 {
                buffer.record_decode_duration(prev_decode_ms);
            }

            if buffer.is_speech_active() {
                // Speech offset: finalize utterance
                buffer.finish_speech_with_chunk(samples.to_vec());
                let audio_samples = buffer.flatten_speech_buffer();
                let global_start = buffer.utterance_start_seconds(SAMPLE_RATE);
                let decoder = self.decoder.clone();
                let punctuation = self.punctuation.clone();
                let observer = self.observer.clone();
                let postprocessor = self.postprocessor.clone();
                let normalization_options = self.normalization_options;
                let instance_id = self.instance_id.clone();
                let triggered_at = Instant::now();
                let seg_id_copy = seg_id;
                let boundary = self.current_boundary();

                let task = move || {
                    execute_inference_pass(
                        &decoder,
                        &audio_samples,
                        DecodeStage::Final,
                        punctuation.as_ref(),
                        &seg_id_copy,
                        global_start,
                        &instance_id,
                        normalization_options,
                        &postprocessor,
                        &observer,
                        triggered_at,
                        None,
                    )?;
                    observer.on_stream_boundary(&boundary);
                    Ok(())
                };

                *pending = Some(queue_inference_task(pending.take(), task));

                buffer.on_utterance_end(true);
                buffer.clear_speech_buffer();
                *self.current_segment_id.lock().await = Some(uuid::Uuid::new_v4().to_string());
            }

            buffer.push_ring_chunk_with_sample_limit(
                samples.to_vec(),
                PRE_ROLL_SAMPLES,
                RING_BUFFER_TRIM_SLACK_SAMPLES,
            );
        }

        buffer.advance_total_samples(samples.len());
        Ok(())
    }
}
