use super::error::SherpaError;
use super::groq;
use super::mistral;
use super::online_traits::OnlineAsrProviderAdapter;
use super::state::SherpaState;
use super::types::AsrTranscriptionRequest;
use super::volcengine;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::OnceLock;
use tauri::{AppHandle, State};

fn online_adapters() -> &'static HashMap<&'static str, Box<dyn OnlineAsrProviderAdapter>> {
    static ONLINE_ADAPTERS: OnceLock<HashMap<&'static str, Box<dyn OnlineAsrProviderAdapter>>> =
        OnceLock::new();
    ONLINE_ADAPTERS.get_or_init(|| {
        let mut map: HashMap<&'static str, Box<dyn OnlineAsrProviderAdapter>> = HashMap::new();
        let volcengine = volcengine::VolcengineAdapter;
        map.insert(volcengine.provider_id(), Box::new(volcengine));
        let groq = groq::GroqWhisperAdapter;
        map.insert(groq.provider_id(), Box::new(groq));
        let mistral = mistral::MistralVoxtralAdapter;
        map.insert(mistral.provider_id(), Box::new(mistral));
        map
    })
}

fn provider_id_from_request(request: &AsrTranscriptionRequest) -> Result<&str, SherpaError> {
    request
        .online_provider
        .as_ref()
        .map(|provider| provider.provider_id.as_str())
        .ok_or(SherpaError::OnlineProviderConfigMissing)
}

fn ensure_provider(request: &AsrTranscriptionRequest) -> Result<&'static str, SherpaError> {
    let provider_id = provider_id_from_request(request)?;
    online_adapters()
        .keys()
        .find(|k| **k == provider_id)
        .copied()
        .ok_or_else(|| SherpaError::UnsupportedOnlineProvider {
            provider_id: provider_id.to_string(),
        })
}

pub async fn init_streaming_recognizer_impl(
    state: State<'_, SherpaState>,
    instance_id: String,
    request: AsrTranscriptionRequest,
) -> Result<(), SherpaError> {
    let provider_id = ensure_provider(&request)?;
    let adapter = online_adapters().get(provider_id).unwrap();
    let config_val = &request.online_provider.as_ref().unwrap().config;

    match adapter.create_streaming_session(config_val, &request)? {
        Some(session) => {
            let mut sessions = state.online_sessions.lock().await;
            sessions.insert(instance_id, Arc::from(session));
            Ok(())
        }
        _ => Err(SherpaError::StreamingNotSupported {
            provider_id: provider_id.to_string(),
        }),
    }
}

pub async fn start_streaming_recognizer_impl(
    app: AppHandle,
    state: State<'_, SherpaState>,
    instance_id: String,
) -> Result<(), SherpaError> {
    let session = {
        let sessions = state.online_sessions.lock().await;
        sessions
            .get(&instance_id)
            .cloned()
            .ok_or(SherpaError::OnlineSessionNotInitialized)?
    };
    session.start(app.clone(), &instance_id).await
}

pub async fn stop_streaming_recognizer_impl(
    state: State<'_, SherpaState>,
    instance_id: String,
) -> Result<(), SherpaError> {
    let session = {
        let sessions = state.online_sessions.lock().await;
        sessions
            .get(&instance_id)
            .cloned()
            .ok_or(SherpaError::OnlineSessionNotInitialized)?
    };
    session.stop(&state, &instance_id).await
}

pub async fn flush_streaming_recognizer_impl(
    app: AppHandle,
    state: State<'_, SherpaState>,
    instance_id: String,
) -> Result<(), SherpaError> {
    let session = {
        let sessions = state.online_sessions.lock().await;
        sessions
            .get(&instance_id)
            .cloned()
            .ok_or(SherpaError::OnlineSessionNotInitialized)?
    };
    session.flush(app.clone(), &state, &instance_id).await
}

pub async fn feed_audio_chunk_impl(
    app: AppHandle,
    state: State<'_, SherpaState>,
    instance_id: String,
    samples: Vec<u8>,
) -> Result<(), SherpaError> {
    let session = {
        let sessions = state.online_sessions.lock().await;
        sessions
            .get(&instance_id)
            .cloned()
            .ok_or(SherpaError::OnlineSessionNotInitialized)?
    };
    session
        .feed_audio_chunk(app.clone(), &state, &instance_id, samples)
        .await
}

pub async fn feed_audio_samples_impl(
    state: &SherpaState,
    instance_id: &str,
    samples: &[f32],
) -> Result<(), SherpaError> {
    let session = {
        let sessions = state.online_sessions.lock().await;
        sessions
            .get(instance_id)
            .cloned()
            .ok_or(SherpaError::OnlineSessionNotInitialized)?
    };
    session
        .feed_audio_samples(state, instance_id, samples)
        .await
}

pub async fn process_batch_file_impl(
    app: AppHandle,
    state: &SherpaState,
    file_path: String,
    request: AsrTranscriptionRequest,
) -> Result<Vec<super::TranscriptSegment>, SherpaError> {
    let provider_id = ensure_provider(&request)?;
    let adapter = online_adapters().get(provider_id).unwrap();
    let config_val = &request.online_provider.as_ref().unwrap().config;

    let processor = adapter.create_batch_processor(config_val)?.ok_or_else(|| {
        SherpaError::Generic(format!(
            "Batch mode not supported for provider {}",
            provider_id
        ))
    })?;

    processor
        .process_file(app.clone(), state, file_path, request)
        .await
}
