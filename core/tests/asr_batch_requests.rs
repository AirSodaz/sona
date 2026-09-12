use sona_core::ports::asr::{
    AsrEngineConfig, AsrMode, AsrPortErrorKind, AsrTranscriptionRequest, BatchSegmentationMode,
    BatchTranscriptionRequest, LocalAsrEngine, LocalSherpaStreamingRequest,
    OnlineAsrProviderRequest, TranscriptNormalizationOptions, TranscriptPostprocessOptions,
    validate_local_asr_mode,
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
        engine: LocalAsrEngine::SherpaOnnx,
        ffmpeg_path: Some("C:/custom/ffmpeg.exe".to_string()),
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
fn local_asr_mode_validation_is_a_core_owned_contract() {
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

    validate_local_asr_mode(&batch_request, AsrMode::Batch).unwrap();

    let mode_error = validate_local_asr_mode(&batch_request, AsrMode::Streaming).unwrap_err();
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

    let engine_error = validate_local_asr_mode(&online_request, AsrMode::Batch).unwrap_err();
    assert_eq!(engine_error.kind, AsrPortErrorKind::Unsupported);
    assert_eq!(
        engine_error.message,
        "Unsupported ASR engine for local ASR adapter"
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
        Some("metal".to_string()),
    );

    let batch_request = BatchTranscriptionRequest::from_local_asr_request(
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
    assert_eq!(batch_request.gpu_acceleration.as_deref(), Some("metal"));
    assert_eq!(batch_request.engine, LocalAsrEngine::SherpaOnnx);
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

    let error = BatchTranscriptionRequest::from_local_asr_request(
        "input.wav".into(),
        None,
        online_request,
        None,
        None,
    )
    .unwrap_err();

    assert_eq!(error.kind, AsrPortErrorKind::InvalidRequest);
    assert_eq!(error.message, "Expected local ASR engine config");
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

#[test]
fn local_sherpa_streaming_rejects_llama_engine_with_shared_error() {
    let mut request = AsrTranscriptionRequest::local_sherpa(
        AsrMode::Streaming,
        "models/qwen".to_string(),
        4,
        false,
        "auto".to_string(),
        None,
        None,
        5.0,
        "qwen3-asr".to_string(),
        None,
        None,
        TranscriptNormalizationOptions::default(),
        TranscriptPostprocessOptions::default(),
        None,
        None,
    );
    let AsrEngineConfig::Local { local_engine, .. } = &mut request.engine_config else {
        unreachable!();
    };
    *local_engine = LocalAsrEngine::LlamaCpp;

    let error =
        LocalSherpaStreamingRequest::from_local_sherpa_request("live-llama".to_string(), request)
            .unwrap_err();

    assert_eq!(error.kind, AsrPortErrorKind::Unsupported);
    assert_eq!(
        error.message,
        "Local ASR adapter 'sherpa-onnx' cannot execute engine 'llama-cpp'."
    );
}

#[test]
fn local_sherpa_streaming_inherits_qwen3_initial_refresh_rate_from_preset_rules() {
    let mut request = AsrTranscriptionRequest::local_sherpa(
        AsrMode::Streaming,
        "models/qwen3-asr".to_string(),
        4,
        false,
        "auto".to_string(),
        None,
        Some("models/silero-vad".to_string()),
        3.0,
        "qwen3-asr".to_string(),
        None,
        None,
        TranscriptNormalizationOptions::default(),
        TranscriptPostprocessOptions::default(),
        None,
        None,
    );
    let AsrEngineConfig::Local { model_id, .. } = &mut request.engine_config else {
        unreachable!();
    };
    *model_id = Some("sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25".to_string());

    let streaming_request =
        LocalSherpaStreamingRequest::from_local_sherpa_request("live-qwen3".to_string(), request)
            .unwrap();

    assert_eq!(streaming_request.instance_id, "live-qwen3");
    assert_eq!(streaming_request.model_type, "qwen3-asr");
    assert_eq!(streaming_request.initial_refresh_rate_ms, Some(400));
}

#[test]
fn local_sherpa_streaming_inherits_whisper_initial_refresh_rate_from_preset_rules() {
    let mut request = AsrTranscriptionRequest::local_sherpa(
        AsrMode::Streaming,
        "models/whisper-turbo".to_string(),
        4,
        false,
        "auto".to_string(),
        None,
        Some("models/silero-vad".to_string()),
        3.0,
        "whisper".to_string(),
        None,
        None,
        TranscriptNormalizationOptions::default(),
        TranscriptPostprocessOptions::default(),
        None,
        None,
    );
    let AsrEngineConfig::Local { model_id, .. } = &mut request.engine_config else {
        unreachable!();
    };
    *model_id = Some("sherpa-onnx-whisper-turbo".to_string());

    let streaming_request =
        LocalSherpaStreamingRequest::from_local_sherpa_request("live-whisper".to_string(), request)
            .unwrap();

    assert_eq!(streaming_request.instance_id, "live-whisper");
    assert_eq!(streaming_request.model_type, "whisper");
    assert_eq!(streaming_request.initial_refresh_rate_ms, Some(400));
}

#[test]
fn local_sherpa_streaming_inherits_funasr_nano_initial_refresh_rate_from_preset_rules() {
    let mut request = AsrTranscriptionRequest::local_sherpa(
        AsrMode::Streaming,
        "models/funasr-nano".to_string(),
        4,
        false,
        "auto".to_string(),
        Some("models/punct".to_string()),
        Some("models/silero-vad".to_string()),
        3.0,
        "funasr-nano".to_string(),
        None,
        None,
        TranscriptNormalizationOptions::default(),
        TranscriptPostprocessOptions::default(),
        None,
        None,
    );
    let AsrEngineConfig::Local { model_id, .. } = &mut request.engine_config else {
        unreachable!();
    };
    *model_id = Some("sherpa-onnx-funasr-nano-int8-2025-12-30".to_string());

    let streaming_request =
        LocalSherpaStreamingRequest::from_local_sherpa_request("live-funasr".to_string(), request)
            .unwrap();

    assert_eq!(streaming_request.instance_id, "live-funasr");
    assert_eq!(streaming_request.model_type, "funasr-nano");
    assert_eq!(streaming_request.initial_refresh_rate_ms, Some(500));
}

#[test]
fn local_sherpa_streaming_inherits_firered_initial_refresh_rate_from_preset_rules() {
    let mut request = AsrTranscriptionRequest::local_sherpa(
        AsrMode::Streaming,
        "models/firered".to_string(),
        4,
        false,
        "auto".to_string(),
        Some("models/punct".to_string()),
        Some("models/silero-vad".to_string()),
        3.0,
        "fire-red-asr".to_string(),
        None,
        None,
        TranscriptNormalizationOptions::default(),
        TranscriptPostprocessOptions::default(),
        None,
        None,
    );
    let AsrEngineConfig::Local { model_id, .. } = &mut request.engine_config else {
        unreachable!();
    };
    *model_id = Some("sherpa-onnx-fire-red-asr2-zh_en-int8-2026-02-26".to_string());

    let streaming_request =
        LocalSherpaStreamingRequest::from_local_sherpa_request("live-firered".to_string(), request)
            .unwrap();

    assert_eq!(streaming_request.instance_id, "live-firered");
    assert_eq!(streaming_request.model_type, "fire-red-asr");
    assert_eq!(streaming_request.initial_refresh_rate_ms, Some(600));
}

#[test]
fn local_sherpa_streaming_inherits_omnilingual_initial_refresh_rate_from_preset_rules() {
    let mut request = AsrTranscriptionRequest::local_sherpa(
        AsrMode::Streaming,
        "models/omnilingual".to_string(),
        4,
        false,
        "auto".to_string(),
        Some("models/punct".to_string()),
        Some("models/silero-vad".to_string()),
        3.0,
        "omnilingual".to_string(),
        None,
        None,
        TranscriptNormalizationOptions::default(),
        TranscriptPostprocessOptions::default(),
        None,
        None,
    );
    let AsrEngineConfig::Local { model_id, .. } = &mut request.engine_config else {
        unreachable!();
    };
    *model_id =
        Some("sherpa-onnx-omnilingual-asr-1600-languages-1B-ctc-v2-int8-2026-02-05".to_string());

    let streaming_request = LocalSherpaStreamingRequest::from_local_sherpa_request(
        "live-omnilingual".to_string(),
        request,
    )
    .unwrap();

    assert_eq!(streaming_request.instance_id, "live-omnilingual");
    assert_eq!(streaming_request.model_type, "omnilingual");
    assert_eq!(streaming_request.initial_refresh_rate_ms, Some(500));
}
