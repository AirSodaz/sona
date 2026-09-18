use sona_core::ports::asr::BatchTranscriberPort;
use sona_core::transcription::runtime::{BatchTranscribePlan, OutputTarget};
use std::path::PathBuf;

fn missing_input_plan() -> BatchTranscribePlan {
    BatchTranscribePlan {
        input_path: PathBuf::from("nonexistent_input.wav"),
        save_to_path: None,
        engine: sona_core::ports::asr::LocalAsrEngine::SherpaOnnx,
        model_path: "C:/models".to_string(),
        num_threads: 4,
        enable_itn: false,
        language: "auto".to_string(),
        punctuation_model: None,
        alignment_model: None,
        vad_model: None,
        vad_buffer: 5.0,
        batch_segmentation_mode: sona_core::ports::asr::BatchSegmentationMode::Vad,
        model_type: "whisper".to_string(),
        file_config: None,
        hotwords: None,
        speaker_processing: None,
        gpu_acceleration: None,
        export_format: sona_core::export::ExportFormat::Json,
        output_target: OutputTarget::Stdout,
        quiet: false,
        ffmpeg_path: None,
    }
}

#[tokio::test]
async fn local_batch_adapter_implements_core_transcriber_port_and_validates_missing_input() {
    let transcriber = sona_sherpa_onnx::batch::LocalBatchAsrAdapter::default();
    let result = transcriber.transcribe(missing_input_plan()).await;
    assert!(result.is_err(), "expected Err for missing input file");
    let error = result.unwrap_err();
    assert_eq!(
        error.kind,
        sona_core::ports::asr::AsrPortErrorKind::InvalidRequest
    );
    assert!(
        error.message.contains("nonexistent_input.wav"),
        "error should mention the missing file, got: {error}"
    );
}

#[tokio::test]
async fn local_batch_adapter_preserves_model_configuration_errors() {
    let input = std::env::temp_dir().join(format!("sona-batch-{}.wav", uuid::Uuid::new_v4()));
    std::fs::write(&input, b"not-a-real-wav").unwrap();
    let mut plan = missing_input_plan();
    plan.input_path = input.clone();

    let error = sona_sherpa_onnx::batch::LocalBatchAsrAdapter::default()
        .transcribe(plan)
        .await
        .unwrap_err();

    assert_eq!(error.kind, sona_core::ports::asr::AsrPortErrorKind::Model);
    assert!(error.message.contains("File configuration is missing"));
    std::fs::remove_file(input).unwrap();
}

#[tokio::test]
async fn local_batch_adapter_fails_loudly_on_nonexistent_alignment_model() {
    let input = std::env::temp_dir().join(format!("sona-batch-{}.wav", uuid::Uuid::new_v4()));
    std::fs::write(&input, b"not-a-real-wav").unwrap();
    let mut plan = missing_input_plan();
    plan.input_path = input.clone();
    plan.alignment_model = Some("C:/definitely/missing/alignment_model.onnx".to_string());

    let error = sona_sherpa_onnx::batch::LocalBatchAsrAdapter::default()
        .transcribe(plan)
        .await
        .unwrap_err();

    assert_eq!(error.kind, sona_core::ports::asr::AsrPortErrorKind::Model);
    assert!(error.message.contains("Aligner model path does not exist"));
    std::fs::remove_file(input).unwrap();
}

#[tokio::test]
async fn local_batch_adapter_supports_custom_aligner_engine_set() {
    use sona_core::ports::aligner::AlignerEngineSet;
    let custom_set = AlignerEngineSet::empty();
    let adapter =
        sona_sherpa_onnx::batch::LocalBatchAsrAdapter::default().with_aligner_engines(custom_set);
    let mut plan = missing_input_plan();
    plan.alignment_model = None;
    let error = adapter.transcribe(plan).await.unwrap_err();
    assert_eq!(
        error.kind,
        sona_core::ports::asr::AsrPortErrorKind::InvalidRequest
    );
}
