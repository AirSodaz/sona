use super::model_config::{Punctuation, Recognizer, SafeStream, SafeVad};
use super::types::{TranscriptNormalizationOptions, TranscriptSegment};
use log::info;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct OfflineState {
    pub speech_buffer: Vec<Vec<f32>>,
    pub ring_buffer: std::collections::VecDeque<Vec<f32>>,
    pub is_speaking: bool,
    pub last_inference_time: std::time::Instant,
    pub utterance_start_sample: usize,
}

impl Default for OfflineState {
    fn default() -> Self {
        Self {
            speech_buffer: Vec::new(),
            ring_buffer: std::collections::VecDeque::new(),
            is_speaking: false,
            last_inference_time: std::time::Instant::now(),
            utterance_start_sample: 0,
        }
    }
}

#[derive(Debug)]
pub struct RecordDiagnosticsState {
    pub first_sample_logged: bool,
    pub skipped_while_stopped_logged: bool,
    pub first_segment_emitted: Arc<AtomicBool>,
}

impl Default for RecordDiagnosticsState {
    fn default() -> Self {
        Self {
            first_sample_logged: false,
            skipped_while_stopped_logged: false,
            first_segment_emitted: Arc::new(AtomicBool::new(false)),
        }
    }
}

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
    pub is_running: bool,
    pub record_diagnostics: RecordDiagnosticsState,
    pub normalization_options: TranscriptNormalizationOptions,
}

impl Default for SherpaInstance {
    fn default() -> Self {
        Self {
            recognizer: None,
            stream: None,
            vad: None,
            punctuation: None,
            total_samples: 0,
            segment_start_time: 0.0,
            offline_state: OfflineState::default(),
            vad_model: None,
            vad_buffer: 5.0,
            current_segment_id: None,
            is_running: false,
            record_diagnostics: RecordDiagnosticsState::default(),
            normalization_options: TranscriptNormalizationOptions::default(),
        }
    }
}

pub(crate) fn diagnostics_instance_label(instance_id: &str) -> Option<&'static str> {
    match instance_id {
        "record" => Some("record"),
        "caption" => Some("caption"),
        "voice-typing" => Some("voice-typing"),
        _ => None,
    }
}

pub(crate) fn buffered_sample_count(chunks: &[Vec<f32>]) -> usize {
    chunks.iter().map(|chunk| chunk.len()).sum()
}

pub(crate) fn reset_instance_runtime_state(instance: &mut SherpaInstance) {
    // Reset only per-run counters and buffers. The shared recognizer/VAD/model
    // attachments stay on the instance so later starts can reuse them.
    instance.total_samples = 0;
    instance.segment_start_time = 0.0;
    instance.offline_state = OfflineState::default();
    instance.current_segment_id = None;
    instance.record_diagnostics = RecordDiagnosticsState::default();
}

pub(crate) fn start_instance_runtime(instance: &mut SherpaInstance, stream: Option<SafeStream>) {
    // Online recognizers get a fresh stream per run; offline recognizers leave
    // this as `None` and rebuild utterances from buffered audio chunks.
    instance.stream = stream;
    reset_instance_runtime_state(instance);
    instance.is_running = true;
}

pub(crate) fn stop_instance_runtime(instance: &mut SherpaInstance) {
    // Stopping a run clears volatile state only; the instance still keeps its
    // recognizer and optional VAD/punctuation attachments for the next start.
    instance.stream = None;
    reset_instance_runtime_state(instance);
    instance.is_running = false;
}

pub(crate) fn log_segment_emit_diagnostics(
    instance_id: &str,
    first_segment_emitted: Option<&Arc<AtomicBool>>,
    segment: &TranscriptSegment,
    stage: &str,
) {
    // These logs are intentionally scoped to the long-lived live instances we
    // debug most often (`record`, `caption`, `voice-typing`), not to every
    // possible recognizer consumer.
    let Some(label) = diagnostics_instance_label(instance_id) else {
        return;
    };

    let text_len = segment.text.chars().count();
    if let Some(first_segment_emitted) = first_segment_emitted {
        if first_segment_emitted
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            info!(
                "[Sherpa] {label} first segment emitted. stage={} segment_id={} final={} text_len={}",
                stage, segment.id, segment.is_final, text_len
            );
        }
    }

    info!(
        "[Sherpa] {label} emit. stage={} segment_id={} final={} text_len={}",
        stage, segment.id, segment.is_final, text_len
    );
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelConfigKey {
    pub model_path: String,
    pub model_type: String,
    pub num_threads: i32,
    pub enable_itn: bool,
    pub language: String,
    pub hotwords: Option<String>,
}

pub struct SherpaState {
    // Each logical instance keeps its own runtime buffers and stream state,
    // while recognizers are pooled separately by configuration.
    pub instances: Mutex<HashMap<String, SherpaInstance>>,
    pub recognizer_pool: Mutex<HashMap<ModelConfigKey, Arc<Recognizer>>>,
}

impl Default for SherpaState {
    fn default() -> Self {
        Self::new()
    }
}

impl SherpaState {
    pub fn new() -> Self {
        Self {
            instances: Mutex::new(HashMap::new()),
            recognizer_pool: Mutex::new(HashMap::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn start_instance_runtime_resets_progress_and_enables_running() {
        let mut instance = SherpaInstance::default();
        instance.total_samples = 42;
        instance.segment_start_time = 3.5;
        instance.current_segment_id = Some("segment-1".to_string());
        instance
            .offline_state
            .speech_buffer
            .push(vec![0.1, 0.2, 0.3]);
        instance.offline_state.is_speaking = true;
        instance.record_diagnostics.first_sample_logged = true;
        instance.record_diagnostics.skipped_while_stopped_logged = true;
        instance
            .record_diagnostics
            .first_segment_emitted
            .store(true, Ordering::SeqCst);

        start_instance_runtime(&mut instance, None);

        assert!(instance.is_running);
        assert!(instance.stream.is_none());
        assert_eq!(instance.total_samples, 0);
        assert_eq!(instance.segment_start_time, 0.0);
        assert!(instance.current_segment_id.is_none());
        assert!(instance.offline_state.speech_buffer.is_empty());
        assert!(!instance.offline_state.is_speaking);
        assert!(!instance.record_diagnostics.first_sample_logged);
        assert!(!instance.record_diagnostics.skipped_while_stopped_logged);
        assert!(!instance
            .record_diagnostics
            .first_segment_emitted
            .load(Ordering::SeqCst));
    }

    #[test]
    fn stop_instance_runtime_clears_progress_and_disables_running() {
        let mut instance = SherpaInstance::default();
        instance.is_running = true;
        instance.total_samples = 128;
        instance.segment_start_time = 1.25;
        instance.current_segment_id = Some("segment-2".to_string());
        instance.offline_state.speech_buffer.push(vec![0.4, 0.5]);
        instance.offline_state.is_speaking = true;
        instance.record_diagnostics.first_sample_logged = true;
        instance
            .record_diagnostics
            .first_segment_emitted
            .store(true, Ordering::SeqCst);

        stop_instance_runtime(&mut instance);

        assert!(!instance.is_running);
        assert!(instance.stream.is_none());
        assert_eq!(instance.total_samples, 0);
        assert_eq!(instance.segment_start_time, 0.0);
        assert!(instance.current_segment_id.is_none());
        assert!(instance.offline_state.speech_buffer.is_empty());
        assert!(!instance.offline_state.is_speaking);
        assert!(!instance.record_diagnostics.first_sample_logged);
        assert!(!instance
            .record_diagnostics
            .first_segment_emitted
            .load(Ordering::SeqCst));
    }
}
