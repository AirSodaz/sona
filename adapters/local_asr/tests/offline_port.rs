use sona_core::export::ExportFormat;
use sona_core::ports::asr::OfflineTranscriber;
use sona_core::transcribe_runtime::{OfflineTranscribePlan, OutputTarget};
use std::path::PathBuf;

fn missing_input_plan() -> OfflineTranscribePlan {
    OfflineTranscribePlan {
        input_path: PathBuf::from("missing.wav"),
        save_to_path: None,
        model_path: "C:/models/demo".to_string(),
        num_threads: 4,
        enable_itn: false,
        language: "auto".to_string(),
        punctuation_model: None,
        vad_model: None,
        vad_buffer: 5.0,
        model_type: "whisper".to_string(),
        file_config: None,
        hotwords: None,
        gpu_acceleration: Some("cpu".to_string()),
        export_format: ExportFormat::Json,
        output_target: OutputTarget::Stdout,
        quiet: true,
    }
}

#[tokio::test]
async fn local_offline_adapter_implements_core_transcriber_port_and_validates_missing_input() {
    let transcriber = sona_local_asr::offline::LocalOfflineAsrAdapter;

    let error = transcriber
        .transcribe(missing_input_plan())
        .await
        .unwrap_err();

    assert!(error.contains("existing file"));
}
