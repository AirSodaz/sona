use std::sync::Arc;

use async_trait::async_trait;
use sona_core::ports::asr::{
    AsrPortError, AsrPortErrorKind, BatchTranscriberPort, BatchTranscriptionObserver,
    LocalAsrEngine,
};
use sona_core::transcription::runtime::BatchTranscribePlan;
use sona_core::transcription::transcript::TranscriptSegment;

#[derive(Clone)]
pub struct LocalBatchTranscriberRouter {
    sherpa_onnx: Arc<dyn BatchTranscriberPort>,
    llama_cpp: Option<Arc<dyn BatchTranscriberPort>>,
}

impl LocalBatchTranscriberRouter {
    pub fn new(
        sherpa_onnx: Arc<dyn BatchTranscriberPort>,
        llama_cpp: Arc<dyn BatchTranscriberPort>,
    ) -> Self {
        Self {
            sherpa_onnx,
            llama_cpp: Some(llama_cpp),
        }
    }

    pub fn sherpa_only(sherpa_onnx: Arc<dyn BatchTranscriberPort>) -> Self {
        Self {
            sherpa_onnx,
            llama_cpp: None,
        }
    }
}

#[async_trait]
impl BatchTranscriberPort for LocalBatchTranscriberRouter {
    async fn transcribe(
        &self,
        plan: BatchTranscribePlan,
    ) -> Result<Vec<TranscriptSegment>, AsrPortError> {
        match plan.engine {
            LocalAsrEngine::SherpaOnnx => self.sherpa_onnx.transcribe(plan).await,
            LocalAsrEngine::LlamaCpp => {
                let adapter = self.llama_cpp.as_ref().ok_or_else(|| {
                    AsrPortError::new(
                        AsrPortErrorKind::Unsupported,
                        "The llama.cpp local ASR engine is not available on this host.",
                    )
                })?;
                adapter.transcribe(plan).await
            }
        }
    }

    async fn transcribe_with_observer(
        &self,
        plan: BatchTranscribePlan,
        observer: Arc<dyn BatchTranscriptionObserver>,
    ) -> Result<Vec<TranscriptSegment>, AsrPortError> {
        match plan.engine {
            LocalAsrEngine::SherpaOnnx => {
                self.sherpa_onnx
                    .transcribe_with_observer(plan, observer)
                    .await
            }
            LocalAsrEngine::LlamaCpp => {
                let adapter = self.llama_cpp.as_ref().ok_or_else(|| {
                    AsrPortError::new(
                        AsrPortErrorKind::Unsupported,
                        "The llama.cpp local ASR engine is not available on this host.",
                    )
                })?;
                adapter.transcribe_with_observer(plan, observer).await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sona_core::export::ExportFormat;
    use sona_core::transcription::runtime::OutputTarget;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct RecordingTranscriber(AtomicUsize);

    #[async_trait]
    impl BatchTranscriberPort for RecordingTranscriber {
        async fn transcribe(
            &self,
            _plan: BatchTranscribePlan,
        ) -> Result<Vec<TranscriptSegment>, AsrPortError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(Vec::new())
        }
    }

    fn plan(engine: LocalAsrEngine) -> BatchTranscribePlan {
        BatchTranscribePlan {
            input_path: PathBuf::from("audio.wav"),
            save_to_path: None,
            engine,
            model_path: "models/demo".to_string(),
            num_threads: 4,
            enable_itn: false,
            language: "auto".to_string(),
            punctuation_model: None,
            vad_model: None,
            vad_buffer: 5.0,
            batch_segmentation_mode: sona_core::ports::asr::BatchSegmentationMode::Vad,
            model_type: "qwen3-asr".to_string(),
            file_config: None,
            hotwords: None,
            speaker_processing: None,
            gpu_acceleration: None,
            export_format: ExportFormat::Json,
            output_target: OutputTarget::Stdout,
            quiet: true,
        }
    }

    #[tokio::test]
    async fn routes_each_local_engine_to_its_adapter() {
        let sherpa = Arc::new(RecordingTranscriber(AtomicUsize::new(0)));
        let llama = Arc::new(RecordingTranscriber(AtomicUsize::new(0)));
        let router = LocalBatchTranscriberRouter::new(sherpa.clone(), llama.clone());

        router
            .transcribe(plan(LocalAsrEngine::SherpaOnnx))
            .await
            .unwrap();
        router
            .transcribe(plan(LocalAsrEngine::LlamaCpp))
            .await
            .unwrap();

        assert_eq!(sherpa.0.load(Ordering::SeqCst), 1);
        assert_eq!(llama.0.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn reports_unavailable_llama_engine_on_sherpa_only_hosts() {
        let router = LocalBatchTranscriberRouter::sherpa_only(Arc::new(RecordingTranscriber(
            AtomicUsize::new(0),
        )));

        let error = router
            .transcribe(plan(LocalAsrEngine::LlamaCpp))
            .await
            .unwrap_err();

        assert_eq!(error.kind, AsrPortErrorKind::Unsupported);
    }
}
