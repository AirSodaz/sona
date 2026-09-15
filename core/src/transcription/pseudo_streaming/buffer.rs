use std::collections::VecDeque;
use std::time::Instant;

use super::backoff::DynamicBackoffState;

/// Internal audio buffer and state tracker for pseudo-streaming sessions.
///
/// Manages pre-roll silence ring buffer to avoid clipping speech onsets,
/// speech chunk accumulation, and dynamic backoff timing for partial hypotheses.
pub struct PseudoStreamAudioBuffer {
    speech_buffer: Vec<Vec<f32>>,
    ring_buffer: VecDeque<Vec<f32>>,
    is_speaking: bool,
    utterance_start_sample: usize,
    total_samples: usize,
    last_inference_time: Instant,
    backoff: DynamicBackoffState,
}

impl Default for PseudoStreamAudioBuffer {
    fn default() -> Self {
        Self::with_initial_refresh_rate(200)
    }
}

impl PseudoStreamAudioBuffer {
    pub fn with_initial_refresh_rate(initial_refresh_rate_ms: u64) -> Self {
        Self {
            speech_buffer: Vec::new(),
            ring_buffer: VecDeque::new(),
            is_speaking: false,
            utterance_start_sample: 0,
            total_samples: 0,
            last_inference_time: Instant::now(),
            backoff: DynamicBackoffState::new(initial_refresh_rate_ms),
        }
    }

    pub fn is_speech_active(&self) -> bool {
        self.is_speaking
    }

    pub fn total_samples(&self) -> usize {
        self.total_samples
    }

    pub fn advance_total_samples(&mut self, count: usize) {
        self.total_samples = self.total_samples.saturating_add(count);
    }

    pub fn utterance_start_seconds(&self, sample_rate: f64) -> f64 {
        self.utterance_start_sample as f64 / sample_rate.max(1.0)
    }

    pub fn speech_chunks(&self) -> &[Vec<f32>] {
        &self.speech_buffer
    }

    pub fn buffered_speech_chunk_count(&self) -> usize {
        self.speech_buffer.len()
    }

    pub fn buffered_speech_sample_count(&self) -> usize {
        self.speech_buffer.iter().map(Vec::len).sum()
    }

    pub fn ring_sample_count(&self) -> usize {
        self.ring_buffer.iter().map(Vec::len).sum()
    }

    /// Extracts up to `max_samples` from the end of the ring buffer as pre-roll speech context.
    pub fn ring_context(&self, max_samples: usize) -> Vec<f32> {
        let mut result = Vec::new();
        let total_in_ring = self.ring_sample_count();
        let samples_to_skip = total_in_ring.saturating_sub(max_samples);

        let mut skipped = 0;
        for chunk in &self.ring_buffer {
            if skipped + chunk.len() <= samples_to_skip {
                skipped += chunk.len();
                continue;
            }

            let start = samples_to_skip.saturating_sub(skipped);
            result.extend_from_slice(&chunk[start..]);
            skipped += chunk.len();
        }

        result
    }

    /// Transitions to speech state, pulling pre-roll context from the ring buffer.
    pub fn begin_speech(&mut self, pre_roll_samples_to_keep: usize) {
        self.is_speaking = true;
        let context = self.ring_context(pre_roll_samples_to_keep);
        let context_len = context.len();
        if !context.is_empty() {
            self.speech_buffer.push(context);
        }
        self.utterance_start_sample = self.total_samples.saturating_sub(context_len);
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

    /// Flattens speech chunks into a single contiguous PCM slice.
    pub fn flatten_speech_buffer(&self) -> Vec<f32> {
        let total_samples = self.buffered_speech_sample_count();
        let mut flattened = Vec::with_capacity(total_samples);
        for chunk in &self.speech_buffer {
            flattened.extend_from_slice(chunk);
        }
        flattened
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

    pub fn reset(&mut self) {
        self.speech_buffer.clear();
        self.ring_buffer.clear();
        self.is_speaking = false;
        self.utterance_start_sample = 0;
        self.total_samples = 0;
        let initial_refresh = self.backoff.initial_interval_ms();
        self.backoff = DynamicBackoffState::new(initial_refresh);
    }

    pub fn backoff(&self) -> &DynamicBackoffState {
        &self.backoff
    }

    pub fn backoff_mut(&mut self) -> &mut DynamicBackoffState {
        &mut self.backoff
    }

    pub fn should_run_partial(&self, now: Instant) -> bool {
        let elapsed = now.duration_since(self.last_inference_time);
        self.backoff.should_run_partial(elapsed)
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_pre_roll_and_speech_lifecycle() {
        let mut buffer = PseudoStreamAudioBuffer::with_initial_refresh_rate(200);

        // Feed non-speech into ring buffer
        buffer.push_ring_chunk_with_sample_limit(vec![0.1; 1600], 4800, 1600);
        buffer.advance_total_samples(1600);
        assert_eq!(buffer.ring_sample_count(), 1600);
        assert!(!buffer.is_speech_active());

        // Speech starts
        buffer.begin_speech(1600);
        assert!(buffer.is_speech_active());
        assert_eq!(buffer.ring_sample_count(), 0);
        assert_eq!(buffer.buffered_speech_sample_count(), 1600);
        assert_eq!(buffer.utterance_start_seconds(16000.0), 0.0);

        // Push speech chunk
        buffer.push_speech_chunk(vec![0.5; 800]);
        buffer.advance_total_samples(800);
        assert_eq!(buffer.buffered_speech_sample_count(), 2400);

        // Flatten
        let flat = buffer.flatten_speech_buffer();
        assert_eq!(flat.len(), 2400);

        // Speech ends
        buffer.finish_speech_with_chunk(vec![0.2; 400]);
        buffer.advance_total_samples(400);
        assert!(!buffer.is_speech_active());
        assert_eq!(buffer.buffered_speech_sample_count(), 2800);

        buffer.clear_speech_buffer();
        assert_eq!(buffer.buffered_speech_sample_count(), 0);
    }
}
