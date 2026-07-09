use super::TranscriptPostprocessor;
use super::metrics::{
    AsrInferenceMetric, AsrMetricsStore, AsrModelLoadMetric, calculate_rss_delta_mb, calculate_rtf,
    capture_process_memory_mb, current_time_millis, duration_to_ms, log_inference_metric,
    log_model_load_metric, samples_to_ms, set_live_inference_metric,
};
use super::transcript::{
    build_transcript_update, emit_transcript_update, finalize_transcript_text, format_transcript,
    log_text_transform_diagnostics, normalize_recognizer_text, preview_text_for_log,
    synthesize_durations,
};
use super::types::{
    LocalSherpaStreamingRequest, TranscriptNormalizationOptions, TranscriptSegment,
};
use crate::integrations::asr::{AsrState, AsrStreamingSession, ModelConfigKey};
use log::{debug, info, trace};
use sona_local_asr::audio::{accept_vad_samples, load_vad, reset_vad, vad_detected};
use sona_local_asr::punctuation::{Punctuation, load_punctuation};
use sona_local_asr::recognizer::{
    Recognizer, RecognizerInner, SafeOfflineRecognizer, accept_online_samples, build_model_config,
    create_online_stream, decode_offline_samples, decode_online_ready, is_online_endpoint,
    online_stream_result, reset_online_stream,
};
use sona_local_asr::runtime::{
    OfflineState, SherpaInstance, buffered_sample_count, start_instance_runtime,
    stop_instance_runtime,
};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
// No AppHandle needed

const PARTIAL_METRIC_INTERVAL_SAMPLES: usize = 16_000;

fn record_live_metric(metrics_store: &AsrMetricsStore, metric: AsrInferenceMetric) {
    set_live_inference_metric(metrics_store, metric.clone());
    log_inference_metric(&metric);
}

fn build_live_metric(
    instance_id: &str,
    stage: &str,
    is_final: bool,
    buffered_samples: usize,
    decode_ms: f64,
    emit_latency_ms: Option<f64>,
    total_ms: Option<f64>,
) -> AsrInferenceMetric {
    let audio_duration_ms = samples_to_ms(buffered_samples, 16000.0);

    AsrInferenceMetric {
        occurred_at_ms: current_time_millis(),
        source: "live".to_string(),
        instance_id: Some(instance_id.to_string()),
        stage: stage.to_string(),
        is_final,
        audio_duration_ms,
        buffered_samples,
        audio_extract_ms: None,
        decode_ms,
        emit_latency_ms,
        total_ms,
        rtf: calculate_rtf(decode_ms, audio_duration_ms),
        segment_count: None,
        process_rss_mb: capture_process_memory_mb(),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_offline_inference(
    speech_buffer: &[Vec<f32>],
    emitter: &dyn crate::platform::event::EventEmitter,
    r: &SafeOfflineRecognizer,
    punctuation: Option<&Punctuation>,
    segment_id: &str,
    global_start: f64,
    is_final: bool,
    instance_id: &str,
    stage: &'static str,
    first_segment_emitted: Option<Arc<AtomicBool>>,
    normalization_options: TranscriptNormalizationOptions,
    postprocessor: TranscriptPostprocessor,
    metrics_store: Option<AsrMetricsStore>,
    triggered_at: Instant,
) {
    if speech_buffer.is_empty() {
        if let Some(label) = diagnostics_instance_label(instance_id) {
            info!(
                "[Sherpa] {label} offline inference skipped because the speech buffer is empty. stage={stage}"
            );
        }
        return;
    }

    // Offline models decode one aggregated utterance at a time, so we flatten
    // the buffered speech chunks into one continuous waveform before calling
    // Sherpa.
    let mut full_audio = Vec::new();
    for chunk in speech_buffer {
        full_audio.extend_from_slice(chunk);
    }
    let decode_started = Instant::now();
    debug!(
        "[Offline] FFI: Calling decode_offline_samples with {} samples",
        full_audio.len()
    );
    let decode_result = decode_offline_samples(r, &full_audio);
    debug!("[Offline] FFI: Decode finished");
    let decode_ms = duration_to_ms(decode_started.elapsed());

    let record_metric = |emit_latency_ms: Option<f64>| {
        if let Some(metrics_store) = metrics_store.as_ref() {
            record_live_metric(
                metrics_store,
                build_live_metric(
                    instance_id,
                    stage,
                    is_final,
                    full_audio.len(),
                    decode_ms,
                    emit_latency_ms,
                    None,
                ),
            );
        }
    };

    if let Some(label) = diagnostics_instance_label(instance_id) {
        info!(
            "[Sherpa] {label} offline inference finished. stage={} segment_id={} final={} buffered_chunks={} buffered_samples={}",
            stage,
            segment_id,
            is_final,
            speech_buffer.len(),
            full_audio.len()
        );
    }

    if let Some(result) = decode_result {
        let raw_text = result.text.trim();
        if !raw_text.is_empty() {
            // The offline path emits only meaningful text: raw recognizer output
            // is normalized first, then final-only formatting/punctuation is
            // applied when this segment closes an utterance.
            let cleaned_text = normalize_recognizer_text(&result.text);
            if cleaned_text.is_empty() {
                if let Some(label) = diagnostics_instance_label(instance_id) {
                    info!(
                        "[Sherpa] {label} offline inference produced empty text after normalization. stage={} segment_id={} final={} raw_preview={:?}",
                        stage,
                        segment_id,
                        is_final,
                        preview_text_for_log(raw_text)
                    );
                }
                record_metric(Some(duration_to_ms(triggered_at.elapsed())));
                return;
            }

            let text = if is_final {
                finalize_transcript_text(&cleaned_text, punctuation)
            } else {
                cleaned_text.clone()
            };

            if text.is_empty() {
                if let Some(label) = diagnostics_instance_label(instance_id) {
                    info!(
                        "[Sherpa] {label} offline inference produced empty output text after normalization/formatting. stage={} segment_id={} final={} raw_preview={:?} cleaned_preview={:?}",
                        stage,
                        segment_id,
                        is_final,
                        preview_text_for_log(raw_text),
                        preview_text_for_log(&cleaned_text)
                    );
                }
                record_metric(Some(duration_to_ms(triggered_at.elapsed())));
                return;
            }

            log_text_transform_diagnostics(
                instance_id,
                stage,
                segment_id,
                is_final,
                raw_text,
                &cleaned_text,
                &text,
            );

            let global_end = global_start + (full_audio.len() as f64 / 16000.0);
            // Sherpa timestamps are relative to the decoded utterance, so we
            // shift them into the global recording timeline before emitting.
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
                tokens: Some(result.tokens),
                timestamps: timestamps_abs,
                durations,
                translation: None,
                speaker: None,
                speaker_attribution: None,
            };
            let update = postprocessor
                .process_update(build_transcript_update(segment, normalization_options));
            emit_transcript_update(
                emitter,
                instance_id,
                &update,
                stage,
                first_segment_emitted.as_ref(),
            );
            record_metric(Some(duration_to_ms(triggered_at.elapsed())));
        } else if let Some(label) = diagnostics_instance_label(instance_id) {
            info!(
                "[Sherpa] {label} offline inference produced empty text after formatting. stage={} segment_id={} final={}",
                stage, segment_id, is_final
            );
            record_metric(Some(duration_to_ms(triggered_at.elapsed())));
        }
    } else if let Some(label) = diagnostics_instance_label(instance_id) {
        info!(
            "[Sherpa] {label} offline inference produced no recognizer result. stage={} segment_id={} final={}",
            stage, segment_id, is_final
        );
        record_metric(Some(duration_to_ms(triggered_at.elapsed())));
    }
}

use crate::integrations::asr::SherpaError;
use async_trait::async_trait;

pub struct LocalSherpaSession {
    pub instance: tokio::sync::Mutex<SherpaInstance>,
}

#[async_trait]
impl AsrStreamingSession for LocalSherpaSession {
    async fn start(
        &self,
        _emitter: std::sync::Arc<dyn crate::platform::event::EventEmitter>,
        _state: &AsrState,
        instance_id: &str,
    ) -> Result<(), SherpaError> {
        let mut instance = self.instance.lock().await;
        start_recognizer_impl_inner(instance_id, &mut instance)
            .await
            .map_err(SherpaError::Generic)
    }

    async fn stop(&self, _state: &AsrState, instance_id: &str) -> Result<(), SherpaError> {
        let mut instance = self.instance.lock().await;
        stop_recognizer_impl_inner(instance_id, &mut instance)
            .await
            .map_err(SherpaError::Generic)
    }

    async fn flush(
        &self,
        emitter: std::sync::Arc<dyn crate::platform::event::EventEmitter>,
        state: &AsrState,
        instance_id: &str,
    ) -> Result<(), SherpaError> {
        let mut instance = self.instance.lock().await;
        flush_recognizer_impl_inner(emitter, state, instance_id, &mut instance)
            .await
            .map_err(SherpaError::Generic)
    }

    async fn feed_audio_chunk(
        &self,
        emitter: std::sync::Arc<dyn crate::platform::event::EventEmitter>,
        state: &AsrState,
        instance_id: &str,
        samples: Vec<u8>,
    ) -> Result<(), SherpaError> {
        let mut instance = self.instance.lock().await;
        feed_audio_chunk_impl_inner(emitter, state, instance_id, &mut instance, samples)
            .await
            .map_err(SherpaError::Generic)
    }

    async fn feed_audio_samples(
        &self,
        emitter: std::sync::Arc<dyn crate::platform::event::EventEmitter>,
        state: &AsrState,
        instance_id: &str,
        samples: &[f32],
    ) -> Result<(), SherpaError> {
        let mut instance = self.instance.lock().await;
        feed_audio_samples_inner(emitter, state, instance_id, &mut instance, samples)
            .await
            .map_err(SherpaError::Generic)
    }
}

pub async fn resolve_punctuation(
    pool: &crate::integrations::asr::RecognizerPool,
    punctuation_model: Option<String>,
) -> Option<Arc<Punctuation>> {
    let p_path = punctuation_model?;
    if p_path.is_empty() || !Path::new(&p_path).exists() {
        return None;
    }
    let cell = {
        let mut map = pool.punctuations.lock().await;
        map.entry(p_path.clone())
            .or_insert_with(|| Arc::new(tokio::sync::OnceCell::new()))
            .clone()
    };
    cell.get_or_try_init(|| async {
        load_punctuation(Some(p_path.clone()))
            .map(Arc::new)
            .ok_or_else(|| "Failed to load punctuation model".to_string())
    })
    .await
    .ok()
    .cloned()
}

pub async fn init_recognizer_impl(
    state: &AsrState,
    request: LocalSherpaStreamingRequest,
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

    let gpu_plan =
        crate::platform::hardware::resolve_gpu_acceleration_plan(gpu_acceleration.as_deref()).await;

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
    let primary_key = config_key.with_gpu_provider(primary_provider.clone());

    let (cell, is_new) = {
        let mut pool_guard = state.recognizer_pool.recognizers.lock().await;
        let existing = gpu_plan
            .provider_options()
            .into_iter()
            .find_map(|provider| {
                pool_guard
                    .get(&config_key.with_gpu_provider(provider))
                    .cloned()
            });

        if let Some(c) = existing {
            (c, false)
        } else {
            let cell = Arc::new(tokio::sync::OnceCell::new());
            pool_guard.insert(primary_key.clone(), cell.clone());
            (cell, true)
        }
    };

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
            let create_result = crate::integrations::asr::create_recognizer_with_gpu_plan(
                config_type,
                num_threads,
                gpu_plan.clone(),
            )?;
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
                let mut pool_guard = state.recognizer_pool.recognizers.lock().await;
                pool_guard.insert(config_key.with_gpu_provider(actual_provider), cell.clone());
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
    state
        .record_model_load_metric(model_load_metric.clone())
        .await;
    log_model_load_metric(&model_load_metric);

    let punctuation = resolve_punctuation(&state.recognizer_pool, punctuation_model).await;
    let vad = load_vad(vad_model.clone());

    let session_instance = SherpaInstance {
        recognizer: Some(recognizer),
        vad,
        punctuation,
        vad_model: vad_model.clone(),
        vad_buffer,
        normalization_options,
        postprocessor: TranscriptPostprocessor::compile(postprocess_options)?,
        ..Default::default()
    };

    let session = std::sync::Arc::new(LocalSherpaSession {
        instance: tokio::sync::Mutex::new(session_instance),
    });

    Ok(session)
}

async fn start_recognizer_impl_inner(
    instance_id: &str,
    instance: &mut SherpaInstance,
) -> Result<(), String> {
    let Some(recognizer) = instance.recognizer.as_ref() else {
        return Err("Recognizer not initialized".to_string());
    };
    let recognizer_kind = match &recognizer.inner {
        RecognizerInner::Offline(_) => "offline",
        RecognizerInner::Online(_) => "online",
    };

    let stream = match &recognizer.inner {
        RecognizerInner::Online(r) => Some(create_online_stream(r)),
        _ => None,
    };

    // Starting a run resets transient buffers and, for online models, creates a
    // fresh Sherpa stream that will accumulate new incremental state.
    start_instance_runtime(instance, stream);

    if instance.vad_model.is_some() {
        if let Some(vad) = instance.vad.as_mut() {
            reset_vad(vad);
        } else {
            instance.vad = load_vad(instance.vad_model.clone());
        }
    }

    if let Some(label) = diagnostics_instance_label(instance_id) {
        info!(
            "[Sherpa] start_recognizer({label}): is_running=true recognizer_kind={} vad_configured={} punctuation_loaded={}",
            recognizer_kind,
            instance.vad_model.is_some(),
            instance.punctuation.is_some()
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
                instance.is_running,
                instance.total_samples,
                instance.offline_state.speech_buffer.len(),
                buffered_sample_count(&instance.offline_state.speech_buffer),
                instance.current_segment_id.as_deref().unwrap_or("none"),
                instance
                    .record_diagnostics
                    .first_segment_emitted
                    .load(Ordering::SeqCst)
            );
        }
        stop_instance_runtime(instance);
    }
    Ok(())
}

async fn flush_recognizer_impl_inner(
    emitter: std::sync::Arc<dyn crate::platform::event::EventEmitter>,
    state: &AsrState,
    instance_id: &str,
    instance: &mut SherpaInstance,
) -> Result<(), String> {
    info!("Flushing recognizer for instance id: {}", instance_id);

    if let Some(label) = diagnostics_instance_label(instance_id) {
        info!(
            "[Sherpa] flush_recognizer({label}): is_running={} total_samples={} buffered_chunks={} buffered_samples={} current_segment={} speaking={}",
            instance.is_running,
            instance.total_samples,
            instance.offline_state.speech_buffer.len(),
            buffered_sample_count(&instance.offline_state.speech_buffer),
            instance.current_segment_id.as_deref().unwrap_or("none"),
            instance.offline_state.is_speaking
        );
    }

    if let Some(recognizer) = instance.recognizer.clone()
        && let RecognizerInner::Offline(_) = &recognizer.inner
    {
        if !instance.offline_state.speech_buffer.is_empty() {
            let seg_id = instance
                .current_segment_id
                .as_ref()
                .cloned()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let global_start = instance.offline_state.utterance_start_sample as f64 / 16000.0;

            let offline_copy = instance.offline_state.speech_buffer.clone();
            let emitter_copy = emitter.clone();
            let recognizer_copy = recognizer.clone();
            let punct_copy = instance.punctuation.clone();
            let seg_id_copy = seg_id.clone();
            let instance_id_copy = instance_id.to_string();
            let first_segment_emitted = diagnostics_instance_label(instance_id)
                .is_some()
                .then(|| instance.record_diagnostics.first_segment_emitted.clone());
            let normalization_options = instance.normalization_options;
            let postprocessor = instance.postprocessor.clone();
            let metrics_store = state.metrics.clone();
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
            crate::platform::asr_runtime::run_blocking_asr_task(move || {
                if let RecognizerInner::Offline(safe_r) = &recognizer_copy.inner {
                    run_offline_inference(
                        &offline_copy,
                        emitter_copy.as_ref(),
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
                        Some(metrics_store),
                        triggered_at,
                    );
                }
            })
            .await
            .map_err(|e| e.to_string())?;

            instance.offline_state.speech_buffer.clear();
            instance.offline_state.is_speaking = false;
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

    if let (Some(recognizer), Some(st)) = (instance.recognizer.as_deref(), instance.stream.as_ref())
        && let RecognizerInner::Online(r) = &recognizer.inner
    {
        let inference_started = Instant::now();
        let metrics_store = state.metrics.clone();
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
            let text = format_transcript(&result.text, instance.punctuation.as_deref());
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
            emit_transcript_update(
                emitter.as_ref(),
                instance_id,
                &update,
                "flush_online",
                Some(&instance.record_diagnostics.first_segment_emitted),
            );
        }

        record_live_metric(
            &metrics_store,
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
        instance.segment_start_time = current_time;
        if let Some(label) = diagnostics_instance_label(instance_id) {
            info!("[Sherpa] flush_recognizer({label}) complete. mode=online");
        }
    }

    Ok(())
}

async fn feed_audio_samples_inner(
    emitter: std::sync::Arc<dyn crate::platform::event::EventEmitter>,
    state: &AsrState,
    instance_id: &str,
    instance: &mut SherpaInstance,
    samples: &[f32],
) -> Result<(), String> {
    // instances removed
    // instances lookup removed

    if !instance.is_running {
        if let Some(label) = diagnostics_instance_label(instance_id)
            && !instance.record_diagnostics.skipped_while_stopped_logged
        {
            println!(
                "[Sherpa] {label} audio chunk skipped because recognizer is not running. samples={} total_samples={}",
                samples.len(),
                instance.total_samples
            );
            instance.record_diagnostics.skipped_while_stopped_logged = true;
        }
        return Ok(());
    }

    if let Some(label) = diagnostics_instance_label(instance_id)
        && !instance.record_diagnostics.first_sample_logged
    {
        println!(
            "[Sherpa] {label} first sample received. samples={} total_samples_before={} current_segment={}",
            samples.len(),
            instance.total_samples,
            instance.current_segment_id.as_deref().unwrap_or("none")
        );
        instance.record_diagnostics.first_sample_logged = true;
    }

    let recognizer = instance
        .recognizer
        .clone()
        .ok_or("Recognizer not initialized")?;

    match &recognizer.inner {
        RecognizerInner::Offline(_) => {
            let Some(vad) = instance.vad.as_ref() else {
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
                        .first_segment_emitted
                        .load(Ordering::SeqCst)
                );
            }

            if instance.current_segment_id.is_none() {
                instance.current_segment_id = Some(uuid::Uuid::new_v4().to_string());
            }
            let seg_id = instance.current_segment_id.as_ref().unwrap().clone();

            if currently_speaking && !instance.offline_state.is_speaking {
                if let Some(label) = diagnostics_instance_label(instance_id) {
                    let ring_buffer_samples: usize = instance
                        .offline_state
                        .ring_buffer
                        .iter()
                        .map(|chunk| chunk.len())
                        .sum();
                    println!(
                        "[Sherpa] {label} detected speech start. segment_id={} total_samples={} ring_buffer_samples={}",
                        seg_id, instance.total_samples, ring_buffer_samples
                    );
                } else {
                    println!("[Sherpa] Instance {} detected speech start.", instance_id);
                }
                instance.offline_state.is_speaking = true;

                let samples_to_keep = (16000.0 * 0.3) as usize;
                let mut context_len = 0;

                if !instance.offline_state.ring_buffer.is_empty() {
                    let ring_flat: Vec<f32> = instance
                        .offline_state
                        .ring_buffer
                        .iter()
                        .flatten()
                        .copied()
                        .collect();

                    let keep_start = ring_flat.len().saturating_sub(samples_to_keep);

                    let context = ring_flat[keep_start..].to_vec();
                    context_len = context.len();
                    instance.offline_state.speech_buffer.push(context);
                }

                instance.offline_state.utterance_start_sample =
                    instance.total_samples - context_len;
                instance.offline_state.ring_buffer.clear();
            }

            if currently_speaking {
                instance.offline_state.speech_buffer.push(samples.to_vec());

                let now = std::time::Instant::now();
                if now
                    .duration_since(instance.offline_state.last_inference_time)
                    .as_millis()
                    > 200
                {
                    let global_start =
                        instance.offline_state.utterance_start_sample as f64 / 16000.0;

                    let offline_copy = instance.offline_state.speech_buffer.clone();
                    let emitter_copy = emitter.clone();
                    let punct_copy = instance.punctuation.clone();
                    let seg_id_copy = seg_id.clone();
                    let instance_id_copy = instance_id.to_string();
                    let recognizer_copy = recognizer.clone();
                    let first_segment_emitted = diagnostics_instance_label(instance_id)
                        .is_some()
                        .then(|| instance.record_diagnostics.first_segment_emitted.clone());
                    let normalization_options = instance.normalization_options;
                    let postprocessor = instance.postprocessor.clone();
                    let should_record_partial_metric = instance.last_partial_metric_sample == 0
                        || instance
                            .total_samples
                            .saturating_sub(instance.last_partial_metric_sample)
                            >= PARTIAL_METRIC_INTERVAL_SAMPLES;
                    let metrics_store = should_record_partial_metric.then(|| {
                        instance.last_partial_metric_sample = instance.total_samples;
                        state.metrics.clone()
                    });
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

                    crate::platform::asr_runtime::spawn_blocking_asr_task(move || {
                        if let RecognizerInner::Offline(safe_r) = &recognizer_copy.inner {
                            run_offline_inference(
                                &offline_copy,
                                emitter_copy.as_ref(),
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
                                metrics_store,
                                triggered_at,
                            );
                        }
                    });
                    instance.offline_state.last_inference_time = now;
                }
            }

            if !currently_speaking {
                if instance.offline_state.is_speaking {
                    if let Some(label) = diagnostics_instance_label(instance_id) {
                        println!(
                            "[Sherpa] {label} detected speech end. segment_id={} total_samples={} buffered_chunks={} buffered_samples={}",
                            seg_id,
                            instance.total_samples,
                            instance.offline_state.speech_buffer.len() + 1,
                            buffered_sample_count(&instance.offline_state.speech_buffer)
                                + samples.len()
                        );
                    } else {
                        println!("[Sherpa] Instance {} detected speech end.", instance_id);
                    }
                    instance.offline_state.is_speaking = false;
                    instance.offline_state.speech_buffer.push(samples.to_vec());

                    let global_start =
                        instance.offline_state.utterance_start_sample as f64 / 16000.0;

                    let offline_copy = instance.offline_state.speech_buffer.clone();
                    let emitter_copy = emitter.clone();
                    let punct_copy = instance.punctuation.clone();
                    let seg_id_copy = seg_id.clone();
                    let instance_id_copy = instance_id.to_string();
                    let recognizer_copy = recognizer.clone();
                    let first_segment_emitted = diagnostics_instance_label(instance_id)
                        .is_some()
                        .then(|| instance.record_diagnostics.first_segment_emitted.clone());
                    let normalization_options = instance.normalization_options;
                    let postprocessor = instance.postprocessor.clone();
                    let metrics_store = state.metrics.clone();
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

                    crate::platform::asr_runtime::spawn_blocking_asr_task(move || {
                        if let RecognizerInner::Offline(safe_r) = &recognizer_copy.inner {
                            run_offline_inference(
                                &offline_copy,
                                emitter_copy.as_ref(),
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
                                Some(metrics_store),
                                triggered_at,
                            );
                        }
                    });

                    instance.offline_state.speech_buffer.clear();
                    instance.last_partial_metric_sample = 0;
                    instance.current_segment_id = Some(uuid::Uuid::new_v4().to_string());
                }

                instance
                    .offline_state
                    .ring_buffer
                    .push_back(samples.to_vec());
                let max_ring_samples = (16000.0 * 0.3) as usize;

                let mut ring_len: usize = instance
                    .offline_state
                    .ring_buffer
                    .iter()
                    .map(|v| v.len())
                    .sum();
                while ring_len > max_ring_samples + 4000 {
                    if let Some(first) = instance.offline_state.ring_buffer.front() {
                        let first_len = first.len();
                        if ring_len - first_len >= max_ring_samples {
                            instance.offline_state.ring_buffer.pop_front();
                            ring_len -= first_len;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }

            instance.total_samples += samples.len();
            Ok(())
        }
        RecognizerInner::Online(r) => {
            let inference_started = Instant::now();
            let metrics_store = state.metrics.clone();
            let st = instance
                .stream
                .as_ref()
                .ok_or("Stream not initialized for online model")?;

            let decode_started = Instant::now();
            accept_online_samples(st, samples);
            instance.total_samples += samples.len();

            decode_online_ready(r, st);
            let decode_ms = duration_to_ms(decode_started.elapsed());

            let current_time = instance.total_samples as f64 / 16000.0;
            let endpoint_detected = is_online_endpoint(r, st);

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
                    emit_transcript_update(
                        emitter.as_ref(),
                        instance_id,
                        &update,
                        "online_partial",
                        Some(&instance.record_diagnostics.first_segment_emitted),
                    );

                    let should_record_partial_metric = !endpoint_detected
                        && (instance.last_partial_metric_sample == 0
                            || instance
                                .total_samples
                                .saturating_sub(instance.last_partial_metric_sample)
                                >= PARTIAL_METRIC_INTERVAL_SAMPLES);

                    if should_record_partial_metric {
                        let buffered_samples = ((current_time - instance.segment_start_time)
                            .max(0.0)
                            * 16000.0) as usize;
                        record_live_metric(
                            &metrics_store,
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
                        instance.last_partial_metric_sample = instance.total_samples;
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
                    let text = format_transcript(&result.text, instance.punctuation.as_deref());

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
                    emit_transcript_update(
                        emitter.as_ref(),
                        instance_id,
                        &update,
                        "online_final",
                        Some(&instance.record_diagnostics.first_segment_emitted),
                    );
                }

                record_live_metric(
                    &metrics_store,
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
                instance.last_partial_metric_sample = 0;
                reset_online_stream(r, st);
                instance.segment_start_time = current_time;
            }

            Ok(())
        }
    }
}

async fn feed_audio_chunk_impl_inner(
    emitter: std::sync::Arc<dyn crate::platform::event::EventEmitter>,
    state: &AsrState,
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
    feed_audio_samples_inner(emitter, state, instance_id, instance, &float_samples).await
}

pub fn diagnostics_instance_label(instance_id: &str) -> Option<&'static str> {
    match instance_id {
        "record" => Some("record"),
        "caption" => Some("caption"),
        "voice-typing" => Some("voice-typing"),
        _ => None,
    }
}

pub fn log_segment_emit_diagnostics(
    instance_id: &str,
    first_segment_emitted: Option<&Arc<AtomicBool>>,
    segment: &TranscriptSegment,
    stage: &str,
) {
    // These logs are intentionally scoped to the long-lived live instances we
    // debug most often (`record`, `caption`, `voice-typing`), not to every
    // possible recognizer consumer.
    let Some(label) = diagnostics_instance_label(instance_id) else {
        return;
    };

    let text_len = segment.text.chars().count();
    if let Some(first_segment_emitted) = first_segment_emitted
        && first_segment_emitted
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    {
        info!(
            "[Sherpa] {label} first segment emitted. stage={} segment_id={} final={} text_len={}",
            stage, segment.id, segment.is_final, text_len
        );
    }

    info!(
        "[Sherpa] {label} emit. stage={} segment_id={} final={} text_len={}",
        stage, segment.id, segment.is_final, text_len
    );
}
