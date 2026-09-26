use crate::audio::{SafeVad, load_vad, reset_vad};
use crate::punctuation::Punctuation;
use crate::recognizer::{Recognizer, SafeStream};
use sona_core::ports::asr::TranscriptNormalizationOptions;
use sona_core::transcription::postprocess::TranscriptPostprocessor;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;
use tokio::sync::{Mutex, OnceCell};

pub type RecognizerCell = Arc<OnceCell<Arc<Recognizer>>>;
pub type PunctuationCell = Arc<OnceCell<Arc<Punctuation>>>;

#[derive(Clone)]
pub struct RecognizerPool {
    recognizers: Arc<Mutex<HashMap<ModelConfigKey, RecognizerCell>>>,
    punctuations: Arc<Mutex<HashMap<String, PunctuationCell>>>,
    vads: Arc<Mutex<HashMap<String, Vec<SafeVad>>>>,
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
            vads: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn prune_idle_recognizers_locked(
        recognizers: &mut HashMap<ModelConfigKey, RecognizerCell>,
        active_key: Option<&ModelConfigKey>,
        allowed_providers: Option<&[Option<String>]>,
    ) {
        recognizers.retain(|k, cell| {
            if let Some(active) = active_key
                && k.is_same_model_config(active)
            {
                if let Some(allowed) = allowed_providers {
                    if allowed.contains(&k.gpu_provider) {
                        return true;
                    }
                } else {
                    return true;
                }
            }
            if let Some(recognizer) = cell.get() {
                Arc::strong_count(recognizer) > 1
            } else {
                Arc::strong_count(cell) > 1
            }
        });
    }

    fn prune_idle_punctuations_locked(
        punctuations: &mut HashMap<String, PunctuationCell>,
        active_path: Option<&str>,
    ) {
        punctuations.retain(|path, cell| {
            if let Some(active) = active_path
                && path.as_str() == active
            {
                return true;
            }
            if let Some(punctuation) = cell.get() {
                Arc::strong_count(punctuation) > 1
            } else {
                Arc::strong_count(cell) > 1
            }
        });
    }

    pub async fn prune_all_idle(&self) {
        let mut recognizers = self.recognizers.lock().await;
        Self::prune_idle_recognizers_locked(&mut recognizers, None, None);
        drop(recognizers);

        let mut punctuations = self.punctuations.lock().await;
        Self::prune_idle_punctuations_locked(&mut punctuations, None);
        drop(punctuations);

        let mut vads = self.vads.lock().await;
        vads.clear();
        drop(vads);

        crate::speaker::clear_speaker_caches();
        crate::batch::prune_offline_batch_caches();
    }
    pub async fn prepare_vad(&self, vad_model_path: &str) -> bool {
        if vad_model_path.trim().is_empty() {
            return false;
        }
        {
            let mut vads = self.vads.lock().await;
            let pool = vads.entry(vad_model_path.to_string()).or_default();
            if !pool.is_empty() {
                return true;
            }
        }

        if let Some(mut vad) = load_vad(Some(vad_model_path.to_string())) {
            reset_vad(&mut vad);
            let mut vads = self.vads.lock().await;
            let pool = vads.entry(vad_model_path.to_string()).or_default();
            if pool.is_empty() {
                pool.push(vad);
            }
            true
        } else {
            false
        }
    }

    pub async fn get_or_create_vad(&self, vad_model_path: &str) -> Option<SafeVad> {
        if vad_model_path.trim().is_empty() {
            return None;
        }
        {
            let mut vads = self.vads.lock().await;
            if let Some(mut vad) = vads.get_mut(vad_model_path).and_then(|pool| pool.pop()) {
                reset_vad(&mut vad);
                return Some(vad);
            }
        }

        load_vad(Some(vad_model_path.to_string()))
    }

    pub async fn recycle_vad(&self, vad_model_path: &str, mut vad: SafeVad) {
        if vad_model_path.trim().is_empty() {
            return;
        }
        reset_vad(&mut vad);
        let mut vads = self.vads.lock().await;
        let pool = vads.entry(vad_model_path.to_string()).or_default();
        if pool.len() < 2 {
            pool.push(vad);
        }
    }

    pub async fn prune_idle_punctuations(&self, active_path: Option<&str>) {
        let mut punctuations = self.punctuations.lock().await;
        Self::prune_idle_punctuations_locked(&mut punctuations, active_path);
    }

    pub async fn recognizer_cell_for_gpu_plan(
        &self,
        key: &ModelConfigKey,
        provider_options: Vec<Option<String>>,
        primary_provider: Option<String>,
    ) -> (RecognizerCell, bool) {
        let mut recognizers = self.recognizers.lock().await;
        Self::prune_idle_recognizers_locked(&mut recognizers, Some(key), Some(&provider_options));

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
        Self::prune_idle_punctuations_locked(&mut punctuations, Some(&path));
        punctuations
            .entry(path)
            .or_insert_with(|| Arc::new(OnceCell::new()))
            .clone()
    }

    #[cfg(test)]
    pub async fn cached_recognizer_count(&self) -> usize {
        self.recognizers.lock().await.len()
    }

    #[cfg(test)]
    pub async fn cached_punctuation_count(&self) -> usize {
        self.punctuations.lock().await.len()
    }
    #[cfg(test)]
    pub async fn cached_vad_count(&self) -> usize {
        self.vads.lock().await.values().map(|v| v.len()).sum()
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

    pub fn is_same_model_config(&self, other: &ModelConfigKey) -> bool {
        self.model_path == other.model_path
            && self.model_type == other.model_type
            && self.num_threads == other.num_threads
            && self.enable_itn == other.enable_itn
            && self.language == other.language
            && self.hotwords == other.hotwords
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
    let initial_refresh_rate = instance.offline_state.backoff().initial_interval_ms();
    instance.total_samples = 0;
    instance.segment_start_time = 0.0;
    instance.offline_state = OfflineState::with_initial_refresh_rate(initial_refresh_rate);
    instance.current_segment_id = None;
    instance.clear_partial_metric_sample();
    instance.record_diagnostics = RecordDiagnosticsState::default();
    instance.last_partial_decode_ms.store(0, Ordering::Release);
    instance.current_turn_samples.clear();
    if let Some(tracker) = instance.speaker_tracker.as_ref()
        && let Ok(mut guard) = tracker.lock()
    {
        guard.reset_session();
    }
}

#[derive(Default)]
pub struct SherpaInstance {
    recognizer: Option<Arc<Recognizer>>,
    stream: Option<SafeStream>,
    vad: Option<SafeVad>,
    punctuation: Option<Arc<Punctuation>>,
    pub total_samples: usize,
    pub segment_start_time: f64,
    pub offline_state: OfflineState,
    vad_model: Option<String>,
    vad_buffer: f32,
    pub current_segment_id: Option<String>,
    last_partial_metric_sample: usize,
    is_running: bool,
    pub record_diagnostics: RecordDiagnosticsState,
    pub normalization_options: TranscriptNormalizationOptions,
    pub postprocessor: TranscriptPostprocessor,
    pub last_partial_decode_ms: Arc<AtomicU64>,
    pub speaker_tracker:
        Option<Arc<std::sync::Mutex<crate::streaming::speaker_tracker::OnlineSpeakerTracker>>>,
    pub current_turn_samples: Vec<f32>,
}

impl SherpaInstance {
    pub fn is_running(&self) -> bool {
        self.is_running
    }

    pub fn recognizer(&self) -> Option<&Recognizer> {
        self.recognizer.as_deref()
    }

    pub fn recognizer_clone(&self) -> Option<Arc<Recognizer>> {
        self.recognizer.clone()
    }

    pub fn set_recognizer(&mut self, recognizer: Arc<Recognizer>) {
        self.recognizer = Some(recognizer);
    }

    pub fn punctuation(&self) -> Option<&Punctuation> {
        self.punctuation.as_deref()
    }

    pub fn punctuation_clone(&self) -> Option<Arc<Punctuation>> {
        self.punctuation.clone()
    }

    pub fn has_punctuation(&self) -> bool {
        self.punctuation.is_some()
    }

    pub fn set_punctuation(&mut self, punctuation: Option<Arc<Punctuation>>) {
        self.punctuation = punctuation;
    }

    pub fn configure_vad(&mut self, vad_model: Option<String>, vad_buffer: f32) {
        self.vad = load_vad(vad_model.clone());
        self.vad_model = vad_model;
        self.vad_buffer = vad_buffer;
    }

    pub fn configure_vad_instance(
        &mut self,
        vad: Option<SafeVad>,
        vad_model: Option<String>,
        vad_buffer: f32,
    ) {
        self.vad = vad;
        self.vad_model = vad_model;
        self.vad_buffer = vad_buffer;
    }

    pub fn take_vad(&mut self) -> Option<SafeVad> {
        self.vad.take()
    }

    pub fn vad_model(&self) -> Option<&str> {
        self.vad_model.as_deref()
    }

    pub fn reset_or_reload_vad(&mut self) {
        if self.vad_model.is_none() {
            return;
        }

        if let Some(vad) = self.vad.as_mut() {
            reset_vad(vad);
        } else {
            self.vad = load_vad(self.vad_model.clone());
        }
    }

    pub fn has_vad_configuration(&self) -> bool {
        self.vad_model.is_some()
    }

    pub fn vad(&self) -> Option<&SafeVad> {
        self.vad.as_ref()
    }

    pub fn vad_buffer(&self) -> f32 {
        self.vad_buffer
    }

    pub fn stream(&self) -> Option<&SafeStream> {
        self.stream.as_ref()
    }

    pub fn take_stream(&mut self) -> Option<SafeStream> {
        self.stream.take()
    }

    pub fn restore_stream(&mut self, stream: SafeStream) {
        self.stream = Some(stream);
    }

    pub fn should_record_partial_metric(&self, interval_samples: usize) -> bool {
        self.last_partial_metric_sample == 0
            || self
                .total_samples
                .saturating_sub(self.last_partial_metric_sample)
                >= interval_samples
    }

    pub fn mark_partial_metric_sample(&mut self) {
        self.last_partial_metric_sample = self.total_samples;
    }

    pub fn clear_partial_metric_sample(&mut self) {
        self.last_partial_metric_sample = 0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackoffLevel {
    /// Level 0: baseline refresh rate (e.g. 200ms)
    Normal = 0,
    /// Level 1: 1.5x baseline (e.g. 300ms)
    Relaxed = 1,
    /// Level 2: 2.5x baseline (e.g. 500ms)
    Conservative = 2,
    /// Level 3: 4.0x baseline (e.g. 800ms)
    Sparse = 3,
    /// Level 4: SentenceOnly (0 partial refreshes during speech, only VAD segment final)
    SentenceOnly = 4,
}

impl BackoffLevel {
    pub fn step_down(self) -> Self {
        match self {
            Self::Normal => Self::Relaxed,
            Self::Relaxed => Self::Conservative,
            Self::Conservative => Self::Sparse,
            Self::Sparse | Self::SentenceOnly => Self::SentenceOnly,
        }
    }

    pub fn step_up(self) -> Self {
        match self {
            Self::SentenceOnly => Self::Sparse,
            Self::Sparse => Self::Conservative,
            Self::Conservative => Self::Relaxed,
            Self::Relaxed | Self::Normal => Self::Normal,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicBackoffState {
    initial_interval_ms: u64,
    level: BackoffLevel,
    consecutive_clean_utterances: u32,
    overrun_in_current_utterance: bool,
}

impl Default for DynamicBackoffState {
    fn default() -> Self {
        Self::new(200)
    }
}

impl DynamicBackoffState {
    pub fn new(initial_interval_ms: u64) -> Self {
        Self {
            initial_interval_ms: initial_interval_ms.max(50),
            level: BackoffLevel::Normal,
            consecutive_clean_utterances: 0,
            overrun_in_current_utterance: false,
        }
    }

    pub fn initial_interval_ms(&self) -> u64 {
        self.initial_interval_ms
    }

    pub fn set_initial_interval_ms(&mut self, interval_ms: u64) {
        self.initial_interval_ms = interval_ms.max(50);
    }

    pub fn level(&self) -> BackoffLevel {
        self.level
    }

    pub fn is_sentence_only(&self) -> bool {
        self.level == BackoffLevel::SentenceOnly
    }

    pub fn current_interval_ms(&self) -> Option<u64> {
        match self.level {
            BackoffLevel::Normal => Some(self.initial_interval_ms),
            BackoffLevel::Relaxed => Some((self.initial_interval_ms * 3) / 2),
            BackoffLevel::Conservative => Some((self.initial_interval_ms * 5) / 2),
            BackoffLevel::Sparse => Some(self.initial_interval_ms * 4),
            BackoffLevel::SentenceOnly => None,
        }
    }

    pub fn should_run_partial(&self, elapsed: std::time::Duration) -> bool {
        match self.current_interval_ms() {
            Some(interval_ms) => elapsed.as_millis() as u64 >= interval_ms,
            None => false,
        }
    }

    pub fn record_overrun(&mut self) {
        self.overrun_in_current_utterance = true;
        self.consecutive_clean_utterances = 0;
        self.level = self.level.step_down();
    }

    pub fn record_decode_duration(&mut self, decode_ms: u64) {
        if let Some(target_interval) = self.current_interval_ms() {
            // If decode time exceeds 85% of target interval, step down
            if decode_ms * 100 > target_interval * 85 {
                self.overrun_in_current_utterance = true;
                self.consecutive_clean_utterances = 0;
                self.level = self.level.step_down();
            }
        }
    }

    pub fn on_utterance_end(&mut self, clean: bool) {
        if clean && !self.overrun_in_current_utterance {
            self.consecutive_clean_utterances = self.consecutive_clean_utterances.saturating_add(1);
            if self.consecutive_clean_utterances >= 2 {
                self.level = self.level.step_up();
                self.consecutive_clean_utterances = 0;
            }
        } else {
            self.consecutive_clean_utterances = 0;
        }
        self.overrun_in_current_utterance = false;
    }
}

pub struct OfflineState {
    speech_buffer: Vec<Vec<f32>>,
    ring_buffer: VecDeque<Vec<f32>>,
    is_speaking: bool,
    last_inference_time: Instant,
    utterance_start_sample: usize,
    backoff: DynamicBackoffState,
}

impl Default for OfflineState {
    fn default() -> Self {
        Self {
            speech_buffer: Vec::new(),
            ring_buffer: VecDeque::new(),
            is_speaking: false,
            last_inference_time: Instant::now(),
            utterance_start_sample: 0,
            backoff: DynamicBackoffState::default(),
        }
    }
}

impl OfflineState {
    pub fn with_initial_refresh_rate(initial_refresh_rate_ms: u64) -> Self {
        Self {
            speech_buffer: Vec::new(),
            ring_buffer: VecDeque::new(),
            is_speaking: false,
            last_inference_time: Instant::now(),
            utterance_start_sample: 0,
            backoff: DynamicBackoffState::new(initial_refresh_rate_ms),
        }
    }

    pub fn backoff(&self) -> &DynamicBackoffState {
        &self.backoff
    }

    pub fn backoff_mut(&mut self) -> &mut DynamicBackoffState {
        &mut self.backoff
    }

    pub fn set_initial_refresh_rate(&mut self, rate_ms: u64) {
        self.backoff.set_initial_interval_ms(rate_ms);
    }

    pub fn is_speech_active(&self) -> bool {
        self.is_speaking
    }

    pub fn begin_speech(&mut self, total_samples: usize, samples_to_keep: usize) {
        self.is_speaking = true;
        let context = self.ring_context(samples_to_keep);
        let context_len = context.len();
        if !context.is_empty() {
            self.speech_buffer.push(context);
        }
        self.utterance_start_sample = total_samples.saturating_sub(context_len);
        self.ring_buffer.clear();
    }

    pub fn push_speech_chunk(&mut self, samples: Vec<f32>) {
        self.speech_buffer.push(samples);
    }

    pub fn finish_speech_with_chunk(&mut self, samples: Vec<f32>) {
        self.is_speaking = false;
        self.push_speech_chunk(samples);
    }

    pub fn clear_speech_buffer(&mut self) {
        self.speech_buffer.clear();
    }

    pub fn buffered_speech_chunk_count(&self) -> usize {
        self.speech_buffer.len()
    }

    pub fn buffered_speech_sample_count(&self) -> usize {
        buffered_sample_count(&self.speech_buffer)
    }

    pub fn push_ring_chunk(&mut self, samples: Vec<f32>, max_chunks: usize) {
        self.ring_buffer.push_back(samples);
        while self.ring_buffer.len() > max_chunks {
            self.ring_buffer.pop_front();
        }
    }

    pub fn push_ring_chunk_with_sample_limit(
        &mut self,
        samples: Vec<f32>,
        max_samples: usize,
        trim_slack_samples: usize,
    ) {
        self.ring_buffer.push_back(samples);
        let mut ring_len = self.ring_sample_count();
        while ring_len > max_samples + trim_slack_samples {
            if let Some(first) = self.ring_buffer.front() {
                let first_len = first.len();
                if ring_len.saturating_sub(first_len) >= max_samples {
                    self.ring_buffer.pop_front();
                    ring_len = ring_len.saturating_sub(first_len);
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    pub fn ring_sample_count(&self) -> usize {
        self.ring_buffer.iter().map(Vec::len).sum()
    }

    pub fn speech_chunks(&self) -> &[Vec<f32>] {
        &self.speech_buffer
    }

    pub fn utterance_start_seconds(&self, sample_rate: f64) -> f64 {
        self.utterance_start_sample as f64 / sample_rate
    }

    pub fn should_run_partial(&self, now: Instant) -> bool {
        let elapsed = now.duration_since(self.last_inference_time);
        self.backoff.should_run_partial(elapsed)
    }

    pub fn should_run_inference(&self, now: Instant, min_interval_ms: u128) -> bool {
        now.duration_since(self.last_inference_time).as_millis() > min_interval_ms
    }

    pub fn mark_inference_time(&mut self, now: Instant) {
        self.last_inference_time = now;
    }

    pub fn record_overrun(&mut self) {
        self.backoff.record_overrun();
    }

    pub fn record_decode_duration(&mut self, decode_ms: u64) {
        self.backoff.record_decode_duration(decode_ms);
    }

    pub fn on_utterance_end(&mut self, clean: bool) {
        self.backoff.on_utterance_end(clean);
    }

    fn ring_context(&self, samples_to_keep: usize) -> Vec<f32> {
        if self.ring_buffer.is_empty() {
            return Vec::new();
        }

        let ring_flat: Vec<f32> = self.ring_buffer.iter().flatten().copied().collect();
        let keep_start = ring_flat.len().saturating_sub(samples_to_keep);
        ring_flat[keep_start..].to_vec()
    }
}

#[derive(Default)]
pub struct RecordDiagnosticsState {
    first_sample_logged: bool,
    skipped_while_stopped_logged: bool,
    first_segment_emitted: Arc<AtomicBool>,
}

impl RecordDiagnosticsState {
    pub fn should_log_first_sample(&self) -> bool {
        !self.first_sample_logged
    }

    pub fn mark_first_sample_logged(&mut self) {
        self.first_sample_logged = true;
    }

    pub fn should_log_skipped_while_stopped(&self) -> bool {
        !self.skipped_while_stopped_logged
    }

    pub fn mark_skipped_while_stopped_logged(&mut self) {
        self.skipped_while_stopped_logged = true;
    }

    pub fn first_segment_emitted_flag(&self) -> &Arc<AtomicBool> {
        &self.first_segment_emitted
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

    #[tokio::test]
    async fn recognizer_pool_evicts_idle_models_on_key_switch() {
        let pool = RecognizerPool::new();
        let key1 = key(None);
        let mut key2 = key(None);
        key2.model_type = "whisper".to_string();
        key2.model_path = "C:/models/whisper".to_string();
        let mut key3 = key(None);
        key3.model_type = "paraformer".to_string();
        key3.model_path = "C:/models/paraformer".to_string();

        // 1. Load model 1 into pool
        let (cell1, is_new1) = pool
            .recognizer_cell_for_gpu_plan(
                &key1,
                vec![Some("cpu".to_string())],
                Some("cpu".to_string()),
            )
            .await;
        assert!(is_new1);
        let rec1 = Arc::new(Recognizer::test_dummy());
        assert!(cell1.set(rec1.clone()).is_ok());
        // Drop local reference so only cell1 holds rec1 (strong_count == 1)
        drop(rec1);
        assert_eq!(pool.cached_recognizer_count().await, 1);

        // 2. Switch to model 2: model 1 is idle and should be evicted
        let (cell2, is_new2) = pool
            .recognizer_cell_for_gpu_plan(
                &key2,
                vec![Some("cpu".to_string())],
                Some("cpu".to_string()),
            )
            .await;
        assert!(is_new2);
        // Only model 2 remains in pool
        assert_eq!(pool.cached_recognizer_count().await, 1);

        // 3. Model 2 is actively used by a session (strong_count == 2)
        let rec2 = Arc::new(Recognizer::test_dummy());
        assert!(cell2.set(rec2.clone()).is_ok());
        let active_session_rec2 = rec2.clone();
        drop(rec2);
        assert_eq!(Arc::strong_count(&active_session_rec2), 2);

        // 4. Switch to model 3 while model 2 is still active in a session
        let (cell3, is_new3) = pool
            .recognizer_cell_for_gpu_plan(
                &key3,
                vec![Some("cpu".to_string())],
                Some("cpu".to_string()),
            )
            .await;
        assert!(is_new3);
        // Both model 2 (active) and model 3 (new) are in the pool
        assert_eq!(pool.cached_recognizer_count().await, 2);

        // 5. Session finishes and releases model 2 (strong_count drops to 1)
        drop(active_session_rec2);

        // Re-querying or pruning with model 3 active evicts model 2
        let (cell3_again, is_new3_again) = pool
            .recognizer_cell_for_gpu_plan(
                &key3,
                vec![Some("cpu".to_string())],
                Some("cpu".to_string()),
            )
            .await;
        assert!(!is_new3_again);
        assert!(Arc::ptr_eq(&cell3, &cell3_again));
        // Model 2 was evicted, only model 3 remains
        assert_eq!(pool.cached_recognizer_count().await, 1);

        // 6. prune_all_idle removes model 3 as well if it's idle
        let rec3 = Arc::new(Recognizer::test_dummy());
        assert!(cell3.set(rec3).is_ok());
        pool.prune_all_idle().await;
        assert_eq!(pool.cached_recognizer_count().await, 0);
    }

    #[tokio::test]
    async fn recognizer_pool_evicts_idle_model_on_gpu_provider_switch() {
        let pool = RecognizerPool::new();
        let key_base = key(None);

        // Load model on directml
        let (cell_dml, is_new1) = pool
            .recognizer_cell_for_gpu_plan(
                &key_base,
                vec![Some("directml".to_string())],
                Some("directml".to_string()),
            )
            .await;
        assert!(is_new1);
        let rec_dml = Arc::new(Recognizer::test_dummy());
        assert!(cell_dml.set(rec_dml).is_ok());
        assert_eq!(pool.cached_recognizer_count().await, 1);

        // Switch to cpu only: directml is idle and should be evicted
        let (cell_cpu, is_new2) = pool
            .recognizer_cell_for_gpu_plan(
                &key_base,
                vec![Some("cpu".to_string())],
                Some("cpu".to_string()),
            )
            .await;
        assert!(is_new2);
        assert!(!Arc::ptr_eq(&cell_dml, &cell_cpu));
        // DirectML instance was evicted, only CPU instance remains
        assert_eq!(pool.cached_recognizer_count().await, 1);
    }

    #[tokio::test]
    async fn punctuation_pool_evicts_idle_punctuations_on_switch() {
        let pool = RecognizerPool::new();
        let path1 = "C:/models/punct1.onnx".to_string();
        let path2 = "C:/models/punct2.onnx".to_string();

        let cell1 = pool.punctuation_cell_for_path(path1.clone()).await;
        let punct1 = Arc::new(Punctuation::test_dummy());
        assert!(cell1.set(punct1).is_ok());
        assert_eq!(pool.cached_punctuation_count().await, 1);

        // Switch to punct2: punct1 is idle and should be evicted
        let cell2 = pool.punctuation_cell_for_path(path2.clone()).await;
        assert_eq!(pool.cached_punctuation_count().await, 1);

        // Actively held punct2
        let punct2 = Arc::new(Punctuation::test_dummy());
        assert!(cell2.set(punct2.clone()).is_ok());
        let active_session_punct = punct2.clone();
        drop(punct2);

        // Re-accessing punct1 while punct2 is active keeps punct2
        let _ = pool.punctuation_cell_for_path(path1.clone()).await;
        assert_eq!(pool.cached_punctuation_count().await, 2);

        drop(active_session_punct);
        pool.prune_idle_punctuations(Some(&path1)).await;
        assert_eq!(pool.cached_punctuation_count().await, 1);

        pool.prune_all_idle().await;
        assert_eq!(pool.cached_punctuation_count().await, 0);
    }
    #[tokio::test]
    async fn vad_pool_caches_recycles_and_prunes_vad() {
        let pool = RecognizerPool::new();
        assert_eq!(pool.cached_vad_count().await, 0);

        // Empty path returns false / None
        assert!(!pool.prepare_vad("").await);
        assert!(pool.get_or_create_vad("").await.is_none());
        assert_eq!(pool.cached_vad_count().await, 0);

        // Prune all clears any idle vads
        pool.prune_all_idle().await;
        assert_eq!(pool.cached_vad_count().await, 0);
    }

    #[test]
    fn vad_configuration_is_owned_by_the_runtime_instance() {
        let mut instance = SherpaInstance::default();

        instance.configure_vad(Some(String::new()), 0.75);

        assert!(instance.has_vad_configuration());
        assert_eq!(instance.vad_buffer(), 0.75);
        assert!(instance.vad().is_none());
    }

    #[test]
    fn partial_metric_sample_tracking_is_interval_based() {
        let mut instance = SherpaInstance::default();

        assert!(instance.should_record_partial_metric(16_000));

        instance.total_samples = 8_000;
        instance.mark_partial_metric_sample();

        instance.total_samples = 23_999;
        assert!(!instance.should_record_partial_metric(16_000));

        instance.total_samples = 24_000;
        assert!(instance.should_record_partial_metric(16_000));

        instance.clear_partial_metric_sample();
        assert!(instance.should_record_partial_metric(16_000));
    }

    #[test]
    fn start_and_stop_reset_per_run_state_without_dropping_attachments() {
        let mut instance = SherpaInstance {
            total_samples: 42,
            segment_start_time: 1.25,
            vad_model: Some("vad.onnx".to_string()),
            current_segment_id: Some("segment-1".to_string()),
            record_diagnostics: RecordDiagnosticsState {
                first_sample_logged: true,
                skipped_while_stopped_logged: true,
                first_segment_emitted: Arc::new(AtomicBool::new(true)),
            },
            ..Default::default()
        };
        instance.total_samples = 24;
        instance.mark_partial_metric_sample();
        instance.total_samples = 42;
        instance.offline_state.push_speech_chunk(vec![1.0, 2.0]);
        instance.offline_state.push_ring_chunk(vec![3.0], 10);

        start_instance_runtime(&mut instance, None);

        assert!(instance.is_running());
        assert_eq!(instance.total_samples, 0);
        assert_eq!(instance.segment_start_time, 0.0);
        assert!(instance.offline_state.speech_chunks().is_empty());
        assert_eq!(instance.offline_state.ring_sample_count(), 0);
        assert_eq!(instance.current_segment_id, None);
        assert!(instance.should_record_partial_metric(1));
        assert!(instance.record_diagnostics.should_log_first_sample());
        assert!(
            instance
                .record_diagnostics
                .should_log_skipped_while_stopped()
        );
        assert_eq!(instance.vad_model.as_deref(), Some("vad.onnx"));

        instance.total_samples = 9;
        instance.current_segment_id = Some("segment-2".to_string());
        stop_instance_runtime(&mut instance);

        assert!(!instance.is_running());
        assert_eq!(instance.total_samples, 0);
        assert_eq!(instance.current_segment_id, None);
        assert!(instance.stream().is_none());
        assert_eq!(instance.vad_model.as_deref(), Some("vad.onnx"));
    }

    #[test]
    fn buffered_sample_count_sums_chunk_lengths() {
        assert_eq!(
            buffered_sample_count(&[vec![0.0, 1.0], vec![2.0], vec![]]),
            3
        );
    }

    #[test]
    fn offline_state_moves_ring_context_into_speech_buffer() {
        let mut state = OfflineState::default();
        state.push_ring_chunk(vec![1.0, 2.0], 10);
        state.push_ring_chunk(vec![3.0, 4.0, 5.0], 10);

        state.begin_speech(100, 3);

        assert!(state.is_speech_active());
        assert_eq!(state.speech_chunks(), &[vec![3.0, 4.0, 5.0]]);
        assert!((state.utterance_start_seconds(10.0) - 9.7).abs() < f64::EPSILON);

        state.push_speech_chunk(vec![6.0]);
        state.finish_speech_with_chunk(vec![7.0]);

        assert!(!state.is_speech_active());
        assert_eq!(
            state.speech_chunks(),
            &[vec![3.0, 4.0, 5.0], vec![6.0], vec![7.0]]
        );

        state.clear_speech_buffer();

        assert!(state.speech_chunks().is_empty());
    }

    #[test]
    fn offline_state_trims_ring_buffer_by_sample_budget() {
        let mut state = OfflineState::default();

        state.push_ring_chunk_with_sample_limit(vec![1.0; 3_000], 4_800, 4_000);
        state.push_ring_chunk_with_sample_limit(vec![2.0; 3_000], 4_800, 4_000);
        state.push_ring_chunk_with_sample_limit(vec![3.0; 3_000], 4_800, 4_000);

        assert_eq!(state.ring_sample_count(), 6_000);
    }

    #[test]
    fn offline_state_tracks_inference_interval() {
        let mut state = OfflineState::default();
        let now = Instant::now();

        state.mark_inference_time(now - std::time::Duration::from_millis(250));

        assert!(state.should_run_inference(now, 200));

        state.mark_inference_time(now);

        assert!(!state.should_run_inference(now, 200));
    }

    #[test]
    fn dynamic_backoff_steps_down_on_overrun_and_recovers_after_clean_utterances() {
        let mut backoff = DynamicBackoffState::new(200);
        assert_eq!(backoff.level(), BackoffLevel::Normal);
        assert_eq!(backoff.current_interval_ms(), Some(200));
        assert!(!backoff.is_sentence_only());

        // Overrun 1: Relaxed (1.5x -> 300ms)
        backoff.record_overrun();
        assert_eq!(backoff.level(), BackoffLevel::Relaxed);
        assert_eq!(backoff.current_interval_ms(), Some(300));

        // Overrun 2: Conservative (2.5x -> 500ms)
        backoff.record_overrun();
        assert_eq!(backoff.level(), BackoffLevel::Conservative);
        assert_eq!(backoff.current_interval_ms(), Some(500));

        // Overrun 3: Sparse (4.0x -> 800ms)
        backoff.record_overrun();
        assert_eq!(backoff.level(), BackoffLevel::Sparse);
        assert_eq!(backoff.current_interval_ms(), Some(800));

        // Overrun 4: SentenceOnly (None)
        backoff.record_overrun();
        assert_eq!(backoff.level(), BackoffLevel::SentenceOnly);
        assert_eq!(backoff.current_interval_ms(), None);
        assert!(backoff.is_sentence_only());
        assert!(!backoff.should_run_partial(std::time::Duration::from_secs(10)));

        // Overrun occurred in current utterance
        backoff.record_overrun();
        assert_eq!(backoff.level(), BackoffLevel::SentenceOnly);

        // Utterance with overrun ends (cleans overrun flag, clean count = 0)
        backoff.on_utterance_end(true);
        assert_eq!(backoff.level(), BackoffLevel::SentenceOnly);

        // Clean utterance 1: clean count = 1 (needs 2 consecutive clean utterances to step up)
        backoff.on_utterance_end(true);
        assert_eq!(backoff.level(), BackoffLevel::SentenceOnly);

        // Clean utterance 2: clean count = 2 -> recovers to Sparse
        backoff.on_utterance_end(true);
        assert_eq!(backoff.level(), BackoffLevel::Sparse);
        assert_eq!(backoff.current_interval_ms(), Some(800));

        // Clean utterance 3: clean count = 1
        backoff.on_utterance_end(true);
        assert_eq!(backoff.level(), BackoffLevel::Sparse);

        // Clean utterance 4: clean count = 2 -> recovers to Conservative
        backoff.on_utterance_end(true);
        assert_eq!(backoff.level(), BackoffLevel::Conservative);
        backoff.on_utterance_end(false);
        backoff.on_utterance_end(true);
        assert_eq!(backoff.level(), BackoffLevel::Conservative);
    }

    #[test]
    fn dynamic_backoff_steps_down_on_high_decode_duration() {
        let mut backoff = DynamicBackoffState::new(200);
        assert_eq!(backoff.level(), BackoffLevel::Normal);

        // Decode time 150ms <= 200 * 0.85 (170ms) -> no step down
        backoff.record_decode_duration(150);
        assert_eq!(backoff.level(), BackoffLevel::Normal);

        // Decode time 180ms > 200 * 0.85 -> steps down to Relaxed (300ms)
        backoff.record_decode_duration(180);
        assert_eq!(backoff.level(), BackoffLevel::Relaxed);
        assert_eq!(backoff.current_interval_ms(), Some(300));
    }
}
