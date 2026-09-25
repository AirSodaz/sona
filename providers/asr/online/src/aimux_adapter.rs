use std::path::Path;
use std::sync::Arc;

use aimux_core::transcription_model::{AudioInput, TranscriptionCallOptions, TranscriptionModel};
use aimux_providers::openai::{OpenAIConfig, OpenAITranscriptionModel};
use sona_core::ports::asr::{
    AsrMode, AsrPortError, AsrPortErrorKind, AsrTranscriptionRequest, GROQ_WHISPER_PROVIDER_ID,
    MISTRAL_VOXTRAL_PROVIDER_ID, OnlineBatchTranscriptionOutput, OnlineBatchTranscriptionRequest,
    VOLCENGINE_DOUBAO_PROVIDER_ID,
};
use sona_core::transcription::transcript::TranscriptSegment;

use crate::error::map_aimux_asr_error;
use crate::volcengine::VolcengineTranscriptionModel;
use crate::{
    VolcengineMode, WhisperCompatibleProvider, resolve_online_asr_provider_id,
    resolve_volcengine_config, resolve_whisper_config, whisper_language_form_field,
};

/// Detect MIME type based on audio file extension.
pub fn detect_audio_mime_type(file_path: &Path) -> &'static str {
    match file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "wav" => "audio/wav",
        "mp3" => "audio/mp3",
        "ogg" => "audio/ogg",
        "m4a" => "audio/m4a",
        "aac" => "audio/aac",
        "flac" => "audio/flac",
        _ => "audio/wav",
    }
}

/// Create an aimux [`TranscriptionModel`] from a Sona [`AsrTranscriptionRequest`].
pub fn create_aimux_transcription_model(
    request: &AsrTranscriptionRequest,
) -> Result<Arc<dyn TranscriptionModel>, AsrPortError> {
    let provider_id = resolve_online_asr_provider_id(request)?;

    match provider_id {
        GROQ_WHISPER_PROVIDER_ID => {
            if request.mode != AsrMode::Batch {
                return Err(AsrPortError::invalid_request(
                    WhisperCompatibleProvider::GroqWhisper.batch_only_error(),
                ));
            }
            let config = resolve_whisper_config(request, WhisperCompatibleProvider::GroqWhisper)
                .map_err(|error| AsrPortError::invalid_request(error.to_string()))?;
            let base_url = config
                .batch_endpoint
                .trim_end_matches("/audio/transcriptions")
                .trim_end_matches('/');
            let mut openai_config = OpenAIConfig::new(&config.api_key).with_base_url(base_url);
            openai_config.provider = "groq".to_string();
            Ok(Arc::new(OpenAITranscriptionModel::new(
                config.model,
                openai_config,
            )))
        }
        MISTRAL_VOXTRAL_PROVIDER_ID => {
            if request.mode != AsrMode::Batch {
                return Err(AsrPortError::invalid_request(
                    WhisperCompatibleProvider::MistralVoxtral.batch_only_error(),
                ));
            }
            let config = resolve_whisper_config(request, WhisperCompatibleProvider::MistralVoxtral)
                .map_err(|error| AsrPortError::invalid_request(error.to_string()))?;
            let base_url = config
                .batch_endpoint
                .trim_end_matches("/audio/transcriptions")
                .trim_end_matches('/');
            let mut openai_config = OpenAIConfig::new(&config.api_key).with_base_url(base_url);
            openai_config.provider = "mistral".to_string();
            Ok(Arc::new(OpenAITranscriptionModel::new(
                config.model,
                openai_config,
            )))
        }
        VOLCENGINE_DOUBAO_PROVIDER_ID => {
            if request.mode != AsrMode::Batch {
                return Err(AsrPortError::from(
                    crate::SherpaError::VolcengineBatchModeMismatch,
                ));
            }
            let config = resolve_volcengine_config(request, VolcengineMode::Batch)
                .map_err(AsrPortError::from)?;
            Ok(Arc::new(VolcengineTranscriptionModel::new(config)))
        }
        _ => Err(AsrPortError::new(
            AsrPortErrorKind::Unsupported,
            format!("不支持的在线 ASR provider：{provider_id}"),
        )
        .with_code("UNSUPPORTED_ONLINE_PROVIDER")),
    }
}

/// Execute batch transcription through an aimux [`TranscriptionModel`].
pub async fn execute_aimux_batch(
    model: &dyn TranscriptionModel,
    input: OnlineBatchTranscriptionRequest,
) -> Result<OnlineBatchTranscriptionOutput, AsrPortError> {
    let bytes = tokio::fs::read(&input.file_path).await.map_err(|error| {
        AsrPortError::new(
            AsrPortErrorKind::FileSystem,
            format!("Failed to read audio file: {error}"),
        )
    })?;

    let media_type = detect_audio_mime_type(&input.file_path);
    let mut call_options =
        TranscriptionCallOptions::new(AudioInput::Binary(bytes.clone()), media_type);

    // Build provider options
    let mut po = std::collections::HashMap::new();

    // 1. OpenAI-compatible options
    let mut openai_options = serde_json::Map::new();
    if let Some(language) = whisper_language_form_field(&input.request.language) {
        openai_options.insert("language".to_string(), serde_json::Value::String(language));
    }
    if let Some(hotwords) = input
        .request
        .hotwords
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        openai_options.insert(
            "prompt".to_string(),
            serde_json::Value::String(hotwords.replace('\n', ", ")),
        );
    }
    if !openai_options.is_empty() {
        po.insert(
            "openai".to_string(),
            serde_json::Value::Object(openai_options),
        );
    }

    // 2. Volcengine options
    let mut volc_options = serde_json::Map::new();
    volc_options.insert(
        "enable_itn".to_string(),
        serde_json::Value::Bool(input.request.enable_itn),
    );
    if !input.request.language.trim().is_empty() && input.request.language != "auto" {
        volc_options.insert(
            "language".to_string(),
            serde_json::Value::String(input.request.language.clone()),
        );
    }
    if let Some(hotwords) = input
        .request
        .hotwords
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        volc_options.insert(
            "hotwords".to_string(),
            serde_json::Value::String(hotwords.to_string()),
        );
    }
    po.insert(
        "volcengine".to_string(),
        serde_json::Value::Object(volc_options),
    );

    if !po.is_empty() {
        call_options.provider_options = Some(po);
    }

    let result = model
        .do_generate(&call_options)
        .await
        .map_err(map_aimux_asr_error)?;

    let mut segments: Vec<TranscriptSegment> = result
        .segments
        .into_iter()
        .map(|segment| TranscriptSegment {
            id: uuid::Uuid::new_v4().to_string(),
            text: segment.text.trim().to_string(),
            start: segment.start_second,
            end: segment.end_second,
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            speaker: None,
            speaker_attribution: None,
        })
        .collect();

    let trimmed_text = result.text.trim();
    if segments.is_empty() && !trimmed_text.is_empty() {
        let duration = result.duration_in_seconds.unwrap_or(0.0);
        segments.push(TranscriptSegment {
            id: uuid::Uuid::new_v4().to_string(),
            text: trimmed_text.to_string(),
            start: 0.0,
            end: duration,
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

    let audio_duration_ms = result.duration_in_seconds.unwrap_or(0.0) * 1000.0;
    let stage = format!("{}_batch_complete", model.provider().replace('-', "_"));

    Ok(OnlineBatchTranscriptionOutput {
        segments,
        audio_duration_ms,
        buffered_samples: bytes.len() / 2,
        stage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use aimux_core::error::{AiMuxError, ApiCallError};
    use serde_json::json;
    use sona_core::ports::asr::{
        AsrEngineConfig, AsrMode, AsrPortErrorKind, AsrTranscriptionRequest,
        GROQ_WHISPER_PROVIDER_ID, MISTRAL_VOXTRAL_PROVIDER_ID, OnlineAsrProviderRequest,
        VOLCENGINE_DOUBAO_PROVIDER_ID,
    };
    use sona_core::transcription::postprocess::{
        TranscriptNormalizationOptions, TranscriptPostprocessOptions,
    };

    fn online_request(provider_id: &str, config: serde_json::Value) -> AsrTranscriptionRequest {
        AsrTranscriptionRequest {
            mode: AsrMode::Batch,
            language: "auto".to_string(),
            enable_itn: false,
            normalization_options: TranscriptNormalizationOptions::default(),
            postprocess_options: TranscriptPostprocessOptions::default(),
            hotwords: None,
            speaker_processing: None,
            engine_config: AsrEngineConfig::Online {
                provider: OnlineAsrProviderRequest {
                    provider_id: provider_id.to_string(),
                    profile_id: format!("{provider_id}-default"),
                    config,
                },
            },
        }
    }

    #[test]
    fn detects_audio_mime_types() {
        assert_eq!(detect_audio_mime_type(Path::new("test.wav")), "audio/wav");
        assert_eq!(detect_audio_mime_type(Path::new("test.mp3")), "audio/mp3");
        assert_eq!(detect_audio_mime_type(Path::new("test.ogg")), "audio/ogg");
        assert_eq!(detect_audio_mime_type(Path::new("test.flac")), "audio/flac");
        assert_eq!(detect_audio_mime_type(Path::new("test.m4a")), "audio/m4a");
        assert_eq!(
            detect_audio_mime_type(Path::new("test.unknown")),
            "audio/wav"
        );
    }

    #[test]
    fn creates_aimux_model_for_groq_whisper() {
        let request = online_request(
            GROQ_WHISPER_PROVIDER_ID,
            json!({
                "apiKey": "test-groq-key",
                "model": "whisper-large-v3-turbo"
            }),
        );

        let model = create_aimux_transcription_model(&request).unwrap();
        assert_eq!(model.provider(), "groq");
        assert_eq!(model.model_id(), "whisper-large-v3-turbo");
    }

    #[test]
    fn creates_aimux_model_for_mistral_voxtral() {
        let request = online_request(
            MISTRAL_VOXTRAL_PROVIDER_ID,
            json!({
                "apiKey": "test-mistral-key",
                "model": "mistral-small-latest"
            }),
        );

        let model = create_aimux_transcription_model(&request).unwrap();
        assert_eq!(model.provider(), "mistral");
        assert_eq!(model.model_id(), "mistral-small-latest");
    }

    #[test]
    fn creates_aimux_model_for_volcengine_doubao() {
        let request = online_request(
            VOLCENGINE_DOUBAO_PROVIDER_ID,
            json!({
                "apiKey": "test-volc-key",
                "batchEndpoint": "https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash",
                "batchResourceId": "volc.bigasr.auc_turbo"
            }),
        );

        let model = create_aimux_transcription_model(&request).unwrap();
        assert_eq!(model.provider(), "volcengine");
        assert_eq!(model.model_id(), "volc.bigasr.auc_turbo");
    }

    #[test]
    fn maps_aimux_errors_to_typed_asr_port_errors() {
        let auth_error = AiMuxError::ApiCall(ApiCallError {
            status_code: Some(401),
            provider_code: None,
            message: "Unauthorized".to_string(),
            response_body: None,
            request_id: None,
            retry_after_ms: None,
            is_retryable: false,
        });
        assert_eq!(
            map_aimux_asr_error(auth_error).kind,
            AsrPortErrorKind::Authentication
        );

        let rate_limit_error = AiMuxError::ApiCall(ApiCallError {
            status_code: Some(429),
            provider_code: None,
            message: "Rate limit reached".to_string(),
            response_body: None,
            request_id: None,
            retry_after_ms: Some(1000),
            is_retryable: true,
        });
        assert_eq!(
            map_aimux_asr_error(rate_limit_error).kind,
            AsrPortErrorKind::RateLimited
        );

        let unavailable_error = AiMuxError::ApiCall(ApiCallError {
            status_code: Some(503),
            provider_code: None,
            message: "Service Unavailable".to_string(),
            response_body: None,
            request_id: None,
            retry_after_ms: None,
            is_retryable: true,
        });
        assert_eq!(
            map_aimux_asr_error(unavailable_error).kind,
            AsrPortErrorKind::Unavailable
        );

        let timeout_error = AiMuxError::Timeout("connection timed out".to_string());
        assert_eq!(
            map_aimux_asr_error(timeout_error).kind,
            AsrPortErrorKind::Timeout
        );

        let not_found_error = AiMuxError::ApiCall(ApiCallError {
            status_code: Some(404),
            provider_code: None,
            message: "Not found".to_string(),
            response_body: None,
            request_id: None,
            retry_after_ms: None,
            is_retryable: false,
        });
        assert_eq!(
            map_aimux_asr_error(not_found_error).kind,
            AsrPortErrorKind::InvalidRequest
        );

        let payload_too_large = AiMuxError::ApiCall(ApiCallError {
            status_code: Some(413),
            provider_code: None,
            message: "Payload Too Large".to_string(),
            response_body: None,
            request_id: None,
            retry_after_ms: None,
            is_retryable: false,
        });
        assert_eq!(
            map_aimux_asr_error(payload_too_large).kind,
            AsrPortErrorKind::InvalidRequest
        );

        let invalid_arg_error = AiMuxError::InvalidArgument("bad param".to_string());
        assert_eq!(
            map_aimux_asr_error(invalid_arg_error).kind,
            AsrPortErrorKind::InvalidRequest
        );

        let no_model_error = AiMuxError::NoSuchModel {
            model_id: "whisper-nonexistent".to_string(),
            model_type: "transcriptionModel".to_string(),
        };
        assert_eq!(
            map_aimux_asr_error(no_model_error).kind,
            AsrPortErrorKind::InvalidRequest
        );

        let no_provider_error = AiMuxError::NoSuchProvider {
            provider_id: "unknown".to_string(),
        };
        assert_eq!(
            map_aimux_asr_error(no_provider_error).kind,
            AsrPortErrorKind::InvalidRequest
        );
        let unsupported_error =
            AiMuxError::UnsupportedFunctionality("streaming not supported".to_string());
        assert_eq!(
            map_aimux_asr_error(unsupported_error).kind,
            AsrPortErrorKind::Unsupported
        );

        let parse_error = AiMuxError::JsonParse("failed to parse".to_string());
        assert_eq!(
            map_aimux_asr_error(parse_error).kind,
            AsrPortErrorKind::Protocol
        );
    }
}
