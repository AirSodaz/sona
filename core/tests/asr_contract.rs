use sona_core::ports::asr::{
    AsrEngine, AsrEngineConfig, AsrMode, AsrTranscriptionRequest, BatchSegmentationMode,
    OnlineAsrProviderRequest,
};
use sona_core::transcript_postprocess::{
    TranscriptNormalizationOptions, TranscriptPostprocessOptions,
};

#[test]
fn local_sherpa_request_builder_preserves_shared_contract_fields() {
    let request = AsrTranscriptionRequest::local_sherpa(
        AsrMode::Offline,
        "/models/sherpa".to_string(),
        8,
        true,
        "ja".to_string(),
        Some("punct".to_string()),
        Some("vad".to_string()),
        3.5,
        "whisper".to_string(),
        Some(sona_core::model_config::ModelFileConfig::default()),
        Some("hotwords".to_string()),
        TranscriptNormalizationOptions {
            enable_timeline: true,
        },
        TranscriptPostprocessOptions::default(),
        None,
        Some("cuda".to_string()),
    );

    assert_eq!(request.engine(), AsrEngine::LocalSherpa);
    assert_eq!(request.mode, AsrMode::Offline);
    assert_eq!(request.language, "ja");
    assert_eq!(request.enable_itn, true);
    assert_eq!(request.hotwords.as_deref(), Some("hotwords"));
    assert!(matches!(
        request.engine_config,
        AsrEngineConfig::LocalSherpa {
            ref model_path,
            num_threads,
            ref punctuation_model,
            ref vad_model,
            vad_buffer,
            batch_segmentation_mode,
            ref model_type,
            ref file_config,
            ref gpu_acceleration,
            ..
        } if model_path == "/models/sherpa"
            && num_threads == 8
            && punctuation_model.as_deref() == Some("punct")
            && vad_model.as_deref() == Some("vad")
            && (vad_buffer - 3.5).abs() < f32::EPSILON
            && batch_segmentation_mode == BatchSegmentationMode::Vad
            && model_type == "whisper"
            && file_config.as_ref().is_some()
            && gpu_acceleration.as_deref() == Some("cuda")
    ));
}

#[test]
fn online_asr_request_roundtrips_through_json() {
    let request = AsrTranscriptionRequest {
        mode: AsrMode::Streaming,
        language: "auto".to_string(),
        enable_itn: false,
        normalization_options: TranscriptNormalizationOptions::default(),
        postprocess_options: TranscriptPostprocessOptions::default(),
        hotwords: None,
        speaker_processing: None,
        engine_config: AsrEngineConfig::Online {
            provider: OnlineAsrProviderRequest {
                provider_id: "volcengine".to_string(),
                profile_id: "default".to_string(),
                config: serde_json::json!({"apiKey": "secret"}),
            },
        },
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["engine"], "online");
    assert_eq!(json["mode"], "streaming");
    assert_eq!(json["onlineProvider"]["providerId"], "volcengine");

    let decoded: AsrTranscriptionRequest = serde_json::from_value(json).unwrap();
    assert_eq!(decoded.engine(), AsrEngine::Online);
    assert_eq!(decoded.language, "auto");
}
