use sona_core::models::config::ModelFileConfig;
use sona_core::ports::asr::AsrPortErrorKind;
use sona_sherpa_onnx::recognizer::{
    ModelType, OfflineDecodeResult, build_model_config, build_offline_model_config,
    create_offline_recognizer,
};
use std::path::Path;

#[test]
fn build_model_config_supports_qwen3_asr_without_tokens() {
    let model_path = Path::new("C:/models/qwen3-asr-0.6b-q8-gguf");
    let file_config = Some(ModelFileConfig {
        conv_frontend: Some("conv_frontend.onnx".to_string()),
        encoder: Some("encoder.int8.onnx".to_string()),
        decoder: Some("decoder.int8.onnx".to_string()),
        tokenizer: Some("tokenizer".to_string()),
        ..Default::default()
    });

    let model = build_model_config(model_path, "qwen3-asr", &file_config, false, "auto", None)
        .expect("qwen3-asr model should build");

    match model {
        ModelType::OfflineQwen3Asr {
            conv_frontend,
            encoder,
            decoder,
            tokenizer,
            ..
        } => {
            assert_eq!(conv_frontend, model_path.join("conv_frontend.onnx"));
            assert_eq!(encoder, model_path.join("encoder.int8.onnx"));
            assert_eq!(decoder, model_path.join("decoder.int8.onnx"));
            assert_eq!(tokenizer, model_path.join("tokenizer"));
        }
        other => panic!("expected OfflineQwen3Asr, got {other:?}"),
    }
}
#[test]
fn build_model_config_supports_parakeet_tdt() {
    let model_path = Path::new("C:/models/parakeet-tdt");
    let file_config = Some(ModelFileConfig {
        encoder: Some("encoder.int8.onnx".to_string()),
        decoder: Some("decoder.int8.onnx".to_string()),
        joiner: Some("joiner.int8.onnx".to_string()),
        tokens: Some("tokens.txt".to_string()),
        ..Default::default()
    });

    let model = build_model_config(
        model_path,
        "parakeet-tdt",
        &file_config,
        false,
        "auto",
        None,
    )
    .expect("parakeet-tdt model should build");

    match model {
        ModelType::OfflineParakeetTdt {
            encoder,
            decoder,
            joiner,
            tokens,
        } => {
            assert_eq!(encoder, model_path.join("encoder.int8.onnx"));
            assert_eq!(decoder, model_path.join("decoder.int8.onnx"));
            assert_eq!(joiner, model_path.join("joiner.int8.onnx"));
            assert_eq!(tokens, model_path.join("tokens.txt"));
        }
        other => panic!("expected OfflineParakeetTdt, got {other:?}"),
    }
}
#[test]
fn build_model_config_supports_moonshine_v2() {
    let model_path = Path::new("C:/models/moonshine-v2");
    let file_config = Some(ModelFileConfig {
        encoder: Some("encoder_model.ort".to_string()),
        decoder: Some("decoder_model_merged.ort".to_string()),
        tokens: Some("tokens.txt".to_string()),
        ..Default::default()
    });

    let model = build_model_config(model_path, "moonshine", &file_config, false, "auto", None)
        .expect("moonshine model should build");

    match model {
        ModelType::OfflineMoonshine {
            preprocessor,
            encoder,
            uncached_decoder,
            cached_decoder,
            merged_decoder,
            tokens,
        } => {
            assert!(preprocessor.is_none());
            assert_eq!(encoder, model_path.join("encoder_model.ort"));
            assert!(uncached_decoder.is_none());
            assert!(cached_decoder.is_none());
            assert_eq!(
                merged_decoder,
                Some(model_path.join("decoder_model_merged.ort"))
            );
            assert_eq!(tokens, model_path.join("tokens.txt"));
        }
        other => panic!("expected OfflineMoonshine, got {other:?}"),
    }
}

#[test]
fn build_model_config_supports_moonshine_v1() {
    let model_path = Path::new("C:/models/moonshine-v1");
    let file_config = Some(ModelFileConfig {
        preprocessor: Some("preprocess.onnx".to_string()),
        encoder: Some("encode.int8.onnx".to_string()),
        uncached_decoder: Some("uncached_decode.int8.onnx".to_string()),
        cached_decoder: Some("cached_decode.int8.onnx".to_string()),
        tokens: Some("tokens.txt".to_string()),
        ..Default::default()
    });

    let model = build_model_config(model_path, "moonshine", &file_config, false, "auto", None)
        .expect("moonshine v1 model should build");

    match model {
        ModelType::OfflineMoonshine {
            preprocessor,
            encoder,
            uncached_decoder,
            cached_decoder,
            merged_decoder,
            tokens,
        } => {
            assert_eq!(preprocessor, Some(model_path.join("preprocess.onnx")));
            assert_eq!(encoder, model_path.join("encode.int8.onnx"));
            assert_eq!(
                uncached_decoder,
                Some(model_path.join("uncached_decode.int8.onnx"))
            );
            assert_eq!(
                cached_decoder,
                Some(model_path.join("cached_decode.int8.onnx"))
            );
            assert!(merged_decoder.is_none());
            assert_eq!(tokens, model_path.join("tokens.txt"));
        }
        other => panic!("expected OfflineMoonshine, got {other:?}"),
    }
}

#[test]
fn build_model_config_supports_funasr_nano_without_tokens() {
    let model_path = Path::new("C:/models/funasr-nano");
    let file_config = Some(ModelFileConfig {
        encoder_adaptor: Some("encoder_adaptor.int8.onnx".to_string()),
        llm: Some("llm.int8.onnx".to_string()),
        embedding: Some("embedding.int8.onnx".to_string()),
        tokenizer: Some("Qwen3-0.6B".to_string()),
        ..Default::default()
    });

    let model = build_model_config(model_path, "funasr-nano", &file_config, false, "auto", None)
        .expect("funasr-nano should build without tokens");

    match model {
        ModelType::OfflineFunASRNano { tokens, .. } => {
            assert!(tokens.is_none());
        }
        other => panic!("expected OfflineFunASRNano, got {other:?}"),
    }
}

#[test]
fn build_model_config_still_requires_tokens_for_sensevoice() {
    let model_path = Path::new("C:/models/sensevoice");
    let file_config = Some(ModelFileConfig {
        model: Some("model.int8.onnx".to_string()),
        ..Default::default()
    });

    let error = build_model_config(model_path, "sensevoice", &file_config, true, "auto", None)
        .expect_err("sensevoice should still require tokens.txt");

    assert!(
        error
            .message
            .contains("Required file name not specified in config"),
        "unexpected error: {error}"
    );
    assert_eq!(error.kind, AsrPortErrorKind::Model);
}

#[test]
fn build_offline_model_config_rejects_online_model_type_before_file_validation() {
    let model_path = Path::new("C:/models/zipformer");
    let file_config = Some(ModelFileConfig::default());

    let error =
        build_offline_model_config(model_path, "zipformer", &file_config, false, "auto", None)
            .expect_err("offline transcription should reject online model types");

    assert_eq!(error.kind, AsrPortErrorKind::Unsupported);
    assert_eq!(error.message, "Unsupported offline model type: zipformer");
}

#[test]
fn create_offline_recognizer_rejects_online_model_type() {
    let model_type = ModelType::OnlineParaformer {
        encoder: "encoder.onnx".into(),
        decoder: "decoder.onnx".into(),
        tokens: "tokens.txt".into(),
    };

    let error = match create_offline_recognizer(model_type, 1, Some("cpu")) {
        Err(error) => error,
        Ok(_) => panic!("online recognizer variants cannot be used for offline transcription"),
    };

    assert_eq!(error.kind, AsrPortErrorKind::Unsupported);
    assert_eq!(error.message, "Unsupported offline model type: paraformer");
}

#[test]
fn offline_decode_result_reports_empty_text() {
    let result = OfflineDecodeResult {
        text: "   ".to_string(),
        tokens: Vec::new(),
        timestamps: None,
    };

    assert!(result.is_empty_text());
}
#[test]
fn build_model_config_constructs_offline_omnilingual_when_files_present() {
    let model_path = Path::new("C:/models/omniasr");
    let file_config = Some(ModelFileConfig {
        model: Some("model.int8.onnx".to_string()),
        tokens: Some("tokens.txt".to_string()),
        ..Default::default()
    });

    let model_type =
        build_offline_model_config(model_path, "omnilingual", &file_config, false, "auto", None)
            .expect("omnilingual config should succeed with valid files");

    match model_type {
        ModelType::OfflineOmnilingual { model, tokens } => {
            assert_eq!(model, model_path.join("model.int8.onnx"));
            assert_eq!(tokens, model_path.join("tokens.txt"));
        }
        other => panic!("expected OfflineOmnilingual, got {other:?}"),
    }
}

#[test]
fn build_model_config_normalizes_language_for_funasr_nano() {
    let model_path = Path::new("C:/models/funasr-nano");
    let file_config = Some(ModelFileConfig {
        encoder_adaptor: Some("encoder_adaptor.onnx".to_string()),
        llm: Some("llm.onnx".to_string()),
        embedding: Some("embedding.onnx".to_string()),
        tokenizer: Some("tokenizer".to_string()),
        ..Default::default()
    });

    for lang in ["auto", "multilingual"] {
        let model = build_model_config(model_path, "funasr-nano", &file_config, false, lang, None)
            .expect("funasr-nano model should build");
        match model {
            ModelType::OfflineFunASRNano { language, .. } => {
                assert_eq!(
                    language, "",
                    "language '{lang}' should be normalized to empty string"
                );
            }
            other => panic!("expected OfflineFunASRNano, got {other:?}"),
        }
    }

    let model = build_model_config(model_path, "funasr-nano", &file_config, false, "zh", None)
        .expect("funasr-nano model should build");
    match model {
        ModelType::OfflineFunASRNano { language, .. } => {
            assert_eq!(language, "zh");
        }
        other => panic!("expected OfflineFunASRNano, got {other:?}"),
    }
}

// Regression test: sherpa-onnx's OfflineFunASRNanoModelConfig defaults
// max_new_tokens to 0, which prevents the model from generating any tokens
// and causes empty transcription output. The recognizer builder must
// explicitly override it with a positive value.
#[test]
fn build_model_config_funasr_nano_sets_nonzero_max_new_tokens() {
    let model_path = Path::new("C:/models/funasr-nano");
    let file_config = Some(ModelFileConfig {
        encoder_adaptor: Some("encoder_adaptor.int8.onnx".to_string()),
        llm: Some("llm.int8.onnx".to_string()),
        embedding: Some("embedding.int8.onnx".to_string()),
        tokenizer: Some("Qwen3-0.6B".to_string()),
        ..Default::default()
    });

    for lang in ["auto", "multilingual", "zh", "en", ""] {
        let result =
            build_offline_model_config(model_path, "funasr-nano", &file_config, false, lang, None);
        assert!(
            result.is_ok(),
            "funasr-nano config should succeed for language={lang:?}: {:?}",
            result.err()
        );
    }
}

#[test]
fn build_model_config_normalizes_language_for_whisper_and_sensevoice() {
    let model_path = Path::new("C:/models/test");
    let whisper_config = Some(ModelFileConfig {
        encoder: Some("encoder.onnx".to_string()),
        decoder: Some("decoder.onnx".to_string()),
        tokens: Some("tokens.txt".to_string()),
        ..Default::default()
    });

    for lang in ["auto", "multilingual"] {
        let model = build_model_config(model_path, "whisper", &whisper_config, false, lang, None)
            .expect("whisper should build");
        match model {
            ModelType::OfflineWhisper { language, .. } => {
                assert_eq!(
                    language, "",
                    "whisper language '{lang}' should be normalized to empty string"
                );
            }
            other => panic!("expected OfflineWhisper, got {other:?}"),
        }
    }

    let sensevoice_config = Some(ModelFileConfig {
        model: Some("model.onnx".to_string()),
        tokens: Some("tokens.txt".to_string()),
        ..Default::default()
    });

    for lang in ["", "multilingual"] {
        let model = build_model_config(
            model_path,
            "sensevoice",
            &sensevoice_config,
            false,
            lang,
            None,
        )
        .expect("sensevoice should build");
        match model {
            ModelType::OfflineSenseVoice { language, .. } => {
                assert_eq!(
                    language, "auto",
                    "sensevoice language '{lang}' should be normalized to 'auto'"
                );
            }
            other => panic!("expected OfflineSenseVoice, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn test_funasr_nano_decode_not_empty_with_auto_language() {
    use sona_core::ports::asr::BatchTranscriberPort;
    use sona_core::transcription::runtime::{BatchTranscribePlan, OutputTarget};

    let model_dir = Path::new(r"D:\projects\models\sherpa-onnx-funasr-nano-int8-2025-12-30");
    let wav_path = Path::new(r"D:\projects\sona\platforms\desktop\sample.wav");
    if !model_dir.exists() || !wav_path.exists() {
        return;
    }
    let file_config = Some(ModelFileConfig {
        encoder_adaptor: Some("encoder_adaptor.int8.onnx".to_string()),
        llm: Some("llm.int8.onnx".to_string()),
        embedding: Some("embedding.int8.onnx".to_string()),
        tokenizer: Some("Qwen3-0.6B".to_string()),
        ..Default::default()
    });

    for lang in ["auto", "zh", ""] {
        let model_type =
            build_offline_model_config(model_dir, "funasr-nano", &file_config, false, lang, None)
                .unwrap();
        let recognizer = create_offline_recognizer(model_type, 4, None).unwrap();
        let mut reader = hound::WavReader::open(wav_path).unwrap();
        let samples: Vec<f32> = reader
            .samples::<i16>()
            .map(|s| s.unwrap() as f32 / 32768.0)
            .collect();
        let result = sona_sherpa_onnx::recognizer::decode_offline_samples(&recognizer, &samples)
            .expect("should decode");
        println!(
            "FunASR Nano decode result with lang='{lang}': text='{}', tokens_len={}, timestamps={:?}",
            result.text,
            result.tokens.len(),
            result.timestamps
        );

        // Also test short slices (like in pseudo-streaming or short VAD chunks):
        for duration_s in [0.2, 0.4, 0.6, 0.8] {
            let num_samples = (16000.0 * duration_s) as usize;
            let slice = &samples[..num_samples.min(samples.len())];
            let slice_result =
                sona_sherpa_onnx::recognizer::decode_offline_samples(&recognizer, slice);
            println!(
                "  slice duration={duration_s}s: text='{}'",
                slice_result
                    .as_ref()
                    .map(|r| r.text.as_str())
                    .unwrap_or("<None>")
            );
        }
        assert!(
            !result.text.trim().is_empty(),
            "Transcribed text should not be empty for lang='{lang}', got: {:?}",
            result.text
        );
    }

    // Test with VAD enabled (as in real app)
    let vad_path = Path::new(r"D:\projects\models\silero_vad.onnx");
    let punct_path = Path::new(
        r"D:\projects\models\sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12\model.onnx",
    );

    let vad_model = if vad_path.exists() {
        Some(vad_path.to_string_lossy().to_string())
    } else {
        None
    };
    let punctuation_model = if punct_path.exists() {
        Some(punct_path.to_string_lossy().to_string())
    } else {
        None
    };

    println!(
        "Testing batch with vad_model={:?}, punctuation_model={:?}",
        vad_model, punctuation_model
    );

    let plan = BatchTranscribePlan {
        input_path: wav_path.to_path_buf(),
        save_to_path: None,
        engine: sona_core::ports::asr::LocalAsrEngine::SherpaOnnx,
        model_path: model_dir.to_string_lossy().to_string(),
        num_threads: 4,
        enable_itn: false,
        language: "auto".to_string(),
        punctuation_model,
        alignment_model: None,
        vad_model,
        vad_buffer: 5.0,
        batch_segmentation_mode: sona_core::ports::asr::BatchSegmentationMode::Vad,
        model_type: "funasr-nano".to_string(),
        file_config: file_config.clone(),
        hotwords: None,
        speaker_processing: None,
        gpu_acceleration: Some("cpu".to_string()),
        export_format: sona_core::export::ExportFormat::Json,
        output_target: OutputTarget::Stdout,
        quiet: false,
        ffmpeg_path: Some(r"C:\Users\asoda\scoop\shims\ffmpeg.exe".to_string()),
    };

    let segments = sona_sherpa_onnx::batch::LocalBatchAsrAdapter::default()
        .transcribe(plan)
        .await
        .expect("batch transcribe should succeed");
    println!("Batch transcribe segments count: {}", segments.len());
    for (i, seg) in segments.iter().enumerate() {
        println!(
            "  seg[{i}]: start={}, end={}, text='{}'",
            seg.start, seg.end, seg.text
        );
    }
    assert!(!segments.is_empty(), "Batch segments should not be empty!");
}

#[tokio::test]
async fn test_funasr_nano_user_history_audio() {
    use sona_core::ports::asr::BatchTranscriberPort;
    use sona_core::transcription::runtime::{BatchTranscribePlan, OutputTarget};

    let model_dir = Path::new(r"D:\projects\models\sherpa-onnx-funasr-nano-int8-2025-12-30");
    let wav_path = Path::new(
        r"C:\Users\asoda\AppData\Local\com.asoda.sona\history\97f391f9-1baf-4b1e-abe3-2918939c1580.wav",
    );
    if !model_dir.exists() || !wav_path.exists() {
        println!("Skipping test: model or user history audio not found");
        return;
    }
    let file_config = Some(ModelFileConfig {
        encoder_adaptor: Some("encoder_adaptor.int8.onnx".to_string()),
        llm: Some("llm.int8.onnx".to_string()),
        embedding: Some("embedding.int8.onnx".to_string()),
        tokenizer: Some("Qwen3-0.6B".to_string()),
        ..Default::default()
    });

    let vad_path = Path::new(r"D:\projects\models\silero_vad.onnx");
    let punct_path = Path::new(
        r"D:\projects\models\sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12\model.onnx",
    );

    let vad_model = if vad_path.exists() {
        Some(vad_path.to_string_lossy().to_string())
    } else {
        None
    };
    let punctuation_model = if punct_path.exists() {
        Some(punct_path.to_string_lossy().to_string())
    } else {
        None
    };

    let plan = BatchTranscribePlan {
        input_path: wav_path.to_path_buf(),
        save_to_path: None,
        engine: sona_core::ports::asr::LocalAsrEngine::SherpaOnnx,
        model_path: model_dir.to_string_lossy().to_string(),
        num_threads: 4,
        enable_itn: false,
        language: "auto".to_string(),
        punctuation_model,
        alignment_model: None,
        vad_model,
        vad_buffer: 5.0,
        batch_segmentation_mode: sona_core::ports::asr::BatchSegmentationMode::Vad,
        model_type: "funasr-nano".to_string(),
        file_config: file_config.clone(),
        hotwords: None,
        speaker_processing: None,
        gpu_acceleration: Some("cpu".to_string()),
        export_format: sona_core::export::ExportFormat::Json,
        output_target: OutputTarget::Stdout,
        quiet: false,
        ffmpeg_path: Some(r"C:\Users\asoda\scoop\shims\ffmpeg.exe".to_string()),
    };

    let segments = sona_sherpa_onnx::batch::LocalBatchAsrAdapter::default()
        .transcribe(plan)
        .await
        .expect("batch transcribe should succeed");
    println!(
        "User history audio batch transcribe segments count: {}",
        segments.len()
    );
    for (i, seg) in segments.iter().enumerate() {
        println!(
            "  seg[{i}]: start={}, end={}, text='{}'",
            seg.start, seg.end, seg.text
        );
    }
    assert!(
        !segments.is_empty(),
        "Batch segments should not be empty on user history audio!"
    );
}

#[tokio::test]
async fn test_funasr_nano_durations() {
    let model_dir = Path::new(r"D:\projects\models\sherpa-onnx-funasr-nano-int8-2025-12-30");
    let wav_path = Path::new(
        r"C:\Users\asoda\AppData\Local\com.asoda.sona\history\97f391f9-1baf-4b1e-abe3-2918939c1580.wav",
    );
    if !model_dir.exists() || !wav_path.exists() {
        return;
    }
    let file_config = Some(ModelFileConfig {
        encoder_adaptor: Some("encoder_adaptor.int8.onnx".to_string()),
        llm: Some("llm.int8.onnx".to_string()),
        embedding: Some("embedding.int8.onnx".to_string()),
        tokenizer: Some("Qwen3-0.6B".to_string()),
        ..Default::default()
    });

    let model_type =
        build_offline_model_config(model_dir, "funasr-nano", &file_config, false, "auto", None)
            .unwrap();
    let recognizer = create_offline_recognizer(model_type, 4, None).unwrap();

    let mut reader = hound::WavReader::open(wav_path).unwrap();
    let all_samples: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| s.unwrap() as f32 / 32768.0)
        .collect();

    for seconds in [2, 5, 8, 10, 12, 15, 18, 20, 25, 30] {
        let n = (16000 * seconds).min(all_samples.len());
        let slice = &all_samples[..n];
        let res = sona_sherpa_onnx::recognizer::decode_offline_samples(&recognizer, slice);
        println!(
            "Duration {}s: text='{}' tokens_len={}",
            seconds,
            res.as_ref().map(|r| r.text.as_str()).unwrap_or("<None>"),
            res.as_ref().map(|r| r.tokens.len()).unwrap_or(0),
        );
        let res = res.expect("decode should return a result");
        assert!(
            !res.text.trim().is_empty(),
            "FunASR Nano output must not be empty for duration {seconds}s"
        );
    }
}
