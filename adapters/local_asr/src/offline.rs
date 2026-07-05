use sona_core::transcribe_runtime::OfflineTranscribePlan;
use sona_core::transcript::TranscriptSegment;

pub async fn run_offline_transcription(
    _plan: OfflineTranscribePlan,
) -> Result<Vec<TranscriptSegment>, String> {
    Err("offline transcription adapter is not implemented yet".to_string())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use sona_core::export::ExportFormat;
    use sona_core::transcribe_runtime::{OfflineTranscribePlan, OutputTarget};

    use super::run_offline_transcription;

    #[tokio::test]
    async fn standalone_offline_adapter_exposes_transcription_entrypoint() {
        let plan = OfflineTranscribePlan {
            input_path: PathBuf::from("sample.wav"),
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
        };

        let error = run_offline_transcription(plan).await.unwrap_err();
        assert!(error.contains("not implemented"));
    }
}
