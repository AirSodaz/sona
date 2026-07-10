use super::inference::{
    build_transcript_update, diagnostics_instance_label, format_transcript,
    observe_streaming_transcript_update, run_offline_inference, synthesize_durations,
};
use super::telemetry::{
    build_live_metric, capture_process_memory_mb, current_time_millis, log_model_load_metric,
    record_live_metric,
};
use crate::audio::{accept_vad_samples, vad_detected};
use crate::gpu::resolve_gpu_acceleration_plan;
use crate::punctuation::{Punctuation, load_punctuation};
use crate::recognizer::{
    Recognizer, accept_online_samples, build_model_config, create_online_stream,
    create_recognizer_with_gpu_plan, decode_online_ready, is_online_endpoint, online_stream_result,
    reset_online_stream,
};
use crate::runtime::{
    ModelConfigKey, OfflineState, RecognizerPool, SherpaInstance, buffered_sample_count,
    start_instance_runtime, stop_instance_runtime,
};
use async_trait::async_trait;
use log::{debug, info, trace};
use sona_core::ports::asr::{
    AsrRuntimeObserver, AsrStreamingSession, LocalSherpaStreamingRequest, SherpaError,
};
use sona_core::transcription::asr_metrics::{
    AsrModelLoadMetric, calculate_rss_delta_mb, duration_to_ms,
};
use sona_core::transcription::postprocess::TranscriptPostprocessor;
use sona_core::transcription::transcript::TranscriptSegment;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;

const PARTIAL_METRIC_INTERVAL_SAMPLES: usize = 16_000;

pub struct LocalSherpaSession {
    instance_id: String,
    observer: Arc<dyn AsrRuntimeObserver>,
    instance: tokio::sync::Mutex<SherpaInstance>,
}

#[async_trait]
impl AsrStreamingSession for LocalSherpaSession {
    async fn start(&self) -> Result<(), SherpaError> {
        let mut instance = self.instance.lock().await;
        start_recognizer_impl_inner(&self.instance_id, &mut instance)
            .await
            .map_err(SherpaError::Generic)
    }

    async fn stop(&self) -> Result<(), SherpaError> {
        let mut instance = self.instance.lock().await;
        stop_recognizer_impl_inner(&self.instance_id, &mut instance)
            .await
            .map_err(SherpaError::Generic)
    }

    async fn flush(&self) -> Result<(), SherpaError> {
        let mut instance = self.instance.lock().await;
        flush_recognizer_impl_inner(self.observer.clone(), &self.instance_id, &mut instance)
            .await
            .map_err(SherpaError::Generic)
    }

    async fn feed_audio_chunk(&self, samples: Vec<u8>) -> Result<(), SherpaError> {
        let mut instance = self.instance.lock().await;
        feed_audio_chunk_impl_inner(
            self.observer.clone(),
            &self.instance_id,
            &mut instance,
            samples,
        )
        .await
        .map_err(SherpaError::Generic)
    }

    async fn feed_audio_samples(&self, samples: &[f32]) -> Result<(), SherpaError> {
        let mut instance = self.instance.lock().await;
        feed_audio_samples_inner(
            self.observer.clone(),
            &self.instance_id,
            &mut instance,
            samples,
        )
        .await
        .map_err(SherpaError::Generic)
    }
}

pub async fn resolve_punctuation(
    pool: &RecognizerPool,
    punctuation_model: Option<String>,
) -> Option<Arc<Punctuation>> {
    let p_path = punctuation_model?;
    if p_path.is_empty() || !Path::new(&p_path).exists() {
        return None;
    }
    let cell = pool.punctuation_cell_for_path(p_path.clone()).await;
    cell.get_or_try_init(|| async {
        load_punctuation(Some(p_path.clone()))
            .map(Arc::new)
            .ok_or_else(|| "Failed to load punctuation model".to_string())
    })
    .await
    .ok()
    .cloned()
}

pub async fn create_streaming_session(
    recognizer_pool: RecognizerPool,
    request: LocalSherpaStreamingRequest,
    observer: Arc<dyn AsrRuntimeObserver>,
) -> Result<Arc<LocalSherpaSession>, String> {
    let LocalSherpaStreamingRequest {
        instance_id,
        model_path,
        num_threads,
        enable_itn,
        language,
        punctuation_model,
        vad_model,
        vad_buffer,
        model_type,
        file_config,
        hotwords,
        normalization_options,
        postprocess_options,
        gpu_acceleration,
    } = request;

    let gpu_plan = resolve_gpu_acceleration_plan(gpu_acceleration.as_deref()).await;

    info!(
        "[init_recognizer] start instance_id={instance_id} model_path={model_path} model_type={model_type} num_threads={num_threads} enable_itn={enable_itn} language={language} punctuation_model={:?} vad_model={:?} vad_buffer={vad_buffer} hotwords={:?} gpu_acceleration={:?} gpu_plan={:?}",
        punctuation_model, vad_model, hotwords, gpu_acceleration, gpu_plan
    );

    let config_key = ModelConfigKey::new(
        model_path.clone(),
        model_type.clone(),
        num_threads,
        enable_itn,
        language.clone(),
        hotwords.clone(),
        None,
    );

    let load_started = Instant::now();
    let rss_before_mb = capture_process_memory_mb();
    let mut reused_from_pool = false;

    let primary_provider = gpu_plan.provider_options().first().cloned().flatten();
    let (cell, is_new) = recognizer_pool
        .recognizer_cell_for_gpu_plan(
            &config_key,
            gpu_plan.provider_options(),
            primary_provider.clone(),
        )
        .await;

    // A cell may exist in the pool but still be uninitialized if a prior
    // `get_or_try_init` failed. Only count it as reused when it actually holds
    // a recognizer, so the load metric reflects a genuine reuse rather than a
    // retry that has to build the model this time around.
    if !is_new && cell.initialized() {
        reused_from_pool = true;
    }

    let recognizer = cell
        .get_or_try_init(|| async {
            info!("[init_recognizer] Creating new recognizer and adding to pool");
            let config_type = build_model_config(
                Path::new(&model_path),
                &model_type,
                &file_config,
                enable_itn,
                &language,
                hotwords.clone(),
            )?;
            let create_result =
                create_recognizer_with_gpu_plan(config_type, num_threads, gpu_plan.clone())?;
            if let Some(notice) = create_result.fallback_notice.as_ref() {
                log::warn!(
                    "[init_recognizer] {} recognizer creation failed, retrying with {}: {}",
                    notice.from_provider,
                    notice.to_provider,
                    notice.error
                );
            }
            let actual_provider = create_result.provider.clone();
            let r = Arc::new(create_result.recognizer);

            if actual_provider != primary_provider {
                recognizer_pool
                    .register_recognizer_gpu_provider(&config_key, actual_provider, cell.clone())
                    .await;
            }

            Ok::<Arc<Recognizer>, String>(r)
        })
        .await?
        .clone();
    let load_ms = duration_to_ms(load_started.elapsed());
    let rss_after_mb = capture_process_memory_mb();
    let model_load_metric = AsrModelLoadMetric {
        occurred_at_ms: current_time_millis(),
        instance_id: instance_id.clone(),
        model_path: model_path.clone(),
        model_type: model_type.clone(),
        recognizer_kind: recognizer.kind_label().to_string(),
        num_threads,
        reused_from_pool,
        load_ms,
        rss_before_mb,
        rss_after_mb,
        rss_delta_mb: calculate_rss_delta_mb(rss_before_mb, rss_after_mb, reused_from_pool),
        process_rss_mb: rss_after_mb,
    };
    observer.on_model_load(&model_load_metric);
    log_model_load_metric(&model_load_metric);

    let punctuation = resolve_punctuation(&recognizer_pool, punctuation_model).await;
    let mut session_instance = SherpaInstance::default();
    session_instance.set_recognizer(recognizer);
    session_instance.set_punctuation(punctuation);
    session_instance.configure_vad(vad_model.clone(), vad_buffer);
    session_instance.normalization_options = normalization_options;
    session_instance.postprocessor = TranscriptPostprocessor::compile(postprocess_options)?;

    let session = std::sync::Arc::new(LocalSherpaSession {
        instance_id,
        observer,
        instance: tokio::sync::Mutex::new(session_instance),
    });

    Ok(session)
}

async fn start_recognizer_impl_inner(
    instance_id: &str,
    instance: &mut SherpaInstance,
) -> Result<(), String> {
    let Some(recognizer) = instance.recognizer_clone() else {
        return Err("Recognizer not initialized".to_string());
    };
    let recognizer_kind = recognizer.kind_label();

    let stream = recognizer.online().map(create_online_stream);

    // Starting a run resets transient buffers and, for online models, creates a
    // fresh Sherpa stream that will accumulate new incremental state.
    start_instance_runtime(instance, stream);

    instance.reset_or_reload_vad();

    if let Some(label) = diagnostics_instance_label(instance_id) {
        info!(
            "[Sherpa] start_recognizer({label}): is_running=true recognizer_kind={} vad_configured={} punctuation_loaded={}",
            recognizer_kind,
            instance.has_vad_configuration(),
            instance.has_punctuation()
        );
    }

    Ok(())
}

async fn stop_recognizer_impl_inner(
    instance_id: &str,
    instance: &mut SherpaInstance,
) -> Result<(), String> {
    {
        if let Some(label) = diagnostics_instance_label(instance_id) {
            info!(
                "[Sherpa] stop_recognizer({label}): was_running={} total_samples={} buffered_chunks={} buffered_samples={} current_segment={} emitted_any={}",
                instance.is_running(),
                instance.total_samples,
                instance.offline_state.buffered_speech_chunk_count(),
                instance.offline_state.buffered_speech_sample_count(),
                instance.current_segment_id.as_deref().unwrap_or("none"),
                instance
                    .record_diagnostics
                    .first_segment_emitted_flag()
                    .load(Ordering::SeqCst)
            );
        }
        stop_instance_runtime(instance);
    }
    Ok(())
}

async fn flush_recognizer_impl_inner(
    observer: Arc<dyn AsrRuntimeObserver>,
    instance_id: &str,
    instance: &mut SherpaInstance,
) -> Result<(), String> {
    info!("Flushing recognizer for instance id: {}", instance_id);

    if let Some(label) = diagnostics_instance_label(instance_id) {
        info!(
            "[Sherpa] flush_recognizer({label}): is_running={} total_samples={} buffered_chunks={} buffered_samples={} current_segment={} speaking={}",
            instance.is_running(),
            instance.total_samples,
            instance.offline_state.buffered_speech_chunk_count(),
            instance.offline_state.buffered_speech_sample_count(),
            instance.current_segment_id.as_deref().unwrap_or("none"),
            instance.offline_state.is_speech_active()
        );
    }

    if let Some(recognizer) = instance.recognizer_clone()
        && recognizer.is_offline()
    {
        if !instance.offline_state.speech_chunks().is_empty() {
            let seg_id = instance
                .current_segment_id
                .as_ref()
                .cloned()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let global_start = instance.offline_state.utterance_start_seconds(16000.0);

            let offline_copy = instance.offline_state.speech_chunks().to_vec();
            let observer_copy = observer.clone();
            let recognizer_copy = recognizer.clone();
            let punct_copy = instance.punctuation_clone();
            let seg_id_copy = seg_id.clone();
            let instance_id_copy = instance_id.to_string();
            let first_segment_emitted =
                diagnostics_instance_label(instance_id).is_some().then(|| {
                    instance
                        .record_diagnostics
                        .first_segment_emitted_flag()
                        .clone()
                });
            let normalization_options = instance.normalization_options;
            let postprocessor = instance.postprocessor.clone();
            let triggered_at = Instant::now();

            if let Some(label) = diagnostics_instance_label(instance_id) {
                info!(
                    "[Sherpa] {label} flush triggering offline inference. segment_id={} buffered_chunks={} buffered_samples={}",
                    seg_id,
                    offline_copy.len(),
                    buffered_sample_count(&offline_copy)
                );
            }

            // Offline decoding can be CPU-heavy, so the final utterance pass
            // runs on a blocking worker and then emits one final segment.
            let task = move || {
                if let Some(safe_r) = recognizer_copy.offline() {
                    run_offline_inference(
                        &offline_copy,
                        observer_copy.as_ref(),
                        safe_r,
                        punct_copy.as_deref(),
                        &seg_id_copy,
                        global_start,
                        true,
                        &instance_id_copy,
                        "flush_offline",
                        first_segment_emitted,
                        normalization_options,
                        postprocessor,
                        true,
                        triggered_at,
                    );
                }
            };
            tokio::task::spawn_blocking(task)
                .await
                .map_err(|error| error.to_string())?;

            instance.offline_state.clear_speech_buffer();
        } else if let Some(label) = diagnostics_instance_label(instance_id) {
            info!("[Sherpa] {label} flush found no pending offline speech buffer.");
        }
        instance.current_segment_id = None;
        instance.offline_state = OfflineState::default();
        if let Some(label) = diagnostics_instance_label(instance_id) {
            info!("[Sherpa] flush_recognizer({label}) complete. mode=offline");
        }
        return Ok(());
    }

    if let Some(recognizer) = instance.recognizer_clone()
        && let Some(r) = recognizer.online()
        && let Some(stream) = instance.take_stream()
    {
        let st = &stream;
        let inference_started = Instant::now();
        let current_time = instance.total_samples as f64 / 16000.0;
        let buffered_samples =
            ((current_time - instance.segment_start_time).max(0.0) * 16000.0) as usize;

        // Online models need a short tail of silence to finalize the last
        // partial hypothesis before we reset the stream.
        let decode_started = Instant::now();
        let tail_padding = vec![0.0; (16000.0 * 0.8) as usize];
        debug!("FFI: Calling accept_waveform (Online, tail_padding)");
        accept_online_samples(st, &tail_padding);
        debug!("FFI: Successfully returned from accept_waveform (Online, tail_padding)");
        decode_online_ready(r, st);
        let decode_ms = duration_to_ms(decode_started.elapsed());

        if let Some(result) = online_stream_result(r, st)
            && !result.text.trim().is_empty()
        {
            let text = format_transcript(&result.text, instance.punctuation());
            let timestamps_abs = result.timestamps.as_ref().map(|ts| {
                ts.iter()
                    .map(|t| *t + instance.segment_start_time as f32)
                    .collect::<Vec<_>>()
            });
            let durations = timestamps_abs
                .as_ref()
                .and_then(|ts| synthesize_durations(ts, current_time as f32));

            let id = instance
                .current_segment_id
                .take()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

            let segment = TranscriptSegment {
                id,
                text,
                start: instance.segment_start_time,
                end: current_time,
                is_final: true,
                timing: None,
                tokens: Some(result.tokens),
                timestamps: timestamps_abs,
                durations,
                translation: None,
                speaker: None,
                speaker_attribution: None,
            };
            let update = instance
                .postprocessor
                .process_update(build_transcript_update(
                    segment,
                    instance.normalization_options,
                ));
            observe_streaming_transcript_update(
                observer.as_ref(),
                instance_id,
                &update,
                "flush_online",
                Some(instance.record_diagnostics.first_segment_emitted_flag()),
            );
        }

        record_live_metric(
            observer.as_ref(),
            build_live_metric(
                instance_id,
                "flush_online",
                true,
                buffered_samples,
                decode_ms,
                Some(duration_to_ms(inference_started.elapsed())),
                None,
            ),
        );

        instance.current_segment_id = None;
        reset_online_stream(r, st);
        instance.restore_stream(stream);
        instance.segment_start_time = current_time;
        if let Some(label) = diagnostics_instance_label(instance_id) {
            info!("[Sherpa] flush_recognizer({label}) complete. mode=online");
        }
    }

    Ok(())
}

async fn feed_audio_samples_inner(
    observer: Arc<dyn AsrRuntimeObserver>,
    instance_id: &str,
    instance: &mut SherpaInstance,
    samples: &[f32],
) -> Result<(), String> {
    // instances removed
    // instances lookup removed

    if !instance.is_running() {
        if let Some(label) = diagnostics_instance_label(instance_id)
            && instance
                .record_diagnostics
                .should_log_skipped_while_stopped()
        {
            println!(
                "[Sherpa] {label} audio chunk skipped because recognizer is not running. samples={} total_samples={}",
                samples.len(),
                instance.total_samples
            );
            instance
                .record_diagnostics
                .mark_skipped_while_stopped_logged();
        }
        return Ok(());
    }

    if let Some(label) = diagnostics_instance_label(instance_id)
        && instance.record_diagnostics.should_log_first_sample()
    {
        println!(
            "[Sherpa] {label} first sample received. samples={} total_samples_before={} current_segment={}",
            samples.len(),
            instance.total_samples,
            instance.current_segment_id.as_deref().unwrap_or("none")
        );
        instance.record_diagnostics.mark_first_sample_logged();
    }

    let recognizer = instance
        .recognizer_clone()
        .ok_or("Recognizer not initialized")?;

    if recognizer.is_offline() {
        let Some(vad) = instance.vad() else {
            println!(
                "[Sherpa] feed_audio_samples: VAD model is missing for instance {}",
                instance_id
            );
            return Err("VAD model is missing or not configured. This model requires VAD for live transcription. Please download the Silero VAD model in Settings -> Model Center.".to_string());
        };

        // Offline live transcription is VAD-driven: we keep feeding audio to
        // the detector, grow/trim utterance buffers, and only run full
        // recognizer inference when a speech segment boundary is reached.
        accept_vad_samples(vad, samples);
        let currently_speaking = vad_detected(vad);

        if let Some(label) = diagnostics_instance_label(instance_id)
            && instance.total_samples % 160000 < 2000
        {
            // Print once every ~10 seconds
            println!(
                "[Sherpa] instance '{label}' running, total_samples: {}, currently_speaking: {}, emitted_any: {}",
                instance.total_samples,
                currently_speaking,
                instance
                    .record_diagnostics
                    .first_segment_emitted_flag()
                    .load(Ordering::SeqCst)
            );
        }

        if instance.current_segment_id.is_none() {
            instance.current_segment_id = Some(uuid::Uuid::new_v4().to_string());
        }
        let seg_id = instance.current_segment_id.as_ref().unwrap().clone();

        if currently_speaking && !instance.offline_state.is_speech_active() {
            if let Some(label) = diagnostics_instance_label(instance_id) {
                let ring_buffer_samples = instance.offline_state.ring_sample_count();
                println!(
                    "[Sherpa] {label} detected speech start. segment_id={} total_samples={} ring_buffer_samples={}",
                    seg_id, instance.total_samples, ring_buffer_samples
                );
            } else {
                println!("[Sherpa] Instance {} detected speech start.", instance_id);
            }

            let samples_to_keep = (16000.0 * 0.3) as usize;
            instance
                .offline_state
                .begin_speech(instance.total_samples, samples_to_keep);
        }

        if currently_speaking {
            instance.offline_state.push_speech_chunk(samples.to_vec());

            let now = std::time::Instant::now();
            if instance.offline_state.should_run_inference(now, 200) {
                let global_start = instance.offline_state.utterance_start_seconds(16000.0);

                let offline_copy = instance.offline_state.speech_chunks().to_vec();
                let observer_copy = observer.clone();
                let punct_copy = instance.punctuation_clone();
                let seg_id_copy = seg_id.clone();
                let instance_id_copy = instance_id.to_string();
                let recognizer_copy = recognizer.clone();
                let first_segment_emitted =
                    diagnostics_instance_label(instance_id).is_some().then(|| {
                        instance
                            .record_diagnostics
                            .first_segment_emitted_flag()
                            .clone()
                    });
                let normalization_options = instance.normalization_options;
                let postprocessor = instance.postprocessor.clone();
                let should_record_partial_metric =
                    instance.should_record_partial_metric(PARTIAL_METRIC_INTERVAL_SAMPLES);
                if should_record_partial_metric {
                    instance.mark_partial_metric_sample();
                }
                let triggered_at = Instant::now();

                if let Some(label) = diagnostics_instance_label(instance_id) {
                    println!(
                        "[Sherpa] {label} triggering offline inference. stage=partial segment_id={} buffered_chunks={} buffered_samples={} global_start={:.3}",
                        seg_id,
                        offline_copy.len(),
                        buffered_sample_count(&offline_copy),
                        global_start
                    );
                }

                let task = move || {
                    if let Some(safe_r) = recognizer_copy.offline() {
                        run_offline_inference(
                            &offline_copy,
                            observer_copy.as_ref(),
                            safe_r,
                            punct_copy.as_deref(),
                            &seg_id_copy,
                            global_start,
                            false,
                            &instance_id_copy,
                            "partial",
                            first_segment_emitted,
                            normalization_options,
                            postprocessor,
                            should_record_partial_metric,
                            triggered_at,
                        );
                    }
                };
                drop(tokio::task::spawn_blocking(task));
                instance.offline_state.mark_inference_time(now);
            }
        }

        if !currently_speaking {
            if instance.offline_state.is_speech_active() {
                if let Some(label) = diagnostics_instance_label(instance_id) {
                    println!(
                        "[Sherpa] {label} detected speech end. segment_id={} total_samples={} buffered_chunks={} buffered_samples={}",
                        seg_id,
                        instance.total_samples,
                        instance.offline_state.buffered_speech_chunk_count() + 1,
                        instance.offline_state.buffered_speech_sample_count() + samples.len()
                    );
                } else {
                    println!("[Sherpa] Instance {} detected speech end.", instance_id);
                }
                instance
                    .offline_state
                    .finish_speech_with_chunk(samples.to_vec());

                let global_start = instance.offline_state.utterance_start_seconds(16000.0);

                let offline_copy = instance.offline_state.speech_chunks().to_vec();
                let observer_copy = observer.clone();
                let punct_copy = instance.punctuation_clone();
                let seg_id_copy = seg_id.clone();
                let instance_id_copy = instance_id.to_string();
                let recognizer_copy = recognizer.clone();
                let first_segment_emitted =
                    diagnostics_instance_label(instance_id).is_some().then(|| {
                        instance
                            .record_diagnostics
                            .first_segment_emitted_flag()
                            .clone()
                    });
                let normalization_options = instance.normalization_options;
                let postprocessor = instance.postprocessor.clone();
                let triggered_at = Instant::now();

                if let Some(label) = diagnostics_instance_label(instance_id) {
                    println!(
                        "[Sherpa] {label} triggering offline inference. stage=final segment_id={} buffered_chunks={} buffered_samples={} global_start={:.3}",
                        seg_id,
                        offline_copy.len(),
                        buffered_sample_count(&offline_copy),
                        global_start
                    );
                }

                let task = move || {
                    if let Some(safe_r) = recognizer_copy.offline() {
                        run_offline_inference(
                            &offline_copy,
                            observer_copy.as_ref(),
                            safe_r,
                            punct_copy.as_deref(),
                            &seg_id_copy,
                            global_start,
                            true,
                            &instance_id_copy,
                            "final",
                            first_segment_emitted,
                            normalization_options,
                            postprocessor,
                            true,
                            triggered_at,
                        );
                    }
                };
                drop(tokio::task::spawn_blocking(task));

                instance.offline_state.clear_speech_buffer();
                instance.clear_partial_metric_sample();
                instance.current_segment_id = Some(uuid::Uuid::new_v4().to_string());
            }

            let max_ring_samples = (16000.0 * 0.3) as usize;
            instance.offline_state.push_ring_chunk_with_sample_limit(
                samples.to_vec(),
                max_ring_samples,
                4_000,
            );
        }

        instance.total_samples += samples.len();
        Ok(())
    } else if let Some(r) = recognizer.online() {
        let inference_started = Instant::now();
        let stream = instance
            .take_stream()
            .ok_or("Stream not initialized for online model")?;
        let st = &stream;

        let decode_started = Instant::now();
        accept_online_samples(st, samples);
        instance.total_samples += samples.len();

        decode_online_ready(r, st);
        let decode_ms = duration_to_ms(decode_started.elapsed());

        let current_time = instance.total_samples as f64 / 16000.0;
        let endpoint_detected = is_online_endpoint(r, st);
        let mut did_record_partial_metric = false;

        if let Some(result) = online_stream_result(r, st) {
            let has_text = !result.text.trim().is_empty();
            if has_text || instance.current_segment_id.is_some() {
                let id = instance
                    .current_segment_id
                    .get_or_insert_with(|| uuid::Uuid::new_v4().to_string())
                    .clone();
                let timestamps_abs = result.timestamps.as_ref().map(|ts| {
                    ts.iter()
                        .map(|t| *t + instance.segment_start_time as f32)
                        .collect::<Vec<_>>()
                });
                let durations = timestamps_abs
                    .as_ref()
                    .and_then(|ts| synthesize_durations(ts, current_time as f32));

                let segment = TranscriptSegment {
                    id,
                    text: result.text.clone(),
                    start: instance.segment_start_time,
                    end: current_time,
                    is_final: false,
                    timing: None,
                    tokens: Some(result.tokens.clone()),
                    timestamps: timestamps_abs,
                    durations,
                    translation: None,
                    speaker: None,
                    speaker_attribution: None,
                };
                let update = instance
                    .postprocessor
                    .process_update(build_transcript_update(
                        segment,
                        instance.normalization_options,
                    ));
                observe_streaming_transcript_update(
                    observer.as_ref(),
                    instance_id,
                    &update,
                    "online_partial",
                    Some(instance.record_diagnostics.first_segment_emitted_flag()),
                );

                let should_record_partial_metric = !endpoint_detected
                    && instance.should_record_partial_metric(PARTIAL_METRIC_INTERVAL_SAMPLES);

                if should_record_partial_metric {
                    let buffered_samples =
                        ((current_time - instance.segment_start_time).max(0.0) * 16000.0) as usize;
                    record_live_metric(
                        observer.as_ref(),
                        build_live_metric(
                            instance_id,
                            "online_partial",
                            false,
                            buffered_samples,
                            decode_ms,
                            Some(duration_to_ms(inference_started.elapsed())),
                            None,
                        ),
                    );
                    did_record_partial_metric = true;
                }
            }
        }

        if endpoint_detected {
            let endpoint_decode_started = Instant::now();
            let tail_padding = vec![0.0; (16000.0 * 0.8) as usize];
            debug!("FFI: Calling accept_waveform (Online, tail_padding)");
            accept_online_samples(st, &tail_padding);
            debug!("FFI: Successfully returned from accept_waveform (Online, tail_padding)");
            decode_online_ready(r, st);
            let final_decode_ms = decode_ms + duration_to_ms(endpoint_decode_started.elapsed());
            let final_buffered_samples =
                ((current_time - instance.segment_start_time).max(0.0) * 16000.0) as usize;

            if let Some(result) = online_stream_result(r, st)
                && !result.text.trim().is_empty()
            {
                let text = format_transcript(&result.text, instance.punctuation());

                let timestamps_abs = result.timestamps.as_ref().map(|ts| {
                    ts.iter()
                        .map(|t| *t + instance.segment_start_time as f32)
                        .collect::<Vec<_>>()
                });
                let durations = timestamps_abs
                    .as_ref()
                    .and_then(|ts| synthesize_durations(ts, current_time as f32));

                let id = instance
                    .current_segment_id
                    .take()
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

                let segment = TranscriptSegment {
                    id,
                    text,
                    start: instance.segment_start_time,
                    end: current_time,
                    is_final: true,
                    timing: None,
                    tokens: Some(result.tokens),
                    timestamps: timestamps_abs,
                    durations,
                    translation: None,
                    speaker: None,
                    speaker_attribution: None,
                };
                let update = instance
                    .postprocessor
                    .process_update(build_transcript_update(
                        segment,
                        instance.normalization_options,
                    ));
                observe_streaming_transcript_update(
                    observer.as_ref(),
                    instance_id,
                    &update,
                    "online_final",
                    Some(instance.record_diagnostics.first_segment_emitted_flag()),
                );
            }

            record_live_metric(
                observer.as_ref(),
                build_live_metric(
                    instance_id,
                    "online_final",
                    true,
                    final_buffered_samples,
                    final_decode_ms,
                    Some(duration_to_ms(inference_started.elapsed())),
                    None,
                ),
            );

            instance.current_segment_id = None;
            reset_online_stream(r, st);
            instance.clear_partial_metric_sample();
            instance.segment_start_time = current_time;
        }

        if did_record_partial_metric {
            instance.mark_partial_metric_sample();
        }

        instance.restore_stream(stream);

        Ok(())
    } else {
        Err("Unsupported recognizer type".to_string())
    }
}

async fn feed_audio_chunk_impl_inner(
    observer: Arc<dyn AsrRuntimeObserver>,
    instance_id: &str,
    instance: &mut SherpaInstance,
    samples: Vec<u8>,
) -> Result<(), String> {
    trace!(
        "feed_audio_chunk called with id: {}, samples bytes: {}",
        instance_id,
        samples.len()
    );
    let mut float_samples = Vec::with_capacity(samples.len() / 2);
    for chunk in samples.chunks_exact(2) {
        let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
        float_samples.push(sample as f32 / 32768.0);
    }
    feed_audio_samples_inner(observer, instance_id, instance, &float_samples).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sona_core::ports::asr::{
        AsrStreamingSession, LocalSherpaStreamingRequest, NoopAsrRuntimeObserver,
    };
    use sona_core::transcription::postprocess::{
        TranscriptNormalizationOptions, TranscriptPostprocessOptions,
    };

    fn assert_streaming_session<T: AsrStreamingSession>() {}

    #[test]
    fn local_sherpa_session_implements_core_streaming_port() {
        assert_streaming_session::<LocalSherpaSession>();
    }

    #[tokio::test]
    async fn factory_rejects_missing_file_config_without_loading_a_model() {
        let result = create_streaming_session(
            RecognizerPool::new(),
            LocalSherpaStreamingRequest {
                instance_id: "test-live".to_string(),
                model_path: "unused".to_string(),
                num_threads: 1,
                enable_itn: false,
                language: "auto".to_string(),
                punctuation_model: None,
                vad_model: None,
                vad_buffer: 0.0,
                model_type: "sensevoice".to_string(),
                file_config: None,
                hotwords: None,
                normalization_options: TranscriptNormalizationOptions::default(),
                postprocess_options: TranscriptPostprocessOptions::default(),
                gpu_acceleration: None,
            },
            Arc::new(NoopAsrRuntimeObserver),
        )
        .await;

        assert_eq!(
            result.err().as_deref(),
            Some("File configuration is missing for this model.")
        );
    }

    #[tokio::test]
    async fn punctuation_resolution_ignores_missing_paths() {
        let pool = RecognizerPool::new();

        assert!(resolve_punctuation(&pool, None).await.is_none());
        assert!(
            resolve_punctuation(&pool, Some("".to_string()))
                .await
                .is_none()
        );
        assert!(
            resolve_punctuation(&pool, Some("nonexistent_path_123".to_string()))
                .await
                .is_none()
        );
    }
}
