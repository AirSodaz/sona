use super::TranscriptPostprocessor;
use super::types::{AsrEngine, AsrMode, AsrTranscriptionRequest, BatchTranscriptionRequest};

pub trait AsrEngineAdapter {
    fn engine(&self) -> AsrEngine;
    fn batch_request(
        &self,
        file_path: String,
        save_to_path: Option<String>,
        request: AsrTranscriptionRequest,
        speaker_processing: Option<crate::speaker::SpeakerProcessingConfig>,
    ) -> Result<BatchTranscriptionRequest, String>;
}

#[derive(Debug, Clone, Copy)]
pub struct LocalSherpaAdapter;

impl LocalSherpaAdapter {
    pub fn ensure_mode(request: &AsrTranscriptionRequest, expected: AsrMode) -> Result<(), String> {
        if request.engine() != AsrEngine::LocalSherpa {
            return Err("Unsupported ASR engine for local Sherpa adapter".to_string());
        }
        if request.mode != expected {
            return Err(format!(
                "ASR request mode mismatch: expected {:?}, got {:?}",
                expected, request.mode
            ));
        }
        Ok(())
    }
}

impl AsrEngineAdapter for LocalSherpaAdapter {
    fn engine(&self) -> AsrEngine {
        AsrEngine::LocalSherpa
    }

    fn batch_request(
        &self,
        file_path: String,
        save_to_path: Option<String>,
        request: AsrTranscriptionRequest,
        speaker_processing: Option<crate::speaker::SpeakerProcessingConfig>,
    ) -> Result<BatchTranscriptionRequest, String> {
        Self::ensure_mode(&request, AsrMode::Offline)?;
        
        let config = match request.engine_config {
            crate::asr::types::AsrEngineConfig::LocalSherpa {
                model_path,
                num_threads,
                punctuation_model,
                vad_model,
                vad_buffer,
                batch_segmentation_mode,
                model_type,
                file_config,
                gpu_acceleration,
                ..
            } => BatchTranscriptionRequest {
                file_path,
                save_to_path,
                model_path,
                num_threads,
                enable_itn: request.enable_itn,
                language: request.language,
                punctuation_model,
                vad_model,
                vad_buffer,
                batch_segmentation_mode,
                model_type,
                file_config,
                hotwords: request.hotwords,
                speaker_processing,
                normalization_options: request.normalization_options,
                postprocessor: TranscriptPostprocessor::compile(request.postprocess_options)?,
                gpu_acceleration,
            },
            _ => return Err("Expected LocalSherpa engine config".to_string()),
        };
        
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr::types::{TranscriptNormalizationOptions, TranscriptPostprocessOptions};

    fn request(mode: AsrMode) -> AsrTranscriptionRequest {
        AsrTranscriptionRequest::local_sherpa(
            mode,
            "C:/models/asr".to_string(),
            4,
            true,
            "auto".to_string(),
            Some("C:/models/punct".to_string()),
            Some("C:/models/vad.onnx".to_string()),
            5.0,
            "sensevoice".to_string(),
            None,
            Some("Sona".to_string()),
            TranscriptNormalizationOptions::default(),
            TranscriptPostprocessOptions::default(),
            None,
        )
    }

    #[test]
    fn local_sherpa_adapter_builds_batch_request_from_asr_request() {
        let adapter = LocalSherpaAdapter;

        let batch = adapter
            .batch_request(
                "C:/audio/demo.wav".to_string(),
                Some("C:/tmp/demo.wav".to_string()),
                request(AsrMode::Offline),
                None,
            )
            .expect("offline local request should convert");

        assert_eq!(adapter.engine(), AsrEngine::LocalSherpa);
        assert_eq!(batch.model_path, "C:/models/asr");
        assert_eq!(batch.num_threads, 4);
        assert!(batch.enable_itn);
        assert_eq!(batch.punctuation_model.as_deref(), Some("C:/models/punct"));
        assert_eq!(batch.vad_model.as_deref(), Some("C:/models/vad.onnx"));
        assert_eq!(batch.hotwords.as_deref(), Some("Sona"));
    }

    #[test]
    fn local_sherpa_adapter_rejects_streaming_request_for_batch() {
        let adapter = LocalSherpaAdapter;
        let error = adapter
            .batch_request(
                "C:/audio/demo.wav".to_string(),
                None,
                request(AsrMode::Streaming),
                None,
            )
            .expect_err("streaming request should not be used for batch");

        assert!(error.contains("ASR request mode mismatch"));
    }
}
