use super::metrics::{
    current_time_millis, duration_to_ms, log_inference_metric, set_batch_inference_metric,
    AsrInferenceMetric,
};
use super::error::SherpaError;
use super::online_traits::{OnlineAsrProviderAdapter, OnlineBatchProcessor, OnlineStreamingSession};
use super::state::SherpaState;
use super::types::{AsrMode, AsrTranscriptionRequest, TranscriptSegment};
use crate::asr_providers::{fill_mistral_voxtral_config_fields, MistralVoxtralConfigFields};
use async_trait::async_trait;
use crate::sherpa::postprocess::TranscriptPostprocessor;
use crate::sherpa::transcript::apply_timeline_normalization;
use reqwest::multipart;
use serde_json::Value;
use std::time::Instant;
use tauri::AppHandle;
use tauri::Emitter;

#[derive(Debug)]
pub enum MistralMode {
    Batch,
}

pub struct MistralVoxtralAdapter;

impl OnlineAsrProviderAdapter for MistralVoxtralAdapter {
    fn provider_id(&self) -> &'static str {
        crate::asr_providers::MISTRAL_VOXTRAL_PROVIDER_ID
    }

    fn create_batch_processor(
        &self,
        _config: &Value,
    ) -> Result<Option<Box<dyn OnlineBatchProcessor>>, SherpaError> {
        Ok(Some(Box::new(MistralVoxtralBatchProcessor)))
    }

    fn create_streaming_session(
        &self,
        _config: &Value,
        _request: &AsrTranscriptionRequest,
    ) -> Result<Option<Box<dyn OnlineStreamingSession>>, SherpaError> {
        Ok(None)
    }
}

pub struct MistralVoxtralBatchProcessor;

#[async_trait]
impl OnlineBatchProcessor for MistralVoxtralBatchProcessor {
    async fn process_file(
        &self,
        app: AppHandle,
        state: &SherpaState,
        file_path: String,
        request: AsrTranscriptionRequest,
    ) -> Result<Vec<TranscriptSegment>, SherpaError> {
        process_batch_file_impl(app, state, file_path, request).await.map_err(SherpaError::Generic)
    }
}

fn config_from_request(
    request: &AsrTranscriptionRequest,
    _mode: MistralMode,
) -> Result<MistralVoxtralConfigFields, String> {
    let provider_request = request
        .online_provider
        .as_ref()
        .ok_or_else(|| "Online ASR provider request is missing for Mistral Voxtral.".to_string())?;

    let fields = fill_mistral_voxtral_config_fields(
        provider_request
            .config
            .get("apiKey")
            .and_then(Value::as_str),
        provider_request
            .config
            .get("batchEndpoint")
            .and_then(Value::as_str),
        provider_request.config.get("model").and_then(Value::as_str),
    );

    if fields.api_key.trim().is_empty() {
        return Err("Mistral API Key is not configured.".to_string());
    }
    if fields.batch_endpoint.trim().is_empty() || fields.model.trim().is_empty() {
        return Err("Mistral batch endpoint or model is not configured.".to_string());
    }

    Ok(fields)
}

pub async fn process_batch_file_impl<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: &SherpaState,
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
        crate::sherpa::BATCH_PROGRESS_EVENT,
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
