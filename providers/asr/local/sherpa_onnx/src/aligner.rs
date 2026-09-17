use async_trait::async_trait;
use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig};
use sona_core::ports::aligner::{
    AlignerEngineKind, AlignerEnginePort, AlignerPortError, AlignerPortErrorKind,
    SegmentAlignerPort,
};
use sona_core::transcription::forced_alignment::{
    MmsDictionary, apply_alignment_to_transcript_segment,
    apply_fallback_timing_to_transcript_segment, slice_audio_with_padding,
};
use sona_core::transcription::transcript::{TranscriptSegment, TranscriptTimingUnit};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::audio::resolve_model_onnx_path;

const DEFAULT_PADDING_SEC: f64 = 0.200;

enum AlignerInner {
    Recognizer {
        recognizer: OfflineRecognizer,
        #[allow(dead_code)]
        dictionary: Option<MmsDictionary>,
    },
    #[cfg(test)]
    TestDummy,
}

pub struct SherpaCtcAligner {
    inner: AlignerInner,
}

unsafe impl Send for SherpaCtcAligner {}
unsafe impl Sync for SherpaCtcAligner {}

impl SherpaCtcAligner {
    #[cfg(test)]
    pub fn test_dummy() -> Self {
        Self {
            inner: AlignerInner::TestDummy,
        }
    }

    pub fn new(model_path: &Path, num_threads: i32) -> Result<Self, AlignerPortError> {
        let onnx_path = resolve_model_onnx_path(model_path)
            .map_err(|err| AlignerPortError::model_not_found(err.to_string()))?;
        let tokens_path = resolve_model_tokens_path(model_path);

        let mut config = OfflineRecognizerConfig::default();
        config.model_config.nemo_ctc.model = Some(onnx_path.to_string_lossy().to_string());
        if let Some(tp) = tokens_path.as_ref() {
            config.model_config.tokens = Some(tp.to_string_lossy().to_string());
        }
        config.model_config.num_threads = if num_threads > 0 { num_threads } else { 1 };
        config.model_config.debug = false;
        config.model_config.provider = Some("cpu".to_string());

        let dictionary = tokens_path.and_then(|tp| {
            std::fs::read_to_string(&tp)
                .ok()
                .map(|content| MmsDictionary::from_lines(content.lines()))
        });

        let recognizer = OfflineRecognizer::create(&config).ok_or_else(|| {
            AlignerPortError::new(
                AlignerPortErrorKind::ModelNotFound,
                format!(
                    "Failed to create OfflineRecognizer for aligner at {}",
                    onnx_path.display()
                ),
            )
        })?;

        Ok(Self {
            inner: AlignerInner::Recognizer {
                recognizer,
                dictionary,
            },
        })
    }
}

pub fn resolve_model_tokens_path(path: &Path) -> Option<PathBuf> {
    let dir = if path.is_file() { path.parent()? } else { path };
    for candidate in ["tokens.txt", "vocab.txt", "dict.txt"] {
        let candidate_path = dir.join(candidate);
        if candidate_path.exists() {
            return Some(candidate_path);
        }
    }
    None
}

#[async_trait]
impl SegmentAlignerPort for SherpaCtcAligner {
    async fn align_segments(
        &self,
        audio_samples: &[f32],
        sample_rate: u32,
        segments: &[TranscriptSegment],
    ) -> Result<Vec<TranscriptSegment>, AlignerPortError> {
        if segments.is_empty() {
            return Ok(Vec::new());
        }

        let mut aligned_segments = segments.to_vec();

        match &self.inner {
            #[cfg(test)]
            AlignerInner::TestDummy => {
                for segment in &mut aligned_segments {
                    apply_fallback_timing_to_transcript_segment(segment);
                }
                return Ok(aligned_segments);
            }
            AlignerInner::Recognizer { recognizer, .. } => {
                for segment in &mut aligned_segments {
                    if segment.text.trim().is_empty() {
                        apply_fallback_timing_to_transcript_segment(segment);
                        continue;
                    }

                    // 1. Slice audio around segment with 200ms padding
                    let (slice, slice_start_sec) = slice_audio_with_padding(
                        audio_samples,
                        sample_rate,
                        segment.start,
                        segment.end,
                        DEFAULT_PADDING_SEC,
                    );

                    if slice.is_empty() {
                        apply_fallback_timing_to_transcript_segment(segment);
                        continue;
                    }

                    // 2. Run CTC decoding on the sliced audio
                    let stream = recognizer.create_stream();
                    stream.accept_waveform(sample_rate as i32, &slice);
                    recognizer.decode(&stream);

                    let mut applied = false;
                    if let Some(res) = stream.get_result()
                        && let Some(timestamps) = res
                            .timestamps
                            .filter(|t| !t.is_empty() && t.len() == res.tokens.len())
                    {
                        let slice_end_sec =
                            slice_start_sec + (slice.len() as f64 / sample_rate as f64);
                        let units: Vec<TranscriptTimingUnit> = res
                            .tokens
                            .iter()
                            .enumerate()
                            .map(|(i, tok)| {
                                let rel_start = timestamps[i] as f64;
                                let rel_end = if i + 1 < timestamps.len() {
                                    (timestamps[i + 1] as f64).max(rel_start)
                                } else {
                                    (slice.len() as f64 / sample_rate as f64).max(rel_start)
                                };
                                let start = (slice_start_sec + rel_start).max(segment.start);
                                let end = (slice_start_sec + rel_end).min(slice_end_sec).max(start);
                                TranscriptTimingUnit {
                                    text: tok.clone(),
                                    start,
                                    end,
                                }
                            })
                            .collect();

                        if !units.is_empty() {
                            apply_alignment_to_transcript_segment(segment, units);
                            applied = true;
                        }
                    }

                    if !applied {
                        // Fallback if CTC decoding returned no timestamps or alignment failed
                        apply_fallback_timing_to_transcript_segment(segment);
                    }
                }
            }
        }

        Ok(aligned_segments)
    }
}

/// CTC aligner engine factory backed by sherpa-onnx.
#[derive(Debug, Clone, Copy, Default)]
pub struct SherpaCtcAlignerEngine;

impl AlignerEnginePort for SherpaCtcAlignerEngine {
    fn engine_kind(&self) -> AlignerEngineKind {
        AlignerEngineKind::CtcTrellisOnnx
    }

    fn can_handle(&self, model_path: &Path) -> bool {
        resolve_model_onnx_path(model_path).is_ok()
    }

    fn load(
        &self,
        model_path: &Path,
        num_threads: i32,
    ) -> Result<Arc<dyn SegmentAlignerPort>, AlignerPortError> {
        let aligner = SherpaCtcAligner::new(model_path, num_threads)?;
        Ok(Arc::new(aligner))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dummy_aligner_populates_token_timing() {
        let aligner = SherpaCtcAligner::test_dummy();
        let segments = vec![TranscriptSegment {
            id: "s1".to_string(),
            text: "hello world".to_string(),
            start: 1.0,
            end: 2.0,
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            speaker: None,
            speaker_attribution: None,
        }];

        let aligned = aligner
            .align_segments(&[0.0; 16000], 16000, &segments)
            .await
            .expect("should align");

        assert_eq!(aligned.len(), 1);
        let timing = aligned[0].timing.as_ref().expect("timing populated");
        assert_eq!(
            timing.level,
            sona_core::transcription::transcript::TranscriptTimingLevel::Token
        );
        assert_eq!(timing.units.len(), 2);
        assert_eq!(timing.units[0].text, "hello");
        assert_eq!(timing.units[1].text, "world");
    }

    #[test]
    fn test_resolve_model_tokens_path() {
        let temp_dir = std::env::temp_dir().join("tokens_path_test");
        std::fs::create_dir_all(&temp_dir).unwrap();

        assert!(resolve_model_tokens_path(&temp_dir).is_none());

        let vocab_file = temp_dir.join("vocab.txt");
        std::fs::write(&vocab_file, "a 1\nb 2\n").unwrap();

        let resolved = resolve_model_tokens_path(&temp_dir).expect("should resolve vocab.txt");
        assert_eq!(resolved, vocab_file);

        std::fs::remove_dir_all(&temp_dir).unwrap();
    }
}
