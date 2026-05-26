use log::trace;
use tauri::{AppHandle, State};

mod adapter;
mod batch;
mod metrics;
mod model_config;
mod online;
mod postprocess;
mod runtime;
mod state;
mod transcript;
mod types;
mod volcengine;

const BATCH_PROGRESS_EVENT: &str = "batch-progress";

fn recognizer_output_event(instance_id: &str) -> String {
    format!("recognizer-output-{instance_id}")
}

pub use adapter::{AsrEngineAdapter, LocalSherpaAdapter};
pub use batch::transcribe_batch_with_progress;
pub use metrics::{AsrInferenceMetric, AsrModelLoadMetric, AsrRuntimeMetricsSnapshot};
pub use model_config::ModelFileConfig;
pub use postprocess::TranscriptPostprocessor;
pub use state::SherpaState;
pub(crate) use transcript::ensure_transcript_segment_timing;
pub use types::{
    AsrEngine, AsrMode, AsrTranscriptionRequest, BatchTranscriptionRequest,
    OnlineAsrProviderRequest, TranscriptNormalizationOptions, TranscriptPostprocessOptions,
    TranscriptSegment, TranscriptTextReplacementRule, TranscriptTextReplacementRuleSet,
    TranscriptTiming, TranscriptTimingLevel, TranscriptTimingSource, TranscriptTimingUnit,
    TranscriptUpdate, VolcengineDoubaoAsrConfig,
};

async fn route_engine(state: &SherpaState, instance_id: &str) -> AsrEngine {
    state
        .instance_engine(instance_id)
        .await
        .unwrap_or(AsrEngine::LocalSherpa)
}

/// Feed f32 audio samples from the hardware capture worker to the correct
/// ASR backend. Routes by the engine selected during `init_recognizer` so an
/// expected cloud recognizer cannot silently fall through to local Sherpa when
/// the cloud session is missing or failed.
pub async fn feed_audio_samples<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &SherpaState,
    instance_id: &str,
    samples: &[f32],
) -> Result<(), String> {
    match route_engine(state, instance_id).await {
        AsrEngine::Online => online::feed_audio_samples_impl(state, instance_id, samples).await,
        AsrEngine::LocalSherpa => {
            runtime::feed_audio_samples(app, state, instance_id, samples).await
        }
    }
}

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
    match request.engine {
        AsrEngine::LocalSherpa => {
            LocalSherpaAdapter::ensure_mode(&request, AsrMode::Streaming)?;
            let init_result = runtime::init_recognizer_impl(
                state.clone(),
                instance_id.clone(),
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
            .await;
            if init_result.is_ok() {
                state
                    .set_instance_engine(&instance_id, AsrEngine::LocalSherpa)
                    .await;
            }
            init_result
        }
        AsrEngine::Online => {
            let init_result =
                online::init_streaming_recognizer_impl(state.clone(), instance_id.clone(), request)
                    .await;
            if init_result.is_ok() {
                state
                    .set_instance_engine(&instance_id, AsrEngine::Online)
                    .await;
            }
            init_result
        }
    }
}

#[tauri::command]
pub async fn start_recognizer<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, SherpaState>,
    instance_id: String,
) -> Result<(), String> {
    match route_engine(&state, &instance_id).await {
        AsrEngine::Online => online::start_streaming_recognizer_impl(app, state, instance_id).await,
        AsrEngine::LocalSherpa => runtime::start_recognizer_impl(state, instance_id).await,
    }
}

#[tauri::command]
pub async fn stop_recognizer(
    state: State<'_, SherpaState>,
    instance_id: String,
) -> Result<(), String> {
    match route_engine(&state, &instance_id).await {
        AsrEngine::Online => online::stop_streaming_recognizer_impl(state, instance_id).await,
        AsrEngine::LocalSherpa => runtime::stop_recognizer_impl(state, instance_id).await,
    }
}

#[tauri::command]
pub async fn flush_recognizer<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, SherpaState>,
    instance_id: String,
) -> Result<(), String> {
    match route_engine(&state, &instance_id).await {
        AsrEngine::Online => online::flush_streaming_recognizer_impl(app, state, instance_id).await,
        AsrEngine::LocalSherpa => runtime::flush_recognizer_impl(app, state, instance_id).await,
    }
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
    match route_engine(&state, &instance_id).await {
        AsrEngine::Online => online::feed_audio_chunk_impl(app, state, instance_id, samples).await,
        AsrEngine::LocalSherpa => {
            runtime::feed_audio_chunk_impl(app, state, instance_id, samples).await
        }
    }
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
    match request.engine {
        AsrEngine::LocalSherpa => {
            let adapter = LocalSherpaAdapter;
            let batch_request =
                adapter.batch_request(file_path, save_to_path, request, speaker_processing)?;
            batch::process_batch_request_impl(app, state.inner(), batch_request).await
        }
        AsrEngine::Online => {
            online::process_batch_file_impl(app, state.inner(), file_path, request).await
        }
    }
}

#[tauri::command]
pub async fn get_asr_runtime_metrics(
    state: State<'_, SherpaState>,
) -> Result<AsrRuntimeMetricsSnapshot, String> {
    Ok(state.metrics_snapshot().await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn route_engine_uses_selected_online_engine_even_without_active_session() {
        let state = SherpaState::new();
        state.set_instance_engine("record", AsrEngine::Online).await;

        assert_eq!(route_engine(&state, "record").await, AsrEngine::Online);
        assert!(!state.has_online_session("record").await);
    }

    #[tokio::test]
    async fn route_engine_defaults_to_local_for_legacy_instances() {
        let state = SherpaState::new();

        assert_eq!(route_engine(&state, "record").await, AsrEngine::LocalSherpa);
    }
}
