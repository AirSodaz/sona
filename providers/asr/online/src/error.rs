use serde::Serialize;
use sona_core::ports::asr::{AsrPortError, AsrPortErrorKind};

/// Provider-specific runtime errors for ASR streaming and batch operations.
///
/// This type lives in the `sona-online-asr` adapter and must not be used at
/// Core port boundaries.  Adapter code converts it to [`AsrPortError`] using
/// [`From<SherpaError> for AsrPortError`]; the conversion preserves the
/// stable public code string so that FFI and HTTP consumers continue to receive
/// the same `{code, message}` payload they received before.
#[derive(thiserror::Error, Debug, Clone)]
pub enum SherpaError {
    #[error("在线 ASR provider 配置缺失。")]
    OnlineProviderConfigMissing,

    #[error("不支持的在线 ASR provider：{provider_id}")]
    UnsupportedOnlineProvider { provider_id: String },

    #[error("在线 ASR session 未初始化。")]
    OnlineSessionNotInitialized,

    #[error("provider {provider_id} 不支持流式识别")]
    StreamingNotSupported { provider_id: String },

    #[error("火山 ASR API Key 未配置。")]
    VolcengineApiKeyMissing,

    #[error("火山实时 ASR endpoint 或 Resource ID 未配置。")]
    VolcengineStreamingConfigMissing,

    #[error("火山批量 ASR endpoint 或 Resource ID 未配置。")]
    VolcengineBatchConfigMissing,

    #[error("火山 ASR provider 配置缺失。")]
    VolcengineProviderConfigMissing,

    #[error("不支持的火山 ASR provider：{provider_id}")]
    UnsupportedVolcengineProvider { provider_id: String },

    #[error("火山 ASR provider 配置无效：{error}")]
    VolcengineProviderConfigInvalid { error: String },

    #[error("火山 ASR 响应帧过短。")]
    VolcengineFrameTooShort,

    #[error("火山 ASR 返回错误帧。")]
    VolcengineErrorFrame,

    #[error("火山错误码解析失败。")]
    VolcengineErrorCodeParseFailed,

    #[error("火山错误长度解析失败。")]
    VolcengineErrorLengthParseFailed,

    #[error("火山 ASR 返回错误：{code} {message}")]
    VolcengineApiError { code: u32, message: String },

    #[error("火山 ASR 响应 payload 长度缺失。")]
    VolcenginePayloadLengthMissing,

    #[error("火山 ASR 响应 payload 长度解析失败。")]
    VolcenginePayloadLengthParseFailed,

    #[error("火山 ASR 响应 payload 不完整。")]
    VolcenginePayloadIncomplete,

    #[error("火山 ASR 响应解析失败：{error}")]
    VolcengineResponseParseFailed { error: String },

    #[error("火山 ASR WebSocket endpoint 无效：{error}")]
    VolcengineEndpointInvalid { error: String },

    #[error("火山 ASR WebSocket 连接失败：{error}")]
    VolcengineConnectionFailed { error: String },

    #[error("火山 ASR 初始化帧发送失败：{error}")]
    VolcengineInitFrameSendFailed { error: String },

    #[error("火山 ASR WebSocket 尚未连接。")]
    VolcengineWebSocketNotConnected,

    #[error("火山 ASR WebSocket 读取失败：{error}")]
    VolcengineWebSocketReadFailed { error: String },

    #[error("火山 ASR WebSocket 连接意外关闭。")]
    VolcengineWebSocketClosed,

    #[error("等待火山 ASR 最终响应超时。")]
    VolcengineFinalResponseTimeout,

    #[error("火山 ASR 音频发送失败：{error}")]
    VolcengineAudioSendFailed { error: String },

    #[error("火山 ASR 结束帧发送失败：{error}")]
    VolcengineEndFrameSendFailed { error: String },

    #[error("读取音频文件失败：{error}")]
    AudioFileReadFailed { error: String },

    #[error("火山批量 ASR 网络请求失败：{error}")]
    VolcengineBatchRequestFailed { error: String },

    #[error("火山批量 ASR 响应解析失败：{error}")]
    VolcengineBatchResponseParseFailed { error: String },

    #[error("{message}")]
    VolcengineLocalFileBatchUnsupported { message: String },

    #[error("火山实时 ASR 只能用于 streaming 槽位。")]
    VolcengineRealtimeOnlyForStreaming,

    #[error("火山批量 ASR 只能用于 batch 槽位。")]
    VolcengineBatchModeMismatch,

    #[error("{provider} WebSocket 尚未连接。")]
    StreamingWebSocketNotConnected { provider: &'static str },

    #[error("{provider} WebSocket 读取失败：{error}")]
    StreamingWebSocketReadFailed {
        provider: &'static str,
        error: String,
    },

    #[error("{provider} WebSocket 连接意外关闭。")]
    StreamingWebSocketClosed { provider: &'static str },

    #[error("等待 {provider} 最终响应超时。")]
    StreamingFinalResponseTimeout { provider: &'static str },

    #[error("{provider} 音频发送失败：{error}")]
    StreamingAudioSendFailed {
        provider: &'static str,
        error: String,
    },

    #[error("{provider} 结束帧发送失败：{error}")]
    StreamingEndFrameSendFailed {
        provider: &'static str,
        error: String,
    },

    #[error("{provider} WebSocket 连接失败：{error}")]
    StreamingConnectionFailed {
        provider: &'static str,
        error: String,
    },

    #[error("{provider} WebSocket endpoint 无效：{error}")]
    StreamingEndpointInvalid {
        provider: &'static str,
        error: String,
    },

    #[error("{provider} 响应解析失败：{error}")]
    StreamingResponseParseFailed {
        provider: &'static str,
        error: String,
    },
    #[error("{0}")]
    Generic(String),
}

impl SherpaError {
    /// Stable SCREAMING_SNAKE_CASE code preserved for public-facing contracts.
    pub fn code(&self) -> &'static str {
        match self {
            Self::OnlineProviderConfigMissing => "ONLINE_PROVIDER_CONFIG_MISSING",
            Self::UnsupportedOnlineProvider { .. } => "UNSUPPORTED_ONLINE_PROVIDER",
            Self::OnlineSessionNotInitialized => "ONLINE_SESSION_NOT_INITIALIZED",
            Self::StreamingNotSupported { .. } => "STREAMING_NOT_SUPPORTED",
            Self::VolcengineApiKeyMissing => "VOLCENGINE_API_KEY_MISSING",
            Self::VolcengineStreamingConfigMissing => "VOLCENGINE_STREAMING_CONFIG_MISSING",
            Self::VolcengineBatchConfigMissing => "VOLCENGINE_BATCH_CONFIG_MISSING",
            Self::VolcengineProviderConfigMissing => "VOLCENGINE_PROVIDER_CONFIG_MISSING",
            Self::UnsupportedVolcengineProvider { .. } => "UNSUPPORTED_VOLCENGINE_PROVIDER",
            Self::VolcengineProviderConfigInvalid { .. } => "VOLCENGINE_PROVIDER_CONFIG_INVALID",
            Self::VolcengineFrameTooShort => "VOLCENGINE_FRAME_TOO_SHORT",
            Self::VolcengineErrorFrame => "VOLCENGINE_ERROR_FRAME",
            Self::VolcengineErrorCodeParseFailed => "VOLCENGINE_ERROR_CODE_PARSE_FAILED",
            Self::VolcengineErrorLengthParseFailed => "VOLCENGINE_ERROR_LENGTH_PARSE_FAILED",
            Self::VolcengineApiError { .. } => "VOLCENGINE_API_ERROR",
            Self::VolcenginePayloadLengthMissing => "VOLCENGINE_PAYLOAD_LENGTH_MISSING",
            Self::VolcenginePayloadLengthParseFailed => "VOLCENGINE_PAYLOAD_LENGTH_PARSE_FAILED",
            Self::VolcenginePayloadIncomplete => "VOLCENGINE_PAYLOAD_INCOMPLETE",
            Self::VolcengineResponseParseFailed { .. } => "VOLCENGINE_RESPONSE_PARSE_FAILED",
            Self::VolcengineEndpointInvalid { .. } => "VOLCENGINE_ENDPOINT_INVALID",
            Self::VolcengineConnectionFailed { .. } => "VOLCENGINE_CONNECTION_FAILED",
            Self::VolcengineInitFrameSendFailed { .. } => "VOLCENGINE_INIT_FRAME_SEND_FAILED",
            Self::VolcengineWebSocketNotConnected => "VOLCENGINE_WEB_SOCKET_NOT_CONNECTED",
            Self::VolcengineWebSocketReadFailed { .. } => "VOLCENGINE_WEB_SOCKET_READ_FAILED",
            Self::VolcengineWebSocketClosed => "VOLCENGINE_WEB_SOCKET_CLOSED",
            Self::VolcengineFinalResponseTimeout => "VOLCENGINE_FINAL_RESPONSE_TIMEOUT",
            Self::VolcengineAudioSendFailed { .. } => "VOLCENGINE_AUDIO_SEND_FAILED",
            Self::VolcengineEndFrameSendFailed { .. } => "VOLCENGINE_END_FRAME_SEND_FAILED",
            Self::AudioFileReadFailed { .. } => "AUDIO_FILE_READ_FAILED",
            Self::VolcengineBatchRequestFailed { .. } => "VOLCENGINE_BATCH_REQUEST_FAILED",
            Self::VolcengineBatchResponseParseFailed { .. } => {
                "VOLCENGINE_BATCH_RESPONSE_PARSE_FAILED"
            }
            Self::VolcengineLocalFileBatchUnsupported { .. } => {
                "VOLCENGINE_LOCAL_FILE_BATCH_UNSUPPORTED"
            }
            Self::VolcengineRealtimeOnlyForStreaming => "VOLCENGINE_REALTIME_ONLY_FOR_STREAMING",
            Self::VolcengineBatchModeMismatch => "VOLCENGINE_BATCH_MODE_MISMATCH",
            Self::StreamingWebSocketNotConnected { .. } => "STREAMING_WEB_SOCKET_NOT_CONNECTED",
            Self::StreamingWebSocketReadFailed { .. } => "STREAMING_WEB_SOCKET_READ_FAILED",
            Self::StreamingWebSocketClosed { .. } => "STREAMING_WEB_SOCKET_CLOSED",
            Self::StreamingFinalResponseTimeout { .. } => "STREAMING_FINAL_RESPONSE_TIMEOUT",
            Self::StreamingAudioSendFailed { .. } => "STREAMING_AUDIO_SEND_FAILED",
            Self::StreamingEndFrameSendFailed { .. } => "STREAMING_END_FRAME_SEND_FAILED",
            Self::StreamingConnectionFailed { .. } => "STREAMING_CONNECTION_FAILED",
            Self::StreamingEndpointInvalid { .. } => "STREAMING_ENDPOINT_INVALID",
            Self::StreamingResponseParseFailed { .. } => "STREAMING_RESPONSE_PARSE_FAILED",
            Self::Generic(_) => "GENERIC_ERROR",
        }
    }

    /// Classify the error into a generic kind for `AsrPortError`.
    fn kind(&self) -> AsrPortErrorKind {
        match self {
            Self::OnlineProviderConfigMissing
            | Self::OnlineSessionNotInitialized
            | Self::VolcengineRealtimeOnlyForStreaming
            | Self::VolcengineBatchModeMismatch => AsrPortErrorKind::InvalidRequest,

            Self::UnsupportedOnlineProvider { .. }
            | Self::StreamingNotSupported { .. }
            | Self::UnsupportedVolcengineProvider { .. }
            | Self::VolcengineLocalFileBatchUnsupported { .. } => AsrPortErrorKind::Unsupported,

            Self::VolcengineApiKeyMissing => AsrPortErrorKind::Authentication,

            Self::VolcengineStreamingConfigMissing
            | Self::VolcengineBatchConfigMissing
            | Self::VolcengineProviderConfigMissing
            | Self::VolcengineProviderConfigInvalid { .. } => AsrPortErrorKind::InvalidRequest,

            Self::VolcengineConnectionFailed { .. }
            | Self::VolcengineWebSocketNotConnected
            | Self::VolcengineWebSocketReadFailed { .. }
            | Self::VolcengineWebSocketClosed
            | Self::VolcengineFinalResponseTimeout
            | Self::VolcengineAudioSendFailed { .. }
            | Self::VolcengineEndFrameSendFailed { .. }
            | Self::VolcengineInitFrameSendFailed { .. } => AsrPortErrorKind::Network,
            Self::StreamingConnectionFailed { .. }
            | Self::StreamingWebSocketNotConnected { .. }
            | Self::StreamingWebSocketReadFailed { .. }
            | Self::StreamingWebSocketClosed { .. }
            | Self::StreamingFinalResponseTimeout { .. }
            | Self::StreamingAudioSendFailed { .. }
            | Self::StreamingEndFrameSendFailed { .. } => AsrPortErrorKind::Network,

            Self::StreamingEndpointInvalid { .. } | Self::StreamingResponseParseFailed { .. } => {
                AsrPortErrorKind::Protocol
            }

            Self::VolcengineFrameTooShort
            | Self::VolcengineErrorFrame
            | Self::VolcengineErrorCodeParseFailed
            | Self::VolcengineErrorLengthParseFailed
            | Self::VolcengineApiError { .. }
            | Self::VolcenginePayloadLengthMissing
            | Self::VolcenginePayloadLengthParseFailed
            | Self::VolcenginePayloadIncomplete
            | Self::VolcengineResponseParseFailed { .. }
            | Self::VolcengineEndpointInvalid { .. }
            | Self::VolcengineBatchResponseParseFailed { .. } => AsrPortErrorKind::Protocol,

            Self::VolcengineBatchRequestFailed { .. } => AsrPortErrorKind::Network,

            Self::AudioFileReadFailed { .. } => AsrPortErrorKind::FileSystem,

            Self::Generic(_) => AsrPortErrorKind::Runtime,
        }
    }
}

impl Serialize for SherpaError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("SherpaError", 2)?;
        s.serialize_field("code", self.code())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

impl From<String> for SherpaError {
    fn from(s: String) -> Self {
        Self::Generic(s)
    }
}

impl From<&str> for SherpaError {
    fn from(s: &str) -> Self {
        Self::Generic(s.to_string())
    }
}

/// Convert a `SherpaError` to the Core port error type, preserving the
/// stable public code string via [`AsrPortError::with_code`].
impl From<SherpaError> for AsrPortError {
    fn from(error: SherpaError) -> Self {
        let code = error.code().to_string();
        let kind = error.kind();
        let message = error.to_string();
        AsrPortError::new(kind, message).with_code(code)
    }
}

/// Map an [`aimux_core::error::AiMuxError`] to the Core [`AsrPortError`].
pub fn map_aimux_asr_error(error: aimux_core::error::AiMuxError) -> AsrPortError {
    let msg = error.to_string();
    let lower = msg.to_ascii_lowercase();

    let status_code = error.status_code().or_else(|| {
        if let Some(idx) = msg.find("HTTP ") {
            let rest = &msg[idx + 5..];
            rest.split(|c: char| !c.is_ascii_digit())
                .next()
                .and_then(|code_str| code_str.parse::<u16>().ok())
        } else {
            None
        }
    });

    let kind = match status_code {
        Some(401 | 403) => AsrPortErrorKind::Authentication,
        Some(429) => AsrPortErrorKind::RateLimited,
        Some(408) => AsrPortErrorKind::Timeout,
        Some(400 | 404 | 413 | 422) => AsrPortErrorKind::InvalidRequest,
        Some(500..=599) => AsrPortErrorKind::Unavailable,
        _ => match &error {
            aimux_core::error::AiMuxError::InvalidArgument(_)
            | aimux_core::error::AiMuxError::NoSuchModel { .. }
            | aimux_core::error::AiMuxError::NoSuchProvider { .. } => {
                AsrPortErrorKind::InvalidRequest
            }
            aimux_core::error::AiMuxError::UnsupportedFunctionality(_) => {
                AsrPortErrorKind::Unsupported
            }
            aimux_core::error::AiMuxError::JsonParse(_)
            | aimux_core::error::AiMuxError::InvalidResponseData(_) => AsrPortErrorKind::Protocol,
            _ => {
                if lower.contains("unauthorized")
                    || lower.contains("api key")
                    || lower.contains("forbidden")
                {
                    AsrPortErrorKind::Authentication
                } else if lower.contains("rate limit")
                    || lower.contains("429")
                    || lower.contains("too many requests")
                {
                    AsrPortErrorKind::RateLimited
                } else if lower.contains("timeout") || lower.contains("timed out") {
                    AsrPortErrorKind::Timeout
                } else if lower.contains("bad request") || lower.contains("invalid") {
                    AsrPortErrorKind::InvalidRequest
                } else if lower.contains("unavailable")
                    || lower.contains("overloaded")
                    || lower.contains("bad gateway")
                {
                    AsrPortErrorKind::Unavailable
                } else if lower.contains("not supported") || lower.contains("unsupported") {
                    AsrPortErrorKind::Unsupported
                } else {
                    AsrPortErrorKind::Network
                }
            }
        },
    };

    AsrPortError::new(kind, msg)
}
