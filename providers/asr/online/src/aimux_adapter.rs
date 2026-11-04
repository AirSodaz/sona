use std::path::Path;
use std::sync::Arc;

use aimux_core::shared::SharedProviderOptions;
use aimux_core::transcription_model::{AudioInput, TranscriptionCallOptions, TranscriptionModel};
use aimux_providers::assemblyai::{AssemblyAIProviderSettings, create_assemblyai};
use aimux_providers::deepgram::{DeepgramProviderSettings, create_deepgram};
use aimux_providers::elevenlabs::{ElevenLabsProviderSettings, create_elevenlabs};
use aimux_providers::openai::{OpenAIProviderSettings, create_openai};
use aimux_providers::{
    AzureOpenAIProviderSettings, FalProviderSettings, GladiaProviderSettings,
    GoogleProviderSettings, RevaiProviderSettings, XAIProviderSettings, create_azure, create_fal,
    create_gladia, create_google, create_revai, create_xai,
};
use serde_json::Value;
use sona_core::ports::asr::{
    ASSEMBLYAI_PROVIDER_ID, AZURE_SPEECH_PROVIDER_ID, AsrEngineConfig, AsrMode, AsrPortError,
    AsrPortErrorKind, AsrTranscriptionRequest, DEEPGRAM_PROVIDER_ID, ELEVENLABS_PROVIDER_ID,
    FAL_WHISPER_PROVIDER_ID, GLADIA_PROVIDER_ID, GOOGLE_GEMINI_ASR_PROVIDER_ID,
    GROQ_WHISPER_PROVIDER_ID, MISTRAL_VOXTRAL_PROVIDER_ID, OPENAI_WHISPER_PROVIDER_ID,
    OnlineBatchTranscriptionOutput, OnlineBatchTranscriptionRequest, REVAI_PROVIDER_ID,
    VOLCENGINE_DOUBAO_PROVIDER_ID, XAI_ASR_PROVIDER_ID, find_online_asr_provider,
};
use sona_core::transcription::transcript::{
    TranscriptSegment, TranscriptTiming, TranscriptTimingLevel, TranscriptTimingSource,
    TranscriptTimingUnit,
};

use crate::error::map_aimux_asr_error;
use crate::volcengine::VolcengineTranscriptionModel;
use crate::{
    VolcengineMode, create_cloud_speaker, resolve_online_asr_provider_id,
    resolve_volcengine_config, segments_from_volcengine_response,
};

/// Normalized fields resolved from an online ASR provider request and manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnlineProviderConfigFields {
    pub api_key: String,
    pub model: String,
    pub batch_endpoint: Option<String>,
    pub streaming_endpoint: Option<String>,
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
    let streaming_endpoint_str = get_string("streamingEndpoint");
    let streaming_endpoint = if streaming_endpoint_str.is_empty() {
        None
    } else {
        Some(streaming_endpoint_str)
    };

    Ok(OnlineProviderConfigFields {
        api_key,
        model,
        batch_endpoint,
        streaming_endpoint,
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
        "m4a" | "mp4" => "audio/m4a",
        "aac" => "audio/aac",
        "flac" => "audio/flac",
        "webm" => "audio/webm",
        "opus" => "audio/ogg",
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
            let mut settings = OpenAIProviderSettings {
                api_key: Some(config.api_key),
                ..Default::default()
            };
            if let Some(endpoint) = &config.batch_endpoint {
                let base_url = endpoint
                    .trim_end_matches("/audio/transcriptions")
                    .trim_end_matches('/');
                settings.base_url = Some(base_url.to_string());
            }
            let provider = create_openai(settings).map_err(map_aimux_asr_error)?;
            let model_id = if config.model.is_empty() {
                "whisper-1"
            } else {
                &config.model
            };
            Ok(Arc::new(provider.transcription(model_id)))
        }
        GROQ_WHISPER_PROVIDER_ID => {
            let config = resolve_online_provider_config(request, GROQ_WHISPER_PROVIDER_ID)?;
            let mut settings = OpenAIProviderSettings {
                api_key: Some(config.api_key),
                name: Some("groq".to_string()),
                ..Default::default()
            };
            if let Some(endpoint) = &config.batch_endpoint {
                let base_url = endpoint
                    .trim_end_matches("/audio/transcriptions")
                    .trim_end_matches('/');
                settings.base_url = Some(base_url.to_string());
            }
            let provider = create_openai(settings).map_err(map_aimux_asr_error)?;
            let model_id = if config.model.is_empty() {
                "whisper-large-v3-turbo"
            } else {
                &config.model
            };
            Ok(Arc::new(provider.transcription(model_id)))
        }
        MISTRAL_VOXTRAL_PROVIDER_ID => {
            let config = resolve_online_provider_config(request, MISTRAL_VOXTRAL_PROVIDER_ID)?;
            let mut settings = OpenAIProviderSettings {
                api_key: Some(config.api_key),
                name: Some("mistral".to_string()),
                ..Default::default()
            };
            if let Some(endpoint) = &config.batch_endpoint {
                let base_url = endpoint
                    .trim_end_matches("/audio/transcriptions")
                    .trim_end_matches('/');
                settings.base_url = Some(base_url.to_string());
            }
            let provider = create_openai(settings).map_err(map_aimux_asr_error)?;
            let model_id = if config.model.is_empty() {
                "mistral-small-latest"
            } else {
                &config.model
            };
            Ok(Arc::new(provider.transcription(model_id)))
        }
        DEEPGRAM_PROVIDER_ID => {
            let config = resolve_online_provider_config(request, DEEPGRAM_PROVIDER_ID)?;
            let mut settings = DeepgramProviderSettings {
                api_key: Some(config.api_key.into()),
                ..Default::default()
            };
            if let Some(endpoint) = &config.batch_endpoint {
                let base_url = endpoint
                    .trim_end_matches("/v1/listen")
                    .trim_end_matches('/');
                settings.base_url = Some(base_url.to_string());
            }
            let provider = create_deepgram(settings).map_err(map_aimux_asr_error)?;
            let model_id = if config.model.is_empty() {
                "nova-2"
            } else {
                &config.model
            };
            Ok(Arc::new(provider.transcription(model_id)))
        }
        ASSEMBLYAI_PROVIDER_ID => {
            let config = resolve_online_provider_config(request, ASSEMBLYAI_PROVIDER_ID)?;
            let mut settings = AssemblyAIProviderSettings {
                api_key: Some(config.api_key.into()),
                ..Default::default()
            };
            if let Some(endpoint) = &config.batch_endpoint {
                let base_url = endpoint
                    .trim_end_matches("/v2/transcript")
                    .trim_end_matches('/');
                settings.base_url = Some(base_url.to_string());
            }
            let provider = create_assemblyai(settings).map_err(map_aimux_asr_error)?;
            let model_id = if config.model.is_empty() {
                "best"
            } else {
                &config.model
            };
            Ok(Arc::new(provider.transcription(model_id)))
        }
        ELEVENLABS_PROVIDER_ID => {
            let config = resolve_online_provider_config(request, ELEVENLABS_PROVIDER_ID)?;
            let mut settings = ElevenLabsProviderSettings {
                api_key: Some(config.api_key.into()),
                ..Default::default()
            };
            if let Some(endpoint) = &config.batch_endpoint {
                let base_url = endpoint
                    .trim_end_matches("/v1/speech-to-text")
                    .trim_end_matches('/');
                settings.base_url = Some(base_url.to_string());
            }
            let provider = create_elevenlabs(settings).map_err(map_aimux_asr_error)?;
            let model_id = if config.model.is_empty() {
                "scribe_v1"
            } else {
                &config.model
            };
            Ok(Arc::new(provider.transcription(model_id)))
        }
        AZURE_SPEECH_PROVIDER_ID => {
            let config = resolve_online_provider_config(request, AZURE_SPEECH_PROVIDER_ID)?;
            let mut settings = AzureOpenAIProviderSettings::default();
            if !config.api_key.is_empty() {
                settings.api_key = Some(config.api_key.into());
            }
            if let Some(endpoint) = &config.batch_endpoint {
                settings.base_url = Some(endpoint.clone());
                settings.speech_base_url = Some(endpoint.clone());
            }
            let provider = create_azure(settings).map_err(map_aimux_asr_error)?;
            let model_id = if config.model.is_empty() {
                "whisper-1"
            } else {
                &config.model
            };
            Ok(Arc::new(provider.transcription(model_id)))
        }
        GOOGLE_GEMINI_ASR_PROVIDER_ID => {
            let config = resolve_online_provider_config(request, GOOGLE_GEMINI_ASR_PROVIDER_ID)?;
            let mut settings = GoogleProviderSettings::default();
            if !config.api_key.is_empty() {
                settings.api_key = Some(config.api_key);
            }
            if let Some(endpoint) = &config.batch_endpoint {
                settings.base_url = Some(endpoint.clone());
            }
            let provider = create_google(settings).map_err(map_aimux_asr_error)?;
            let model_id = if config.model.is_empty() {
                "gemini-2.0-flash"
            } else {
                &config.model
            };
            Ok(Arc::new(provider.transcription(model_id)))
        }
        XAI_ASR_PROVIDER_ID => {
            let config = resolve_online_provider_config(request, XAI_ASR_PROVIDER_ID)?;
            let mut settings = XAIProviderSettings::default();
            if !config.api_key.is_empty() {
                settings.api_key = Some(config.api_key);
            }
            if let Some(endpoint) = &config.batch_endpoint {
                settings.base_url = Some(endpoint.clone());
            }
            let provider = create_xai(settings).map_err(map_aimux_asr_error)?;
            Ok(Arc::new(provider.transcription()))
        }
        GLADIA_PROVIDER_ID => {
            let config = resolve_online_provider_config(request, GLADIA_PROVIDER_ID)?;
            let mut settings = GladiaProviderSettings::default();
            if !config.api_key.is_empty() {
                settings.api_key = Some(config.api_key.into());
            }
            if let Some(endpoint) = &config.batch_endpoint {
                settings.base_url = Some(endpoint.clone());
            }
            let provider = create_gladia(settings).map_err(map_aimux_asr_error)?;
            let model_id = if config.model.is_empty() {
                "default"
            } else {
                &config.model
            };
            Ok(Arc::new(provider.transcription(model_id)))
        }
        REVAI_PROVIDER_ID => {
            let config = resolve_online_provider_config(request, REVAI_PROVIDER_ID)?;
            let mut settings = RevaiProviderSettings::default();
            if !config.api_key.is_empty() {
                settings.api_key = Some(config.api_key.into());
            }
            if let Some(endpoint) = &config.batch_endpoint {
                settings.base_url = Some(endpoint.clone());
            }
            let provider = create_revai(settings).map_err(map_aimux_asr_error)?;
            let model_id = if config.model.is_empty() {
                "default"
            } else {
                &config.model
            };
            Ok(Arc::new(provider.transcription(model_id)))
        }
        FAL_WHISPER_PROVIDER_ID => {
            let config = resolve_online_provider_config(request, FAL_WHISPER_PROVIDER_ID)?;
            let mut settings = FalProviderSettings::default();
            if !config.api_key.is_empty() {
                settings.api_key = Some(config.api_key.into());
            }
            if let Some(endpoint) = &config.batch_endpoint {
                settings.base_url = Some(endpoint.clone());
            }
            let provider = create_fal(settings).map_err(map_aimux_asr_error)?;
            let model_id = if config.model.is_empty() {
                "whisper"
            } else {
                &config.model
            };
            Ok(Arc::new(provider.transcription(model_id)))
        }
        _ => Err(AsrPortError::new(
            AsrPortErrorKind::Unsupported,
            format!("不支持的在线 ASR provider：{provider_id}"),
        )
        .with_code("UNSUPPORTED_ONLINE_PROVIDER")),
    }
}

/// Returns whether speaker diarization is enabled for an online ASR request.
/// Defaults to `true` if not explicitly set to `false`.
pub fn is_cloud_speaker_diarization_enabled(request: &AsrTranscriptionRequest) -> bool {
    if let sona_core::ports::asr::AsrEngineConfig::Online { provider } = &request.engine_config {
        if let Some(b) = provider
            .config
            .get("speakerDiarization")
            .and_then(Value::as_bool)
        {
            return b;
        }
        if let Some(b) = provider
            .config
            .get("speaker_diarization")
            .and_then(Value::as_bool)
        {
            return b;
        }
        if let Some(provider_def) =
            sona_core::ports::asr::find_online_asr_provider(&provider.provider_id)
            && let Some(b) = provider_def
                .defaults
                .get("speakerDiarization")
                .and_then(Value::as_bool)
        {
            return b;
        }
    }
    true
}

const MAX_ONLINE_STREAM_DOWNLOAD_BYTES: usize = 100 * 1024 * 1024;

fn map_content_type_to_audio_mime(content_type: &str) -> Option<&'static str> {
    let raw = content_type.split(';').next()?.trim().to_ascii_lowercase();
    match raw.as_str() {
        "audio/mpeg" | "audio/mp3" => Some("audio/mp3"),
        "audio/wav" | "audio/x-wav" | "audio/wave" => Some("audio/wav"),
        "audio/ogg" | "application/ogg" | "audio/opus" => Some("audio/ogg"),
        "audio/m4a" | "audio/x-m4a" | "audio/mp4" | "video/mp4" => Some("audio/m4a"),
        "audio/aac" | "audio/x-aac" => Some("audio/aac"),
        "audio/flac" | "audio/x-flac" => Some("audio/flac"),
        "audio/webm" | "video/webm" => Some("audio/webm"),
        _ => None,
    }
}

fn sniff_audio_mime_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE" {
        Some("audio/wav")
    } else if (bytes.len() >= 3 && &bytes[0..3] == b"ID3")
        || (bytes.len() >= 2
            && bytes[0] == 0xFF
            && (bytes[1] & 0xFE) == 0xFA
            && (bytes[1] & 0x06) == 0x02)
    {
        Some("audio/mp3")
    } else if bytes.len() >= 2 && bytes[0] == 0xFF && (bytes[1] & 0xF6) == 0xF0 {
        Some("audio/aac")
    } else if bytes.len() >= 4 && &bytes[0..4] == b"OggS" {
        Some("audio/ogg")
    } else if bytes.len() >= 8 && &bytes[4..8] == b"ftyp" {
        Some("audio/m4a")
    } else if bytes.len() >= 4 && &bytes[0..4] == b"fLaC" {
        Some("audio/flac")
    } else if bytes.len() >= 4 && bytes[0..4] == [0x1A, 0x45, 0xDF, 0xA3] {
        Some("audio/webm")
    } else {
        None
    }
}

/// Execute batch transcription through an aimux [`TranscriptionModel`].
pub async fn execute_aimux_batch(
    model: &dyn TranscriptionModel,
    input: OnlineBatchTranscriptionRequest,
) -> Result<OnlineBatchTranscriptionOutput, AsrPortError> {
    let path_str = input.file_path.to_string_lossy();
    let trimmed_url = path_str.trim();
    let is_network_stream = sona_core::history::is_network_url(trimmed_url);
    let lower_url = trimmed_url.to_ascii_lowercase();
    if is_network_stream && !lower_url.starts_with("http://") && !lower_url.starts_with("https://")
    {
        return Err(AsrPortError::new(
            AsrPortErrorKind::Unsupported,
            format!(
                "Online batch ASR only supports HTTP/HTTPS audio streams; cannot fetch: {trimmed_url}"
            ),
        ));
    }
    let clean_path = lower_url
        .split('?')
        .next()
        .unwrap_or(&lower_url)
        .split('#')
        .next()
        .unwrap_or(&lower_url);
    if is_network_stream && (clean_path.ends_with(".m3u8") || clean_path.ends_with(".mpd")) {
        return Err(AsrPortError::new(
            AsrPortErrorKind::InvalidRequest,
            "Online batch ASR does not support playlist streams (.m3u8, .mpd). Please use local transcription or provide a direct audio stream URL.",
        ));
    }
    let max_stream_bytes = if input.request.provider_id() == OPENAI_WHISPER_PROVIDER_ID
        || input.request.provider_id() == GROQ_WHISPER_PROVIDER_ID
    {
        25 * 1024 * 1024
    } else {
        MAX_ONLINE_STREAM_DOWNLOAD_BYTES
    };
    let (bytes, media_type) = if is_network_stream {
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(15))
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| AsrPortError::runtime(format!("Failed to build HTTP client: {e}")))?;

        let response = client.get(trimmed_url).send().await.map_err(|e| {
            let kind = if e.is_timeout() {
                AsrPortErrorKind::Timeout
            } else {
                AsrPortErrorKind::Network
            };
            AsrPortError::new(kind, format!("Failed to fetch stream URL: {e}"))
        })?;

        let status = response.status();
        if !status.is_success() {
            let kind = if status.is_server_error() {
                AsrPortErrorKind::Unavailable
            } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                AsrPortErrorKind::RateLimited
            } else {
                AsrPortErrorKind::InvalidRequest
            };
            return Err(AsrPortError::new(
                kind,
                format!("Stream URL returned status {status}: {path_str}"),
            ));
        }

        if let Some(content_length) = response.content_length()
            && content_length > max_stream_bytes as u64
        {
            return Err(AsrPortError::new(
                AsrPortErrorKind::InvalidRequest,
                format!(
                    "Stream content length ({content_length} bytes) exceeds maximum limit ({max_stream_bytes} bytes)"
                ),
            ));
        }

        if let Some(content_type) = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
        {
            let ct_lower = content_type.to_ascii_lowercase();
            if ct_lower.starts_with("text/html")
                || ct_lower.starts_with("application/json")
                || ct_lower.contains("mpegurl")
                || ct_lower.contains("dash+xml")
            {
                return Err(AsrPortError::new(
                    AsrPortErrorKind::InvalidRequest,
                    format!("Stream URL returned non-audio Content-Type: {content_type}"),
                ));
            }
        }
        let header_mime = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .and_then(map_content_type_to_audio_mime);

        let mut stream_bytes = Vec::new();
        let mut resp = response;
        while let Some(chunk) = resp.chunk().await.map_err(|e| {
            let kind = if e.is_timeout() {
                AsrPortErrorKind::Timeout
            } else {
                AsrPortErrorKind::Network
            };
            AsrPortError::new(kind, format!("Failed reading stream chunk: {e}"))
        })? {
            if stream_bytes.len() + chunk.len() > max_stream_bytes {
                return Err(AsrPortError::new(
                    AsrPortErrorKind::InvalidRequest,
                    format!(
                        "Stream downloaded size exceeds maximum limit ({max_stream_bytes} bytes)"
                    ),
                ));
            }
            stream_bytes.extend_from_slice(&chunk);
        }

        let media_type = if let Some(mime) = header_mime {
            mime
        } else {
            let clean_url = trimmed_url
                .split('?')
                .next()
                .unwrap_or(trimmed_url)
                .split('#')
                .next()
                .unwrap_or(trimmed_url);
            let ext_mime = detect_audio_mime_type(Path::new(clean_url));
            let clean_lower = clean_url.to_ascii_lowercase();
            if ext_mime == "audio/wav" && !clean_lower.ends_with(".wav") {
                sniff_audio_mime_type(&stream_bytes).unwrap_or("audio/mp3")
            } else {
                ext_mime
            }
        };

        (stream_bytes, media_type)
    } else {
        let bytes = tokio::fs::read(&input.file_path).await.map_err(|error| {
            AsrPortError::new(
                AsrPortErrorKind::FileSystem,
                format!("Failed to read audio file: {error}"),
            )
        })?;
        let media_type = detect_audio_mime_type(&input.file_path);
        (bytes, media_type)
    };

    let buffered_samples = bytes.len() / 2;
    let mut call_options = TranscriptionCallOptions::new(AudioInput::Binary(bytes), media_type);
    // Build provider options
    let mut po = SharedProviderOptions::new();

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
        po.insert("openai".to_string(), openai_options);
    }

    let speaker_diarization_enabled = is_cloud_speaker_diarization_enabled(&input.request);

    // 2. Volcengine options
    let mut volc_options = serde_json::Map::new();
    volc_options.insert(
        "enable_itn".to_string(),
        serde_json::Value::Bool(input.request.enable_itn),
    );
    if speaker_diarization_enabled {
        volc_options.insert(
            "enable_speaker_info".to_string(),
            serde_json::Value::Bool(true),
        );
    }
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
    po.insert("volcengine".to_string(), volc_options);

    // 3. Deepgram options
    let mut deepgram_options = serde_json::Map::new();
    if speaker_diarization_enabled {
        deepgram_options.insert("diarize".to_string(), serde_json::Value::Bool(true));
        deepgram_options.insert("paragraphs".to_string(), serde_json::Value::Bool(true));
        deepgram_options.insert("utterances".to_string(), serde_json::Value::Bool(true));
    }
    if let Some(language) = normalize_language_hint(&input.request.language) {
        deepgram_options.insert("language".to_string(), serde_json::Value::String(language));
    }
    if !deepgram_options.is_empty() {
        po.insert("deepgram".to_string(), deepgram_options);
    }

    // 4. AssemblyAI options
    let mut assemblyai_options = serde_json::Map::new();
    if speaker_diarization_enabled {
        assemblyai_options.insert("speaker_labels".to_string(), serde_json::Value::Bool(true));
    }
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
        po.insert("assemblyai".to_string(), assemblyai_options);
    }

    // 5. ElevenLabs options
    let mut elevenlabs_options = serde_json::Map::new();
    if speaker_diarization_enabled {
        elevenlabs_options.insert("diarize".to_string(), serde_json::Value::Bool(true));
    }
    if let Some(language) = normalize_language_hint(&input.request.language) {
        elevenlabs_options.insert(
            "languageCode".to_string(),
            serde_json::Value::String(language),
        );
    }
    if !elevenlabs_options.is_empty() {
        po.insert("elevenlabs".to_string(), elevenlabs_options);
    }

    // 6. Azure Speech options
    let mut azure_options = serde_json::Map::new();
    azure_options.insert("api".to_string(), serde_json::json!("speech"));
    if let Some(language) = normalize_language_hint(&input.request.language) {
        azure_options.insert("locales".to_string(), serde_json::json!([language]));
    }
    if speaker_diarization_enabled {
        azure_options.insert(
            "diarization".to_string(),
            serde_json::json!({ "enabled": true }),
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
            azure_options.insert(
                "phraseList".to_string(),
                serde_json::json!({ "phrases": words }),
            );
        }
    }
    po.insert("azure".to_string(), azure_options);

    // 7. FAL options
    let mut fal_options = serde_json::Map::new();
    if let Some(language) = normalize_language_hint(&input.request.language) {
        fal_options.insert("language".to_string(), serde_json::Value::String(language));
    }
    fal_options.insert(
        "diarize".to_string(),
        serde_json::Value::Bool(speaker_diarization_enabled),
    );
    po.insert("fal".to_string(), fal_options);

    // 8. Google options
    let mut google_options = serde_json::Map::new();
    if let Some(language) = normalize_language_hint(&input.request.language) {
        google_options.insert("languageCodes".to_string(), serde_json::json!([language]));
    }
    if speaker_diarization_enabled {
        google_options.insert("diarization".to_string(), serde_json::Value::Bool(true));
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
            google_options.insert("customVocabulary".to_string(), serde_json::json!(words));
        }
    }
    if !google_options.is_empty() {
        po.insert("google".to_string(), google_options);
    }

    // 9. Gladia options
    let mut gladia_options = serde_json::Map::new();
    if let Some(language) = normalize_language_hint(&input.request.language) {
        gladia_options.insert("language".to_string(), serde_json::Value::String(language));
    }
    if speaker_diarization_enabled {
        gladia_options.insert("diarization".to_string(), serde_json::Value::Bool(true));
    }
    if !gladia_options.is_empty() {
        po.insert("gladia".to_string(), gladia_options);
    }

    // 10. Rev.ai options
    let mut revai_options = serde_json::Map::new();
    if let Some(language) = normalize_language_hint(&input.request.language) {
        revai_options.insert("language".to_string(), serde_json::Value::String(language));
    }
    if !revai_options.is_empty() {
        po.insert("revai".to_string(), revai_options);
    }

    if !po.is_empty() {
        call_options.provider_options = Some(po);
    }

    let result = model
        .do_generate(&call_options)
        .await
        .map_err(map_aimux_asr_error)?;

    let segments = if speaker_diarization_enabled
        && let Some(raw_body) = result.response.body.as_ref()
        && let Some(parsed) = parse_raw_provider_segments(model.provider(), raw_body)
        && !parsed.is_empty()
    {
        parsed
    } else {
        let mut fallback_segments: Vec<TranscriptSegment> = result
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
                draft_translation: None,
                speaker: None,
                speaker_attribution: None,
            })
            .collect();

        let trimmed_text = result.text.trim();
        if fallback_segments.is_empty() && !trimmed_text.is_empty() {
            let duration = result.duration_in_seconds.unwrap_or(0.0);
            fallback_segments.push(TranscriptSegment {
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
                draft_translation: None,
                speaker: None,
                speaker_attribution: None,
            });
        }
        fallback_segments
    };

    let audio_duration_ms = result.duration_in_seconds.unwrap_or(0.0) * 1000.0;
    let base_provider = model
        .provider()
        .split('.')
        .next()
        .unwrap_or(model.provider());
    let stage = format!("{}_batch_complete", base_provider.replace('-', "_"));

    Ok(OnlineBatchTranscriptionOutput {
        segments,
        audio_duration_ms,
        buffered_samples,
        stage,
    })
}

fn append_word_to_sentence(sentence: &mut String, word: &str) {
    let word_trimmed = word.trim();
    if word_trimmed.is_empty() {
        return;
    }
    if sentence.is_empty() {
        sentence.push_str(word_trimmed);
        return;
    }
    let last_char = sentence.chars().last().unwrap_or(' ');
    let first_char = word_trimmed.chars().next().unwrap_or(' ');

    if (sona_core::text::cjk::is_cjk_char(last_char)
        && sona_core::text::cjk::is_cjk_char(first_char))
        || matches!(
            first_char,
            ',' | '.' | '!' | '?' | ';' | ':' | '\'' | ')' | ']' | '}'
        )
    {
        sentence.push_str(word_trimmed);
    } else {
        sentence.push(' ');
        sentence.push_str(word_trimmed);
    }
}

fn extract_speaker_id_from_value(v: &Value) -> Option<String> {
    v.as_str()
        .map(ToString::to_string)
        .or_else(|| v.as_i64().map(|n| n.to_string()))
        .or_else(|| v.as_f64().map(|n| (n as i64).to_string()))
}

/// Attempt to parse rich provider-specific transcription segments from raw response body.
pub fn parse_raw_provider_segments(
    provider: &str,
    raw_body: &Value,
) -> Option<Vec<TranscriptSegment>> {
    let base_provider = provider.split('.').next().unwrap_or(provider);
    match base_provider {
        "volcengine" => segments_from_volcengine_response(raw_body, true, "volc-batch").ok(),
        "assemblyai" => parse_assemblyai_raw_segments(raw_body),
        "deepgram" => parse_deepgram_raw_segments(raw_body),
        "elevenlabs" => parse_elevenlabs_raw_segments(raw_body),
        "azure" => parse_azure_raw_segments(raw_body)
            .or_else(|| parse_openai_compatible_raw_segments(raw_body)),
        "gladia" => parse_gladia_raw_segments(raw_body),
        "revai" => parse_revai_raw_segments(raw_body),
        "openai" | "groq" | "mistral" | "xai" | "fal" => {
            parse_openai_compatible_raw_segments(raw_body)
        }
        _ => None,
    }
}

/// Parse AssemblyAI raw transcript response containing `utterances`.
pub fn parse_assemblyai_raw_segments(body: &Value) -> Option<Vec<TranscriptSegment>> {
    let utterances = body.get("utterances")?.as_array()?;
    if utterances.is_empty() {
        return None;
    }

    let mut segments = Vec::new();
    for (i, u) in utterances.iter().enumerate() {
        let text = u.get("text").and_then(Value::as_str)?.trim();
        if text.is_empty() {
            continue;
        }
        let start = u.get("start").and_then(Value::as_f64).unwrap_or(0.0) / 1000.0;
        let end = u
            .get("end")
            .and_then(Value::as_f64)
            .unwrap_or(start * 1000.0)
            / 1000.0;

        let (speaker, speaker_attribution) = u
            .get("speaker")
            .and_then(Value::as_str)
            .map(create_cloud_speaker)
            .map(|(tag, attr)| (Some(tag), Some(attr)))
            .unwrap_or((None, None));

        let mut timing_units = Vec::new();
        let mut tokens = Vec::new();
        let mut timestamps = Vec::new();
        let mut durations = Vec::new();

        if let Some(words) = u.get("words").and_then(Value::as_array) {
            for w in words {
                let w_text = w
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim();
                if w_text.is_empty() {
                    continue;
                }
                let w_start = w
                    .get("start")
                    .and_then(Value::as_f64)
                    .unwrap_or(start * 1000.0)
                    / 1000.0;
                let w_end = w
                    .get("end")
                    .and_then(Value::as_f64)
                    .unwrap_or(w_start * 1000.0)
                    / 1000.0;

                tokens.push(w_text.to_string());
                timestamps.push(w_start as f32);
                durations.push((w_end.max(w_start) - w_start) as f32);
                timing_units.push(TranscriptTimingUnit {
                    text: w_text.to_string(),
                    start: w_start,
                    end: w_end.max(w_start),
                });
            }
        }

        segments.push(TranscriptSegment {
            id: format!("aai-{i}"),
            text: text.to_string(),
            start,
            end: end.max(start),
            is_final: true,
            timing: (!timing_units.is_empty()).then_some(TranscriptTiming {
                level: TranscriptTimingLevel::Token,
                source: TranscriptTimingSource::Model,
                units: timing_units,
            }),
            tokens: (!tokens.is_empty()).then_some(tokens),
            timestamps: (!timestamps.is_empty()).then_some(timestamps),
            durations: (!durations.is_empty()).then_some(durations),
            translation: None,
            draft_translation: None,
            speaker,
            speaker_attribution,
        });
    }

    if segments.is_empty() {
        None
    } else {
        Some(segments)
    }
}

/// Parse Deepgram raw listen response containing `utterances`, `paragraphs`, or word clusters.
pub fn parse_deepgram_raw_segments(body: &Value) -> Option<Vec<TranscriptSegment>> {
    let alt = body
        .get("results")
        .and_then(|r| r.get("channels"))
        .and_then(Value::as_array)
        .and_then(|ch| ch.first())
        .and_then(|c| c.get("alternatives"))
        .and_then(Value::as_array)
        .and_then(|a| a.first());

    // 1. Try utterances
    let utterances = alt
        .and_then(|a| a.get("utterances"))
        .and_then(Value::as_array)
        .or_else(|| {
            body.get("results")
                .and_then(|r| r.get("utterances"))
                .and_then(Value::as_array)
        });

    if let Some(utterances) = utterances
        && !utterances.is_empty()
    {
        let mut segments = Vec::new();
        for (i, u) in utterances.iter().enumerate() {
            let text = u
                .get("transcript")
                .or_else(|| u.get("text"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim();
            if text.is_empty() {
                continue;
            }
            let start = u.get("start").and_then(Value::as_f64).unwrap_or(0.0);
            let end = u.get("end").and_then(Value::as_f64).unwrap_or(start);

            let speaker_id = u.get("speaker").and_then(extract_speaker_id_from_value);

            let (speaker, speaker_attribution) = match &speaker_id {
                Some(id) => {
                    let (tag, attr) = create_cloud_speaker(id);
                    (Some(tag), Some(attr))
                }
                None => (None, None),
            };

            let mut timing_units = Vec::new();
            let mut tokens = Vec::new();
            let mut timestamps = Vec::new();
            let mut durations = Vec::new();

            if let Some(words) = u.get("words").and_then(Value::as_array) {
                for w in words {
                    let w_text = w
                        .get("punctuated_word")
                        .or_else(|| w.get("word"))
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .trim();
                    if w_text.is_empty() {
                        continue;
                    }
                    let w_start = w.get("start").and_then(Value::as_f64).unwrap_or(start);
                    let w_end = w.get("end").and_then(Value::as_f64).unwrap_or(w_start);

                    tokens.push(w_text.to_string());
                    timestamps.push(w_start as f32);
                    durations.push((w_end.max(w_start) - w_start) as f32);
                    timing_units.push(TranscriptTimingUnit {
                        text: w_text.to_string(),
                        start: w_start,
                        end: w_end.max(w_start),
                    });
                }
            }

            segments.push(TranscriptSegment {
                id: format!("dg-utt-{i}"),
                text: text.to_string(),
                start,
                end: end.max(start),
                is_final: true,
                timing: (!timing_units.is_empty()).then_some(TranscriptTiming {
                    level: TranscriptTimingLevel::Token,
                    source: TranscriptTimingSource::Model,
                    units: timing_units,
                }),
                tokens: (!tokens.is_empty()).then_some(tokens),
                timestamps: (!timestamps.is_empty()).then_some(timestamps),
                durations: (!durations.is_empty()).then_some(durations),
                translation: None,
                draft_translation: None,
                speaker,
                speaker_attribution,
            });
        }
        if !segments.is_empty() {
            return Some(segments);
        }
    }

    // 2. Try paragraphs
    let paragraphs = alt
        .and_then(|a| a.get("paragraphs"))
        .and_then(|p| p.get("paragraphs"))
        .and_then(Value::as_array);

    if let Some(paragraphs) = paragraphs
        && !paragraphs.is_empty()
    {
        let mut segments = Vec::new();
        for (i, p) in paragraphs.iter().enumerate() {
            let start = p.get("start").and_then(Value::as_f64).unwrap_or(0.0);
            let end = p.get("end").and_then(Value::as_f64).unwrap_or(start);

            let speaker_id = p.get("speaker").and_then(extract_speaker_id_from_value);

            let (speaker, speaker_attribution) = match &speaker_id {
                Some(id) => {
                    let (tag, attr) = create_cloud_speaker(id);
                    (Some(tag), Some(attr))
                }
                None => (None, None),
            };

            let mut text = String::new();
            if let Some(sentences) = p.get("sentences").and_then(Value::as_array) {
                for s in sentences {
                    if let Some(st) = s.get("text").and_then(Value::as_str) {
                        append_word_to_sentence(&mut text, st);
                    }
                }
            }
            if text.trim().is_empty() {
                continue;
            }

            segments.push(TranscriptSegment {
                id: format!("dg-para-{i}"),
                text,
                start,
                end: end.max(start),
                is_final: true,
                timing: None,
                tokens: None,
                timestamps: None,
                durations: None,
                translation: None,
                draft_translation: None,
                speaker,
                speaker_attribution,
            });
        }
        if !segments.is_empty() {
            return Some(segments);
        }
    }

    // 3. Fallback: words clustering by speaker
    let words = alt.and_then(|a| a.get("words")).and_then(Value::as_array);
    if let Some(words) = words
        && !words.is_empty()
    {
        let mut segments = Vec::new();
        let mut current_speaker: Option<String> = None;
        let mut current_text = String::new();
        let mut current_start = 0.0;
        let mut current_end = 0.0;
        let mut current_timing = Vec::new();
        let mut current_tokens = Vec::new();
        let mut current_timestamps = Vec::new();
        let mut current_durations = Vec::new();

        let flush_segment = |segments: &mut Vec<TranscriptSegment>,
                             speaker_id: Option<String>,
                             text: &mut String,
                             start: f64,
                             end: f64,
                             timing: &mut Vec<TranscriptTimingUnit>,
                             tokens: &mut Vec<String>,
                             timestamps: &mut Vec<f32>,
                             durations: &mut Vec<f32>| {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                let (speaker, speaker_attribution) = match &speaker_id {
                    Some(id) => {
                        let (tag, attr) = create_cloud_speaker(id);
                        (Some(tag), Some(attr))
                    }
                    None => (None, None),
                };
                let idx = segments.len();
                segments.push(TranscriptSegment {
                    id: format!("dg-word-{idx}"),
                    text: trimmed.to_string(),
                    start,
                    end: end.max(start),
                    is_final: true,
                    timing: (!timing.is_empty()).then_some(TranscriptTiming {
                        level: TranscriptTimingLevel::Token,
                        source: TranscriptTimingSource::Model,
                        units: std::mem::take(timing),
                    }),
                    tokens: (!tokens.is_empty()).then_some(std::mem::take(tokens)),
                    timestamps: (!timestamps.is_empty()).then_some(std::mem::take(timestamps)),
                    durations: (!durations.is_empty()).then_some(std::mem::take(durations)),
                    translation: None,
                    draft_translation: None,
                    speaker,
                    speaker_attribution,
                });
            }
            text.clear();
        };

        for w in words {
            let w_text = w
                .get("punctuated_word")
                .or_else(|| w.get("word"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim();
            if w_text.is_empty() {
                continue;
            }
            let w_start = w.get("start").and_then(Value::as_f64).unwrap_or(0.0);
            let w_end = w.get("end").and_then(Value::as_f64).unwrap_or(w_start);
            let w_speaker = w.get("speaker").and_then(extract_speaker_id_from_value);

            let speaker_changed = w_speaker != current_speaker;
            let long_pause = !current_text.is_empty() && (w_start - current_end > 3.0);

            if (speaker_changed || long_pause) && !current_text.is_empty() {
                flush_segment(
                    &mut segments,
                    current_speaker.take(),
                    &mut current_text,
                    current_start,
                    current_end,
                    &mut current_timing,
                    &mut current_tokens,
                    &mut current_timestamps,
                    &mut current_durations,
                );
            }

            if current_text.is_empty() {
                current_start = w_start;
                current_speaker = w_speaker;
            }
            current_end = w_end;
            append_word_to_sentence(&mut current_text, w_text);

            current_tokens.push(w_text.to_string());
            current_timestamps.push(w_start as f32);
            current_durations.push((w_end.max(w_start) - w_start) as f32);
            current_timing.push(TranscriptTimingUnit {
                text: w_text.to_string(),
                start: w_start,
                end: w_end.max(w_start),
            });
        }

        if !current_text.is_empty() {
            flush_segment(
                &mut segments,
                current_speaker.take(),
                &mut current_text,
                current_start,
                current_end,
                &mut current_timing,
                &mut current_tokens,
                &mut current_timestamps,
                &mut current_durations,
            );
        }

        if !segments.is_empty() {
            return Some(segments);
        }
    }

    None
}

/// Parse ElevenLabs raw response containing `words` with `speaker_id`.
pub fn parse_elevenlabs_raw_segments(body: &Value) -> Option<Vec<TranscriptSegment>> {
    let words = body.get("words").and_then(Value::as_array)?;
    if words.is_empty() {
        return None;
    }

    let mut segments = Vec::new();
    let mut current_speaker: Option<String> = None;
    let mut current_text = String::new();
    let mut current_start = 0.0;
    let mut current_end = 0.0;
    let mut current_timing = Vec::new();
    let mut current_tokens = Vec::new();
    let mut current_timestamps = Vec::new();
    let mut current_durations = Vec::new();

    let flush_segment = |segments: &mut Vec<TranscriptSegment>,
                         speaker_id: Option<String>,
                         text: &mut String,
                         start: f64,
                         end: f64,
                         timing: &mut Vec<TranscriptTimingUnit>,
                         tokens: &mut Vec<String>,
                         timestamps: &mut Vec<f32>,
                         durations: &mut Vec<f32>| {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            let (speaker, speaker_attribution) = match &speaker_id {
                Some(id) => {
                    let (tag, attr) = create_cloud_speaker(id);
                    (Some(tag), Some(attr))
                }
                None => (None, None),
            };
            let idx = segments.len();
            segments.push(TranscriptSegment {
                id: format!("el-word-{idx}"),
                text: trimmed.to_string(),
                start,
                end: end.max(start),
                is_final: true,
                timing: (!timing.is_empty()).then_some(TranscriptTiming {
                    level: TranscriptTimingLevel::Token,
                    source: TranscriptTimingSource::Model,
                    units: std::mem::take(timing),
                }),
                tokens: (!tokens.is_empty()).then_some(std::mem::take(tokens)),
                timestamps: (!timestamps.is_empty()).then_some(std::mem::take(timestamps)),
                durations: (!durations.is_empty()).then_some(std::mem::take(durations)),
                translation: None,
                draft_translation: None,
                speaker,
                speaker_attribution,
            });
        }
        text.clear();
    };

    for w in words {
        let w_text = w
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        if w_text.is_empty() {
            continue;
        }
        let w_start = w.get("start").and_then(Value::as_f64).unwrap_or(0.0);
        let w_end = w.get("end").and_then(Value::as_f64).unwrap_or(w_start);
        let w_speaker = w.get("speaker_id").and_then(extract_speaker_id_from_value);

        let speaker_changed = w_speaker != current_speaker;
        let long_pause = !current_text.is_empty() && (w_start - current_end > 3.0);

        if (speaker_changed || long_pause) && !current_text.is_empty() {
            flush_segment(
                &mut segments,
                current_speaker.take(),
                &mut current_text,
                current_start,
                current_end,
                &mut current_timing,
                &mut current_tokens,
                &mut current_timestamps,
                &mut current_durations,
            );
        }

        if current_text.is_empty() {
            current_start = w_start;
            current_speaker = w_speaker;
        }
        current_end = w_end;
        append_word_to_sentence(&mut current_text, w_text);

        current_tokens.push(w_text.to_string());
        current_timestamps.push(w_start as f32);
        current_durations.push((w_end.max(w_start) - w_start) as f32);
        current_timing.push(TranscriptTimingUnit {
            text: w_text.to_string(),
            start: w_start,
            end: w_end.max(w_start),
        });
    }

    if !current_text.is_empty() {
        flush_segment(
            &mut segments,
            current_speaker.take(),
            &mut current_text,
            current_start,
            current_end,
            &mut current_timing,
            &mut current_tokens,
            &mut current_timestamps,
            &mut current_durations,
        );
    }

    if segments.is_empty() {
        None
    } else {
        Some(segments)
    }
}

/// Parse OpenAI / Mistral compatible response containing `segments` with `speaker` field.
pub fn parse_openai_compatible_raw_segments(body: &Value) -> Option<Vec<TranscriptSegment>> {
    let raw_segments = body.get("segments").and_then(Value::as_array)?;
    if raw_segments.is_empty() {
        return None;
    }

    let has_speaker = raw_segments
        .iter()
        .any(|s| s.get("speaker").is_some() || s.get("speaker_id").is_some());
    if !has_speaker {
        return None;
    }

    let mut segments = Vec::new();
    for (i, s) in raw_segments.iter().enumerate() {
        let text = s.get("text").and_then(Value::as_str)?.trim();
        let start = s.get("start").and_then(Value::as_f64).unwrap_or(0.0);
        let end = s.get("end").and_then(Value::as_f64).unwrap_or(start);

        let speaker_id = s
            .get("speaker")
            .or_else(|| s.get("speaker_id"))
            .and_then(extract_speaker_id_from_value);

        let (speaker, speaker_attribution) = match &speaker_id {
            Some(id) => {
                let (tag, attr) = create_cloud_speaker(id);
                (Some(tag), Some(attr))
            }
            None => (None, None),
        };

        segments.push(TranscriptSegment {
            id: format!("openai-seg-{i}"),
            text: text.to_string(),
            start,
            end: end.max(start),
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            draft_translation: None,
            speaker,
            speaker_attribution,
        });
    }

    if segments.is_empty() {
        None
    } else {
        Some(segments)
    }
}

/// Parse Azure Speech REST API response containing `phrases` with `offsetMilliseconds` and `speaker`.
pub fn parse_azure_raw_segments(body: &Value) -> Option<Vec<TranscriptSegment>> {
    let phrases = body.get("phrases").and_then(Value::as_array)?;
    if phrases.is_empty() {
        return None;
    }

    let mut segments = Vec::new();
    for (i, p) in phrases.iter().enumerate() {
        let text = p.get("text").and_then(Value::as_str)?.trim();
        if text.is_empty() {
            continue;
        }
        let offset_ms = p
            .get("offsetMilliseconds")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let duration_ms = p
            .get("durationMilliseconds")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let start = offset_ms / 1000.0;
        let end = start + (duration_ms / 1000.0);

        let speaker_id = p
            .get("speaker")
            .or_else(|| p.get("speaker_id"))
            .and_then(extract_speaker_id_from_value);

        let (speaker, speaker_attribution) = match &speaker_id {
            Some(id) => {
                let (tag, attr) = create_cloud_speaker(id);
                (Some(tag), Some(attr))
            }
            None => (None, None),
        };

        segments.push(TranscriptSegment {
            id: format!("azure-seg-{i}"),
            text: text.to_string(),
            start,
            end: end.max(start),
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            draft_translation: None,
            speaker,
            speaker_attribution,
        });
    }

    if segments.is_empty() {
        None
    } else {
        Some(segments)
    }
}

/// Parse Gladia API response containing utterances with speaker diarization.
pub fn parse_gladia_raw_segments(body: &Value) -> Option<Vec<TranscriptSegment>> {
    let utterances = body
        .get("utterances")
        .or_else(|| {
            body.get("result")
                .and_then(|r| r.get("transcription"))
                .and_then(|t| t.get("utterances"))
        })
        .or_else(|| body.get("prediction").and_then(|p| p.get("utterances")))
        .and_then(Value::as_array)?;

    if utterances.is_empty() {
        return None;
    }

    let mut segments = Vec::new();
    for (i, u) in utterances.iter().enumerate() {
        let text = u.get("text").and_then(Value::as_str)?.trim();
        if text.is_empty() {
            continue;
        }
        let start = u.get("start").and_then(Value::as_f64).unwrap_or(0.0);
        let end = u.get("end").and_then(Value::as_f64).unwrap_or(start);

        let speaker_id = u
            .get("speaker")
            .or_else(|| u.get("speaker_id"))
            .and_then(extract_speaker_id_from_value);

        let (speaker, speaker_attribution) = match &speaker_id {
            Some(id) => {
                let (tag, attr) = create_cloud_speaker(id);
                (Some(tag), Some(attr))
            }
            None => (None, None),
        };

        segments.push(TranscriptSegment {
            id: format!("gladia-seg-{i}"),
            text: text.to_string(),
            start,
            end: end.max(start),
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            draft_translation: None,
            speaker,
            speaker_attribution,
        });
    }

    if segments.is_empty() {
        None
    } else {
        Some(segments)
    }
}

/// Parse Rev.ai API response containing `monologues` with `speaker` and `elements`.
pub fn parse_revai_raw_segments(body: &Value) -> Option<Vec<TranscriptSegment>> {
    let monologues = body.get("monologues").and_then(Value::as_array)?;
    if monologues.is_empty() {
        return None;
    }

    let mut segments = Vec::new();
    for (i, m) in monologues.iter().enumerate() {
        let elements = m.get("elements").and_then(Value::as_array);
        let mut text = String::new();
        let mut start: Option<f64> = None;
        let mut end: Option<f64> = None;

        if let Some(elems) = elements {
            for elem in elems {
                if let Some(val) = elem.get("value").and_then(Value::as_str) {
                    text.push_str(val);
                }
                if start.is_none() {
                    start = elem.get("ts").and_then(Value::as_f64);
                }
                if let Some(e) = elem.get("end_ts").and_then(Value::as_f64) {
                    end = Some(e);
                }
            }
        }

        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        let s = start.unwrap_or(0.0);
        let e = end.unwrap_or(s);

        let speaker_id = m
            .get("speaker")
            .or_else(|| m.get("speaker_id"))
            .and_then(extract_speaker_id_from_value);

        let (speaker, speaker_attribution) = match &speaker_id {
            Some(id) => {
                let (tag, attr) = create_cloud_speaker(id);
                (Some(tag), Some(attr))
            }
            None => (None, None),
        };

        segments.push(TranscriptSegment {
            id: format!("revai-seg-{i}"),
            text: trimmed.to_string(),
            start: s,
            end: e.max(s),
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            draft_translation: None,
            speaker,
            speaker_attribution,
        });
    }

    if segments.is_empty() {
        None
    } else {
        Some(segments)
    }
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
    fn test_map_content_type_to_audio_mime() {
        assert_eq!(
            map_content_type_to_audio_mime("audio/mpeg"),
            Some("audio/mp3")
        );
        assert_eq!(
            map_content_type_to_audio_mime("audio/mp3; charset=utf-8"),
            Some("audio/mp3")
        );
        assert_eq!(
            map_content_type_to_audio_mime("audio/wav"),
            Some("audio/wav")
        );
        assert_eq!(
            map_content_type_to_audio_mime("audio/x-wav"),
            Some("audio/wav")
        );
        assert_eq!(
            map_content_type_to_audio_mime("audio/ogg"),
            Some("audio/ogg")
        );
        assert_eq!(
            map_content_type_to_audio_mime("application/ogg"),
            Some("audio/ogg")
        );
        assert_eq!(
            map_content_type_to_audio_mime("audio/m4a"),
            Some("audio/m4a")
        );
        assert_eq!(
            map_content_type_to_audio_mime("audio/mp4"),
            Some("audio/m4a")
        );
        assert_eq!(
            map_content_type_to_audio_mime("video/mp4"),
            Some("audio/m4a")
        );
        assert_eq!(
            map_content_type_to_audio_mime("audio/webm"),
            Some("audio/webm")
        );
        assert_eq!(
            map_content_type_to_audio_mime("video/webm"),
            Some("audio/webm")
        );
        assert_eq!(
            map_content_type_to_audio_mime("audio/aac"),
            Some("audio/aac")
        );
        assert_eq!(
            map_content_type_to_audio_mime("audio/flac"),
            Some("audio/flac")
        );
        assert_eq!(map_content_type_to_audio_mime("text/html"), None);
        assert_eq!(
            map_content_type_to_audio_mime("application/octet-stream"),
            None
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
        assert_eq!(model.provider(), "openai.transcription");
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
        assert_eq!(model.provider(), "groq.transcription");
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
        assert_eq!(model.provider(), "mistral.transcription");
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
        assert_eq!(model.provider(), "deepgram.transcription");
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
        assert_eq!(model.provider(), "assemblyai.transcription");
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
        assert_eq!(model.provider(), "elevenlabs.transcription");
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
        let auth_error = AiMuxError::ApiCall(Box::new(ApiCallError {
            url: "https://api.example.com".to_string(),
            request_body_values: serde_json::json!({}),
            status_code: Some(401),
            provider_code: None,
            message: "Unauthorized".to_string(),
            response_body: None,
            response_headers: None,
            data: None,
            cause: None,
            origin: aimux_core::error::ApiCallErrorOrigin::HttpResponse,
            is_retryable: false,
        }));
        assert_eq!(
            map_aimux_asr_error(auth_error).kind,
            AsrPortErrorKind::Authentication
        );

        let rate_limit_error = AiMuxError::ApiCall(Box::new(ApiCallError {
            url: "https://api.example.com".to_string(),
            request_body_values: serde_json::json!({}),
            status_code: Some(429),
            provider_code: None,
            message: "Rate limit reached".to_string(),
            response_body: None,
            response_headers: None,
            data: None,
            cause: None,
            origin: aimux_core::error::ApiCallErrorOrigin::HttpResponse,
            is_retryable: true,
        }));
        assert_eq!(
            map_aimux_asr_error(rate_limit_error).kind,
            AsrPortErrorKind::RateLimited
        );

        let unavailable_error = AiMuxError::ApiCall(Box::new(ApiCallError {
            url: "https://api.example.com".to_string(),
            request_body_values: serde_json::json!({}),
            status_code: Some(503),
            provider_code: None,
            message: "Service Unavailable".to_string(),
            response_body: None,
            response_headers: None,
            data: None,
            cause: None,
            origin: aimux_core::error::ApiCallErrorOrigin::HttpResponse,
            is_retryable: true,
        }));
        assert_eq!(
            map_aimux_asr_error(unavailable_error).kind,
            AsrPortErrorKind::Unavailable
        );

        let timeout_error = AiMuxError::Timeout("connection timed out".to_string());
        assert_eq!(
            map_aimux_asr_error(timeout_error).kind,
            AsrPortErrorKind::Timeout
        );

        let not_found_error = AiMuxError::ApiCall(Box::new(ApiCallError {
            url: "https://api.example.com".to_string(),
            request_body_values: serde_json::json!({}),
            status_code: Some(404),
            provider_code: None,
            message: "Not found".to_string(),
            response_body: None,
            response_headers: None,
            data: None,
            cause: None,
            origin: aimux_core::error::ApiCallErrorOrigin::HttpResponse,
            is_retryable: false,
        }));
        assert_eq!(
            map_aimux_asr_error(not_found_error).kind,
            AsrPortErrorKind::InvalidRequest
        );

        let payload_too_large = AiMuxError::ApiCall(Box::new(ApiCallError {
            url: "https://api.example.com".to_string(),
            request_body_values: serde_json::json!({}),
            status_code: Some(413),
            provider_code: None,
            message: "Payload Too Large".to_string(),
            response_body: None,
            response_headers: None,
            data: None,
            cause: None,
            origin: aimux_core::error::ApiCallErrorOrigin::HttpResponse,
            is_retryable: false,
        }));
        assert_eq!(
            map_aimux_asr_error(payload_too_large).kind,
            AsrPortErrorKind::InvalidRequest
        );

        let invalid_arg_error = AiMuxError::invalid_argument("bad param");
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
            model_id: "whisper-1".to_string(),
            model_type: "transcriptionModel".to_string(),
            available_providers: vec![],
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

        let parse_error = AiMuxError::json_parse("failed to parse");
        assert_eq!(
            map_aimux_asr_error(parse_error).kind,
            AsrPortErrorKind::Protocol
        );
    }

    #[test]
    fn test_create_cloud_speaker_labels() {
        let (tag0, attr0) = create_cloud_speaker("0");
        assert_eq!(tag0.id, "cloud-speaker-0");
        assert_eq!(tag0.label, "Speaker 1");
        assert_eq!(tag0.kind, "anonymous");
        assert_eq!(attr0.group_id, "cloud-speaker-0");
        assert_eq!(attr0.anonymous_label, "Speaker 1");
        assert_eq!(attr0.source, "cloud");

        let (tag1, attr1) = create_cloud_speaker("speaker_1");
        assert_eq!(tag1.id, "cloud-speaker-speaker_1");
        assert_eq!(tag1.label, "Speaker 2");
        assert_eq!(attr1.anonymous_label, "Speaker 2");

        let (tag_a, attr_a) = create_cloud_speaker("A");
        assert_eq!(tag_a.id, "cloud-speaker-A");
        assert_eq!(tag_a.label, "Speaker A");
        assert_eq!(attr_a.anonymous_label, "Speaker A");
    }

    #[test]
    fn test_is_cloud_speaker_diarization_enabled() {
        let req_default = online_request(VOLCENGINE_DOUBAO_PROVIDER_ID, json!({ "apiKey": "k" }));
        assert!(is_cloud_speaker_diarization_enabled(&req_default));

        let req_explicit_true = online_request(
            DEEPGRAM_PROVIDER_ID,
            json!({ "apiKey": "k", "speakerDiarization": true }),
        );
        assert!(is_cloud_speaker_diarization_enabled(&req_explicit_true));

        let req_disabled = online_request(
            DEEPGRAM_PROVIDER_ID,
            json!({ "apiKey": "k", "speakerDiarization": false }),
        );
        assert!(!is_cloud_speaker_diarization_enabled(&req_disabled));

        let req_disabled_snake = online_request(
            ASSEMBLYAI_PROVIDER_ID,
            json!({ "apiKey": "k", "speaker_diarization": false }),
        );
        assert!(!is_cloud_speaker_diarization_enabled(&req_disabled_snake));
    }

    #[test]
    fn test_parse_assemblyai_raw_segments() {
        let raw = json!({
            "status": "completed",
            "utterances": [
                {
                    "start": 0,
                    "end": 1500,
                    "speaker": "A",
                    "text": "Hello world.",
                    "words": [
                        { "start": 0, "end": 500, "text": "Hello", "confidence": 0.98, "speaker": "A" },
                        { "start": 550, "end": 1500, "text": "world.", "confidence": 0.99, "speaker": "A" }
                    ]
                },
                {
                    "start": 1600,
                    "end": 3000,
                    "speaker": "B",
                    "text": "Good morning.",
                    "words": [
                        { "start": 1600, "end": 2200, "text": "Good", "confidence": 0.95, "speaker": "B" },
                        { "start": 2250, "end": 3000, "text": "morning.", "confidence": 0.97, "speaker": "B" }
                    ]
                }
            ]
        });

        let segments = parse_assemblyai_raw_segments(&raw).expect("parsed segments");
        assert_eq!(segments.len(), 2);

        // Verify aimux 0.6 namespaced provider string works with parse_raw_provider_segments
        let parsed_namespaced = parse_raw_provider_segments("assemblyai.transcription", &raw)
            .expect("parsed namespaced provider segments");
        assert_eq!(parsed_namespaced.len(), 2);

        assert_eq!(segments[0].id, "aai-0");
        assert_eq!(segments[0].text, "Hello world.");
        assert_eq!(segments[0].start, 0.0);
        assert_eq!(segments[0].end, 1.5);
        assert_eq!(segments[0].speaker.as_ref().unwrap().label, "Speaker A");
        assert_eq!(segments[0].speaker.as_ref().unwrap().id, "cloud-speaker-A");
        assert_eq!(
            segments[0].tokens.as_ref().unwrap(),
            &vec!["Hello", "world."]
        );
        assert_eq!(segments[0].timestamps.as_ref().unwrap(), &vec![0.0, 0.55]);

        assert_eq!(segments[1].id, "aai-1");
        assert_eq!(segments[1].text, "Good morning.");
        assert_eq!(segments[1].start, 1.6);
        assert_eq!(segments[1].end, 3.0);
        assert_eq!(segments[1].speaker.as_ref().unwrap().label, "Speaker B");
    }

    #[test]
    fn test_parse_deepgram_raw_segments_utterances() {
        let raw = json!({
            "results": {
                "channels": [
                    {
                        "alternatives": [
                            {
                                "transcript": "Hello from Deepgram.",
                                "utterances": [
                                    {
                                        "start": 0.0,
                                        "end": 2.0,
                                        "transcript": "Hello from Deepgram.",
                                        "speaker": 0,
                                        "words": [
                                            { "punctuated_word": "Hello", "start": 0.0, "end": 0.6, "speaker": 0 },
                                            { "punctuated_word": "from", "start": 0.7, "end": 1.0, "speaker": 0 },
                                            { "punctuated_word": "Deepgram.", "start": 1.1, "end": 2.0, "speaker": 0 }
                                        ]
                                    }
                                ]
                            }
                        ]
                    }
                ]
            }
        });

        let segments = parse_deepgram_raw_segments(&raw).expect("parsed segments");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].id, "dg-utt-0");
        assert_eq!(segments[0].text, "Hello from Deepgram.");
        assert_eq!(segments[0].start, 0.0);
        assert_eq!(segments[0].end, 2.0);
        assert_eq!(segments[0].speaker.as_ref().unwrap().label, "Speaker 1");
        assert_eq!(segments[0].speaker.as_ref().unwrap().id, "cloud-speaker-0");
    }

    #[test]
    fn test_parse_deepgram_raw_segments_paragraphs() {
        let raw = json!({
            "results": {
                "channels": [
                    {
                        "alternatives": [
                            {
                                "paragraphs": {
                                    "paragraphs": [
                                        {
                                            "speaker": 1,
                                            "start": 1.0,
                                            "end": 3.5,
                                            "sentences": [
                                                { "text": "Paragraph one.", "start": 1.0, "end": 2.0 },
                                                { "text": "Second sentence.", "start": 2.1, "end": 3.5 }
                                            ]
                                        }
                                    ]
                                }
                            }
                        ]
                    }
                ]
            }
        });

        let segments = parse_deepgram_raw_segments(&raw).expect("parsed segments");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].id, "dg-para-0");
        assert_eq!(segments[0].text, "Paragraph one. Second sentence.");
        assert_eq!(segments[0].start, 1.0);
        assert_eq!(segments[0].end, 3.5);
        assert_eq!(segments[0].speaker.as_ref().unwrap().label, "Speaker 2");
        assert_eq!(segments[0].speaker.as_ref().unwrap().id, "cloud-speaker-1");
    }

    #[test]
    fn test_parse_deepgram_raw_segments_words() {
        let raw = json!({
            "results": {
                "channels": [
                    {
                        "alternatives": [
                            {
                                "words": [
                                    { "punctuated_word": "First", "start": 0.0, "end": 0.5, "speaker": 0 },
                                    { "punctuated_word": "speaker.", "start": 0.6, "end": 1.0, "speaker": 0 },
                                    { "punctuated_word": "Second", "start": 1.5, "end": 2.0, "speaker": 1 },
                                    { "punctuated_word": "speaker.", "start": 2.1, "end": 2.5, "speaker": 1 }
                                ]
                            }
                        ]
                    }
                ]
            }
        });

        let segments = parse_deepgram_raw_segments(&raw).expect("parsed segments");
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].text, "First speaker.");
        assert_eq!(segments[0].speaker.as_ref().unwrap().label, "Speaker 1");
        assert_eq!(segments[1].text, "Second speaker.");
        assert_eq!(segments[1].speaker.as_ref().unwrap().label, "Speaker 2");
    }

    #[test]
    fn test_parse_elevenlabs_raw_segments() {
        let raw = json!({
            "text": "Hello, how are you? I'm fine.",
            "words": [
                { "text": "Hello,", "start": 0.0, "end": 0.5, "speaker_id": "speaker_0" },
                { "text": "how", "start": 0.6, "end": 0.8, "speaker_id": "speaker_0" },
                { "text": "are", "start": 0.9, "end": 1.0, "speaker_id": "speaker_0" },
                { "text": "you?", "start": 1.1, "end": 1.5, "speaker_id": "speaker_0" },
                { "text": "I'm", "start": 2.0, "end": 2.4, "speaker_id": "speaker_1" },
                { "text": "fine.", "start": 2.5, "end": 2.9, "speaker_id": "speaker_1" }
            ]
        });

        let segments = parse_elevenlabs_raw_segments(&raw).expect("parsed segments");
        assert_eq!(segments.len(), 2);

        assert_eq!(segments[0].text, "Hello, how are you?");
        assert_eq!(segments[0].start, 0.0);
        assert_eq!(segments[0].end, 1.5);
        assert_eq!(segments[0].speaker.as_ref().unwrap().label, "Speaker 1");
        assert_eq!(
            segments[0].speaker.as_ref().unwrap().id,
            "cloud-speaker-speaker_0"
        );

        assert_eq!(segments[1].text, "I'm fine.");
        assert_eq!(segments[1].start, 2.0);
        assert_eq!(segments[1].end, 2.9);
        assert_eq!(segments[1].speaker.as_ref().unwrap().label, "Speaker 2");
        assert_eq!(
            segments[1].speaker.as_ref().unwrap().id,
            "cloud-speaker-speaker_1"
        );
    }

    #[test]
    fn test_parse_openai_compatible_raw_segments() {
        let raw_with_speaker = json!({
            "segments": [
                {
                    "id": 0,
                    "start": 0.0,
                    "end": 2.0,
                    "text": "Hello from speaker A",
                    "speaker": "A"
                },
                {
                    "id": 1,
                    "start": 2.5,
                    "end": 4.0,
                    "text": "Hello from speaker B",
                    "speaker": "B"
                }
            ]
        });

        let segments =
            parse_openai_compatible_raw_segments(&raw_with_speaker).expect("parsed segments");
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].speaker.as_ref().unwrap().label, "Speaker A");
        assert_eq!(segments[1].speaker.as_ref().unwrap().label, "Speaker B");

        let raw_without_speaker = json!({
            "segments": [
                {
                    "id": 0,
                    "start": 0.0,
                    "end": 2.0,
                    "text": "No speaker info here"
                }
            ]
        });
        assert!(parse_openai_compatible_raw_segments(&raw_without_speaker).is_none());
    }

    #[test]
    fn test_append_word_to_sentence() {
        let mut sentence = String::new();
        append_word_to_sentence(&mut sentence, "Hello");
        append_word_to_sentence(&mut sentence, ",");
        append_word_to_sentence(&mut sentence, "world");
        append_word_to_sentence(&mut sentence, "!");
        assert_eq!(sentence, "Hello, world!");

        let mut cjk = String::new();
        append_word_to_sentence(&mut cjk, "你好");
        append_word_to_sentence(&mut cjk, "世界");
        assert_eq!(cjk, "你好世界");
    }

    #[test]
    fn test_sniff_audio_mime_type() {
        assert_eq!(
            sniff_audio_mime_type(&[0x1A, 0x45, 0xDF, 0xA3, 0x01]),
            Some("audio/webm")
        );
        assert_eq!(
            sniff_audio_mime_type(b"RIFF\0\0\0\0WAVE"),
            Some("audio/wav")
        );
        assert_eq!(sniff_audio_mime_type(b"ID3\x03\0"), Some("audio/mp3"));
        assert_eq!(sniff_audio_mime_type(b"\0\0\0\x18ftyp"), Some("audio/m4a"));
        assert_eq!(sniff_audio_mime_type(b"fLaC\0\0"), Some("audio/flac"));
        assert_eq!(sniff_audio_mime_type(b"unknown bytes"), None);
    }

    #[test]
    fn creates_aimux_model_for_azure_speech() {
        let request = online_request(
            AZURE_SPEECH_PROVIDER_ID,
            json!({
                "apiKey": "test-azure-key",
                "model": "whisper-1",
                "batchEndpoint": "https://eastus.api.cognitive.microsoft.com"
            }),
        );

        let model = create_aimux_transcription_model(&request).unwrap();
        assert_eq!(model.provider(), "azure.transcription");
        assert_eq!(model.model_id(), "whisper-1");
    }

    #[test]
    fn creates_aimux_model_for_google_gemini() {
        let request = online_request(
            GOOGLE_GEMINI_ASR_PROVIDER_ID,
            json!({
                "apiKey": "test-google-key",
                "model": "gemini-2.0-flash"
            }),
        );

        let model = create_aimux_transcription_model(&request).unwrap();
        assert_eq!(model.provider(), "google.generative-ai.transcription");
        assert_eq!(model.model_id(), "gemini-2.0-flash");
    }

    #[test]
    fn creates_aimux_model_for_xai() {
        let request = online_request(
            XAI_ASR_PROVIDER_ID,
            json!({
                "apiKey": "test-xai-key",
                "model": "default"
            }),
        );

        let model = create_aimux_transcription_model(&request).unwrap();
        assert_eq!(model.provider(), "xai.transcription");
        assert_eq!(model.model_id(), "");
    }

    #[test]
    fn creates_aimux_model_for_gladia() {
        let request = online_request(
            GLADIA_PROVIDER_ID,
            json!({
                "apiKey": "test-gladia-key",
                "model": "default"
            }),
        );

        let model = create_aimux_transcription_model(&request).unwrap();
        assert_eq!(model.provider(), "gladia.transcription");
        assert_eq!(model.model_id(), "default");
    }

    #[test]
    fn creates_aimux_model_for_revai() {
        let request = online_request(
            REVAI_PROVIDER_ID,
            json!({
                "apiKey": "test-revai-key",
                "model": "default"
            }),
        );

        let model = create_aimux_transcription_model(&request).unwrap();
        assert_eq!(model.provider(), "revai.transcription");
        assert_eq!(model.model_id(), "default");
    }

    #[test]
    fn creates_aimux_model_for_fal() {
        let request = online_request(
            FAL_WHISPER_PROVIDER_ID,
            json!({
                "apiKey": "test-fal-key",
                "model": "whisper"
            }),
        );

        let model = create_aimux_transcription_model(&request).unwrap();
        assert_eq!(model.provider(), "fal.transcription");
        assert_eq!(model.model_id(), "whisper");
    }

    #[test]
    fn test_parse_azure_raw_segments() {
        let raw = json!({
            "phrases": [
                {
                    "text": "Hello Azure",
                    "offsetMilliseconds": 0.0,
                    "durationMilliseconds": 1200.0,
                    "speaker": 1
                },
                {
                    "text": "World Azure",
                    "offsetMilliseconds": 1300.0,
                    "durationMilliseconds": 1000.0,
                    "speaker": 2
                }
            ]
        });
        let segments = parse_azure_raw_segments(&raw).expect("parsed azure segments");
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].text, "Hello Azure");
        assert_eq!(segments[0].speaker.as_ref().unwrap().label, "Speaker 2");
        assert_eq!(segments[1].text, "World Azure");
        assert_eq!(segments[1].speaker.as_ref().unwrap().label, "Speaker 3");
    }

    #[test]
    fn test_parse_gladia_raw_segments() {
        let raw = json!({
            "utterances": [
                {
                    "text": "Hello Gladia",
                    "start": 0.5,
                    "end": 1.5,
                    "speaker": 0
                }
            ]
        });
        let segments = parse_gladia_raw_segments(&raw).expect("parsed gladia segments");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "Hello Gladia");
        assert_eq!(segments[0].start, 0.5);
        assert_eq!(segments[0].end, 1.5);
    }

    #[test]
    fn test_parse_revai_raw_segments() {
        let raw = json!({
            "monologues": [
                {
                    "speaker": 1,
                    "elements": [
                        {"type": "text", "value": "Hello", "ts": 0.0, "end_ts": 0.5},
                        {"type": "punct", "value": " "},
                        {"type": "text", "value": "Revai", "ts": 0.6, "end_ts": 1.2}
                    ]
                }
            ]
        });
        let segments = parse_revai_raw_segments(&raw).expect("parsed revai segments");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "Hello Revai");
        assert_eq!(segments[0].start, 0.0);
        assert_eq!(segments[0].end, 1.2);
        assert_eq!(segments[0].speaker.as_ref().unwrap().label, "Speaker 2");
    }

    #[test]
    fn test_extract_speaker_id_from_value() {
        assert_eq!(
            extract_speaker_id_from_value(&json!("speaker_1")),
            Some("speaker_1".to_string())
        );
        assert_eq!(
            extract_speaker_id_from_value(&json!(1)),
            Some("1".to_string())
        );
        assert_eq!(
            extract_speaker_id_from_value(&json!(2.0)),
            Some("2".to_string())
        );
        assert_eq!(extract_speaker_id_from_value(&json!(null)), None);
    }
}
