use sona_core::ports::asr::{
    AsrEngineConfig, AsrMode, AsrPortErrorKind, AsrTranscriptionRequest, BatchSegmentationMode,
    BatchTranscriptionRequest, LocalSherpaStreamingRequest, OnlineAsrProviderRequest,
    TranscriptNormalizationOptions, TranscriptPostprocessOptions, validate_local_sherpa_mode,
};
use sona_core::transcription::postprocess::TranscriptPostprocessor;

#[test]
fn local_batch_transcription_request_is_a_core_owned_runtime_contract() {
    let request = BatchTranscriptionRequest {
        instance_id: Some("batch-1".to_string()),
        file_path: "meeting.wav".into(),
        save_to_path: Some("meeting.resampled.wav".into()),
        model_path: "models/sherpa".to_string(),
        num_threads: 4,
        enable_itn: true,
        language: "auto".to_string(),
        punctuation_model: Some("models/punctuation".to_string()),
        vad_model: Some("models/vad".to_string()),
        vad_buffer: 5.0,
        batch_segmentation_mode: BatchSegmentationMode::Vad,
        model_type: "whisper".to_string(),
        file_config: None,
        hotwords: Some("Sona".to_string()),
        speaker_processing: None,
        normalization_options: TranscriptNormalizationOptions {
            enable_timeline: true,
        },
        postprocessor: TranscriptPostprocessor::default(),
        gpu_acceleration: Some("cpu".to_string()),
    };

    let cloned = request.clone();

    assert_eq!(cloned.instance_id.as_deref(), Some("batch-1"));
    assert_eq!(cloned.file_path, std::path::PathBuf::from("meeting.wav"));
    assert_eq!(cloned.save_to_path, Some("meeting.resampled.wav".into()));
    assert_eq!(cloned.model_path, "models/sherpa");
    assert_eq!(cloned.batch_segmentation_mode, BatchSegmentationMode::Vad);
    assert_eq!(cloned.hotwords.as_deref(), Some("Sona"));
    assert!(cloned.normalization_options.enable_timeline);
    assert_eq!(cloned.gpu_acceleration.as_deref(), Some("cpu"));
}

#[test]
fn local_sherpa_mode_validation_is_a_core_owned_contract() {
    let batch_request = AsrTranscriptionRequest::local_sherpa(
        AsrMode::Batch,
        "models/sherpa".to_string(),
        4,
        true,
        "auto".to_string(),
        None,
        None,
        5.0,
        "whisper".to_string(),
        None,
        None,
        TranscriptNormalizationOptions::default(),
        TranscriptPostprocessOptions::default(),
        None,
        Some("cpu".to_string()),
    );

    validate_local_sherpa_mode(&batch_request, AsrMode::Batch).unwrap();

    let mode_error = validate_local_sherpa_mode(&batch_request, AsrMode::Streaming).unwrap_err();
    assert_eq!(mode_error.kind, AsrPortErrorKind::InvalidRequest);
    assert_eq!(
        mode_error.message,
        "ASR request mode mismatch: expected Streaming, got Batch"
    );

    let online_request = AsrTranscriptionRequest {
        mode: AsrMode::Batch,
        language: "auto".to_string(),
        enable_itn: false,
        normalization_options: TranscriptNormalizationOptions::default(),
        postprocess_options: TranscriptPostprocessOptions::default(),
        hotwords: None,
        speaker_processing: None,
        engine_config: AsrEngineConfig::Online {
            provider: OnlineAsrProviderRequest {
                provider_id: "volcengine-doubao".to_string(),
                profile_id: "default".to_string(),
                config: serde_json::json!({}),
            },
        },
    };

    let engine_error = validate_local_sherpa_mode(&online_request, AsrMode::Batch).unwrap_err();
    assert_eq!(engine_error.kind, AsrPortErrorKind::Unsupported);
    assert_eq!(
        engine_error.message,
        "Unsupported ASR engine for local Sherpa adapter"
    );
}

#[test]
fn local_batch_request_mapping_from_asr_request_is_core_owned() {
    let request = AsrTranscriptionRequest::local_sherpa(
        AsrMode::Batch,
        "models/sherpa".to_string(),
        8,
        true,
        "zh".to_string(),
        Some("models/punctuation".to_string()),
        Some("models/vad".to_string()),
        6.5,
        "whisper".to_string(),
        None,
        Some("Sona, meeting".to_string()),
        TranscriptNormalizationOptions {
            enable_timeline: true,
        },
        TranscriptPostprocessOptions::default(),
        None,
        Some("directml".to_string()),
    );

    let batch_request = BatchTranscriptionRequest::from_local_sherpa_request(
        "input.wav".into(),
        Some("output.wav".into()),
        request,
        None,
        Some("batch-42".to_string()),
    )
    .unwrap();

    assert_eq!(batch_request.instance_id.as_deref(), Some("batch-42"));
    assert_eq!(
        batch_request.file_path,
        std::path::PathBuf::from("input.wav")
    );
    assert_eq!(batch_request.save_to_path, Some("output.wav".into()));
    assert_eq!(batch_request.model_path, "models/sherpa");
    assert_eq!(batch_request.num_threads, 8);
    assert_eq!(batch_request.language, "zh");
    assert_eq!(
        batch_request.punctuation_model.as_deref(),
        Some("models/punctuation")
    );
    assert_eq!(batch_request.vad_model.as_deref(), Some("models/vad"));
    assert_eq!(batch_request.vad_buffer, 6.5);
    assert_eq!(
        batch_request.batch_segmentation_mode,
        BatchSegmentationMode::Vad
    );
    assert_eq!(batch_request.hotwords.as_deref(), Some("Sona, meeting"));
    assert!(batch_request.normalization_options.enable_timeline);
    assert_eq!(batch_request.gpu_acceleration.as_deref(), Some("directml"));
}

#[test]
fn local_batch_request_mapping_rejects_online_engine() {
    let online_request = AsrTranscriptionRequest {
        mode: AsrMode::Batch,
        language: "auto".to_string(),
        enable_itn: false,
        normalization_options: TranscriptNormalizationOptions::default(),
        postprocess_options: TranscriptPostprocessOptions::default(),
        hotwords: None,
        speaker_processing: None,
        engine_config: AsrEngineConfig::Online {
            provider: OnlineAsrProviderRequest {
                provider_id: "volcengine-doubao".to_string(),
                profile_id: "default".to_string(),
                config: serde_json::json!({}),
            },
        },
    };

    let error = BatchTranscriptionRequest::from_local_sherpa_request(
        "input.wav".into(),
        None,
        online_request,
        None,
        None,
    )
    .unwrap_err();

    assert_eq!(error.kind, AsrPortErrorKind::InvalidRequest);
    assert_eq!(error.message, "Expected LocalSherpa engine config");
}

#[test]
fn local_streaming_request_mapping_from_asr_request_is_core_owned() {
    let request = AsrTranscriptionRequest::local_sherpa(
        AsrMode::Streaming,
        "models/live-sherpa".to_string(),
        2,
        false,
        "en".to_string(),
        Some("models/live-punctuation".to_string()),
        Some("models/live-vad".to_string()),
        4.5,
        "sense-voice".to_string(),
        None,
        Some("Sona live".to_string()),
        TranscriptNormalizationOptions {
            enable_timeline: true,
        },
        TranscriptPostprocessOptions::default(),
        None,
        Some("cpu".to_string()),
    );

    let streaming_request =
        LocalSherpaStreamingRequest::from_local_sherpa_request("live-1".to_string(), request)
            .unwrap();

    assert_eq!(streaming_request.instance_id, "live-1");
    assert_eq!(streaming_request.model_path, "models/live-sherpa");
    assert_eq!(streaming_request.num_threads, 2);
    assert!(!streaming_request.enable_itn);
    assert_eq!(streaming_request.language, "en");
    assert_eq!(
        streaming_request.punctuation_model.as_deref(),
        Some("models/live-punctuation")
    );
    assert_eq!(
        streaming_request.vad_model.as_deref(),
        Some("models/live-vad")
    );
    assert_eq!(streaming_request.vad_buffer, 4.5);
    assert_eq!(streaming_request.model_type, "sense-voice");
    assert_eq!(streaming_request.hotwords.as_deref(), Some("Sona live"));
    assert!(streaming_request.normalization_options.enable_timeline);
    assert_eq!(streaming_request.gpu_acceleration.as_deref(), Some("cpu"));
}

#[test]
fn local_streaming_request_mapping_rejects_batch_mode() {
    let request = AsrTranscriptionRequest::local_sherpa(
        AsrMode::Batch,
        "models/sherpa".to_string(),
        4,
        true,
        "auto".to_string(),
        None,
        None,
        5.0,
        "whisper".to_string(),
        None,
        None,
        TranscriptNormalizationOptions::default(),
        TranscriptPostprocessOptions::default(),
        None,
        None,
    );

    let error =
        LocalSherpaStreamingRequest::from_local_sherpa_request("live-2".to_string(), request)
            .unwrap_err();

    assert_eq!(error.kind, AsrPortErrorKind::InvalidRequest);
    assert_eq!(
        error.message,
        "ASR request mode mismatch: expected Streaming, got Batch"
    );
}
