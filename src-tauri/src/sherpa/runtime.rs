use super::model_config::{
    build_model_config, load_punctuation, load_vad, ModelFileConfig, Punctuation, Recognizer,
    RecognizerInner, SafeStream, SafeVad,
};
use super::state::{
    buffered_sample_count, diagnostics_instance_label, start_instance_runtime,
    stop_instance_runtime, ModelConfigKey, OfflineState, SherpaInstance, SherpaState,
};
use super::transcript::{
    build_transcript_update, emit_transcript_update, finalize_transcript_text, format_transcript,
    log_text_transform_diagnostics, normalize_recognizer_text, preview_text_for_log,
    synthesize_durations,
};
use super::types::{TranscriptNormalizationOptions, TranscriptSegment};
use log::{debug, info, trace};
use sherpa_onnx::OfflineRecognizer;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, State};

fn run_offline_inference<R: tauri::Runtime>(
    speech_buffer: &[Vec<f32>],
    app: &AppHandle<R>,
    r: &OfflineRecognizer,
    punctuation: Option<&Punctuation>,
    segment_id: &str,
    global_start: f64,
    is_final: bool,
    instance_id: &str,
    stage: &'static str,
    first_segment_emitted: Option<Arc<AtomicBool>>,
    normalization_options: TranscriptNormalizationOptions,
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
    let stream = r.create_stream();
    debug!(
        "[Offline] FFI: Calling accept_waveform (Offline) with {} samples",
        full_audio.len()
    );
    stream.accept_waveform(16000, &full_audio);

    debug!("[Offline] FFI: Calling decode");
    r.decode(&stream);
    debug!("[Offline] FFI: Decode finished");

    if let Some(label) = diagnostics_instance_label(instance_id) {
        info!(
            "[Sherpa] {label} offline inference finished. stage={} segment_id={} final={} buffered_chunks={} buffered_samples={}",
            stage, segment_id, is_final, speech_buffer.len(), full_audio.len()
        );
    }

    if let Some(result) = stream.get_result() {
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
            };
            let update = build_transcript_update(segment, normalization_options);
            emit_transcript_update(
                app,
                instance_id,
                &update,
                stage,
                first_segment_emitted.as_ref(),
            );
        } else if let Some(label) = diagnostics_instance_label(instance_id) {
            info!(
                "[Sherpa] {label} offline inference produced empty text after formatting. stage={} segment_id={} final={}",
                stage, segment_id, is_final
            );
        }
    } else if let Some(label) = diagnostics_instance_label(instance_id) {
        info!(
            "[Sherpa] {label} offline inference produced no recognizer result. stage={} segment_id={} final={}",
            stage, segment_id, is_final
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn init_recognizer_impl(
    state: State<'_, SherpaState>,
    instance_id: String,
    model_path: String,
    num_threads: i32,
    enable_itn: bool,
    language: String,
    punctuation_model: Option<String>,
    vad_model: Option<String>,
    vad_buffer: f32,
    model_type: String,
    file_config: Option<ModelFileConfig>,
    hotwords: Option<String>,
    normalization_options: Option<TranscriptNormalizationOptions>,
) -> Result<(), String> {
    info!(
        "[init_recognizer] start instance_id={instance_id} model_path={model_path} model_type={model_type} num_threads={num_threads} enable_itn={enable_itn} language={language} punctuation_model={:?} vad_model={:?} vad_buffer={vad_buffer} hotwords={:?}",
        punctuation_model,
        vad_model,
        hotwords
    );

    let config_key = ModelConfigKey {
        model_path: model_path.clone(),
        model_type: model_type.clone(),
        num_threads,
        enable_itn,
        language: language.clone(),
        hotwords: hotwords.clone(),
    };

    let recognizer = {
        let mut pool = state.recognizer_pool.lock().await;
        if let Some(r) = pool.get(&config_key) {
            // Heavy recognizers are reused across logical instances when their
            // model path and runtime knobs match exactly.
            info!("[init_recognizer] Reusing existing recognizer from pool");
            r.clone()
        } else {
            info!("[init_recognizer] Creating new recognizer and adding to pool");
            let config_type = build_model_config(
                Path::new(&model_path),
                &model_type,
                &file_config,
                enable_itn,
                &language,
                hotwords,
            )?;
            let r = Arc::new(Recognizer::new(config_type, num_threads)?);
            pool.insert(config_key, r.clone());
            r
        }
    };

    let punctuation = load_punctuation(punctuation_model);
    let vad = load_vad(vad_model.clone());

    let mut instances = state.instances.lock().await;
    let instance = instances
        .entry(instance_id)
        .or_insert_with(SherpaInstance::default);

    // Instance-local attachments can differ even when the core recognizer is
    // shared, so VAD/punctuation/runtime settings are refreshed here.
    instance.recognizer = Some(recognizer);
    instance.vad = vad;
    instance.punctuation = punctuation.map(Arc::new);
    instance.vad_model = vad_model.clone();
    instance.vad_buffer = vad_buffer;
    instance.normalization_options = normalization_options.unwrap_or_default();

    Ok(())
}

pub async fn start_recognizer_impl(
    state: State<'_, SherpaState>,
    instance_id: String,
) -> Result<(), String> {
    let mut instances = state.instances.lock().await;
    let instance = instances
        .get_mut(&instance_id)
        .ok_or("Instance not found")?;

    let Some(recognizer) = instance.recognizer.as_ref() else {
        return Err("Recognizer not initialized".to_string());
    };
    let recognizer_kind = match &recognizer.inner {
        RecognizerInner::Offline(_) => "offline",
        RecognizerInner::Online(_) => "online",
    };

    let stream = match &recognizer.inner {
        RecognizerInner::Online(r) => Some(SafeStream(r.0.create_stream())),
        _ => None,
    };

    // Starting a run resets transient buffers and, for online models, creates a
    // fresh Sherpa stream that will accumulate new incremental state.
    start_instance_runtime(instance, stream);

    if instance.vad_model.is_some() {
        // Reload VAD per start so any prior run-specific detector state cannot
        // bleed into the next recording/caption session.
        instance.vad = load_vad(instance.vad_model.clone());
    }

    if let Some(label) = diagnostics_instance_label(&instance_id) {
        info!(
            "[Sherpa] start_recognizer({label}): is_running=true recognizer_kind={} vad_configured={} punctuation_loaded={}",
            recognizer_kind,
            instance.vad_model.is_some(),
            instance.punctuation.is_some()
        );
    }

    Ok(())
}

pub async fn stop_recognizer_impl(
    state: State<'_, SherpaState>,
    instance_id: String,
) -> Result<(), String> {
    let mut instances = state.instances.lock().await;
    if let Some(instance) = instances.get_mut(&instance_id) {
        if let Some(label) = diagnostics_instance_label(&instance_id) {
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

pub async fn flush_recognizer_impl<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, SherpaState>,
    instance_id: String,
) -> Result<(), String> {
    info!("Flushing recognizer for instance id: {}", instance_id);
    let mut instances = state.instances.lock().await;
    let instance = instances
        .get_mut(&instance_id)
        .ok_or("Instance not found")?;

    if let Some(label) = diagnostics_instance_label(&instance_id) {
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

    if let Some(recognizer) = instance.recognizer.clone() {
        if let RecognizerInner::Offline(_) = &recognizer.inner {
            if !instance.offline_state.speech_buffer.is_empty() {
                let seg_id = instance
                    .current_segment_id
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                let global_start = instance.offline_state.utterance_start_sample as f64 / 16000.0;

                let offline_copy = instance.offline_state.speech_buffer.clone();
                let app_copy = app.clone();
                let recognizer_copy = recognizer.clone();
                let punct_copy = instance.punctuation.clone();
                let seg_id_copy = seg_id.clone();
                let instance_id_copy = instance_id.clone();
                let first_segment_emitted = diagnostics_instance_label(&instance_id)
                    .is_some()
                    .then(|| instance.record_diagnostics.first_segment_emitted.clone());
                let normalization_options = instance.normalization_options;

                if let Some(label) = diagnostics_instance_label(&instance_id) {
                    info!(
                        "[Sherpa] {label} flush triggering offline inference. segment_id={} buffered_chunks={} buffered_samples={}",
                        seg_id, offline_copy.len(), buffered_sample_count(&offline_copy)
                    );
                }

                // Offline decoding can be CPU-heavy, so the final utterance pass
                // runs on a blocking worker and then emits one final segment.
                tauri::async_runtime::spawn_blocking(move || {
                    if let RecognizerInner::Offline(safe_r) = &recognizer_copy.inner {
                        run_offline_inference(
                            &offline_copy,
                            &app_copy,
                            &safe_r.0,
                            punct_copy.as_deref(),
                            &seg_id_copy,
                            global_start,
                            true,
                            &instance_id_copy,
                            "flush_offline",
                            first_segment_emitted,
                            normalization_options,
                        );
                    }
                })
                .await
                .map_err(|e| e.to_string())?;

                instance.offline_state.speech_buffer.clear();
                instance.offline_state.is_speaking = false;
            } else if let Some(label) = diagnostics_instance_label(&instance_id) {
                info!("[Sherpa] {label} flush found no pending offline speech buffer.");
            }
            instance.current_segment_id = None;
            instance.offline_state = OfflineState::default();
            if let Some(label) = diagnostics_instance_label(&instance_id) {
                info!("[Sherpa] flush_recognizer({label}) complete. mode=offline");
            }
            return Ok(());
        }
    }

    if let (Some(recognizer), Some(st)) = (instance.recognizer.as_deref(), instance.stream.as_ref())
    {
        if let RecognizerInner::Online(r) = &recognizer.inner {
            let current_time = instance.total_samples as f64 / 16000.0;

            // Online models need a short tail of silence to finalize the last
            // partial hypothesis before we reset the stream.
            let tail_padding = vec![0.0; (16000.0 * 0.8) as usize];
            debug!("FFI: Calling accept_waveform (Online, tail_padding)");
            st.0.accept_waveform(16000, &tail_padding);
            debug!("FFI: Successfully returned from accept_waveform (Online, tail_padding)");
            while r.0.is_ready(&st.0) {
                r.0.decode(&st.0);
            }

            if let Some(result) = r.0.get_result(&st.0) {
                if !result.text.trim().is_empty() {
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
                    };
                    let update = build_transcript_update(segment, instance.normalization_options);
                    emit_transcript_update(
                        &app,
                        &instance_id,
                        &update,
                        "flush_online",
                        Some(&instance.record_diagnostics.first_segment_emitted),
                    );
                }
            }

            instance.current_segment_id = None;
            r.0.reset(&st.0);
            instance.segment_start_time = current_time;
            if let Some(label) = diagnostics_instance_label(&instance_id) {
                info!("[Sherpa] flush_recognizer({label}) complete. mode=online");
            }
        }
    }

    Ok(())
}

pub async fn feed_audio_samples<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &SherpaState,
    instance_id: &str,
    samples: &[f32],
) -> Result<(), String> {
    let mut instances = state.instances.lock().await;
    let instance = instances.get_mut(instance_id).ok_or("Instance not found")?;

    if !instance.is_running {
        if let Some(label) = diagnostics_instance_label(instance_id) {
            if !instance.record_diagnostics.skipped_while_stopped_logged {
                println!(
                    "[Sherpa] {label} audio chunk skipped because recognizer is not running. samples={} total_samples={}",
                    samples.len(),
                    instance.total_samples
                );
                instance.record_diagnostics.skipped_while_stopped_logged = true;
            }
        }
        return Ok(());
    }

    if let Some(label) = diagnostics_instance_label(instance_id) {
        if !instance.record_diagnostics.first_sample_logged {
            println!(
                "[Sherpa] {label} first sample received. samples={} total_samples_before={} current_segment={}",
                samples.len(),
                instance.total_samples,
                instance.current_segment_id.as_deref().unwrap_or("none")
            );
            instance.record_diagnostics.first_sample_logged = true;
        }
    }

    let recognizer = instance
        .recognizer
        .clone()
        .ok_or("Recognizer not initialized")?;

    match &recognizer.inner {
        RecognizerInner::Offline(_) => {
            let Some(SafeVad(vad)) = instance.vad.as_ref() else {
                println!(
                    "[Sherpa] feed_audio_samples: VAD model is missing for instance {}",
                    instance_id
                );
                return Err("VAD model is missing or not configured. This model requires VAD for live transcription. Please download the Silero VAD model in Settings -> Model Center.".to_string());
            };

            // Offline live transcription is VAD-driven: we keep feeding audio to
            // the detector, grow/trim utterance buffers, and only run full
            // recognizer inference when a speech segment boundary is reached.
            vad.accept_waveform(samples);
            let currently_speaking = vad.detected();

            if let Some(label) = diagnostics_instance_label(instance_id) {
                if instance.total_samples % 160000 < 2000 {
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
                    let app_copy = app.clone();
                    let punct_copy = instance.punctuation.clone();
                    let seg_id_copy = seg_id.clone();
                    let instance_id_copy = instance_id.to_string();
                    let recognizer_copy = recognizer.clone();
                    let first_segment_emitted = diagnostics_instance_label(instance_id)
                        .is_some()
                        .then(|| instance.record_diagnostics.first_segment_emitted.clone());
                    let normalization_options = instance.normalization_options;

                    if let Some(label) = diagnostics_instance_label(instance_id) {
                        println!(
                            "[Sherpa] {label} triggering offline inference. stage=partial segment_id={} buffered_chunks={} buffered_samples={} global_start={:.3}",
                            seg_id,
                            offline_copy.len(),
                            buffered_sample_count(&offline_copy),
                            global_start
                        );
                    }

                    tauri::async_runtime::spawn_blocking(move || {
                        if let RecognizerInner::Offline(safe_r) = &recognizer_copy.inner {
                            run_offline_inference(
                                &offline_copy,
                                &app_copy,
                                &safe_r.0,
                                punct_copy.as_deref(),
                                &seg_id_copy,
                                global_start,
                                false,
                                &instance_id_copy,
                                "partial",
                                first_segment_emitted,
                                normalization_options,
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
                    let app_copy = app.clone();
                    let punct_copy = instance.punctuation.clone();
                    let seg_id_copy = seg_id.clone();
                    let instance_id_copy = instance_id.to_string();
                    let recognizer_copy = recognizer.clone();
                    let first_segment_emitted = diagnostics_instance_label(instance_id)
                        .is_some()
                        .then(|| instance.record_diagnostics.first_segment_emitted.clone());
                    let normalization_options = instance.normalization_options;

                    if let Some(label) = diagnostics_instance_label(instance_id) {
                        println!(
                            "[Sherpa] {label} triggering offline inference. stage=final segment_id={} buffered_chunks={} buffered_samples={} global_start={:.3}",
                            seg_id,
                            offline_copy.len(),
                            buffered_sample_count(&offline_copy),
                            global_start
                        );
                    }

                    tauri::async_runtime::spawn_blocking(move || {
                        if let RecognizerInner::Offline(safe_r) = &recognizer_copy.inner {
                            run_offline_inference(
                                &offline_copy,
                                &app_copy,
                                &safe_r.0,
                                punct_copy.as_deref(),
                                &seg_id_copy,
                                global_start,
                                true,
                                &instance_id_copy,
                                "final",
                                first_segment_emitted,
                                normalization_options,
                            );
                        }
                    });

                    instance.offline_state.speech_buffer.clear();
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
            let st = instance
                .stream
                .as_ref()
                .ok_or("Stream not initialized for online model")?;

            st.0.accept_waveform(16000, samples);
            instance.total_samples += samples.len();

            while r.0.is_ready(&st.0) {
                r.0.decode(&st.0);
            }

            let current_time = instance.total_samples as f64 / 16000.0;

            if let Some(result) = r.0.get_result(&st.0) {
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
                    };
                    let update = build_transcript_update(segment, instance.normalization_options);
                    emit_transcript_update(
                        app,
                        instance_id,
                        &update,
                        "online_partial",
                        Some(&instance.record_diagnostics.first_segment_emitted),
                    );
                }
            }

            if r.0.is_endpoint(&st.0) {
                let tail_padding = vec![0.0; (16000.0 * 0.8) as usize];
                debug!("FFI: Calling accept_waveform (Online, tail_padding)");
                st.0.accept_waveform(16000, &tail_padding);
                debug!("FFI: Successfully returned from accept_waveform (Online, tail_padding)");
                while r.0.is_ready(&st.0) {
                    r.0.decode(&st.0);
                }

                if let Some(result) = r.0.get_result(&st.0) {
                    if !result.text.trim().is_empty() {
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
                        };
                        let update =
                            build_transcript_update(segment, instance.normalization_options);
                        emit_transcript_update(
                            app,
                            instance_id,
                            &update,
                            "online_final",
                            Some(&instance.record_diagnostics.first_segment_emitted),
                        );
                    }
                }

                instance.current_segment_id = None;
                r.0.reset(&st.0);
                instance.segment_start_time = current_time;
            }

            Ok(())
        }
    }
}

pub async fn feed_audio_chunk_impl<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, SherpaState>,
    instance_id: String,
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
    feed_audio_samples(&app, &state, &instance_id, &float_samples).await
}
