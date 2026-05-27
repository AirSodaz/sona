use super::state::SherpaState;
use super::types::AsrTranscriptionRequest;
use super::volcengine;
use super::groq;
pub use crate::asr_providers::{VOLCENGINE_DOUBAO_PROVIDER_ID, GROQ_WHISPER_PROVIDER_ID};
use tauri::{AppHandle, State};

#[derive(Clone)]
pub enum OnlineStreamingSession {
    Volcengine(volcengine::VolcengineStreamingSession),
}

impl OnlineStreamingSession {
    pub fn provider_id(&self) -> &'static str {
        match self {
            Self::Volcengine(_) => VOLCENGINE_DOUBAO_PROVIDER_ID,
        }
    }

    pub fn as_volcengine(&self) -> Option<&volcengine::VolcengineStreamingSession> {
        match self {
            Self::Volcengine(session) => Some(session),
        }
    }
}

fn provider_id_from_request(request: &AsrTranscriptionRequest) -> Result<&str, String> {
    request
        .online_provider
        .as_ref()
        .map(|provider| provider.provider_id.as_str())
        .ok_or_else(|| "在线 ASR provider 配置缺失。".to_string())
}

fn ensure_provider(request: &AsrTranscriptionRequest) -> Result<&str, String> {
    let provider_id = provider_id_from_request(request)?;
    match provider_id {
        VOLCENGINE_DOUBAO_PROVIDER_ID | GROQ_WHISPER_PROVIDER_ID => Ok(provider_id),
        _ => Err(format!("不支持的在线 ASR provider：{provider_id}")),
    }
}

async fn provider_id_from_session(
    state: &SherpaState,
    instance_id: &str,
) -> Result<&'static str, String> {
    state
        .online_sessions
        .lock()
        .await
        .get(instance_id)
        .map(OnlineStreamingSession::provider_id)
        .ok_or_else(|| "在线 ASR session 未初始化。".to_string())
}

pub async fn init_streaming_recognizer_impl(
    state: State<'_, SherpaState>,
    instance_id: String,
    request: AsrTranscriptionRequest,
) -> Result<(), String> {
    let provider_id = ensure_provider(&request)?;
    if provider_id == VOLCENGINE_DOUBAO_PROVIDER_ID {
        volcengine::init_streaming_recognizer_impl(state, instance_id, request).await
    } else {
        Err(format!("provider {} 不支持流式识别", provider_id))
    }
}

pub async fn start_streaming_recognizer_impl<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, SherpaState>,
    instance_id: String,
) -> Result<(), String> {
    match provider_id_from_session(&state, &instance_id).await? {
        VOLCENGINE_DOUBAO_PROVIDER_ID => {
            volcengine::start_streaming_recognizer_impl(app, state, instance_id).await
        }
        provider_id => Err(format!("不支持的在线 ASR provider：{provider_id}")),
    }
}

pub async fn stop_streaming_recognizer_impl(
    state: State<'_, SherpaState>,
    instance_id: String,
) -> Result<(), String> {
    match provider_id_from_session(&state, &instance_id).await? {
        VOLCENGINE_DOUBAO_PROVIDER_ID => {
            volcengine::stop_streaming_recognizer_impl(state, instance_id).await
        }
        provider_id => Err(format!("不支持的在线 ASR provider：{provider_id}")),
    }
}

pub async fn flush_streaming_recognizer_impl<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, SherpaState>,
    instance_id: String,
) -> Result<(), String> {
    match provider_id_from_session(&state, &instance_id).await? {
        VOLCENGINE_DOUBAO_PROVIDER_ID => {
            volcengine::flush_streaming_recognizer_impl(app, state, instance_id).await
        }
        provider_id => Err(format!("不支持的在线 ASR provider：{provider_id}")),
    }
}

pub async fn feed_audio_chunk_impl<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, SherpaState>,
    instance_id: String,
    samples: Vec<u8>,
) -> Result<(), String> {
    match provider_id_from_session(&state, &instance_id).await? {
        VOLCENGINE_DOUBAO_PROVIDER_ID => {
            volcengine::feed_audio_chunk_impl(app, state, instance_id, samples).await
        }
        provider_id => Err(format!("不支持的在线 ASR provider：{provider_id}")),
    }
}

pub async fn feed_audio_samples_impl(
    state: &SherpaState,
    instance_id: &str,
    samples: &[f32],
) -> Result<(), String> {
    match provider_id_from_session(state, instance_id).await? {
        VOLCENGINE_DOUBAO_PROVIDER_ID => {
            volcengine::feed_audio_samples_impl(state, instance_id, samples).await
        }
        provider_id => Err(format!("不支持的在线 ASR provider：{provider_id}")),
    }
}

pub async fn process_batch_file_impl<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: &SherpaState,
    file_path: String,
    request: AsrTranscriptionRequest,
) -> Result<Vec<super::TranscriptSegment>, String> {
    let provider_id = ensure_provider(&request)?;
    match provider_id {
        VOLCENGINE_DOUBAO_PROVIDER_ID => {
            volcengine::process_batch_file_impl(app, state, file_path, request).await
        }
        GROQ_WHISPER_PROVIDER_ID => {
            groq::process_batch_file_impl(app, state, file_path, request).await
        }
        _ => Err(format!("不支持的在线 ASR provider：{provider_id}")),
    }
}
