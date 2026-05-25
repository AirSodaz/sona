use log::trace;
use tauri::{AppHandle, State};

mod adapter;
mod batch;
mod metrics;
mod model_config;
mod postprocess;
mod runtime;
mod state;
mod transcript;
mod types;

const BATCH_PROGRESS_EVENT: &str = "batch-progress";

fn recognizer_output_event(instance_id: &str) -> String {
    format!("recognizer-output-{instance_id}")
}

pub use adapter::{AsrEngineAdapter, LocalSherpaAdapter};
pub use batch::transcribe_batch_with_progress;
pub use metrics::{AsrInferenceMetric, AsrModelLoadMetric, AsrRuntimeMetricsSnapshot};
pub use model_config::ModelFileConfig;
pub use postprocess::TranscriptPostprocessor;
pub use runtime::feed_audio_samples;
pub use state::SherpaState;
pub(crate) use transcript::ensure_transcript_segment_timing;
pub use types::{
    AsrEngine, AsrMode, AsrTranscriptionRequest, BatchTranscriptionRequest,
    TranscriptNormalizationOptions, TranscriptPostprocessOptions, TranscriptSegment,
    TranscriptTextReplacementRule, TranscriptTextReplacementRuleSet, TranscriptTiming,
    TranscriptTimingLevel, TranscriptTimingSource, TranscriptTimingUnit, TranscriptUpdate,
};

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn init_recognizer(
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
    postprocess_options: Option<TranscriptPostprocessOptions>,
    asr_request: Option<AsrTranscriptionRequest>,
) -> Result<(), String> {
    let request = asr_request.unwrap_or_else(|| {
        AsrTranscriptionRequest::local_sherpa(
            AsrMode::Streaming,
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
            normalization_options.unwrap_or_default(),
            postprocess_options.unwrap_or_default(),
        )
    });
    LocalSherpaAdapter::ensure_mode(&request, AsrMode::Streaming)?;
    runtime::init_recognizer_impl(
        state,
        instance_id,
        request.model_path,
        request.num_threads,
        request.enable_itn,
        request.language,
        request.punctuation_model,
        request.vad_model,
        request.vad_buffer,
        request.model_type,
        request.file_config,
        request.hotwords,
        Some(request.normalization_options),
        Some(request.postprocess_options),
    )
    .await
}

#[tauri::command]
pub async fn start_recognizer(
    state: State<'_, SherpaState>,
    instance_id: String,
) -> Result<(), String> {
    runtime::start_recognizer_impl(state, instance_id).await
}

#[tauri::command]
pub async fn stop_recognizer(
    state: State<'_, SherpaState>,
    instance_id: String,
) -> Result<(), String> {
    runtime::stop_recognizer_impl(state, instance_id).await
}

#[tauri::command]
pub async fn flush_recognizer<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, SherpaState>,
    instance_id: String,
) -> Result<(), String> {
    runtime::flush_recognizer_impl(app, state, instance_id).await
}

#[tauri::command]
pub async fn feed_audio_chunk<R: tauri::Runtime>(
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
    runtime::feed_audio_chunk_impl(app, state, instance_id, samples).await
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn process_batch_file<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, SherpaState>,
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
    postprocess_options: Option<TranscriptPostprocessOptions>,
    asr_request: Option<AsrTranscriptionRequest>,
) -> Result<Vec<TranscriptSegment>, String> {
    let request = asr_request.unwrap_or_else(|| {
        AsrTranscriptionRequest::local_sherpa(
            AsrMode::Offline,
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
            normalization_options.unwrap_or_default(),
            postprocess_options.unwrap_or_default(),
        )
    });
    let adapter = LocalSherpaAdapter;
    let batch_request =
        adapter.batch_request(file_path, save_to_path, request, speaker_processing)?;
    batch::process_batch_request_impl(app, state.inner(), batch_request).await
}

#[tauri::command]
pub async fn get_asr_runtime_metrics(
    state: State<'_, SherpaState>,
) -> Result<AsrRuntimeMetricsSnapshot, String> {
    Ok(state.metrics_snapshot().await)
}
