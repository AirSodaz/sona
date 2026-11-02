use crate::ports::asr::BatchSegmentationMode;
use crate::ports::vad::{SpeechSpan, VadDetectionOptions, VadEnginePort};

/// Mono PCM sample rate used by local batch segmentation.
pub const BATCH_SEGMENTATION_SAMPLE_RATE: u32 = 16_000;
const FIXED_CHUNK_DURATION_SECONDS: f32 = 30.0;

#[derive(Debug, Clone, PartialEq)]
pub struct AudioSegment {
    pub samples: Vec<f32>,
    pub start_time: f32,
    pub duration: f32,
}

impl AudioSegment {
    pub fn end_time(&self) -> f32 {
        self.start_time + self.duration
    }
}

pub fn whole_audio_segment(samples: &[f32], sample_rate: u32) -> Vec<AudioSegment> {
    if samples.is_empty() {
        return Vec::new();
    }

    vec![AudioSegment {
        samples: samples.to_vec(),
        start_time: 0.0,
        duration: samples.len() as f32 / sample_rate.max(1) as f32,
    }]
}

pub fn fixed_chunk_audio(
    samples: &[f32],
    sample_rate: u32,
    chunk_duration: f32,
) -> Vec<AudioSegment> {
    let chunk_size = (sample_rate as f32 * chunk_duration) as usize;
    if chunk_size == 0 {
        return Vec::new();
    }

    samples
        .chunks(chunk_size)
        .enumerate()
        .map(|(index, chunk)| {
            let start_sample = index * chunk_size;
            AudioSegment {
                samples: chunk.to_vec(),
                start_time: start_sample as f32 / sample_rate.max(1) as f32,
                duration: chunk.len() as f32 / sample_rate.max(1) as f32,
            }
        })
        .collect()
}

/// Splits batch audio according to the requested segmentation mode.
///
/// `Whole` returns a single full-length segment. `Vad` runs the resolved VAD
/// engine when present and otherwise falls back to fixed chunks; an engine
/// failure also falls back to fixed chunks while an empty detection result is
/// preserved (silent audio yields no segments).
pub fn segment_batch_audio(
    samples: &[f32],
    sample_rate: u32,
    batch_segmentation_mode: BatchSegmentationMode,
    vad_engine: Option<&dyn VadEnginePort>,
    vad_options: &VadDetectionOptions,
) -> Vec<AudioSegment> {
    match batch_segmentation_mode {
        BatchSegmentationMode::Whole => whole_audio_segment(samples, sample_rate),
        BatchSegmentationMode::Vad => {
            match vad_engine
                .and_then(|engine| engine.detect(samples, sample_rate, vad_options).ok())
            {
                Some(spans) => {
                    let max_raw_seconds = (vad_options.max_speech_duration
                        - 2.0 * vad_options.speech_pad_seconds)
                        .max(1.0);
                    let split_spans =
                        split_long_spans(samples, sample_rate, spans, max_raw_seconds);
                    pad_and_clamp_spans(
                        samples,
                        split_spans,
                        sample_rate,
                        vad_options.speech_pad_seconds,
                    )
                }
                None => fixed_chunk_audio(samples, sample_rate, FIXED_CHUNK_DURATION_SECONDS),
            }
        }
    }
}

/// Recursively splits speech spans exceeding `max_span_seconds` at an RMS-detected
/// micro-pause dip, with a fallback to hard-cut if audio energy is uniform.
pub fn split_long_spans(
    samples: &[f32],
    sample_rate: u32,
    spans: Vec<SpeechSpan>,
    max_span_seconds: f32,
) -> Vec<SpeechSpan> {
    if sample_rate == 0 || max_span_seconds <= 0.0 {
        return spans;
    }

    let mut result = Vec::new();
    for span in spans {
        if span.duration() <= max_span_seconds {
            result.push(span);
            continue;
        }

        let search_end_sec = max_span_seconds.min(span.duration());
        let search_start_sec = (search_end_sec - 10.0).max(0.0);

        let search_start_sample =
            span.start_sample + (search_start_sec * sample_rate as f32).round() as usize;
        let search_end_sample =
            span.start_sample + (search_end_sec * sample_rate as f32).round() as usize;
        let search_end_sample = search_end_sample.min(span.end_sample).min(samples.len());

        let window_size = ((sample_rate as f32) * 0.05).round() as usize;
        let window_size = window_size.max(1);
        let step = window_size;

        let mut candidate_windows = Vec::new();
        let mut curr = search_start_sample;
        while curr + window_size <= search_end_sample {
            let slice = &samples[curr..curr + window_size];
            let sum_sq: f64 = slice.iter().map(|&x| (x as f64) * (x as f64)).sum();
            let rms = (sum_sq / window_size as f64).sqrt() as f32;
            let center = curr + window_size / 2;
            candidate_windows.push((rms, center));
            curr += step;
        }

        let hard_cut = span.start_sample + (max_span_seconds * sample_rate as f32).round() as usize;

        let split_sample = if candidate_windows.is_empty() {
            hard_cut
        } else {
            let avg_rms: f32 = candidate_windows.iter().map(|(r, _)| *r).sum::<f32>()
                / (candidate_windows.len() as f32);
            let mut min_rms = f32::MAX;
            let mut best_center = hard_cut;
            for &(rms, center) in &candidate_windows {
                if rms < min_rms {
                    min_rms = rms;
                    best_center = center;
                }
            }

            if avg_rms == 0.0 || min_rms >= avg_rms * 0.95 {
                hard_cut
            } else {
                best_center
            }
        };

        let split_sample = split_sample
            .max(span.start_sample + 1)
            .min(span.end_sample.saturating_sub(1).max(span.start_sample + 1));

        let sub_span1 = SpeechSpan {
            start_sample: span.start_sample,
            end_sample: split_sample,
            sample_rate: span.sample_rate,
        };
        let sub_span2 = SpeechSpan {
            start_sample: split_sample,
            end_sample: span.end_sample,
            sample_rate: span.sample_rate,
        };

        result.push(sub_span1);
        result.extend(split_long_spans(
            samples,
            sample_rate,
            vec![sub_span2],
            max_span_seconds,
        ));
    }
    result
}

/// Applies symmetric speech padding to spans while splitting overlapping inter-span
/// gaps evenly at the midpoint to prevent double-transcription.
pub fn pad_and_clamp_spans(
    samples: &[f32],
    spans: Vec<SpeechSpan>,
    sample_rate: u32,
    pad_seconds: f32,
) -> Vec<AudioSegment> {
    if spans.is_empty() || sample_rate == 0 {
        return Vec::new();
    }

    let total_samples = samples.len();
    let pad_samples = (pad_seconds * sample_rate as f32).round() as usize;

    let mut starts: Vec<usize> = spans
        .iter()
        .map(|s| s.start_sample.saturating_sub(pad_samples))
        .collect();
    let mut ends: Vec<usize> = spans
        .iter()
        .map(|s| s.end_sample.saturating_add(pad_samples).min(total_samples))
        .collect();

    for i in 0..spans.len().saturating_sub(1) {
        if ends[i] > starts[i + 1] {
            let gap = spans[i + 1]
                .start_sample
                .saturating_sub(spans[i].end_sample);
            let mid = spans[i].end_sample + gap / 2;
            ends[i] = mid.min(total_samples);
            starts[i + 1] = mid.min(total_samples);
        }
    }

    spans
        .iter()
        .enumerate()
        .filter_map(|(i, _)| {
            let start = starts[i];
            let end = ends[i].max(start);
            if end == start {
                return None;
            }
            let seg_slice = if end <= total_samples {
                &samples[start..end]
            } else if start < total_samples {
                &samples[start..total_samples]
            } else {
                &[]
            };
            let duration = (end - start) as f32 / sample_rate as f32;
            let start_time = start as f32 / sample_rate as f32;
            Some(AudioSegment {
                samples: seg_slice.to_vec(),
                start_time,
                duration,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        AudioSegment, BATCH_SEGMENTATION_SAMPLE_RATE, fixed_chunk_audio, pad_and_clamp_spans,
        segment_batch_audio, split_long_spans, whole_audio_segment,
    };
    use crate::ports::asr::{AsrPortError, BatchSegmentationMode};
    use crate::ports::vad::{SpeechSpan, VadDetectionOptions, VadEngineKind, VadEnginePort};
    use std::path::Path;

    struct FakeVad {
        result: Result<Vec<SpeechSpan>, ()>,
    }

    impl FakeVad {
        fn ok(spans: Vec<SpeechSpan>) -> Self {
            Self { result: Ok(spans) }
        }

        fn failing() -> Self {
            Self { result: Err(()) }
        }
    }

    impl VadEnginePort for FakeVad {
        fn engine_kind(&self) -> VadEngineKind {
            VadEngineKind::SileroOnnx
        }

        fn can_handle(&self, _model_path: &Path) -> bool {
            true
        }

        fn detect(
            &self,
            _samples: &[f32],
            sample_rate: u32,
            _options: &VadDetectionOptions,
        ) -> Result<Vec<SpeechSpan>, AsrPortError> {
            self.result
                .clone()
                .map_err(|()| AsrPortError::runtime("fake vad failure"))
                .map(|spans| {
                    spans
                        .into_iter()
                        .map(|span| SpeechSpan {
                            sample_rate,
                            ..span
                        })
                        .collect()
                })
        }
    }

    fn vad_options() -> VadDetectionOptions {
        VadDetectionOptions::batch_defaults("unused")
    }

    #[test]
    fn batch_whole_segmentation_uses_one_full_audio_segment() {
        let samples = vec![0.0; 16_000 * 65];
        let segments = segment_batch_audio(
            &samples,
            16_000,
            BatchSegmentationMode::Whole,
            None,
            &vad_options(),
        );

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].start_time, 0.0);
        assert_eq!(segments[0].duration, 65.0);
        assert_eq!(segments[0].samples.len(), samples.len());
    }

    #[test]
    fn batch_vad_segmentation_falls_back_to_fixed_chunks_without_engine() {
        let samples = vec![0.0; 16_000 * 65];
        let segments = segment_batch_audio(
            &samples,
            16_000,
            BatchSegmentationMode::Vad,
            None,
            &vad_options(),
        );

        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].start_time, 0.0);
        assert_eq!(segments[0].duration, 30.0);
        assert_eq!(segments[1].start_time, 30.0);
        assert_eq!(segments[1].duration, 30.0);
        assert_eq!(segments[2].start_time, 60.0);
        assert_eq!(segments[2].duration, 5.0);
    }

    #[test]
    fn batch_vad_segmentation_falls_back_to_fixed_chunks_on_engine_failure() {
        let samples = vec![0.0; 16_000 * 65];
        let engine = FakeVad::failing();
        let segments = segment_batch_audio(
            &samples,
            16_000,
            BatchSegmentationMode::Vad,
            Some(&engine),
            &vad_options(),
        );

        assert_eq!(segments.len(), 3);
        assert_eq!(segments[2].duration, 5.0);
    }

    #[test]
    fn batch_vad_segmentation_slices_samples_at_detected_spans() {
        let samples: Vec<f32> = (0..16_000 * 10).map(|s| s as f32).collect();
        let engine = FakeVad::ok(vec![
            SpeechSpan {
                start_sample: 16_000,
                end_sample: 48_000,
                sample_rate: 16_000,
            },
            SpeechSpan {
                start_sample: 96_000,
                end_sample: 112_000,
                sample_rate: 16_000,
            },
        ]);
        let segments = segment_batch_audio(
            &samples,
            16_000,
            BatchSegmentationMode::Vad,
            Some(&engine),
            &vad_options(),
        );

        assert_eq!(segments.len(), 2);
        // Span 0: 16000 - 4000 = 12000 (0.75s), 48000 + 4000 = 52000 (3.25s), duration = 2.5s
        assert_eq!(segments[0].start_time, 0.75);
        assert_eq!(segments[0].duration, 2.5);
        assert_eq!(segments[0].samples[0], 12_000.0);
        assert_eq!(*segments[0].samples.last().unwrap(), 51_999.0);
        // Span 1: 96000 - 4000 = 92000 (5.75s), 112000 + 4000 = 116000 (7.25s), duration = 1.5s
        assert_eq!(segments[1].start_time, 5.75);
        assert_eq!(segments[1].duration, 1.5);
        assert_eq!(segments[1].samples[0], 92_000.0);
        assert_eq!(*segments[1].samples.last().unwrap(), 115_999.0);
    }

    #[test]
    fn pad_and_clamp_spans_expands_isolated_span() {
        let sample_rate = 16_000;
        let samples = vec![1.0; sample_rate as usize * 4];
        let span = SpeechSpan {
            start_sample: 16_000,
            end_sample: 32_000,
            sample_rate,
        };
        let segments = pad_and_clamp_spans(&samples, vec![span], sample_rate, 0.25);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].start_time, 0.75);
        assert_eq!(segments[0].duration, 1.5);
        assert_eq!(segments[0].samples.len(), 24_000);

        let edge_span = SpeechSpan {
            start_sample: 1_000,
            end_sample: 8_000,
            sample_rate,
        };
        let edge_segments = pad_and_clamp_spans(&samples, vec![edge_span], sample_rate, 0.25);
        assert_eq!(edge_segments.len(), 1);
        assert_eq!(edge_segments[0].start_time, 0.0);
        assert_eq!(edge_segments[0].samples.len(), 12_000);
    }

    #[test]
    fn pad_and_clamp_spans_halves_close_spans_without_overlap() {
        let sample_rate = 16_000;
        let total_samples = sample_rate as usize * 10;
        let samples: Vec<f32> = (0..total_samples).map(|i| i as f32).collect();

        let spans = vec![
            SpeechSpan {
                start_sample: 16_000,
                end_sample: 32_000,
                sample_rate,
            },
            SpeechSpan {
                start_sample: 33_600,
                end_sample: 48_000,
                sample_rate,
            },
        ];

        let segments = pad_and_clamp_spans(&samples, spans, sample_rate, 0.25);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].start_time, 0.75);
        assert_eq!(segments[0].duration, (32_800 - 12_000) as f32 / 16_000.0);
        assert_eq!(segments[0].samples.len(), 32_800 - 12_000);

        assert_eq!(segments[1].start_time, 2.05);
        assert_eq!(segments[1].duration, (52_000 - 32_800) as f32 / 16_000.0);
        assert_eq!(segments[1].samples.len(), 52_000 - 32_800);

        assert_eq!(segments[0].end_time(), segments[1].start_time);
    }

    #[test]
    fn split_long_spans_breaks_at_rms_minimum() {
        let sample_rate = 16_000;
        let total_samples = sample_rate as usize * 35;
        let mut samples = vec![0.8f32; total_samples];

        // Micro-pause dip at 25s
        let dip_start = sample_rate as usize * 25;
        let dip_end = dip_start + 6_400; // 400ms
        for s in &mut samples[dip_start..dip_end] {
            *s = 0.0;
        }

        let long_span = SpeechSpan {
            start_sample: 0,
            end_sample: total_samples,
            sample_rate,
        };

        let split_spans = split_long_spans(&samples, sample_rate, vec![long_span], 29.5);
        assert_eq!(split_spans.len(), 2);
        assert!(split_spans[0].duration() <= 29.5);
        assert!(split_spans[1].duration() <= 29.5);

        let split_sec = split_spans[0].end_time();
        assert!(
            (24.8..=25.4).contains(&split_sec),
            "expected split around 25.0s dip, got {split_sec}"
        );
        assert_eq!(split_spans[0].end_sample, split_spans[1].start_sample);
    }

    #[test]
    fn batch_vad_segmentation_keeps_empty_detection_results() {
        let samples = vec![0.0; 16_000 * 65];
        let engine = FakeVad::ok(Vec::new());
        let segments = segment_batch_audio(
            &samples,
            16_000,
            BatchSegmentationMode::Vad,
            Some(&engine),
            &vad_options(),
        );

        assert!(segments.is_empty());
    }

    #[test]
    fn fixed_chunk_audio_handles_empty_input_and_zero_chunk_size() {
        assert!(fixed_chunk_audio(&[], 16_000, 30.0).is_empty());
        assert!(fixed_chunk_audio(&[0.0], 0, 30.0).is_empty());
    }

    #[test]
    fn whole_audio_segment_handles_empty_input() {
        assert!(whole_audio_segment(&[], 16_000).is_empty());
    }

    #[test]
    fn audio_segment_end_time_adds_duration_to_start() {
        let segment = AudioSegment {
            samples: vec![0.0; 16_000],
            start_time: 2.5,
            duration: 1.0,
        };

        assert_eq!(segment.end_time(), 3.5);
    }

    #[test]
    fn batch_sample_rate_is_the_historical_16khz() {
        assert_eq!(BATCH_SEGMENTATION_SAMPLE_RATE, 16_000);
    }
}
