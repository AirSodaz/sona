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
                Some(spans) => audio_segments_from_spans(samples, sample_rate, spans),
                None => fixed_chunk_audio(samples, sample_rate, FIXED_CHUNK_DURATION_SECONDS),
            }
        }
    }
}

fn audio_segments_from_spans(
    samples: &[f32],
    _sample_rate: u32,
    spans: Vec<SpeechSpan>,
) -> Vec<AudioSegment> {
    spans
        .into_iter()
        .map(|span| AudioSegment {
            samples: span.slice_samples(samples).to_vec(),
            start_time: span.start_time(),
            duration: span.duration(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        AudioSegment, BATCH_SEGMENTATION_SAMPLE_RATE, fixed_chunk_audio, segment_batch_audio,
        whole_audio_segment,
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
        assert_eq!(segments[0].start_time, 1.0);
        assert_eq!(segments[0].duration, 2.0);
        assert_eq!(segments[0].samples[0], 16_000.0);
        assert_eq!(*segments[0].samples.last().unwrap(), 47_999.0);
        assert_eq!(segments[1].start_time, 6.0);
        assert_eq!(segments[1].duration, 1.0);
        assert_eq!(segments[1].samples[0], 96_000.0);
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
