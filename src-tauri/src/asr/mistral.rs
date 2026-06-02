use super::error::SherpaError;
use super::metrics::{
    AsrInferenceMetric, current_time_millis, duration_to_ms, log_inference_metric,
    set_batch_inference_metric,
};
use super::state::AsrState;
use super::traits::{AsrBatchProcessor, AsrProviderAdapter, AsrStreamingSession};
use super::types::{AsrMode, AsrTranscriptionRequest, TranscriptSegment};
use crate::asr::postprocess::TranscriptPostprocessor;
use crate::asr::transcript::apply_timeline_normalization;
use async_trait::async_trait;
use reqwest::multipart;
use serde_json::Value;
use std::time::Instant;
use tauri::AppHandle;
use tauri::Emitter;

#[derive(Debug)]
pub enum MistralMode {
    Batch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MistralVoxtralConfigFields {
    pub api_key: String,
    pub batch_endpoint: String,
    pub model: String,
}

pub struct MistralVoxtralAdapter;

#[async_trait]
impl AsrProviderAdapter for MistralVoxtralAdapter {
    fn provider_id(&self) -> &'static str {
        crate::asr_providers::MISTRAL_VOXTRAL_PROVIDER_ID
    }

    fn create_batch_processor(
        &self,
        _request: &AsrTranscriptionRequest,
    ) -> Result<Option<std::sync::Arc<dyn AsrBatchProcessor>>, SherpaError> {
        Ok(Some(std::sync::Arc::new(MistralVoxtralBatchProcessor)))
    }

    async fn create_streaming_session(
        &self,
        _state: &AsrState,
        _instance_id: &str,
        _request: &AsrTranscriptionRequest,
    ) -> Result<Option<std::sync::Arc<dyn AsrStreamingSession>>, SherpaError> {
        Ok(None)
    }
}

pub struct MistralVoxtralBatchProcessor;

#[async_trait]
impl AsrBatchProcessor for MistralVoxtralBatchProcessor {
    async fn process_file(
        &self,
        app: AppHandle,
        state: &AsrState,
        file_path: String,
        _save_to_path: Option<String>,
        request: AsrTranscriptionRequest,
        _speaker_processing: Option<crate::speaker::SpeakerProcessingConfig>,
    ) -> Result<Vec<TranscriptSegment>, SherpaError> {
        process_batch_file_impl(app, state, file_path, request)
            .await
            .map_err(SherpaError::Generic)
    }
}

fn config_from_request(
    request: &AsrTranscriptionRequest,
    _mode: MistralMode,
) -> Result<MistralVoxtralConfigFields, String> {
    let provider_request = if let crate::asr::types::AsrEngineConfig::Online { provider } = &request.engine_config {
        provider
    } else {
        return Err("Online ASR provider request is missing for Mistral Voxtral.".to_string());
    };

    // Type safety & Deserialization: safely fallback to defaults from manifest if missing or wrong type
    let get_string = |key: &str, default_val: &str| -> String {
        provider_request
            .config
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| default_val.to_string())
    };

    let manifest = crate::asr_providers::find_online_asr_provider(
        crate::asr_providers::MISTRAL_VOXTRAL_PROVIDER_ID,
    )
    .ok_or_else(|| "Mistral Voxtral provider not found in manifest".to_string())?;
    let defaults = manifest.defaults.as_object().unwrap();

    let fields = MistralVoxtralConfigFields {
        api_key: get_string(
            "apiKey",
            defaults.get("apiKey").and_then(Value::as_str).unwrap_or(""),
        ),
        batch_endpoint: get_string(
            "batchEndpoint",
            defaults
                .get("batchEndpoint")
                .and_then(Value::as_str)
                .unwrap_or(""),
        ),
        model: get_string(
            "model",
            defaults.get("model").and_then(Value::as_str).unwrap_or(""),
        ),
    };

    if fields.api_key.is_empty() {
        return Err("Mistral API Key is not configured.".to_string());
    }
    if fields.batch_endpoint.is_empty() || fields.model.is_empty() {
        return Err("Mistral batch endpoint or model is not configured.".to_string());
    }

    Ok(fields)
}

pub async fn process_batch_file_impl<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: &AsrState,
    file_path: String,
    request: AsrTranscriptionRequest,
) -> Result<Vec<TranscriptSegment>, String> {
    if request.mode != AsrMode::Offline {
        return Err("Mistral Voxtral API can only be used in offline/batch mode.".to_string());
    }

    let config = config_from_request(&request, MistralMode::Batch)?;
    let started = Instant::now();

    let bytes = tokio::fs::read(&file_path)
        .await
        .map_err(|error| format!("Failed to read audio file: {error}"))?;

    let file_name = std::path::Path::new(&file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("audio.wav")
        .to_string();

    let part = multipart::Part::bytes(bytes.clone())
        .file_name(file_name)
        .mime_str("audio/wav")
        .map_err(|e| format!("Failed to create multipart file: {}", e))?;

    let form = multipart::Form::new()
        .part("file", part)
        .text("model", config.model.clone())
        .text("response_format", "verbose_json");

    let client = reqwest::Client::new();
    let response = client
        .post(&config.batch_endpoint)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .multipart(form)
        .send()
        .await
        .map_err(|error| format!("Mistral Voxtral network request failed: {error}"))?;

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(format!(
            "Mistral Voxtral API returned error status {}: {}",
            status, text
        ));
    }

    let response_value = response
        .json::<Value>()
        .await
        .map_err(|error| format!("Mistral Voxtral response parsing failed: {error}"))?;

    let mut segments = segments_from_mistral_response(&response_value)?;

    segments = apply_timeline_normalization(segments, request.normalization_options);
    segments =
        TranscriptPostprocessor::compile(request.postprocess_options)?.process_segments(segments);

    let metric = AsrInferenceMetric {
        occurred_at_ms: current_time_millis(),
        source: "batch".to_string(),
        instance_id: None,
        stage: "mistral_batch_complete".to_string(),
        is_final: true,
        audio_duration_ms: response_value
            .get("duration")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            * 1000.0,
        buffered_samples: bytes.len() / 2,
        audio_extract_ms: None,
        decode_ms: duration_to_ms(started.elapsed()),
        emit_latency_ms: None,
        total_ms: Some(duration_to_ms(started.elapsed())),
        rtf: None,
        segment_count: Some(segments.len()),
        process_rss_mb: None,
    };
    set_batch_inference_metric(&state.metrics, metric.clone());
    log_inference_metric(&metric);

    let _ = app.emit(
        crate::asr::BATCH_PROGRESS_EVENT,
        &(file_path.as_str(), 100.0_f32),
    );

    Ok(segments)
}

fn segments_from_mistral_response(response: &Value) -> Result<Vec<TranscriptSegment>, String> {
    let mut segments = Vec::new();
    let segments_array = response
        .get("segments")
        .and_then(Value::as_array)
        .ok_or_else(|| "Mistral Voxtral response is missing 'segments' array.".to_string())?;

    for segment in segments_array {
        let text = segment
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let start = segment.get("start").and_then(Value::as_f64).unwrap_or(0.0);
        let end = segment.get("end").and_then(Value::as_f64).unwrap_or(0.0);

        segments.push(TranscriptSegment {
            id: uuid::Uuid::new_v4().to_string(),
            text: text.trim().to_string(),
            start,
            end,
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            speaker: None,
            speaker_attribution: None,
        });
    }

    Ok(segments)
}
