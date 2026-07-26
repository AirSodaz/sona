use sona_core::ports::asr::{
    AsrEngineConfig, AsrMode, AsrTranscriptionRequest, GROQ_WHISPER_PROVIDER_ID,
    LOCAL_SHERPA_PROVIDER_ID, MISTRAL_VOXTRAL_PROVIDER_ID, OnlineAsrProviderRequest,
    VOLCENGINE_DOUBAO_PROVIDER_ID,
};
use sona_core::transcription::postprocess::{
    TranscriptNormalizationOptions, TranscriptPostprocessOptions,
};
use sona_core::transcription::provider_resolution::{
    AsrProviderCapability, resolve_asr_provider_id, resolve_asr_streaming_provider_id,
};

const DESKTOP_CAPABILITIES: [AsrProviderCapability<'static>; 4] = [
    AsrProviderCapability::new(LOCAL_SHERPA_PROVIDER_ID, true),
    AsrProviderCapability::new(VOLCENGINE_DOUBAO_PROVIDER_ID, true),
    AsrProviderCapability::new(GROQ_WHISPER_PROVIDER_ID, false),
    AsrProviderCapability::new(MISTRAL_VOXTRAL_PROVIDER_ID, false),
];

fn online_request(provider_id: &str, mode: AsrMode) -> AsrTranscriptionRequest {
    AsrTranscriptionRequest {
        mode,
        language: "auto".into(),
        enable_itn: false,
        normalization_options: Default::default(),
        postprocess_options: Default::default(),
        hotwords: None,
        speaker_processing: None,
        engine_config: AsrEngineConfig::Online {
            provider: OnlineAsrProviderRequest {
                provider_id: provider_id.into(),
                profile_id: "test".into(),
                config: serde_json::Value::Null,
            },
        },
    }
}

fn local_request(mode: AsrMode) -> AsrTranscriptionRequest {
    AsrTranscriptionRequest::local_sherpa(
        mode,
        "model".into(),
        1,
        false,
        "auto".into(),
        None,
        None,
        5.0,
        "zipformer".into(),
        None,
        None,
        TranscriptNormalizationOptions::default(),
        TranscriptPostprocessOptions::default(),
        None,
        None,
    )
}

#[test]
fn selects_declared_online_provider() {
    assert_eq!(
        resolve_asr_provider_id(
            &online_request(VOLCENGINE_DOUBAO_PROVIDER_ID, AsrMode::Streaming),
            &DESKTOP_CAPABILITIES,
        )
        .unwrap(),
        VOLCENGINE_DOUBAO_PROVIDER_ID,
    );
}

#[test]
fn selects_declared_local_provider() {
    assert_eq!(
        resolve_asr_provider_id(&local_request(AsrMode::Streaming), &DESKTOP_CAPABILITIES,)
            .unwrap(),
        LOCAL_SHERPA_PROVIDER_ID,
    );
}

#[test]
fn rejects_unknown_online_provider() {
    let unknown = resolve_asr_provider_id(
        &online_request("future-provider", AsrMode::Streaming),
        &DESKTOP_CAPABILITIES,
    )
    .unwrap_err();
    assert_eq!(unknown.code(), "UNSUPPORTED_ONLINE_PROVIDER");
}

#[test]
fn rejects_missing_host_provider() {
    let missing = resolve_asr_provider_id(
        &local_request(AsrMode::Streaming),
        &DESKTOP_CAPABILITIES[1..],
    )
    .unwrap_err();
    assert_eq!(missing.code(), "UNSUPPORTED_ONLINE_PROVIDER");
}

#[test]
fn rejects_catalog_disabled_streaming() {
    let catalog_disabled = resolve_asr_provider_id(
        &online_request(GROQ_WHISPER_PROVIDER_ID, AsrMode::Streaming),
        &DESKTOP_CAPABILITIES,
    )
    .unwrap_err();
    assert_eq!(catalog_disabled.code(), "STREAMING_NOT_SUPPORTED");
}

#[test]
fn rejects_host_disabled_streaming() {
    let host_capabilities = [AsrProviderCapability::new(
        VOLCENGINE_DOUBAO_PROVIDER_ID,
        false,
    )];
    let host_disabled = resolve_asr_provider_id(
        &online_request(VOLCENGINE_DOUBAO_PROVIDER_ID, AsrMode::Streaming),
        &host_capabilities,
    )
    .unwrap_err();
    assert_eq!(host_disabled.code(), "STREAMING_NOT_SUPPORTED");
}

#[test]
fn leaves_batch_validation_to_selected_adapter() {
    assert_eq!(
        resolve_asr_provider_id(
            &online_request(GROQ_WHISPER_PROVIDER_ID, AsrMode::Batch),
            &DESKTOP_CAPABILITIES,
        )
        .unwrap(),
        GROQ_WHISPER_PROVIDER_ID,
    );
}
