use std::path::Path;
use std::sync::Arc;

use aimux_core::transcription_model::{AudioInput, TranscriptionCallOptions, TranscriptionModel};
use aimux_providers::assemblyai::{AssemblyAIConfig, AssemblyAITranscriptionModel};
use aimux_providers::deepgram::{DeepgramConfig, DeepgramTranscriptionModel};
use aimux_providers::elevenlabs::{ElevenLabsConfig, ElevenLabsTranscriptionModel};
use aimux_providers::openai::{OpenAIConfig, OpenAITranscriptionModel};
use sona_core::ports::asr::{
    ASSEMBLYAI_PROVIDER_ID, AsrEngineConfig, AsrMode, AsrPortError, AsrPortErrorKind,
    AsrTranscriptionRequest, DEEPGRAM_PROVIDER_ID, ELEVENLABS_PROVIDER_ID,
    GROQ_WHISPER_PROVIDER_ID, MISTRAL_VOXTRAL_PROVIDER_ID, OPENAI_WHISPER_PROVIDER_ID,
    OnlineBatchTranscriptionOutput, OnlineBatchTranscriptionRequest, VOLCENGINE_DOUBAO_PROVIDER_ID,
    find_online_asr_provider,
};
use sona_core::transcription::transcript::TranscriptSegment;

use crate::error::map_aimux_asr_error;
use crate::volcengine::VolcengineTranscriptionModel;
use crate::{VolcengineMode, resolve_online_asr_provider_id, resolve_volcengine_config};

/// Normalized fields resolved from an online ASR provider request and manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnlineProviderConfigFields {
    pub api_key: String,
    pub model: String,
    pub batch_endpoint: Option<String>,
}

/// Helper to resolve standard online provider configuration (API key, model, endpoint).
pub fn resolve_online_provider_config(
    request: &AsrTranscriptionRequest,
    provider_id: &str,
) -> Result<OnlineProviderConfigFields, AsrPortError> {
    let provider_request = match &request.engine_config {
        AsrEngineConfig::Online { provider } => provider,
        _ => {
            return Err(AsrPortError::invalid_request(format!(
                "Online ASR provider request is missing for {provider_id}."
            )));
        }
    };

    let manifest = find_online_asr_provider(provider_id).ok_or_else(|| {
        AsrPortError::new(
            AsrPortErrorKind::Unsupported,
            format!("Provider {provider_id} not found in manifest"),
        )
    })?;

    let defaults = manifest.defaults.as_object();

    let get_string = |key: &str| -> String {
        provider_request
            .config
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .or_else(|| {
                defaults
                    .and_then(|d| d.get(key))
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
            })
            .unwrap_or("")
            .to_string()
    };

    let api_key = get_string("apiKey");
    if api_key.is_empty() {
        return Err(AsrPortError::new(
            AsrPortErrorKind::Authentication,
            format!("{provider_id} API Key is not configured."),
        ));
    }

    let model = get_string("model");
    let batch_endpoint_str = get_string("batchEndpoint");
    let batch_endpoint = if batch_endpoint_str.is_empty() {
        None
    } else {
        Some(batch_endpoint_str)
    };

    Ok(OnlineProviderConfigFields {
        api_key,
        model,
        batch_endpoint,
    })
}

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

fn normalize_language_hint(language: &str) -> Option<String> {
    let lang = language.trim();
    if lang.is_empty() || lang.eq_ignore_ascii_case("auto") {
        None
    } else {
        Some(lang.to_string())
    }
}

/// Create an aimux [`TranscriptionModel`] from a Sona [`AsrTranscriptionRequest`].
pub fn create_aimux_transcription_model(
    request: &AsrTranscriptionRequest,
) -> Result<Arc<dyn TranscriptionModel>, AsrPortError> {
    if request.mode != AsrMode::Batch {
        return Err(AsrPortError::invalid_request(format!(
            "Online provider {} can only be used in batch mode.",
            request.provider_id()
        )));
    }

    let provider_id = resolve_online_asr_provider_id(request)?;

    match provider_id {
        VOLCENGINE_DOUBAO_PROVIDER_ID => {
            let config = resolve_volcengine_config(request, VolcengineMode::Batch)
                .map_err(AsrPortError::from)?;
            Ok(Arc::new(VolcengineTranscriptionModel::new(config)))
        }
        OPENAI_WHISPER_PROVIDER_ID => {
            let config = resolve_online_provider_config(request, OPENAI_WHISPER_PROVIDER_ID)?;
            let mut openai_config = OpenAIConfig::new(&config.api_key);
            if let Some(endpoint) = &config.batch_endpoint {
                let base_url = endpoint
                    .trim_end_matches("/audio/transcriptions")
                    .trim_end_matches('/');
                openai_config = openai_config.with_base_url(base_url);
            }
            let model_id = if config.model.is_empty() {
                "whisper-1".to_string()
            } else {
                config.model
            };
            Ok(Arc::new(OpenAITranscriptionModel::new(
                model_id,
                openai_config,
            )))
        }
        GROQ_WHISPER_PROVIDER_ID => {
            let config = resolve_online_provider_config(request, GROQ_WHISPER_PROVIDER_ID)?;
            let mut openai_config = OpenAIConfig::new(&config.api_key);
            if let Some(endpoint) = &config.batch_endpoint {
                let base_url = endpoint
                    .trim_end_matches("/audio/transcriptions")
                    .trim_end_matches('/');
                openai_config = openai_config.with_base_url(base_url);
            }
            openai_config.provider = "groq".to_string();
            let model_id = if config.model.is_empty() {
                "whisper-large-v3-turbo".to_string()
            } else {
                config.model
            };
            Ok(Arc::new(OpenAITranscriptionModel::new(
                model_id,
                openai_config,
            )))
        }
        MISTRAL_VOXTRAL_PROVIDER_ID => {
            let config = resolve_online_provider_config(request, MISTRAL_VOXTRAL_PROVIDER_ID)?;
            let mut openai_config = OpenAIConfig::new(&config.api_key);
            if let Some(endpoint) = &config.batch_endpoint {
                let base_url = endpoint
                    .trim_end_matches("/audio/transcriptions")
                    .trim_end_matches('/');
                openai_config = openai_config.with_base_url(base_url);
            }
            openai_config.provider = "mistral".to_string();
            let model_id = if config.model.is_empty() {
                "mistral-small-latest".to_string()
            } else {
                config.model
            };
            Ok(Arc::new(OpenAITranscriptionModel::new(
                model_id,
                openai_config,
            )))
        }
        DEEPGRAM_PROVIDER_ID => {
            let config = resolve_online_provider_config(request, DEEPGRAM_PROVIDER_ID)?;
            let mut dg_config = DeepgramConfig::new(&config.api_key);
            if let Some(endpoint) = &config.batch_endpoint {
                let base_url = endpoint
                    .trim_end_matches("/v1/listen")
                    .trim_end_matches('/');
                dg_config = dg_config.with_base_url(base_url);
            }
            let model_id = if config.model.is_empty() {
                "nova-2".to_string()
            } else {
                config.model
            };
            Ok(Arc::new(DeepgramTranscriptionModel::new(
                model_id, dg_config,
            )))
        }
        ASSEMBLYAI_PROVIDER_ID => {
            let config = resolve_online_provider_config(request, ASSEMBLYAI_PROVIDER_ID)?;
            let mut aai_config = AssemblyAIConfig::new(&config.api_key);
            if let Some(endpoint) = &config.batch_endpoint {
                let base_url = endpoint
                    .trim_end_matches("/v2/transcript")
                    .trim_end_matches('/');
                aai_config = aai_config.with_base_url(base_url);
            }
            let model_id = if config.model.is_empty() {
                "best".to_string()
            } else {
                config.model
            };
            Ok(Arc::new(AssemblyAITranscriptionModel::new(
                model_id, aai_config,
            )))
        }
        ELEVENLABS_PROVIDER_ID => {
            let config = resolve_online_provider_config(request, ELEVENLABS_PROVIDER_ID)?;
            let mut el_config = ElevenLabsConfig::new(&config.api_key);
            if let Some(endpoint) = &config.batch_endpoint {
                let base_url = endpoint
                    .trim_end_matches("/v1/speech-to-text")
                    .trim_end_matches('/');
                el_config = el_config.with_base_url(base_url);
            }
            let model_id = if config.model.is_empty() {
                "scribe_v1".to_string()
            } else {
                config.model
            };
            Ok(Arc::new(ElevenLabsTranscriptionModel::new(
                model_id, el_config,
            )))
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
    if let Some(language) = normalize_language_hint(&input.request.language) {
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
    if let Some(language) = normalize_language_hint(&input.request.language) {
        volc_options.insert("language".to_string(), serde_json::Value::String(language));
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

    // 3. Deepgram options
    let mut deepgram_options = serde_json::Map::new();
    if let Some(language) = normalize_language_hint(&input.request.language) {
        deepgram_options.insert("language".to_string(), serde_json::Value::String(language));
    }
    if !deepgram_options.is_empty() {
        po.insert(
            "deepgram".to_string(),
            serde_json::Value::Object(deepgram_options),
        );
    }

    // 4. AssemblyAI options
    let mut assemblyai_options = serde_json::Map::new();
    if let Some(language) = normalize_language_hint(&input.request.language) {
        assemblyai_options.insert(
            "language_code".to_string(),
            serde_json::Value::String(language),
        );
    }
    if let Some(hotwords) = input
        .request
        .hotwords
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let words = hotwords
            .lines()
            .map(str::trim)
            .filter(|w| !w.is_empty())
            .collect::<Vec<_>>();
        if !words.is_empty() {
            assemblyai_options.insert("word_boost".to_string(), serde_json::json!(words));
        }
    }
    if !assemblyai_options.is_empty() {
        po.insert(
            "assemblyai".to_string(),
            serde_json::Value::Object(assemblyai_options),
        );
    }

    // 5. ElevenLabs options
    let mut elevenlabs_options = serde_json::Map::new();
    if let Some(language) = normalize_language_hint(&input.request.language) {
        elevenlabs_options.insert(
            "languageCode".to_string(),
            serde_json::Value::String(language),
        );
    }
    if !elevenlabs_options.is_empty() {
        po.insert(
            "elevenlabs".to_string(),
            serde_json::Value::Object(elevenlabs_options),
        );
    }

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
        OnlineAsrProviderRequest,
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
    fn creates_aimux_model_for_openai_whisper() {
        let request = online_request(
            OPENAI_WHISPER_PROVIDER_ID,
            json!({
                "apiKey": "test-openai-key",
                "model": "whisper-1"
            }),
        );

        let model = create_aimux_transcription_model(&request).unwrap();
        assert_eq!(model.provider(), "openai");
        assert_eq!(model.model_id(), "whisper-1");
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
    fn creates_aimux_model_for_deepgram() {
        let request = online_request(
            DEEPGRAM_PROVIDER_ID,
            json!({
                "apiKey": "test-deepgram-key",
                "model": "nova-2"
            }),
        );

        let model = create_aimux_transcription_model(&request).unwrap();
        assert_eq!(model.provider(), "deepgram");
        assert_eq!(model.model_id(), "nova-2");
    }

    #[test]
    fn creates_aimux_model_for_assemblyai() {
        let request = online_request(
            ASSEMBLYAI_PROVIDER_ID,
            json!({
                "apiKey": "test-aai-key",
                "model": "best"
            }),
        );

        let model = create_aimux_transcription_model(&request).unwrap();
        assert_eq!(model.provider(), "assemblyai");
        assert_eq!(model.model_id(), "best");
    }

    #[test]
    fn creates_aimux_model_for_elevenlabs() {
        let request = online_request(
            ELEVENLABS_PROVIDER_ID,
            json!({
                "apiKey": "test-el-key",
                "model": "scribe_v1"
            }),
        );

        let model = create_aimux_transcription_model(&request).unwrap();
        assert_eq!(model.provider(), "elevenlabs");
        assert_eq!(model.model_id(), "scribe_v1");
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
    fn missing_api_key_fails_authentication() {
        let request = online_request(
            OPENAI_WHISPER_PROVIDER_ID,
            json!({
                "apiKey": "   "
            }),
        );

        let err = match create_aimux_transcription_model(&request) {
            Err(err) => err,
            Ok(_) => panic!("expected missing API key error"),
        };
        assert_eq!(err.kind, AsrPortErrorKind::Authentication);
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
