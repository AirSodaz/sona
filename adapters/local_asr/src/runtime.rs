use crate::audio::SafeVad;
use crate::punctuation::Punctuation;
use crate::recognizer::{Recognizer, SafeStream};
use sona_core::ports::asr::TranscriptNormalizationOptions;
use sona_core::transcription::postprocess::TranscriptPostprocessor;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tokio::sync::{Mutex, OnceCell};

pub type RecognizerCell = Arc<OnceCell<Arc<Recognizer>>>;
pub type PunctuationCell = Arc<OnceCell<Arc<Punctuation>>>;

#[derive(Clone)]
pub struct RecognizerPool {
    recognizers: Arc<Mutex<HashMap<ModelConfigKey, RecognizerCell>>>,
    punctuations: Arc<Mutex<HashMap<String, PunctuationCell>>>,
}

impl Default for RecognizerPool {
    fn default() -> Self {
        Self::new()
    }
}

impl RecognizerPool {
    pub fn new() -> Self {
        Self {
            recognizers: Arc::new(Mutex::new(HashMap::new())),
            punctuations: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn recognizer_cell_for_gpu_plan(
        &self,
        key: &ModelConfigKey,
        provider_options: Vec<Option<String>>,
        primary_provider: Option<String>,
    ) -> (RecognizerCell, bool) {
        let mut recognizers = self.recognizers.lock().await;
        let existing = provider_options
            .into_iter()
            .find_map(|provider| recognizers.get(&key.with_gpu_provider(provider)).cloned());

        if let Some(cell) = existing {
            (cell, false)
        } else {
            let cell = Arc::new(OnceCell::new());
            recognizers.insert(key.with_gpu_provider(primary_provider), cell.clone());
            (cell, true)
        }
    }

    pub async fn register_recognizer_gpu_provider(
        &self,
        key: &ModelConfigKey,
        provider: Option<String>,
        cell: RecognizerCell,
    ) {
        self.recognizers
            .lock()
            .await
            .insert(key.with_gpu_provider(provider), cell);
    }

    pub async fn punctuation_cell_for_path(&self, path: String) -> PunctuationCell {
        let mut punctuations = self.punctuations.lock().await;
        punctuations
            .entry(path)
            .or_insert_with(|| Arc::new(OnceCell::new()))
            .clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelConfigKey {
    pub model_path: String,
    pub model_type: String,
    pub num_threads: i32,
    pub enable_itn: bool,
    pub language: String,
    pub hotwords: Option<String>,
    pub gpu_provider: Option<String>,
}

impl ModelConfigKey {
    pub fn new(
        model_path: String,
        model_type: String,
        num_threads: i32,
        enable_itn: bool,
        language: String,
        hotwords: Option<String>,
        gpu_provider: Option<String>,
    ) -> Self {
        Self {
            model_path,
            model_type,
            num_threads,
            enable_itn,
            language,
            hotwords,
            gpu_provider,
        }
    }

    pub fn with_gpu_provider(&self, gpu_provider: Option<String>) -> Self {
        Self {
            gpu_provider,
            ..self.clone()
        }
    }
}

pub fn buffered_sample_count(chunks: &[Vec<f32>]) -> usize {
    chunks.iter().map(|chunk| chunk.len()).sum()
}

pub fn start_instance_runtime(instance: &mut SherpaInstance, stream: Option<SafeStream>) {
    instance.stream = stream;
    reset_instance_runtime_state(instance);
    instance.is_running = true;
}

pub fn stop_instance_runtime(instance: &mut SherpaInstance) {
    instance.stream = None;
    reset_instance_runtime_state(instance);
    instance.is_running = false;
}

fn reset_instance_runtime_state(instance: &mut SherpaInstance) {
    instance.total_samples = 0;
    instance.segment_start_time = 0.0;
    instance.offline_state = OfflineState::default();
    instance.current_segment_id = None;
    instance.last_partial_metric_sample = 0;
    instance.record_diagnostics = RecordDiagnosticsState::default();
}

#[derive(Default)]
pub struct SherpaInstance {
    pub recognizer: Option<Arc<Recognizer>>,
    pub stream: Option<SafeStream>,
    pub vad: Option<SafeVad>,
    pub punctuation: Option<Arc<Punctuation>>,
    pub total_samples: usize,
    pub segment_start_time: f64,
    pub offline_state: OfflineState,
    pub vad_model: Option<String>,
    pub vad_buffer: f32,
    pub current_segment_id: Option<String>,
    pub last_partial_metric_sample: usize,
    pub is_running: bool,
    pub record_diagnostics: RecordDiagnosticsState,
    pub normalization_options: TranscriptNormalizationOptions,
    pub postprocessor: TranscriptPostprocessor,
}

pub struct OfflineState {
    pub speech_buffer: Vec<Vec<f32>>,
    pub ring_buffer: VecDeque<Vec<f32>>,
    pub is_speaking: bool,
    pub last_inference_time: Instant,
    pub utterance_start_sample: usize,
}

#[derive(Default)]
pub struct RecordDiagnosticsState {
    pub first_sample_logged: bool,
    pub skipped_while_stopped_logged: bool,
    pub first_segment_emitted: Arc<AtomicBool>,
}

impl Default for OfflineState {
    fn default() -> Self {
        Self {
            speech_buffer: Vec::new(),
            ring_buffer: VecDeque::new(),
            is_speaking: false,
            utterance_start_sample: 0,
            last_inference_time: Instant::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(provider: Option<&str>) -> ModelConfigKey {
        ModelConfigKey {
            model_path: "C:/models/demo".to_string(),
            model_type: "sensevoice".to_string(),
            num_threads: 4,
            enable_itn: true,
            language: "auto".to_string(),
            hotwords: None,
            gpu_provider: provider.map(str::to_string),
        }
    }

    #[test]
    fn model_config_key_separates_gpu_provider() {
        assert_ne!(key(Some("cpu")), key(Some("cuda")));
        assert_ne!(key(Some("cpu")), key(None));
        assert_eq!(key(Some("cpu")), key(Some("cpu")));
    }

    #[tokio::test]
    async fn recognizer_pool_reuses_cells_across_gpu_fallback_aliases() {
        let pool = RecognizerPool::new();
        let base_key = key(None);

        let (cell, is_new) = pool
            .recognizer_cell_for_gpu_plan(
                &base_key,
                vec![Some("cuda".to_string()), Some("cpu".to_string())],
                Some("cuda".to_string()),
            )
            .await;

        assert!(is_new);

        pool.register_recognizer_gpu_provider(&base_key, Some("cpu".to_string()), cell.clone())
            .await;

        let (fallback_cell, fallback_is_new) = pool
            .recognizer_cell_for_gpu_plan(
                &base_key,
                vec![Some("cpu".to_string()), Some("cuda".to_string())],
                Some("cpu".to_string()),
            )
            .await;

        assert!(!fallback_is_new);
        assert!(Arc::ptr_eq(&cell, &fallback_cell));
    }

    #[tokio::test]
    async fn recognizer_pool_reuses_punctuation_cells_by_path() {
        let pool = RecognizerPool::new();

        let first = pool
            .punctuation_cell_for_path("C:/models/punctuation.onnx".to_string())
            .await;
        let second = pool
            .punctuation_cell_for_path("C:/models/punctuation.onnx".to_string())
            .await;

        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn start_and_stop_reset_per_run_state_without_dropping_attachments() {
        let mut instance = SherpaInstance {
            total_samples: 42,
            segment_start_time: 1.25,
            vad_model: Some("vad.onnx".to_string()),
            current_segment_id: Some("segment-1".to_string()),
            last_partial_metric_sample: 24,
            is_running: false,
            record_diagnostics: RecordDiagnosticsState {
                first_sample_logged: true,
                skipped_while_stopped_logged: true,
                first_segment_emitted: Arc::new(AtomicBool::new(true)),
            },
            ..Default::default()
        };
        instance.offline_state.speech_buffer.push(vec![1.0, 2.0]);
        instance.offline_state.ring_buffer.push_back(vec![3.0]);

        start_instance_runtime(&mut instance, None);

        assert!(instance.is_running);
        assert_eq!(instance.total_samples, 0);
        assert_eq!(instance.segment_start_time, 0.0);
        assert!(instance.offline_state.speech_buffer.is_empty());
        assert!(instance.offline_state.ring_buffer.is_empty());
        assert_eq!(instance.current_segment_id, None);
        assert_eq!(instance.last_partial_metric_sample, 0);
        assert!(!instance.record_diagnostics.first_sample_logged);
        assert!(!instance.record_diagnostics.skipped_while_stopped_logged);
        assert_eq!(instance.vad_model.as_deref(), Some("vad.onnx"));

        instance.total_samples = 9;
        instance.current_segment_id = Some("segment-2".to_string());
        stop_instance_runtime(&mut instance);

        assert!(!instance.is_running);
        assert_eq!(instance.total_samples, 0);
        assert_eq!(instance.current_segment_id, None);
        assert!(instance.stream.is_none());
        assert_eq!(instance.vad_model.as_deref(), Some("vad.onnx"));
    }

    #[test]
    fn buffered_sample_count_sums_chunk_lengths() {
        assert_eq!(
            buffered_sample_count(&[vec![0.0, 1.0], vec![2.0], vec![]]),
            3
        );
    }
}
