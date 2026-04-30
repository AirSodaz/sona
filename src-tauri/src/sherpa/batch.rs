use super::model_config::ModelFileConfig;
use super::model_config::{
    build_model_config, load_punctuation, Punctuation, Recognizer, RecognizerInner,
    SafeOfflineRecognizer, SafeOnlineRecognizer, SafeStream,
};
use super::transcript::{apply_timeline_normalization, format_transcript, synthesize_durations};
use super::types::{BatchTranscriptionRequest, TranscriptNormalizationOptions, TranscriptSegment};
use super::BATCH_PROGRESS_EVENT;
use log::debug;
use std::path::Path;
use tauri::{AppHandle, Emitter};

pub async fn process_batch_file_impl<R: tauri::Runtime>(
    app: AppHandle<R>,
    file_path: String,
    save_to_path: Option<String>,
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
    speaker_processing: Option<crate::speaker::SpeakerProcessingConfig>,
    normalization_options: Option<TranscriptNormalizationOptions>,
) -> Result<Vec<TranscriptSegment>, String> {
    let request = BatchTranscriptionRequest {
        file_path,
        save_to_path,
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
        speaker_processing,
        normalization_options: normalization_options.unwrap_or_default(),
    };
    let progress_file_path = request.file_path.clone();

    transcribe_batch_with_progress(&request, |progress| {
        let _ = app.emit(
            BATCH_PROGRESS_EVENT,
            &(progress_file_path.as_str(), progress),
        );
    })
    .await
}

pub async fn transcribe_batch_with_progress<F>(
    request: &BatchTranscriptionRequest,
    mut on_progress: F,
) -> Result<Vec<TranscriptSegment>, String>
where
    F: FnMut(f32),
{
    let samples = crate::pipeline::extract_and_resample_audio(&request.file_path, 16000).await?;

    if let Some(path) = request.save_to_path.as_ref() {
        crate::pipeline::save_wav_file(&samples, 16000, path).map_err(|e| e.to_string())?;
    }

    let config_type = build_model_config(
        Path::new(&request.model_path),
        &request.model_type,
        &request.file_config,
        request.enable_itn,
        &request.language,
        request.hotwords.clone(),
    )?;
    let recognizer = Recognizer::new(config_type, request.num_threads)?;
    let punctuation = load_punctuation(request.punctuation_model.clone());

    let segments = match &recognizer.inner {
        RecognizerInner::Offline(r) => {
            process_batch_offline(
                r,
                &samples,
                request.vad_model.clone(),
                request.vad_buffer,
                punctuation.as_ref(),
                &mut on_progress,
            )
            .await?
        }
        RecognizerInner::Online(r) => {
            process_batch_online(r, &samples, punctuation.as_ref(), &mut on_progress).await?
        }
    };

    let annotated_segments = crate::speaker::annotate_segments_with_speakers(
        &samples,
        &segments,
        request.speaker_processing.as_ref(),
    )?;

    Ok(apply_timeline_normalization(
        annotated_segments,
        request.normalization_options,
    ))
}

async fn process_batch_offline<F>(
    r: &SafeOfflineRecognizer,
    samples: &[f32],
    vad_model: Option<String>,
    vad_buffer: f32,
    punctuation: Option<&Punctuation>,
    on_progress: &mut F,
) -> Result<Vec<TranscriptSegment>, String>
where
    F: FnMut(f32),
{
    let segments = if let Some(v_path) = vad_model {
        if !v_path.is_empty() && Path::new(&v_path).exists() {
            let silero_vad = sherpa_onnx::SileroVadModelConfig {
                model: Some(v_path),
                threshold: 0.35,
                min_silence_duration: 1.0,
                min_speech_duration: 0.25,
                window_size: 512,
                ..Default::default()
            };

            let vad_config = sherpa_onnx::VadModelConfig {
                silero_vad,
                sample_rate: 16000,
                num_threads: 1,
                ..Default::default()
            };

            crate::pipeline::vad_segment_audio(samples, 16000, &vad_config, vad_buffer)
                .unwrap_or_else(|_| crate::pipeline::fixed_chunk_audio(samples, 16000, 30.0))
        } else {
            crate::pipeline::fixed_chunk_audio(samples, 16000, 30.0)
        }
    } else {
        crate::pipeline::fixed_chunk_audio(samples, 16000, 30.0)
    };

    let mut results = Vec::new();
    let total_segments = segments.len();
    if total_segments == 0 {
        on_progress(100.0);
        return Ok(results);
    }

    for (i, seg) in segments.into_iter().enumerate() {
        {
            let stream = r.0.create_stream();
            debug!("FFI: Calling accept_waveform (Offline segment)");
            stream.accept_waveform(16000, &seg.samples);
            debug!("FFI: Successfully returned from accept_waveform (Offline segment)");
            r.0.decode(&stream);

            if let Some(res) = stream.get_result() {
                if !res.text.trim().is_empty() {
                    let text = format_transcript(&res.text, punctuation);
                    let timestamps_abs = res
                        .timestamps
                        .as_ref()
                        .map(|ts| ts.iter().map(|t| *t + seg.start_time).collect::<Vec<_>>());
                    let durations = timestamps_abs
                        .as_ref()
                        .and_then(|ts| synthesize_durations(ts, seg.start_time + seg.duration));

                    results.push(TranscriptSegment {
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
                    });
                }
            }
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
) -> Result<Vec<TranscriptSegment>, String>
where
    F: FnMut(f32),
{
    let stream = SafeStream(r.0.create_stream());
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
        stream.0.accept_waveform(16000, chunk);
        current_samples += chunk.len();
        while r.0.is_ready(&stream.0) {
            r.0.decode(&stream.0);
        }
        if r.0.is_endpoint(&stream.0) {
            let current_time = current_samples as f64 / 16000.0;
            if let Some(result) = r.0.get_result(&stream.0) {
                if !result.text.trim().is_empty() {
                    let text = format_transcript(&result.text, punctuation);
                    let timestamps_abs = result.timestamps.as_ref().map(|ts| {
                        ts.iter()
                            .map(|t| *t + segment_start as f32)
                            .collect::<Vec<_>>()
                    });
                    let durations = timestamps_abs
                        .as_ref()
                        .and_then(|ts| synthesize_durations(ts, current_time as f32));

                    segments.push(TranscriptSegment {
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
                    });
                }
            }
            r.0.reset(&stream.0);
            segment_start = current_time;
        }
        let progress = (current_samples as f32 / total_samples as f32) * 100.0;
        on_progress(progress);
        tokio::task::yield_now().await;
    }

    let tail_padding = vec![0.0; (16000.0 * 0.8) as usize];
    stream.0.accept_waveform(16000, &tail_padding);
    while r.0.is_ready(&stream.0) {
        r.0.decode(&stream.0);
    }

    if let Some(result) = r.0.get_result(&stream.0) {
        if !result.text.trim().is_empty() {
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

            segments.push(TranscriptSegment {
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
            });
        }
    }
    on_progress(100.0);
    Ok(segments)
}
