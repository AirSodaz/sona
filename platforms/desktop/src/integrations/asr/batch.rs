use super::AsrState;
use super::BATCH_PROGRESS_EVENT;
use super::metrics::{
    AsrInferenceMetric, AsrMetricsStore, AsrModelLoadMetric, calculate_rss_delta_mb, calculate_rtf,
    capture_process_memory_mb, current_time_millis, duration_to_ms, log_inference_metric,
    log_model_load_metric, samples_to_ms, set_batch_inference_metric, set_model_load_metric,
};
use super::transcript::{
    apply_timeline_normalization, build_transcript_update, emit_transcript_update,
    format_transcript, synthesize_durations,
};
use super::types::{
    BatchSegmentationMode, BatchTranscriptionRequest, TranscriptNormalizationOptions,
    TranscriptSegment,
};
use sona_local_asr::punctuation::{Punctuation, load_punctuation};
use sona_local_asr::recognizer::{
    Recognizer, SafeOfflineRecognizer, SafeOnlineRecognizer, accept_online_samples,
    build_model_config, create_online_stream, create_recognizer_with_gpu_plan,
    decode_offline_samples, decode_online_ready, is_online_endpoint, online_stream_result,
    reset_online_stream,
};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

pub async fn process_batch_request_impl(
    emitter: std::sync::Arc<dyn crate::platform::event::EventEmitter>,
    state: &AsrState,
    request: BatchTranscriptionRequest,
) -> Result<Vec<TranscriptSegment>, String> {
    let progress_file_path = request.file_path.clone();

    transcribe_batch_with_progress_and_metrics(
        &request,
        |progress| {
            let _ = emitter.emit(
                BATCH_PROGRESS_EVENT,
                serde_json::json!([progress_file_path.to_string_lossy().as_ref(), progress]),
            );
        },
        Some(state.metrics.clone()),
        Some(emitter.as_ref()),
    )
    .await
}

pub async fn transcribe_batch_with_progress<F>(
    request: &BatchTranscriptionRequest,
    mut on_progress: F,
) -> Result<Vec<TranscriptSegment>, String>
where
    F: FnMut(f32),
{
    transcribe_batch_with_progress_and_metrics_inner(
        request,
        None,
        &mut on_progress,
        None,
        |_| {},
        None,
    )
    .await
}

pub(crate) async fn transcribe_batch_with_progress_and_metrics<F>(
    request: &BatchTranscriptionRequest,
    mut on_progress: F,
    metrics_store: Option<AsrMetricsStore>,
    emitter: Option<&dyn crate::platform::event::EventEmitter>,
) -> Result<Vec<TranscriptSegment>, String>
where
    F: FnMut(f32),
{
    transcribe_batch_with_progress_and_metrics_inner(
        request,
        None,
        &mut on_progress,
        metrics_store,
        |_| {},
        emitter,
    )
    .await
}

async fn transcribe_batch_with_progress_and_metrics_inner<F, N>(
    request: &BatchTranscriptionRequest,
    preloaded_recognizer: Option<Arc<Recognizer>>,
    mut on_progress: F,
    metrics_store: Option<AsrMetricsStore>,
    mut on_fallback: N,
    emitter: Option<&dyn crate::platform::event::EventEmitter>,
) -> Result<Vec<TranscriptSegment>, String>
where
    F: FnMut(f32),
    N: FnMut(&crate::platform::hardware::GpuFallbackNotice),
{
    let total_started = Instant::now();
    let audio_extract_started = Instant::now();
    let samples =
        sona_local_asr::audio::extract_and_resample_audio(&request.file_path, 16000).await?;
    let audio_extract_ms = duration_to_ms(audio_extract_started.elapsed());

    if let Some(path) = request.save_to_path.as_ref() {
        sona_local_asr::audio::save_wav_file(&samples, 16000, path).map_err(|e| e.to_string())?;
    }

    let model_load_started = Instant::now();
    let rss_before_mb = capture_process_memory_mb();

    let (recognizer, reused_from_pool, model_load_ms) = if let Some(r) = preloaded_recognizer {
        (r, true, 0.0)
    } else {
        let config_type = build_model_config(
            Path::new(&request.model_path),
            &request.model_type,
            &request.file_config,
            request.enable_itn,
            &request.language,
            request.hotwords.clone(),
        )?;
        let gpu_plan = crate::platform::hardware::resolve_gpu_acceleration_plan(
            request.gpu_acceleration.as_deref(),
        )
        .await;
        let recognizer_result =
            create_recognizer_with_gpu_plan(config_type, request.num_threads, gpu_plan)?;
        if let Some(notice) = recognizer_result.fallback_notice.as_ref() {
            log::warn!(
                "[batch] {} transcription failed, retrying with {}: {}",
                notice.from_provider,
                notice.to_provider,
                notice.error
            );
            on_fallback(notice);
        }
        (
            Arc::new(recognizer_result.recognizer),
            false,
            duration_to_ms(model_load_started.elapsed()),
        )
    };

    let rss_after_mb = capture_process_memory_mb();

    if let Some(metrics_store) = metrics_store.as_ref() {
        let metric = AsrModelLoadMetric {
            occurred_at_ms: current_time_millis(),
            instance_id: "batch".to_string(),
            model_path: request.model_path.clone(),
            model_type: request.model_type.clone(),
            recognizer_kind: recognizer.kind_label().to_string(),
            num_threads: request.num_threads,
            reused_from_pool,
            load_ms: model_load_ms,
            rss_before_mb,
            rss_after_mb,
            rss_delta_mb: calculate_rss_delta_mb(rss_before_mb, rss_after_mb, false),
            process_rss_mb: rss_after_mb,
        };
        set_model_load_metric(metrics_store, metric.clone());
        log_model_load_metric(&metric);
    }

    let punctuation = load_punctuation(request.punctuation_model.clone());

    let instance_id = request.instance_id.as_deref();

    let decode_started = Instant::now();
    let segments = if let Some(r) = recognizer.offline() {
        process_batch_local(
            r,
            &samples,
            request.vad_model.clone(),
            request.vad_buffer,
            request.batch_segmentation_mode,
            punctuation.as_ref(),
            &mut on_progress,
            request.normalization_options,
            emitter,
            instance_id,
        )
        .await?
    } else if let Some(r) = recognizer.online() {
        process_batch_online(
            r,
            &samples,
            punctuation.as_ref(),
            &mut on_progress,
            request.normalization_options,
            emitter,
            instance_id,
        )
        .await?
    } else {
        return Err("Unsupported recognizer type".to_string());
    };
    let decode_ms = duration_to_ms(decode_started.elapsed());

    let annotated_segments = sona_local_asr::speaker_processing::annotate_segments_with_speakers(
        &samples,
        &segments,
        request.speaker_processing.as_ref(),
    )?;

    let normalized_segments =
        apply_timeline_normalization(annotated_segments, request.normalization_options);
    let postprocessed_segments = request.postprocessor.process_segments(normalized_segments);

    if let Some(metrics_store) = metrics_store.as_ref() {
        let audio_duration_ms = samples_to_ms(samples.len(), 16000.0);
        let metric = AsrInferenceMetric {
            occurred_at_ms: current_time_millis(),
            source: "batch".to_string(),
            instance_id: None,
            stage: "batch_complete".to_string(),
            is_final: true,
            audio_duration_ms,
            buffered_samples: samples.len(),
            audio_extract_ms: Some(audio_extract_ms),
            decode_ms,
            emit_latency_ms: None,
            total_ms: Some(duration_to_ms(total_started.elapsed())),
            rtf: calculate_rtf(decode_ms, audio_duration_ms),
            segment_count: Some(postprocessed_segments.len()),
            process_rss_mb: capture_process_memory_mb(),
        };
        set_batch_inference_metric(metrics_store, metric.clone());
        log_inference_metric(&metric);
    }

    Ok(postprocessed_segments)
}

#[allow(clippy::too_many_arguments)]
async fn process_batch_local<F>(
    r: &SafeOfflineRecognizer,
    samples: &[f32],
    vad_model: Option<String>,
    vad_buffer: f32,
    batch_segmentation_mode: BatchSegmentationMode,
    punctuation: Option<&Punctuation>,
    on_progress: &mut F,
    normalization_options: TranscriptNormalizationOptions,
    emitter: Option<&dyn crate::platform::event::EventEmitter>,
    instance_id: Option<&str>,
) -> Result<Vec<TranscriptSegment>, String>
where
    F: FnMut(f32),
{
    let segments = sona_local_asr::audio::segment_batch_audio(
        samples,
        vad_model.as_deref().map(Path::new),
        vad_buffer,
        batch_segmentation_mode,
    );

    let mut results = Vec::new();
    let total_segments = segments.len();
    if total_segments == 0 {
        on_progress(100.0);
        return Ok(results);
    }

    for (i, seg) in segments.into_iter().enumerate() {
        if let Some(res) = decode_offline_samples(r, &seg.samples)
            && !res.text.trim().is_empty()
        {
            let text = format_transcript(&res.text, punctuation);
            let timestamps_abs = res
                .timestamps
                .as_ref()
                .map(|ts| ts.iter().map(|t| *t + seg.start_time).collect::<Vec<_>>());
            let durations = timestamps_abs
                .as_ref()
                .and_then(|ts| synthesize_durations(ts, seg.start_time + seg.duration));

            let segment = TranscriptSegment {
                id: uuid::Uuid::new_v4().to_string(),
                text,
                start: seg.start_time as f64,
                end: (seg.start_time + seg.duration) as f64,
                is_final: true,
                timing: None,
                tokens: Some(res.tokens),
                timestamps: timestamps_abs,
                durations,
                translation: None,
                speaker: None,
                speaker_attribution: None,
            };

            if let (Some(emitter), Some(id)) = (emitter, instance_id) {
                let update = build_transcript_update(segment.clone(), normalization_options);
                emit_transcript_update(emitter, id, &update, "batch-local", None);
            }

            results.push(segment);
        }
        let progress = ((i + 1) as f32 / total_segments as f32) * 100.0;
        on_progress(progress);
        tokio::task::yield_now().await;
    }
    Ok(results)
}

async fn process_batch_online<F>(
    r: &SafeOnlineRecognizer,
    samples: &[f32],
    punctuation: Option<&Punctuation>,
    on_progress: &mut F,
    normalization_options: TranscriptNormalizationOptions,
    emitter: Option<&dyn crate::platform::event::EventEmitter>,
    instance_id: Option<&str>,
) -> Result<Vec<TranscriptSegment>, String>
where
    F: FnMut(f32),
{
    let stream = create_online_stream(r);
    let mut segments = Vec::new();
    let mut segment_start = 0.0;
    let mut current_samples = 0;

    let chunk_size = 8000;
    let total_samples = samples.len();
    if total_samples == 0 {
        on_progress(100.0);
        return Ok(segments);
    }

    for chunk in samples.chunks(chunk_size) {
        accept_online_samples(&stream, chunk);
        current_samples += chunk.len();
        decode_online_ready(r, &stream);
        if is_online_endpoint(r, &stream) {
            let current_time = current_samples as f64 / 16000.0;
            if let Some(result) = online_stream_result(r, &stream)
                && !result.text.trim().is_empty()
            {
                let text = format_transcript(&result.text, punctuation);
                let timestamps_abs = result.timestamps.as_ref().map(|ts| {
                    ts.iter()
                        .map(|t| *t + segment_start as f32)
                        .collect::<Vec<_>>()
                });
                let durations = timestamps_abs
                    .as_ref()
                    .and_then(|ts| synthesize_durations(ts, current_time as f32));

                let segment = TranscriptSegment {
                    id: uuid::Uuid::new_v4().to_string(),
                    text,
                    start: segment_start,
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

                if let (Some(emitter), Some(id)) = (emitter, instance_id) {
                    let update = build_transcript_update(segment.clone(), normalization_options);
                    emit_transcript_update(emitter, id, &update, "batch-online", None);
                }

                segments.push(segment);
            }
            reset_online_stream(r, &stream);
            segment_start = current_time;
        }
        let progress = (current_samples as f32 / total_samples as f32) * 100.0;
        on_progress(progress);
        tokio::task::yield_now().await;
    }

    let tail_padding = vec![0.0; (16000.0 * 0.8) as usize];
    accept_online_samples(&stream, &tail_padding);
    decode_online_ready(r, &stream);

    if let Some(result) = online_stream_result(r, &stream)
        && !result.text.trim().is_empty()
    {
        let text = format_transcript(&result.text, punctuation);
        let current_time = samples.len() as f64 / 16000.0;
        let timestamps_abs = result.timestamps.as_ref().map(|ts| {
            ts.iter()
                .map(|t| *t + segment_start as f32)
                .collect::<Vec<_>>()
        });
        let durations = timestamps_abs
            .as_ref()
            .and_then(|ts| synthesize_durations(ts, current_time as f32));

        let segment = TranscriptSegment {
            id: uuid::Uuid::new_v4().to_string(),
            text,
            start: segment_start,
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

        if let (Some(emitter), Some(id)) = (emitter, instance_id) {
            let update = build_transcript_update(segment.clone(), normalization_options);
            emit_transcript_update(emitter, id, &update, "batch-online-tail", None);
        }

        segments.push(segment);
    }
    on_progress(100.0);
    Ok(segments)
}
