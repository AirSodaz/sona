use sona_core::ports::vad::{SpeechSpan, VadDetectionOptions};

/// Universal dual-threshold hysteresis state machine for voice activity detection.
///
/// Decoupled from neural networks; operates purely on sequential frame speech probabilities
/// and sample offsets. Enforces onset activation (`threshold`), offset deactivation (`neg_threshold`),
/// minimum speech duration filtering, minimum silence duration boundary detection, threshold
/// escalation for natural sentence termination, and hard maximum segment ceilings.
///
/// Emits RAW, unpadded [`SpeechSpan`]s. Padding and micro-pause RMS splitting are owned
/// solely by Core (`core::transcription::segmentation::segment_batch_audio`).
#[derive(Clone, Debug)]
pub struct UniversalVadStateMachine {
    threshold: f32,
    neg_threshold: f32,
    min_silence_samples: usize,
    min_speech_samples: usize,
    max_speech_samples: usize,
    escalation_samples: usize,
    sample_rate: u32,

    // Runtime state
    current_sample: usize,
    triggered: bool,
    speech_start_sample: usize,
    candidate_start_sample: Option<usize>,
    temp_end_sample: Option<usize>,
    prev_end_sample: Option<usize>,
    next_start_sample: usize,
}

impl UniversalVadStateMachine {
    pub fn new(options: &VadDetectionOptions, sample_rate: u32) -> Self {
        let rate = sample_rate.max(1) as f32;
        let threshold = options.threshold.clamp(0.01, 0.99);
        let neg_threshold = options
            .neg_threshold
            .unwrap_or_else(|| (threshold - 0.15).max(0.01))
            .clamp(0.01, threshold);

        let min_silence_samples = (options.min_silence_duration.max(0.0) * rate).round() as usize;
        let min_speech_samples = (options.min_speech_duration.max(0.0) * rate).round() as usize;
        let max_speech_samples = (options.max_speech_duration.max(0.1) * rate).round() as usize;
        let escalation_samples =
            (options.threshold_escalation_duration.max(0.0) * rate).round() as usize;

        Self {
            threshold,
            neg_threshold,
            min_silence_samples,
            min_speech_samples,
            max_speech_samples,
            escalation_samples,
            sample_rate,
            current_sample: 0,
            triggered: false,
            speech_start_sample: 0,
            candidate_start_sample: None,
            temp_end_sample: None,
            prev_end_sample: None,
            next_start_sample: 0,
        }
    }

    /// Resets the internal state machine back to silence, retaining configuration.
    pub fn reset(&mut self) {
        self.current_sample = 0;
        self.triggered = false;
        self.speech_start_sample = 0;
        self.candidate_start_sample = None;
        self.temp_end_sample = None;
        self.prev_end_sample = None;
        self.next_start_sample = 0;
    }

    /// Returns true if speech is currently active.
    #[inline]
    pub fn is_speech_active(&self) -> bool {
        self.triggered
    }

    /// Processes a single frame probability and advances the internal sample cursor.
    ///
    /// If an active speech segment finishes during this frame (due to silence duration
    /// or max speech ceiling), returns the completed, unpadded [`SpeechSpan`].
    pub fn accept_prob(&mut self, prob: f32, frame_samples: usize) -> Option<SpeechSpan> {
        let frame_start = self.current_sample;
        self.current_sample += frame_samples;

        // Determine if speech duration has passed escalation threshold (e.g. 25s)
        let in_escalation = self.triggered
            && self.current_sample.saturating_sub(self.speech_start_sample)
                >= self.escalation_samples;

        let active_threshold = if in_escalation {
            0.90f32.max(self.threshold)
        } else {
            self.threshold
        };

        let active_neg_threshold = if in_escalation {
            0.75f32.max(self.neg_threshold)
        } else {
            self.neg_threshold
        };

        // 1. Speech continuation / Onset
        if prob >= active_threshold {
            if let Some(_temp_end) = self.temp_end_sample.take() {
                // Speech resumed after short dip; cancel pending silence
                if let Some(prev_end) = self.prev_end_sample
                    && self.next_start_sample < prev_end
                {
                    self.next_start_sample = frame_start;
                }
            }

            if !self.triggered {
                let cand_start = *self.candidate_start_sample.get_or_insert(frame_start);
                if self.current_sample.saturating_sub(cand_start) >= self.min_speech_samples {
                    self.triggered = true;
                    self.speech_start_sample = cand_start;
                    self.candidate_start_sample = None;
                }
            }
        } else if !self.triggered {
            // In silence: abort candidate onset if probability does not sustain threshold
            self.candidate_start_sample = None;
        }

        // 2. Maximum speech duration enforcement (hard limit)
        if self.triggered
            && self.current_sample.saturating_sub(self.speech_start_sample)
                >= self.max_speech_samples
        {
            let end_sample = self
                .prev_end_sample
                .filter(|&end| end > self.speech_start_sample)
                .unwrap_or(self.current_sample);

            let span = SpeechSpan {
                start_sample: self.speech_start_sample,
                end_sample,
                sample_rate: self.sample_rate,
            };

            // Reset state for subsequent segment
            if self.next_start_sample > end_sample {
                self.speech_start_sample = self.next_start_sample;
                self.triggered = true;
            } else if prob >= active_neg_threshold {
                self.speech_start_sample = end_sample;
                self.triggered = true;
            } else {
                self.triggered = false;
                self.speech_start_sample = 0;
            }

            self.temp_end_sample = None;
            self.prev_end_sample = None;
            self.candidate_start_sample = None;
            self.next_start_sample = 0;

            return Some(span);
        }

        // 3. Trailing silence deactivation
        if self.triggered && prob < active_neg_threshold {
            let temp_end = *self.temp_end_sample.get_or_insert(frame_start);

            // Record candidate breakpoint for max-speech splitting
            let silence_samples = self.current_sample.saturating_sub(temp_end);
            if silence_samples >= (self.sample_rate as f32 * 0.098).round() as usize {
                self.prev_end_sample = Some(temp_end);
            }

            // Normal silence deactivation condition reached
            if silence_samples >= self.min_silence_samples {
                let span = SpeechSpan {
                    start_sample: self.speech_start_sample,
                    end_sample: temp_end,
                    sample_rate: self.sample_rate,
                };

                self.triggered = false;
                self.speech_start_sample = 0;
                self.temp_end_sample = None;
                self.prev_end_sample = None;
                self.candidate_start_sample = None;
                self.next_start_sample = 0;

                if span.end_sample.saturating_sub(span.start_sample) >= self.min_speech_samples {
                    return Some(span);
                }
            }
        }

        None
    }

    /// Flushes any pending speech span at the end of an audio stream.
    pub fn flush(&mut self) -> Option<SpeechSpan> {
        if !self.triggered {
            return None;
        }

        let end_sample = self.temp_end_sample.unwrap_or(self.current_sample);
        let start_sample = self.speech_start_sample;

        self.reset();

        if end_sample.saturating_sub(start_sample) >= self.min_speech_samples {
            Some(SpeechSpan {
                start_sample,
                end_sample,
                sample_rate: self.sample_rate,
            })
        } else {
            None
        }
    }

    /// Convenience batch segmentation driver running over sequential frame probabilities.
    pub fn batch_detect(&mut self, probs: &[f32], frame_samples: usize) -> Vec<SpeechSpan> {
        self.reset();
        let mut spans = Vec::new();

        for &prob in probs {
            if let Some(span) = self.accept_prob(prob, frame_samples) {
                spans.push(span);
            }
        }

        if let Some(tail_span) = self.flush() {
            spans.push(tail_span);
        }

        spans
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_options(
        threshold: f32,
        neg_threshold: f32,
        min_silence: f32,
        min_speech: f32,
        max_speech: f32,
    ) -> VadDetectionOptions {
        VadDetectionOptions {
            model_path: "mock.onnx".into(),
            threshold,
            neg_threshold: Some(neg_threshold),
            min_silence_duration: min_silence,
            min_speech_duration: min_speech,
            buffer_seconds: 5.0,
            speech_pad_seconds: 0.25,
            max_speech_duration: max_speech,
            threshold_escalation_duration: 25.0,
        }
    }

    #[test]
    fn hysteresis_activates_at_threshold_and_deactivates_at_neg_threshold() {
        // 512 samples per frame (~32ms @ 16kHz)
        let frame_samples = 512;
        // min_speech = 64ms (2 frames), min_silence = 96ms (3 frames)
        let opts = test_options(0.50, 0.35, 0.096, 0.064, 30.0);
        let mut sm = UniversalVadStateMachine::new(&opts, 16_000);

        // Silence (prob 0.1)
        assert_eq!(sm.accept_prob(0.1, frame_samples), None);
        assert!(!sm.is_speech_active());

        // Probe 0.40 (between neg_threshold and threshold) - should not trigger
        assert_eq!(sm.accept_prob(0.40, frame_samples), None);
        assert!(!sm.is_speech_active());

        // Probe 0.60 (frame 1 of onset candidate)
        assert_eq!(sm.accept_prob(0.60, frame_samples), None);
        assert!(!sm.is_speech_active());

        // Probe 0.65 (frame 2 of onset candidate -> triggers speech!)
        assert_eq!(sm.accept_prob(0.65, frame_samples), None);
        assert!(sm.is_speech_active());

        // Prob dips to 0.40 (above neg_threshold 0.35) -> speech remains active!
        assert_eq!(sm.accept_prob(0.40, frame_samples), None);
        assert!(sm.is_speech_active());

        // Prob dips to 0.20 (below neg_threshold 0.35) - silence frame 1
        assert_eq!(sm.accept_prob(0.20, frame_samples), None);
        assert!(sm.is_speech_active());

        // Silence frame 2
        assert_eq!(sm.accept_prob(0.20, frame_samples), None);
        assert!(sm.is_speech_active());

        // Silence frame 3 (hits min_silence 96ms -> emits span!)
        let span = sm.accept_prob(0.20, frame_samples).expect("emits span");
        assert!(!sm.is_speech_active());

        // Span start: frame index 2 (1024 samples)
        assert_eq!(span.start_sample, 1024);
        // Span end: frame index 5 (2560 samples, start of silence)
        assert_eq!(span.end_sample, 2560);
    }

    #[test]
    fn noise_click_below_min_speech_is_ignored() {
        let frame_samples = 512;
        // min_speech = 100ms (~3 frames)
        let opts = test_options(0.50, 0.35, 0.1, 0.1, 30.0);
        let mut sm = UniversalVadStateMachine::new(&opts, 16_000);

        // One loud click frame (prob 0.9)
        assert_eq!(sm.accept_prob(0.9, frame_samples), None);
        assert!(!sm.is_speech_active());

        // Immediately followed by silence
        assert_eq!(sm.accept_prob(0.1, frame_samples), None);
        assert!(!sm.is_speech_active());

        assert_eq!(sm.flush(), None);
    }

    #[test]
    fn max_speech_duration_cuts_segment() {
        let frame_samples = 512; // 32ms
        // max_speech = 0.16s (5 frames), min_speech = 0.032s (1 frame)
        let opts = test_options(0.50, 0.35, 0.5, 0.03, 0.16);
        let mut sm = UniversalVadStateMachine::new(&opts, 16_000);

        // Continuous high speech probabilities
        assert_eq!(sm.accept_prob(0.8, frame_samples), None);
        assert_eq!(sm.accept_prob(0.8, frame_samples), None);
        assert_eq!(sm.accept_prob(0.8, frame_samples), None);
        assert_eq!(sm.accept_prob(0.8, frame_samples), None);

        // 5th frame reaches max speech duration
        let span = sm
            .accept_prob(0.8, frame_samples)
            .expect("cuts at max duration");
        assert_eq!(span.start_sample, 0);
        assert_eq!(span.end_sample, 5 * 512);
        // Speech should remain triggered for subsequent audio because prob was high
        assert!(sm.is_speech_active());
    }

    #[test]
    fn threshold_escalation_triggers_on_dips_after_threshold_duration() {
        let frame_samples = 1600; // 100ms @ 16kHz
        let mut opts = test_options(0.50, 0.35, 0.5, 0.1, 30.0);
        opts.threshold_escalation_duration = 0.5; // escalate after 500ms
        let mut sm = UniversalVadStateMachine::new(&opts, 16_000);

        // 5 frames of speech (0.5s)
        for _ in 0..5 {
            assert_eq!(sm.accept_prob(0.8, frame_samples), None);
        }
        assert!(sm.is_speech_active());

        // Now past 0.5s: threshold escalated to 0.90, neg_threshold escalated to 0.75
        // A dip to 0.60 (which is < 0.75) now counts as silence!
        for _ in 0..4 {
            assert_eq!(sm.accept_prob(0.60, frame_samples), None);
        }
        // 5th frame of silence (500ms min_silence reached) -> emits span!
        let span = sm
            .accept_prob(0.60, frame_samples)
            .expect("emits span on escalated dip");
        assert_eq!(span.start_sample, 0);
        assert_eq!(span.end_sample, 5 * 1600);
    }
    #[test]
    fn ambient_noise_does_not_latch_candidate_onset() {
        let frame_samples = 512;
        // min_speech = 64ms (2 frames)
        let opts = test_options(0.50, 0.35, 0.5, 0.064, 30.0);
        let mut sm = UniversalVadStateMachine::new(&opts, 16_000);

        // Frame 1: isolated spike 0.60 (candidate onset)
        assert_eq!(sm.accept_prob(0.60, frame_samples), None);
        assert!(!sm.is_speech_active());

        // Frame 2..5: ambient noise hovering at 0.40 (between neg_threshold 0.35 and threshold 0.50)
        // Must abort candidate onset rather than latching it
        for _ in 0..4 {
            assert_eq!(sm.accept_prob(0.40, frame_samples), None);
            assert!(!sm.is_speech_active());
        }

        // Frame 6: single high prob frame 0.70. Since candidate was cleared, this is frame 1 of new candidate
        assert_eq!(sm.accept_prob(0.70, frame_samples), None);
        assert!(!sm.is_speech_active());

        // Frame 7: second high prob frame -> NOW triggers speech, starting at frame 6 (index 5)
        assert_eq!(sm.accept_prob(0.70, frame_samples), None);
        assert!(sm.is_speech_active());
        assert_eq!(sm.speech_start_sample, 5 * 512);
    }
}
