use async_trait::async_trait;
use serde_json::Value;
use sona_core::ports::asr::{
    ASSEMBLYAI_PROVIDER_ID, AsrEngineConfig, AsrMode, AsrPortError, AsrPortErrorKind,
    AsrRuntimeObserver, AsrStreamingSession, AsrTranscriptionRequest, DEEPGRAM_PROVIDER_ID,
    ELEVENLABS_PROVIDER_ID, GROQ_WHISPER_PROVIDER_ID, MISTRAL_VOXTRAL_PROVIDER_ID,
    OPENAI_WHISPER_PROVIDER_ID, OnlineBatchTranscriberPort, OnlineBatchTranscriptionOutput,
    OnlineBatchTranscriptionRequest, VOLCENGINE_DOUBAO_PROVIDER_ID, find_online_asr_provider,
};
use sona_core::transcription::provider_resolution::{
    AsrProviderCapability, resolve_asr_provider_id, resolve_asr_streaming_provider_id,
};
use sona_core::transcription::transcript::{
    SpeakerAttribution, SpeakerTag, TranscriptSegment, TranscriptTiming, TranscriptTimingLevel,
    TranscriptTimingSource, TranscriptTimingUnit,
};
use std::fmt;
use std::sync::Arc;

pub mod aimux_adapter;
pub mod assemblyai;
pub mod deepgram;
pub mod elevenlabs;
pub mod error;
pub mod mistral;
pub mod streaming_common;
pub mod volcengine;

pub use aimux_adapter::{
    OnlineProviderConfigFields, create_aimux_transcription_model, detect_audio_mime_type,
    execute_aimux_batch, is_cloud_speaker_diarization_enabled, resolve_online_provider_config,
};
pub use assemblyai::create_assemblyai_streaming_session;
pub use deepgram::create_deepgram_streaming_session;
pub use elevenlabs::create_elevenlabs_streaming_session;
pub use error::{SherpaError, map_aimux_asr_error};
pub use mistral::create_mistral_streaming_session;
pub use volcengine::VolcengineTranscriptionModel;
pub use volcengine::streaming::create_volcengine_streaming_session;

pub const ONLINE_ASR_PROVIDER_CAPABILITIES: [AsrProviderCapability<'static>; 7] = [
    AsrProviderCapability::new(VOLCENGINE_DOUBAO_PROVIDER_ID, true),
    AsrProviderCapability::new(OPENAI_WHISPER_PROVIDER_ID, false),
    AsrProviderCapability::new(GROQ_WHISPER_PROVIDER_ID, false),
    AsrProviderCapability::new(MISTRAL_VOXTRAL_PROVIDER_ID, true),
    AsrProviderCapability::new(DEEPGRAM_PROVIDER_ID, true),
    AsrProviderCapability::new(ASSEMBLYAI_PROVIDER_ID, true),
    AsrProviderCapability::new(ELEVENLABS_PROVIDER_ID, true),
];

pub fn resolve_online_asr_provider_id(
    request: &AsrTranscriptionRequest,
) -> Result<&'static str, AsrPortError> {
    resolve_asr_provider_id(request, &ONLINE_ASR_PROVIDER_CAPABILITIES)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct OnlineAsrAdapter;

impl OnlineAsrAdapter {
    pub async fn transcribe_batch(
        &self,
        input: OnlineBatchTranscriptionRequest,
    ) -> Result<OnlineBatchTranscriptionOutput, AsrPortError> {
        let model = create_aimux_transcription_model(&input.request)?;
        execute_aimux_batch(model.as_ref(), input).await
    }

    pub fn create_streaming_session(
        &self,
        instance_id: String,
        request: AsrTranscriptionRequest,
        observer: Arc<dyn AsrRuntimeObserver>,
    ) -> Result<Arc<dyn AsrStreamingSession>, AsrPortError> {
        let provider_id =
            resolve_asr_streaming_provider_id(&request, &ONLINE_ASR_PROVIDER_CAPABILITIES)?;
        match provider_id {
            VOLCENGINE_DOUBAO_PROVIDER_ID => {
                create_volcengine_streaming_session(instance_id, request, observer)
            }
            DEEPGRAM_PROVIDER_ID => {
                create_deepgram_streaming_session(instance_id, request, observer)
            }
            ASSEMBLYAI_PROVIDER_ID => {
                create_assemblyai_streaming_session(instance_id, request, observer)
            }
            ELEVENLABS_PROVIDER_ID => {
                create_elevenlabs_streaming_session(instance_id, request, observer)
            }
            MISTRAL_VOXTRAL_PROVIDER_ID => {
                create_mistral_streaming_session(instance_id, request, observer)
            }
            _ => Err(AsrPortError::new(
                AsrPortErrorKind::Unsupported,
                format!("不支持的在线 ASR provider：{provider_id}"),
            )
            .with_code("UNSUPPORTED_ONLINE_PROVIDER")),
        }
    }
}

#[async_trait]
impl OnlineBatchTranscriberPort for OnlineAsrAdapter {
    async fn transcribe(
        &self,
        request: OnlineBatchTranscriptionRequest,
    ) -> Result<OnlineBatchTranscriptionOutput, AsrPortError> {
        self.transcribe_batch(request).await
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VolcengineMode {
    Streaming,
    Batch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolcengineDoubaoConfigFields {
    pub api_key: String,
    pub streaming_endpoint: String,
    pub streaming_resource_id: String,
    pub batch_endpoint: String,
    pub batch_resource_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VolcengineConfigError {
    ProviderConfigMissing,
    UnsupportedProvider { provider_id: String },
    ManifestMissing,
    ManifestDefaultsInvalid,
    ApiKeyMissing,
    StreamingConfigMissing,
    BatchConfigMissing,
    LocalFileBatchUnsupported,
}

impl fmt::Display for VolcengineConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProviderConfigMissing => {
                write!(f, "Volcengine ASR provider config is missing.")
            }
            Self::UnsupportedProvider { provider_id } => {
                write!(f, "Unsupported Volcengine ASR provider: {provider_id}")
            }
            Self::ManifestMissing => write!(f, "Volcengine Doubao provider not found in manifest"),
            Self::ManifestDefaultsInvalid => {
                write!(f, "Volcengine Doubao provider defaults should be an object")
            }
            Self::ApiKeyMissing => write!(f, "Volcengine API Key is not configured."),
            Self::StreamingConfigMissing => write!(
                f,
                "Volcengine streaming endpoint or resource id is not configured."
            ),
            Self::BatchConfigMissing => {
                write!(
                    f,
                    "Volcengine batch endpoint or resource id is not configured."
                )
            }
            Self::LocalFileBatchUnsupported => write!(
                f,
                "Volcengine local file batch supports only recognize/flash endpoints."
            ),
        }
    }
}

impl std::error::Error for VolcengineConfigError {}

#[derive(Clone)]
pub struct VolcengineDoubaoBatchTranscriber {
    client: reqwest::Client,
}

impl Default for VolcengineDoubaoBatchTranscriber {
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl VolcengineDoubaoBatchTranscriber {
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl OnlineBatchTranscriberPort for VolcengineDoubaoBatchTranscriber {
    async fn transcribe(
        &self,
        input: OnlineBatchTranscriptionRequest,
    ) -> Result<OnlineBatchTranscriptionOutput, AsrPortError> {
        if input.request.mode != AsrMode::Batch {
            return Err(AsrPortError::from(SherpaError::VolcengineBatchModeMismatch));
        }

        let config = resolve_volcengine_config(&input.request, VolcengineMode::Batch)
            .map_err(AsrPortError::from)?;
        let model = VolcengineTranscriptionModel::with_client(config, self.client.clone());
        execute_aimux_batch(&model, input).await
    }
}

pub fn resolve_volcengine_config(
    request: &sona_core::ports::asr::AsrTranscriptionRequest,
    mode: VolcengineMode,
) -> Result<VolcengineDoubaoConfigFields, SherpaError> {
    resolve_volcengine_config_checked(request, mode).map_err(SherpaError::from)
}

pub fn resolve_volcengine_config_checked(
    request: &sona_core::ports::asr::AsrTranscriptionRequest,
    mode: VolcengineMode,
) -> Result<VolcengineDoubaoConfigFields, VolcengineConfigError> {
    let provider_request = if let AsrEngineConfig::Online { provider } = &request.engine_config {
        provider
    } else {
        return Err(VolcengineConfigError::ProviderConfigMissing);
    };

    if provider_request.provider_id != VOLCENGINE_DOUBAO_PROVIDER_ID {
        return Err(VolcengineConfigError::UnsupportedProvider {
            provider_id: provider_request.provider_id.clone(),
        });
    }

    resolve_volcengine_config_from_value_checked(&provider_request.config, mode)
}

pub fn resolve_volcengine_config_from_value(
    request_config: &Value,
    mode: VolcengineMode,
) -> Result<VolcengineDoubaoConfigFields, SherpaError> {
    resolve_volcengine_config_from_value_checked(request_config, mode).map_err(SherpaError::from)
}

pub fn resolve_volcengine_config_from_value_checked(
    request_config: &Value,
    mode: VolcengineMode,
) -> Result<VolcengineDoubaoConfigFields, VolcengineConfigError> {
    let manifest = find_online_asr_provider(VOLCENGINE_DOUBAO_PROVIDER_ID)
        .ok_or(VolcengineConfigError::ManifestMissing)?;
    let defaults = manifest
        .defaults
        .as_object()
        .ok_or(VolcengineConfigError::ManifestDefaultsInvalid)?;

    let fields = VolcengineDoubaoConfigFields {
        api_key: get_json_string(
            request_config,
            "apiKey",
            defaults.get("apiKey").and_then(Value::as_str).unwrap_or(""),
        ),
        streaming_endpoint: get_json_string(
            request_config,
            "streamingEndpoint",
            defaults
                .get("streamingEndpoint")
                .and_then(Value::as_str)
                .unwrap_or(""),
        ),
        streaming_resource_id: get_json_string(
            request_config,
            "streamingResourceId",
            defaults
                .get("streamingResourceId")
                .and_then(Value::as_str)
                .unwrap_or(""),
        ),
        batch_endpoint: get_json_string(
            request_config,
            "batchEndpoint",
            defaults
                .get("batchEndpoint")
                .and_then(Value::as_str)
                .unwrap_or(""),
        ),
        batch_resource_id: get_json_string(
            request_config,
            "batchResourceId",
            defaults
                .get("batchResourceId")
                .and_then(Value::as_str)
                .unwrap_or(""),
        ),
    };

    if fields.api_key.is_empty() {
        return Err(VolcengineConfigError::ApiKeyMissing);
    }
    match mode {
        VolcengineMode::Streaming => {
            if fields.streaming_endpoint.is_empty() || fields.streaming_resource_id.is_empty() {
                return Err(VolcengineConfigError::StreamingConfigMissing);
            }
        }
        VolcengineMode::Batch => {
            if fields.batch_endpoint.is_empty() || fields.batch_resource_id.is_empty() {
                return Err(VolcengineConfigError::BatchConfigMissing);
            }
            if !fields.batch_endpoint.starts_with("https://")
                && !fields.batch_endpoint.starts_with("http://")
            {
                return Err(VolcengineConfigError::LocalFileBatchUnsupported);
            }
            if fields.batch_endpoint.contains("idle/submit")
                || fields.batch_endpoint.ends_with("/submit")
            {
                return Err(VolcengineConfigError::LocalFileBatchUnsupported);
            }
        }
    }

    Ok(fields)
}

fn get_json_string(config: &Value, key: &str, default_val: &str) -> String {
    config
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_val)
        .to_string()
}

pub fn build_volcengine_flash_batch_request_body(
    file_path: &std::path::Path,
    audio_data: String,
    request: &sona_core::ports::asr::AsrTranscriptionRequest,
) -> Value {
    let audio_format = detect_audio_format(file_path);
    let mut body = serde_json::json!({
        "user": { "uid": "sona" },
        "audio": { "format": audio_format, "data": audio_data },
        "request": {
            "model_name": "bigmodel",
            "enable_itn": request.enable_itn,
            "enable_punc": true,
            "show_utterances": true
        }
    });
    if aimux_adapter::is_cloud_speaker_diarization_enabled(request) {
        body["request"]["enable_speaker_info"] = serde_json::json!(true);
    }
    body
}

pub fn detect_audio_format(file_path: &std::path::Path) -> &'static str {
    match file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("mp3") => "mp3",
        Some("wav") => "wav",
        Some("pcm") => "pcm",
        Some("ogg") | Some("oga") => "ogg",
        Some("m4a") => "m4a",
        Some("aac") => "aac",
        Some("flac") => "flac",
        Some("wma") => "wma",
        Some("amr") => "amr",
        Some("opus") => "opus",
        Some("webm") => "webm",
        _ => "wav",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VolcengineServerFrameError {
    FrameTooShort,
    ErrorFrame,
    ErrorCodeParseFailed,
    ErrorLengthParseFailed,
    ApiError { code: u32, message: String },
    PayloadLengthMissing,
    PayloadLengthParseFailed,
    PayloadIncomplete,
    ResponseParseFailed { error: String },
}

impl fmt::Display for VolcengineServerFrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FrameTooShort => write!(f, "Volcengine ASR response frame is too short."),
            Self::ErrorFrame => write!(f, "Volcengine ASR returned an invalid error frame."),
            Self::ErrorCodeParseFailed => write!(f, "Volcengine ASR error code parse failed."),
            Self::ErrorLengthParseFailed => {
                write!(f, "Volcengine ASR error length parse failed.")
            }
            Self::ApiError { code, message } => {
                write!(f, "Volcengine ASR returned API error {code}: {message}")
            }
            Self::PayloadLengthMissing => {
                write!(f, "Volcengine ASR response payload length is missing.")
            }
            Self::PayloadLengthParseFailed => {
                write!(f, "Volcengine ASR response payload length parse failed.")
            }
            Self::PayloadIncomplete => write!(f, "Volcengine ASR response payload is incomplete."),
            Self::ResponseParseFailed { error } => {
                write!(f, "Volcengine ASR response parse failed: {error}")
            }
        }
    }
}

impl std::error::Error for VolcengineServerFrameError {}

pub fn build_volcengine_full_client_request_frame(
    enable_itn: bool,
    enable_punc: bool,
    language: &str,
    hotwords: Option<&str>,
    enable_speaker_info: bool,
) -> Result<Vec<u8>, SherpaError> {
    let mut request = serde_json::json!({
        "user": {
            "uid": "sona"
        },
        "audio": {
            "format": "pcm",
            "codec": "raw",
            "rate": 16000,
            "bits": 16,
            "channel": 1
        },
        "request": {
            "model_name": "bigmodel",
            "enable_itn": enable_itn,
            "enable_punc": enable_punc,
            "show_utterances": true,
            "result_type": "full"
        }
    });
    if enable_speaker_info {
        request["request"]["enable_speaker_info"] = serde_json::json!(true);
    }

    if language != "auto" {
        request["audio"]["language"] = serde_json::json!(language);
    }
    if let Some(hotwords) = hotwords.filter(|value| !value.trim().is_empty()) {
        request["request"]["corpus"] = serde_json::json!({
            "context": serde_json::to_string(&serde_json::json!({
                "hotwords": hotwords
                    .split(',')
                    .map(str::trim)
                    .filter(|word| !word.is_empty())
                    .map(|word| serde_json::json!({ "word": word }))
                    .collect::<Vec<_>>()
            })).unwrap_or_default()
        });
    }

    let payload =
        serde_json::to_vec(&request).map_err(|error| SherpaError::Generic(error.to_string()))?;
    Ok(build_volcengine_frame(0x10, 0x10, &payload))
}

pub fn build_volcengine_audio_frame(samples: &[u8], is_final: bool) -> Vec<u8> {
    let flags = if is_final { 0x22 } else { 0x20 };
    build_volcengine_frame(flags, 0x00, samples)
}

fn build_volcengine_frame(
    message_type_and_flags: u8,
    serialization_and_compression: u8,
    payload: &[u8],
) -> Vec<u8> {
    let mut frame = Vec::with_capacity(8 + payload.len());
    frame.extend_from_slice(&[
        0x11,
        message_type_and_flags,
        serialization_and_compression,
        0x00,
    ]);
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(payload);
    frame
}

#[derive(Debug, Clone, PartialEq)]
pub struct VolcengineServerResponseFrame {
    pub payload: Option<Value>,
    pub is_final: bool,
}

pub fn parse_volcengine_server_response_frame(
    frame: &[u8],
) -> Result<VolcengineServerResponseFrame, VolcengineServerFrameError> {
    if frame.len() < 8 {
        return Err(VolcengineServerFrameError::FrameTooShort);
    }
    let message_type = frame[1] >> 4;
    if message_type == 0x0f {
        if frame.len() < 12 {
            return Err(VolcengineServerFrameError::ErrorFrame);
        }
        let code = u32::from_be_bytes(
            frame[4..8]
                .try_into()
                .map_err(|_| VolcengineServerFrameError::ErrorCodeParseFailed)?,
        );
        let size = u32::from_be_bytes(
            frame[8..12]
                .try_into()
                .map_err(|_| VolcengineServerFrameError::ErrorLengthParseFailed)?,
        ) as usize;
        let message = frame
            .get(12..12 + size)
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .unwrap_or("unknown error");
        return Err(VolcengineServerFrameError::ApiError {
            code,
            message: message.to_string(),
        });
    }
    if message_type != 0x09 {
        return Ok(VolcengineServerResponseFrame {
            payload: None,
            is_final: false,
        });
    }

    let header_size = ((frame[0] & 0x0f) as usize) * 4;
    let flags = frame[1] & 0x0f;
    let mut offset = header_size;
    let sequence = if flags & 0x01 != 0 {
        let sequence = frame
            .get(offset..offset + 4)
            .ok_or(VolcengineServerFrameError::PayloadLengthMissing)?;
        offset += 4;
        Some(i32::from_be_bytes(sequence.try_into().map_err(|_| {
            VolcengineServerFrameError::PayloadLengthParseFailed
        })?))
    } else {
        None
    };
    if frame.len() < offset + 4 {
        return Err(VolcengineServerFrameError::PayloadLengthMissing);
    }
    let payload_size = u32::from_be_bytes(
        frame[offset..offset + 4]
            .try_into()
            .map_err(|_| VolcengineServerFrameError::PayloadLengthParseFailed)?,
    ) as usize;
    let payload_start = offset + 4;
    let Some(payload) = frame.get(payload_start..payload_start + payload_size) else {
        return Err(VolcengineServerFrameError::PayloadIncomplete);
    };
    let payload = serde_json::from_slice(payload).map_err(|error| {
        VolcengineServerFrameError::ResponseParseFailed {
            error: error.to_string(),
        }
    })?;
    Ok(VolcengineServerResponseFrame {
        payload: Some(payload),
        is_final: flags & 0x02 != 0 || sequence.is_some_and(|value| value < 0),
    })
}

pub fn volcengine_streaming_segments_from_response(
    response: &Value,
    flush_final: bool,
) -> Result<Vec<TranscriptSegment>, SherpaError> {
    segments_from_volcengine_response(response, flush_final, "volc-live")
}

pub fn f32_samples_to_i16_pcm_bytes(samples: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let value = (clamped * i16::MAX as f32) as i16;
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

pub fn segments_from_volcengine_response(
    response: &Value,
    default_final: bool,
    id_prefix: &str,
) -> Result<Vec<TranscriptSegment>, SherpaError> {
    let result = response.get("result").unwrap_or(response);
    let utterances = result.get("utterances").and_then(Value::as_array);
    if let Some(utterances) = utterances {
        let mut segments = Vec::new();
        for (index, utterance) in utterances.iter().enumerate() {
            if let Some(segment) =
                volcengine_segment_from_utterance(utterance, index, default_final, id_prefix)
            {
                segments.push(segment);
            }
        }
        if !segments.is_empty() {
            return Ok(segments);
        }
    }

    let text = result
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let duration = response
        .get("audio_info")
        .and_then(|value| value.get("duration"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
        / 1000.0;
    Ok(vec![TranscriptSegment {
        id: format!("{id_prefix}-0"),
        text: text.to_string(),
        start: 0.0,
        end: duration.max(0.0),
        is_final: default_final,
        timing: None,
        tokens: None,
        timestamps: None,
        durations: None,
        translation: None,
        speaker: None,
        speaker_attribution: None,
    }])
}

fn volcengine_segment_from_utterance(
    utterance: &Value,
    index: usize,
    default_final: bool,
    id_prefix: &str,
) -> Option<TranscriptSegment> {
    let text = utterance.get("text").and_then(Value::as_str)?.trim();
    if text.is_empty() {
        return None;
    }
    let start = ms_value(utterance.get("start_time")).unwrap_or(0.0);
    let end = ms_value(utterance.get("end_time")).unwrap_or(start);
    let is_final = utterance
        .get("definite")
        .and_then(Value::as_bool)
        .unwrap_or(default_final);

    let speaker_info = utterance
        .get("additions")
        .and_then(|a| a.get("speaker_id"))
        .or_else(|| utterance.get("speaker_id"))
        .or_else(|| utterance.get("speaker"))
        .and_then(|v| {
            v.as_str()
                .map(|s| s.trim().to_string())
                .or_else(|| v.as_i64().map(|n| n.to_string()))
        })
        .filter(|s| !s.is_empty());

    let (speaker, speaker_attribution) = match &speaker_info {
        Some(id) => {
            let (tag, attr) = create_cloud_speaker(id);
            (Some(tag), Some(attr))
        }
        None => (None, None),
    };

    let words = utterance.get("words").and_then(Value::as_array);
    let mut tokens = Vec::new();
    let mut timestamps = Vec::new();
    let mut durations = Vec::new();
    let mut timing_units = Vec::new();
    if let Some(words) = words {
        for word in words {
            let word_text = word.get("text").and_then(Value::as_str).unwrap_or_default();
            if word_text.is_empty() {
                continue;
            }
            let word_start = ms_value(word.get("start_time")).unwrap_or(start);
            let word_end = ms_value(word.get("end_time"))
                .unwrap_or(word_start)
                .max(word_start);
            tokens.push(word_text.to_string());
            timestamps.push(word_start as f32);
            durations.push((word_end - word_start) as f32);
            timing_units.push(TranscriptTimingUnit {
                text: word_text.to_string(),
                start: word_start,
                end: word_end,
            });
        }
    }

    Some(TranscriptSegment {
        id: format!("{id_prefix}-{index}"),
        text: text.to_string(),
        start,
        end: end.max(start),
        is_final,
        timing: (!timing_units.is_empty()).then_some(TranscriptTiming {
            level: TranscriptTimingLevel::Token,
            source: TranscriptTimingSource::Model,
            units: timing_units,
        }),
        tokens: (!tokens.is_empty()).then_some(tokens),
        timestamps: (!timestamps.is_empty()).then_some(timestamps),
        durations: (!durations.is_empty()).then_some(durations),
        translation: None,
        speaker,
        speaker_attribution,
    })
}

pub fn create_cloud_speaker(raw_id: &str) -> (SpeakerTag, SpeakerAttribution) {
    let raw_trimmed = raw_id.trim();
    let numeric = if let Ok(n) = raw_trimmed.parse::<usize>() {
        Some(n + 1)
    } else if let Some(stripped) = raw_trimmed
        .strip_prefix("speaker_")
        .or_else(|| raw_trimmed.strip_prefix("speaker-"))
    {
        stripped.parse::<usize>().ok().map(|n| n + 1)
    } else {
        None
    };

    let label = if let Some(n) = numeric {
        format!("Speaker {n}")
    } else {
        format!("Speaker {raw_trimmed}")
    };
    let group_id = format!("cloud-speaker-{raw_trimmed}");

    (
        SpeakerTag {
            id: group_id.clone(),
            label: label.clone(),
            kind: "anonymous".to_string(),
            score: None,
        },
        SpeakerAttribution {
            group_id,
            anonymous_label: label,
            state: "anonymous".to_string(),
            source: "cloud".to_string(),
            confidence: "high".to_string(),
            candidates: Vec::new(),
        },
    )
}

fn ms_value(value: Option<&Value>) -> Option<f64> {
    value.and_then(Value::as_f64).map(|value| value / 1000.0)
}

pub fn map_volcengine_status_error(
    status: u16,
    api_code: Option<&str>,
    api_message: Option<&str>,
) -> String {
    let code = api_code.unwrap_or_default();
    let message = api_message.unwrap_or_default();
    let category = match (status, code) {
        (401 | 403, _) => "Volcengine authentication failed",
        (429, _) | (_, "55000031") => "Volcengine quota or rate limit reached",
        (400, _) | (_, "45000001" | "45000002" | "45000151") => {
            "Volcengine request configuration error"
        }
        (500..=599, _) => "Volcengine service error",
        _ => "Volcengine ASR request failed",
    };
    if code.is_empty() && message.is_empty() {
        format!("{category} (HTTP {status})")
    } else {
        format!("{category} (HTTP {status}, code {code}: {message})")
    }
}

#[cfg(test)]
mod tests {
    use crate::SherpaError;
    use serde_json::json;
    use sona_core::ports::asr::{
        ASSEMBLYAI_PROVIDER_ID, AsrEngineConfig, AsrMode, AsrPortErrorKind,
        AsrTranscriptionRequest, DEEPGRAM_PROVIDER_ID, ELEVENLABS_PROVIDER_ID,
        GROQ_WHISPER_PROVIDER_ID, MISTRAL_VOXTRAL_PROVIDER_ID, NoopAsrRuntimeObserver,
        OPENAI_WHISPER_PROVIDER_ID, OnlineAsrProviderRequest, OnlineBatchTranscriberPort,
        OnlineBatchTranscriptionRequest, VOLCENGINE_DOUBAO_PROVIDER_ID,
    };
    use sona_core::transcription::postprocess::{
        TranscriptNormalizationOptions, TranscriptPostprocessOptions,
    };

    use crate::{
        ONLINE_ASR_PROVIDER_CAPABILITIES, OnlineAsrAdapter, VolcengineConfigError, VolcengineMode,
        build_volcengine_audio_frame, build_volcengine_flash_batch_request_body,
        build_volcengine_full_client_request_frame, detect_audio_format,
        f32_samples_to_i16_pcm_bytes, parse_volcengine_server_response_frame,
        resolve_online_asr_provider_id, resolve_volcengine_config,
        resolve_volcengine_config_checked, segments_from_volcengine_response,
        volcengine_streaming_segments_from_response,
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
    fn online_runtime_adapter_owns_the_provider_capability_table() {
        let capabilities = ONLINE_ASR_PROVIDER_CAPABILITIES
            .iter()
            .map(|capability| (capability.provider_id, capability.supports_streaming))
            .collect::<Vec<_>>();

        assert_eq!(
            capabilities,
            vec![
                (VOLCENGINE_DOUBAO_PROVIDER_ID, true),
                (OPENAI_WHISPER_PROVIDER_ID, false),
                (GROQ_WHISPER_PROVIDER_ID, false),
                (MISTRAL_VOXTRAL_PROVIDER_ID, true),
                (DEEPGRAM_PROVIDER_ID, true),
                (ASSEMBLYAI_PROVIDER_ID, true),
                (ELEVENLABS_PROVIDER_ID, true),
            ]
        );
    }

    #[test]
    fn online_runtime_adapter_implements_the_core_batch_port() {
        fn assert_batch_port<T: OnlineBatchTranscriberPort>() {}

        assert_batch_port::<OnlineAsrAdapter>();
    }

    #[tokio::test]
    async fn online_runtime_adapter_routes_batch_requests_before_network_access() {
        let request = online_request(
            VOLCENGINE_DOUBAO_PROVIDER_ID,
            json!({
                "apiKey": "  "
            }),
        );

        let error = OnlineAsrAdapter
            .transcribe_batch(OnlineBatchTranscriptionRequest {
                file_path: "missing.wav".into(),
                request,
            })
            .await
            .expect_err("invalid provider config should fail before reading the audio file");

        assert_eq!(error.code(), "VOLCENGINE_API_KEY_MISSING");
    }

    #[test]
    fn online_runtime_adapter_routes_streaming_requests_before_network_access() {
        let mut groq = online_request(
            GROQ_WHISPER_PROVIDER_ID,
            json!({
                "apiKey": "groq-key"
            }),
        );
        groq.mode = AsrMode::Streaming;

        let error = OnlineAsrAdapter
            .create_streaming_session(
                "live-groq".to_string(),
                groq,
                std::sync::Arc::new(NoopAsrRuntimeObserver),
            )
            .err()
            .expect("batch-only providers must be rejected before network access");
        assert_eq!(error.code(), "STREAMING_NOT_SUPPORTED");
        assert!(error.message.contains(GROQ_WHISPER_PROVIDER_ID));

        let mut volcengine = online_request(
            VOLCENGINE_DOUBAO_PROVIDER_ID,
            json!({
                "apiKey": "  "
            }),
        );
        volcengine.mode = AsrMode::Streaming;

        let error = OnlineAsrAdapter
            .create_streaming_session(
                "live-volcengine".to_string(),
                volcengine,
                std::sync::Arc::new(NoopAsrRuntimeObserver),
            )
            .err()
            .expect("invalid provider config should fail before network access");
        assert_eq!(error.code(), "VOLCENGINE_API_KEY_MISSING");
    }

    #[test]
    fn online_runtime_adapter_rejects_unknown_providers() {
        let request = online_request("custom-online-provider", json!({}));

        let error = resolve_online_asr_provider_id(&request)
            .expect_err("unregistered providers must not cross the adapter boundary");

        assert_eq!(error.code(), "UNSUPPORTED_ONLINE_PROVIDER");
        assert!(error.message.contains("custom-online-provider"));
    }

    #[tokio::test]
    async fn core_batch_port_preserves_unsupported_and_filesystem_categories() {
        let unsupported = OnlineBatchTranscriberPort::transcribe(
            &OnlineAsrAdapter,
            OnlineBatchTranscriptionRequest {
                file_path: "missing.wav".into(),
                request: online_request("custom-online-provider", json!({})),
            },
        )
        .await
        .unwrap_err();
        assert_eq!(unsupported.kind, AsrPortErrorKind::Unsupported);

        let missing_audio = OnlineBatchTranscriberPort::transcribe(
            &OnlineAsrAdapter,
            OnlineBatchTranscriptionRequest {
                file_path: "missing.wav".into(),
                request: online_request(
                    GROQ_WHISPER_PROVIDER_ID,
                    json!({
                        "apiKey": "groq-key"
                    }),
                ),
            },
        )
        .await
        .unwrap_err();
        assert_eq!(missing_audio.kind, AsrPortErrorKind::FileSystem);
    }

    #[test]
    fn online_asr_manifest_contains_expected_providers() {
        use sona_core::models::preset_models::LanguageMode;
        use sona_core::ports::asr::{find_online_asr_provider, online_asr_providers};

        for provider_id in [
            VOLCENGINE_DOUBAO_PROVIDER_ID,
            OPENAI_WHISPER_PROVIDER_ID,
            GROQ_WHISPER_PROVIDER_ID,
            MISTRAL_VOXTRAL_PROVIDER_ID,
            DEEPGRAM_PROVIDER_ID,
            ASSEMBLYAI_PROVIDER_ID,
            ELEVENLABS_PROVIDER_ID,
        ] {
            let provider = find_online_asr_provider(provider_id)
                .unwrap_or_else(|| panic!("{provider_id} should exist in the manifest"));
            assert_eq!(provider.language_mode, LanguageMode::Selectable);
            assert!(!provider.languages.is_empty());
        }
        assert_eq!(online_asr_providers().len(), 7);
        let volcengine = find_online_asr_provider(VOLCENGINE_DOUBAO_PROVIDER_ID).unwrap();
        assert!(volcengine.languages.contains(&"zh".to_string()));
    }
    #[test]
    fn resolves_volcengine_batch_config_from_request_with_manifest_defaults() {
        let request = online_request(
            VOLCENGINE_DOUBAO_PROVIDER_ID,
            json!({
                "apiKey": " volc-key ",
                "batchResourceId": " custom-batch-resource "
            }),
        );

        let config = resolve_volcengine_config(&request, VolcengineMode::Batch).unwrap();

        assert_eq!(config.api_key, "volc-key");
        assert_eq!(
            config.batch_endpoint,
            "https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash"
        );
        assert_eq!(config.batch_resource_id, "custom-batch-resource");
    }

    #[test]
    fn resolves_volcengine_config_with_typed_errors() {
        let missing_key = online_request(
            VOLCENGINE_DOUBAO_PROVIDER_ID,
            json!({
                "apiKey": "  "
            }),
        );
        let error =
            resolve_volcengine_config_checked(&missing_key, VolcengineMode::Batch).unwrap_err();
        assert_eq!(error, VolcengineConfigError::ApiKeyMissing);
        assert!(matches!(
            resolve_volcengine_config(&missing_key, VolcengineMode::Batch).unwrap_err(),
            SherpaError::VolcengineApiKeyMissing
        ));

        let unsupported_provider = online_request(
            "other-volcengine",
            json!({
                "apiKey": "volc-key"
            }),
        );
        let error =
            resolve_volcengine_config_checked(&unsupported_provider, VolcengineMode::Streaming)
                .unwrap_err();
        assert_eq!(
            error,
            VolcengineConfigError::UnsupportedProvider {
                provider_id: "other-volcengine".to_string()
            }
        );

        let async_endpoint = online_request(
            VOLCENGINE_DOUBAO_PROVIDER_ID,
            json!({
                "apiKey": "volc-key",
                "batchEndpoint": "https://openspeech.bytedance.com/api/v3/auc/bigmodel/submit",
                "batchResourceId": "volc.seedasr.auc"
            }),
        );
        let error =
            resolve_volcengine_config_checked(&async_endpoint, VolcengineMode::Batch).unwrap_err();
        assert_eq!(error, VolcengineConfigError::LocalFileBatchUnsupported);
        assert!(error.to_string().contains("recognize/flash"));
    }

    #[test]
    fn builds_volcengine_flash_batch_request_body_with_local_audio_data() {
        let request = online_request(
            VOLCENGINE_DOUBAO_PROVIDER_ID,
            json!({
                "apiKey": "volc-key"
            }),
        );

        let body = build_volcengine_flash_batch_request_body(
            std::path::Path::new("C:/recordings/meeting.mp3"),
            "bG9jYWwtYXVkaW8=".to_string(),
            &request,
        );

        assert_eq!(body["audio"]["format"], "mp3");
        assert_eq!(body["audio"]["data"], "bG9jYWwtYXVkaW8=");
        assert!(body["audio"].get("url").is_none());
        assert_eq!(body["request"]["enable_itn"], false);
        assert_eq!(body["request"]["enable_punc"], true);
        assert_eq!(body["request"]["show_utterances"], true);
        assert_eq!(body["request"]["enable_speaker_info"], true);

        let mut request_disabled = request.clone();
        if let sona_core::ports::asr::AsrEngineConfig::Online { provider } =
            &mut request_disabled.engine_config
        {
            provider.config = json!({
                "apiKey": "volc-key",
                "speakerDiarization": false
            });
        }
        let body_disabled = build_volcengine_flash_batch_request_body(
            std::path::Path::new("C:/recordings/meeting.mp3"),
            "bG9jYWwtYXVkaW8=".to_string(),
            &request_disabled,
        );
        assert!(
            body_disabled["request"]
                .get("enable_speaker_info")
                .is_none()
        );
    }

    #[test]
    fn detects_volcengine_audio_format_from_extension() {
        assert_eq!(
            detect_audio_format(std::path::Path::new("recording.mp3")),
            "mp3"
        );
        assert_eq!(
            detect_audio_format(std::path::Path::new("recording.MP3")),
            "mp3"
        );
        assert_eq!(
            detect_audio_format(std::path::Path::new("recording.wav")),
            "wav"
        );
        assert_eq!(
            detect_audio_format(std::path::Path::new("recording.flac")),
            "flac"
        );
        assert_eq!(
            detect_audio_format(std::path::Path::new("recording.m4a")),
            "m4a"
        );
        assert_eq!(
            detect_audio_format(std::path::Path::new("recording.ogg")),
            "ogg"
        );
        assert_eq!(
            detect_audio_format(std::path::Path::new("recording.oga")),
            "ogg"
        );
        assert_eq!(
            detect_audio_format(std::path::Path::new("recording.aac")),
            "aac"
        );
        assert_eq!(
            detect_audio_format(std::path::Path::new("recording.opus")),
            "opus"
        );
        assert_eq!(
            detect_audio_format(std::path::Path::new("recording.webm")),
            "webm"
        );
        assert_eq!(
            detect_audio_format(std::path::Path::new("recording.pcm")),
            "pcm"
        );
        assert_eq!(
            detect_audio_format(std::path::Path::new("recording.amr")),
            "amr"
        );
        assert_eq!(
            detect_audio_format(std::path::Path::new("recording.wma")),
            "wma"
        );
        assert_eq!(
            detect_audio_format(std::path::Path::new("recording.xyz")),
            "wav"
        );
        assert_eq!(
            detect_audio_format(std::path::Path::new("recording")),
            "wav"
        );
        assert_eq!(detect_audio_format(std::path::Path::new("")), "wav");
    }

    #[test]
    fn parses_volcengine_utterances_and_word_timing() {
        let response = json!({
            "audio_info": { "duration": 2499 },
            "result": {
                "text": "hello world",
                "utterances": [
                    {
                        "end_time": 1530,
                        "start_time": 450,
                        "text": "hello",
                        "definite": true,
                        "words": [
                            { "end_time": 770, "start_time": 450, "text": "hel" },
                            { "end_time": 1530, "start_time": 770, "text": "lo" }
                        ]
                    }
                ]
            }
        });

        let segments = segments_from_volcengine_response(&response, true, "volc").unwrap();

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].id, "volc-0");
        assert_eq!(segments[0].text, "hello");
        assert_eq!(segments[0].start, 0.45);
        assert_eq!(segments[0].end, 1.53);
        assert!(segments[0].is_final);
        assert_eq!(segments[0].tokens.as_ref().unwrap(), &vec!["hel", "lo"]);
        assert_eq!(segments[0].timestamps.as_ref().unwrap(), &vec![0.45, 0.77]);
        assert_eq!(segments[0].durations.as_ref().unwrap(), &vec![0.32, 0.76]);
    }

    #[test]
    fn parses_volcengine_utterances_with_speaker_id() {
        let response = json!({
            "result": {
                "utterances": [
                    {
                        "start_time": 0,
                        "end_time": 1500,
                        "text": "first speaker",
                        "definite": true,
                        "additions": {
                            "speaker_id": "0"
                        }
                    },
                    {
                        "start_time": 1600,
                        "end_time": 3000,
                        "text": "second speaker",
                        "definite": true,
                        "speaker_id": 1
                    }
                ]
            }
        });

        let segments = segments_from_volcengine_response(&response, true, "volc").unwrap();
        assert_eq!(segments.len(), 2);

        let spk0 = segments[0].speaker.as_ref().unwrap();
        assert_eq!(spk0.id, "cloud-speaker-0");
        assert_eq!(spk0.label, "Speaker 1");
        assert_eq!(spk0.kind, "anonymous");
        assert_eq!(
            segments[0].speaker_attribution.as_ref().unwrap().group_id,
            "cloud-speaker-0"
        );
        assert_eq!(
            segments[0].speaker_attribution.as_ref().unwrap().source,
            "cloud"
        );

        let spk1 = segments[1].speaker.as_ref().unwrap();
        assert_eq!(spk1.id, "cloud-speaker-1");
        assert_eq!(spk1.label, "Speaker 2");
    }

    #[test]
    fn builds_volcengine_streaming_frames_with_expected_headers() {
        let request_frame =
            build_volcengine_full_client_request_frame(true, true, "auto", None, true)
                .expect("request frame");
        let audio_frame = build_volcengine_audio_frame(&[1, 2, 3, 4], false);
        let final_audio_frame = build_volcengine_audio_frame(&[], true);

        assert_eq!(&request_frame[0..4], &[0x11, 0x10, 0x10, 0x00]);
        assert_eq!(
            u32::from_be_bytes(request_frame[4..8].try_into().unwrap()) as usize,
            request_frame.len() - 8
        );
        let payload_str = String::from_utf8_lossy(&request_frame[8..]);
        assert!(payload_str.contains("\"model_name\":\"bigmodel\""));
        assert!(payload_str.contains("\"enable_speaker_info\":true"));

        let frame_no_speaker =
            build_volcengine_full_client_request_frame(true, true, "auto", None, false)
                .expect("frame without speaker");
        let payload_no_spk = String::from_utf8_lossy(&frame_no_speaker[8..]);
        assert!(!payload_no_spk.contains("enable_speaker_info"));

        assert_eq!(&audio_frame[0..4], &[0x11, 0x20, 0x00, 0x00]);
        assert_eq!(u32::from_be_bytes(audio_frame[4..8].try_into().unwrap()), 4);
        assert_eq!(&audio_frame[8..], &[1, 2, 3, 4]);
        assert_eq!(&final_audio_frame[0..4], &[0x11, 0x22, 0x00, 0x00]);
    }

    #[test]
    fn parses_volcengine_streaming_server_response_frame() {
        let payload = serde_json::to_vec(&json!({
            "result": {
                "utterances": [
                    { "start_time": 0, "end_time": 500, "text": "hello", "definite": true }
                ]
            }
        }))
        .unwrap();
        let mut frame = vec![0x11, 0x93, 0x10, 0x00];
        frame.extend_from_slice(&(-1_i32).to_be_bytes());
        frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        frame.extend_from_slice(&payload);

        let parsed = parse_volcengine_server_response_frame(&frame).expect("frame should parse");
        let segments = volcengine_streaming_segments_from_response(
            parsed.payload.as_ref().expect("payload should be present"),
            parsed.is_final,
        )
        .expect("segments");

        assert_eq!(
            (
                parsed.is_final,
                segments[0].text.as_str(),
                segments[0].is_final
            ),
            (true, "hello", true),
        );
    }

    #[test]
    fn converts_f32_samples_to_i16_pcm_bytes() {
        let bytes = f32_samples_to_i16_pcm_bytes(&[0.0_f32, 1.0, -1.0, 0.5]);

        assert_eq!(bytes.len(), 8);
        assert_eq!(i16::from_le_bytes([bytes[0], bytes[1]]), 0);
        assert_eq!(i16::from_le_bytes([bytes[2], bytes[3]]), i16::MAX);
        assert_eq!(i16::from_le_bytes([bytes[4], bytes[5]]), -i16::MAX);
        assert_eq!(
            i16::from_le_bytes([bytes[6], bytes[7]]),
            (0.5 * i16::MAX as f32) as i16
        );
    }
}
