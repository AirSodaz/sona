use sona_core::ports::vad::{SpeechSpan, VadDetectionOptions, VadStreamEvent};

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
    sub_threshold_jitter_count: usize,
    temp_end_sample: Option<usize>,
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
        let escalation_samples = if options.threshold_escalation_duration > 0.0 {
            (options.threshold_escalation_duration * rate).round() as usize
        } else {
            usize::MAX
        };

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
            sub_threshold_jitter_count: 0,
            temp_end_sample: None,
        }
    }

    /// Resets the internal state machine back to silence, retaining configuration.
    pub fn reset(&mut self) {
        self.current_sample = 0;
        self.triggered = false;
        self.speech_start_sample = 0;
        self.candidate_start_sample = None;
        self.sub_threshold_jitter_count = 0;
        self.temp_end_sample = None;
    }

    /// Returns true if speech is currently active.
    #[inline]
    pub fn is_speech_active(&self) -> bool {
        self.triggered
    }

    /// Processes a single frame probability and advances the internal sample cursor.
    ///
    /// Emits discrete [`VadStreamEvent`] transitions (e.g. onset `SpeechStart` or offset `SpeechEnd`).
    pub fn accept_prob(&mut self, prob: f32, frame_samples: usize) -> Vec<VadStreamEvent> {
        let prob = if prob.is_nan() {
            0.0
        } else {
            prob.clamp(0.0, 1.0)
        };
        let frame_start = self.current_sample;
        self.current_sample += frame_samples;

        let rate = self.sample_rate.max(1) as f32;
        let jitter_tolerance_frames = (0.020 * rate / (frame_samples.max(1) as f32))
            .round()
            .max(1.0) as usize;
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

        let mut events = Vec::new();

        // 1. Speech continuation / Onset
        if self.triggered {
            if prob >= active_neg_threshold {
                self.sub_threshold_jitter_count = 0;
                self.temp_end_sample.take();
            }
        } else if prob >= active_threshold {
            self.sub_threshold_jitter_count = 0;
            let cand_start = *self.candidate_start_sample.get_or_insert(frame_start);
            if self.current_sample.saturating_sub(cand_start) >= self.min_speech_samples {
                self.triggered = true;
                self.speech_start_sample = cand_start;
                self.candidate_start_sample = None;
                events.push(VadStreamEvent::SpeechStart {
                    sample_offset: cand_start,
                });
            }
        } else if self.candidate_start_sample.is_some() {
            if prob >= active_neg_threshold {
                self.sub_threshold_jitter_count += 1;
                if self.sub_threshold_jitter_count > jitter_tolerance_frames {
                    self.candidate_start_sample = None;
                    self.sub_threshold_jitter_count = 0;
                }
            } else {
                // Hard silence below neg_threshold immediately aborts candidate onset
                self.candidate_start_sample = None;
                self.sub_threshold_jitter_count = 0;
            }
        }

        // 2. Maximum speech duration enforcement (hard limit)
        if self.triggered
            && self.current_sample.saturating_sub(self.speech_start_sample)
                >= self.max_speech_samples
        {
            let end_sample = self.temp_end_sample.unwrap_or(self.current_sample);

            if end_sample.saturating_sub(self.speech_start_sample) >= self.min_speech_samples {
                events.push(VadStreamEvent::SpeechEnd {
                    start_sample: self.speech_start_sample,
                    end_sample,
                });
            }

            // Reset state for subsequent segment
            let continue_speech = prob >= active_neg_threshold;
            if continue_speech {
                self.speech_start_sample = end_sample;
                self.triggered = true;
                events.push(VadStreamEvent::SpeechStart {
                    sample_offset: end_sample,
                });
            } else {
                self.triggered = false;
                self.speech_start_sample = 0;
            }

            self.temp_end_sample = None;
            self.candidate_start_sample = None;
            self.sub_threshold_jitter_count = 0;

            return events;
        }

        // 3. Trailing silence deactivation
        if self.triggered && prob < active_neg_threshold {
            let temp_end = *self.temp_end_sample.get_or_insert(frame_start);
            let silence_samples = self.current_sample.saturating_sub(temp_end);

            // Normal silence deactivation condition reached
            if silence_samples >= self.min_silence_samples {
                let start_sample = self.speech_start_sample;
                let end_sample = temp_end;

                self.triggered = false;
                self.speech_start_sample = 0;
                self.temp_end_sample = None;
                self.candidate_start_sample = None;
                self.sub_threshold_jitter_count = 0;

                if end_sample.saturating_sub(start_sample) >= self.min_speech_samples {
                    events.push(VadStreamEvent::SpeechEnd {
                        start_sample,
                        end_sample,
                    });
                }
            }
        }

        events
    }

    /// Flushes any pending speech span at the end of an audio stream.
    pub fn flush(&mut self) -> Option<VadStreamEvent> {
        if !self.triggered {
            return None;
        }

        let end_sample = self.temp_end_sample.unwrap_or(self.current_sample);
        let start_sample = self.speech_start_sample;

        self.reset();

        if end_sample.saturating_sub(start_sample) >= self.min_speech_samples {
            Some(VadStreamEvent::SpeechEnd {
                start_sample,
                end_sample,
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
            for event in self.accept_prob(prob, frame_samples) {
                if let VadStreamEvent::SpeechEnd {
                    start_sample,
                    end_sample,
                } = event
                {
                    spans.push(SpeechSpan {
                        start_sample,
                        end_sample,
                        sample_rate: self.sample_rate,
                    });
                }
            }
        }

        if let Some(VadStreamEvent::SpeechEnd {
            start_sample,
            end_sample,
        }) = self.flush()
        {
            spans.push(SpeechSpan {
                start_sample,
                end_sample,
                sample_rate: self.sample_rate,
            });
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
            sample_rate: 16_000,
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
        assert_eq!(sm.accept_prob(0.1, frame_samples), vec![]);
        assert!(!sm.is_speech_active());

        // Probe 0.40 (between neg_threshold and threshold) - should not trigger
        assert_eq!(sm.accept_prob(0.40, frame_samples), vec![]);
        assert!(!sm.is_speech_active());

        // Probe 0.60 (frame 1 of onset candidate)
        assert_eq!(sm.accept_prob(0.60, frame_samples), vec![]);
        assert!(!sm.is_speech_active());

        // Probe 0.65 (frame 2 of onset candidate -> triggers speech!)
        let events = sm.accept_prob(0.65, frame_samples);
        assert_eq!(
            events,
            vec![VadStreamEvent::SpeechStart {
                sample_offset: 1024
            }]
        );
        assert!(sm.is_speech_active());

        // Prob dips to 0.40 (above neg_threshold 0.35) -> speech remains active!
        assert_eq!(sm.accept_prob(0.40, frame_samples), vec![]);
        assert!(sm.is_speech_active());

        // Prob dips to 0.20 (below neg_threshold 0.35) - silence frame 1
        assert_eq!(sm.accept_prob(0.20, frame_samples), vec![]);
        assert!(sm.is_speech_active());

        // Silence frame 2
        assert_eq!(sm.accept_prob(0.20, frame_samples), vec![]);
        assert!(sm.is_speech_active());

        // Silence frame 3 (hits min_silence 96ms -> emits SpeechEnd!)
        let events = sm.accept_prob(0.20, frame_samples);
        assert_eq!(
            events,
            vec![VadStreamEvent::SpeechEnd {
                start_sample: 1024,
                end_sample: 2560,
            }]
        );
        assert!(!sm.is_speech_active());
    }

    #[test]
    fn noise_click_below_min_speech_is_ignored() {
        let frame_samples = 512;
        // min_speech = 100ms (~3 frames)
        let opts = test_options(0.50, 0.35, 0.1, 0.1, 30.0);
        let mut sm = UniversalVadStateMachine::new(&opts, 16_000);

        // One loud click frame (prob 0.9)
        assert_eq!(sm.accept_prob(0.9, frame_samples), vec![]);
        assert!(!sm.is_speech_active());

        // Followed by silence (prob 0.1) for > jitter_tolerance_frames (jitter is 1 frame @ 32ms)
        assert_eq!(sm.accept_prob(0.1, frame_samples), vec![]);
        assert_eq!(sm.accept_prob(0.1, frame_samples), vec![]);
        assert!(!sm.is_speech_active());

        assert_eq!(sm.flush(), None);
    }

    #[test]
    fn max_speech_duration_cuts_segment() {
        let frame_samples = 512; // 32ms
        // max_speech = 0.16s (5 frames), min_speech = 0.032s (1 frame)
        let opts = test_options(0.50, 0.35, 0.5, 0.03, 0.16);
        let mut sm = UniversalVadStateMachine::new(&opts, 16_000);

        // Frame 1 triggers SpeechStart
        let events = sm.accept_prob(0.8, frame_samples);
        assert_eq!(
            events,
            vec![VadStreamEvent::SpeechStart { sample_offset: 0 }]
        );

        // Frames 2..4: speech active
        for _ in 0..3 {
            assert_eq!(sm.accept_prob(0.8, frame_samples), vec![]);
        }

        // 5th frame reaches max speech duration
        let events = sm.accept_prob(0.8, frame_samples);
        assert_eq!(
            events,
            vec![
                VadStreamEvent::SpeechEnd {
                    start_sample: 0,
                    end_sample: 5 * 512,
                },
                VadStreamEvent::SpeechStart {
                    sample_offset: 5 * 512,
                },
            ]
        );
        assert!(sm.is_speech_active());
    }

    #[test]
    fn threshold_escalation_triggers_on_dips_after_threshold_duration() {
        let frame_samples = 1600; // 100ms @ 16kHz
        let mut opts = test_options(0.50, 0.35, 0.5, 0.1, 30.0);
        opts.threshold_escalation_duration = 0.5; // escalate after 500ms
        let mut sm = UniversalVadStateMachine::new(&opts, 16_000);

        // Frame 1: speech onset
        let events = sm.accept_prob(0.8, frame_samples);
        assert_eq!(
            events,
            vec![VadStreamEvent::SpeechStart { sample_offset: 0 }]
        );
        assert!(sm.is_speech_active());

        // 4 more frames of speech (0.5s total)
        for _ in 0..4 {
            assert_eq!(sm.accept_prob(0.8, frame_samples), vec![]);
        }
        assert!(sm.is_speech_active());

        // Now past 0.5s: threshold escalated to 0.90, neg_threshold escalated to 0.75
        // A dip to 0.60 (which is < 0.75) now counts as silence!
        for _ in 0..4 {
            assert_eq!(sm.accept_prob(0.60, frame_samples), vec![]);
        }
        // 5th frame of silence (500ms min_silence reached) -> emits SpeechEnd!
        let events = sm.accept_prob(0.60, frame_samples);
        assert_eq!(
            events,
            vec![VadStreamEvent::SpeechEnd {
                start_sample: 0,
                end_sample: 5 * 1600,
            }]
        );
        assert!(!sm.is_speech_active());
    }

    #[test]
    fn test_onset_jitter_tolerance() {
        let frame_samples = 512; // 32ms @ 16kHz
        // min_speech = 150ms (~5 frames of 512 = 160ms)
        let opts = test_options(0.50, 0.35, 0.5, 0.15, 30.0);
        let mut sm = UniversalVadStateMachine::new(&opts, 16_000);

        // Frame 1: prob 0.70 (candidate onset at 0)
        assert_eq!(sm.accept_prob(0.70, frame_samples), vec![]);
        assert!(!sm.is_speech_active());

        // Frame 2: 1 frame dip to 0.40 (in hysteresis zone: 0.35 <= 0.40 < 0.50)
        // Must NOT abort candidate onset!
        assert_eq!(sm.accept_prob(0.40, frame_samples), vec![]);
        assert!(!sm.is_speech_active());

        // Frame 3: prob 0.70 (elapsed = 3 * 512 = 1536 < 2400 samples)
        assert_eq!(sm.accept_prob(0.70, frame_samples), vec![]);
        assert!(!sm.is_speech_active());

        // Frame 4: prob 0.75 (elapsed = 4 * 512 = 2048 < 2400 samples)
        assert_eq!(sm.accept_prob(0.75, frame_samples), vec![]);
        assert!(!sm.is_speech_active());

        // Frame 5: prob 0.80 (elapsed = 5 * 512 = 2560 >= 2400 samples [150ms])
        // Triggers speech onset starting at sample_offset: 0 (not 1024!)
        let events = sm.accept_prob(0.80, frame_samples);
        assert_eq!(
            events,
            vec![VadStreamEvent::SpeechStart { sample_offset: 0 }]
        );
        assert!(sm.is_speech_active());
    }

    #[test]
    fn test_continuous_speech_max_duration_emits_both_end_and_start() {
        let frame_samples = 512;
        // max_speech = 0.16s (5 frames), min_speech = 0.03s (1 frame)
        let opts = test_options(0.50, 0.35, 0.5, 0.03, 0.16);
        let mut sm = UniversalVadStateMachine::new(&opts, 16_000);

        // Frame 1 triggers SpeechStart
        let events = sm.accept_prob(0.8, frame_samples);
        assert_eq!(
            events,
            vec![VadStreamEvent::SpeechStart { sample_offset: 0 }]
        );

        // Frames 2..4: speech active
        for _ in 0..3 {
            assert_eq!(sm.accept_prob(0.8, frame_samples), vec![]);
        }

        // Frame 5 hits max speech ceiling (5 * 512 = 2560)
        // Prob is 0.8 >= active_neg_threshold, so it must emit SpeechEnd and immediate SpeechStart
        let events = sm.accept_prob(0.8, frame_samples);
        assert_eq!(
            events,
            vec![
                VadStreamEvent::SpeechEnd {
                    start_sample: 0,
                    end_sample: 5 * 512,
                },
                VadStreamEvent::SpeechStart {
                    sample_offset: 5 * 512,
                },
            ]
        );
        assert!(sm.is_speech_active());
    }

    #[test]
    fn ambient_silence_aborts_candidate_onset_after_jitter_tolerance() {
        let frame_samples = 512;
        // min_speech = 64ms (2 frames)
        let opts = test_options(0.50, 0.35, 0.5, 0.064, 30.0);
        let mut sm = UniversalVadStateMachine::new(&opts, 16_000);

        // Frame 1: isolated spike 0.60 (candidate onset)
        assert_eq!(sm.accept_prob(0.60, frame_samples), vec![]);
        assert!(!sm.is_speech_active());

        // Frame 2: prob 0.10 (< neg_threshold 0.35) - jitter count = 1 (tolerated for 1 frame)
        assert_eq!(sm.accept_prob(0.10, frame_samples), vec![]);
        assert!(!sm.is_speech_active());

        // Frame 3: prob 0.10 (< neg_threshold 0.35) - jitter count = 2 > 1 -> candidate aborted!
        assert_eq!(sm.accept_prob(0.10, frame_samples), vec![]);
        assert!(!sm.is_speech_active());

        // Frame 4: single high prob frame 0.70. Since candidate was cleared, this is frame 1 of new candidate
        assert_eq!(sm.accept_prob(0.70, frame_samples), vec![]);
        assert!(!sm.is_speech_active());

        // Frame 5: second high prob frame -> NOW triggers speech, starting at frame 4 (index 3 * 512)
        let events = sm.accept_prob(0.70, frame_samples);
        assert_eq!(
            events,
            vec![VadStreamEvent::SpeechStart {
                sample_offset: 3 * 512
            }]
        );
        assert!(sm.is_speech_active());
        assert_eq!(sm.speech_start_sample, 3 * 512);
    }

    #[test]
    fn ambient_noise_in_hysteresis_band_aborts_candidate_onset_after_jitter_tolerance() {
        let frame_samples = 512;
        // min_speech = 64ms (2 frames)
        let opts = test_options(0.50, 0.35, 0.5, 0.064, 30.0);
        let mut sm = UniversalVadStateMachine::new(&opts, 16_000);

        // Frame 1: isolated spike 0.60 (candidate onset)
        assert_eq!(sm.accept_prob(0.60, frame_samples), vec![]);
        assert!(!sm.is_speech_active());

        // Frame 2: prob 0.40 (in hysteresis band) - jitter count = 1 <= 1
        assert_eq!(sm.accept_prob(0.40, frame_samples), vec![]);
        assert!(!sm.is_speech_active());

        // Frame 3: prob 0.40 (in hysteresis band) - jitter count = 2 > 1 -> candidate aborted!
        assert_eq!(sm.accept_prob(0.40, frame_samples), vec![]);
        assert!(!sm.is_speech_active());

        // Frame 4: single high prob frame 0.70. Since candidate was cleared, this is frame 1 of new candidate
        assert_eq!(sm.accept_prob(0.70, frame_samples), vec![]);
        assert!(!sm.is_speech_active());

        // Frame 5: second high prob frame -> NOW triggers speech, starting at frame 4 (index 3 * 512)
        let events = sm.accept_prob(0.70, frame_samples);
        assert_eq!(
            events,
            vec![VadStreamEvent::SpeechStart {
                sample_offset: 3 * 512
            }]
        );
        assert!(sm.is_speech_active());
        assert_eq!(sm.speech_start_sample, 3 * 512);
    }

    #[test]
    fn hysteresis_band_continuation_clears_pending_silence() {
        let frame_samples = 512;
        // min_silence = 96ms (3 frames)
        let opts = test_options(0.50, 0.35, 0.096, 0.03, 30.0);
        let mut sm = UniversalVadStateMachine::new(&opts, 16_000);

        // Trigger speech
        let events = sm.accept_prob(0.80, frame_samples);
        assert_eq!(
            events,
            vec![VadStreamEvent::SpeechStart { sample_offset: 0 }]
        );
        assert!(sm.is_speech_active());

        // 1 frame dip to 0.20 (< 0.35) -> sets temp_end_sample
        assert_eq!(sm.accept_prob(0.20, frame_samples), vec![]);
        assert!(sm.is_speech_active());
        assert!(sm.temp_end_sample.is_some());

        // Prob returns to 0.40 (hysteresis band: 0.35 <= 0.40 < 0.50) -> must CLEAR temp_end_sample!
        assert_eq!(sm.accept_prob(0.40, frame_samples), vec![]);
        assert!(sm.is_speech_active());
        assert!(sm.temp_end_sample.is_none());

        // 1 more frame dip to 0.20 -> new temp_end_sample at 3 * 512 = 1536
        assert_eq!(sm.accept_prob(0.20, frame_samples), vec![]);
        assert_eq!(sm.temp_end_sample, Some(3 * 512));
    }

    #[test]
    fn escalation_duration_zero_disables_escalation() {
        let frame_samples = 1600;
        let mut opts = test_options(0.50, 0.35, 0.5, 0.1, 30.0);
        opts.threshold_escalation_duration = 0.0; // disabled
        let mut sm = UniversalVadStateMachine::new(&opts, 16_000);

        // Onset
        let events = sm.accept_prob(0.80, frame_samples);
        assert_eq!(
            events,
            vec![VadStreamEvent::SpeechStart { sample_offset: 0 }]
        );

        // Since escalation is disabled, prob of 0.60 should NOT be considered silence (0.60 > 0.35)
        for _ in 0..10 {
            assert_eq!(sm.accept_prob(0.60, frame_samples), vec![]);
            assert!(sm.is_speech_active());
        }
    }

    #[test]
    fn nan_probabilities_are_sanitized() {
        let frame_samples = 512;
        let opts = test_options(0.50, 0.35, 0.096, 0.03, 30.0);
        let mut sm = UniversalVadStateMachine::new(&opts, 16_000);

        // NaN should be treated as 0.0, not triggering speech
        assert_eq!(sm.accept_prob(f32::NAN, frame_samples), vec![]);
        assert!(!sm.is_speech_active());
    }
}
