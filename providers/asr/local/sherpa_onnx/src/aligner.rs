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
        if let Some(tp) = tokens_path.as_ref() {
            config.model_config.tokens = Some(tp.to_string_lossy().to_string());
        }
        config.model_config.num_threads = if num_threads > 0 { num_threads } else { 1 };
        config.model_config.debug = false;
        config.model_config.provider = Some("cpu".to_string());

        let dictionary = tokens_path.as_ref().and_then(|tp| {
            std::fs::read_to_string(tp)
                .ok()
                .map(|content| MmsDictionary::from_lines(content.lines()))
        });

        let model_str = onnx_path.to_string_lossy().to_string();
        let recognizer = {
            let mut omni_config = config.clone();
            omni_config.model_config.omnilingual.model = Some(model_str.clone());
            if let Some(rec) = OfflineRecognizer::create(&omni_config) {
                Some(rec)
            } else {
                let mut nemo_config = config;
                nemo_config.model_config.nemo_ctc.model = Some(model_str);
                OfflineRecognizer::create(&nemo_config)
            }
        }
        .ok_or_else(|| {
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
                        let slice_duration_sec = slice.len() as f64 / sample_rate as f64;
                        let units = project_tokens_to_timing_units(
                            &segment.text,
                            &res.tokens,
                            &timestamps,
                            slice_start_sec,
                            slice_duration_sec,
                            segment.start,
                        );
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
pub fn project_tokens_to_timing_units(
    text: &str,
    tokens: &[String],
    timestamps: &[f32],
    slice_start_sec: f64,
    slice_duration_sec: f64,
    segment_start: f64,
) -> Vec<TranscriptTimingUnit> {
    let slice_end_sec = slice_start_sec + slice_duration_sec;
    let aligned_text_units =
        sona_core::transcription::text_alignment::align_text_units_to_tokens(text, tokens);

    if let Some(text_units) = aligned_text_units {
        text_units
            .into_iter()
            .filter(|u| !u.text.trim().is_empty())
            .map(|u| {
                let rel_start = timestamps.get(u.token_index).copied().unwrap_or(0.0) as f64;
                let token_end = u.token_end_exclusive.max(u.token_index + 1);
                let rel_end = if token_end < timestamps.len() {
                    (timestamps[token_end] as f64).max(rel_start)
                } else {
                    slice_duration_sec.max(rel_start)
                };
                let start = (slice_start_sec + rel_start).max(segment_start);
                let end = (slice_start_sec + rel_end).min(slice_end_sec).max(start);
                TranscriptTimingUnit {
                    text: u.text,
                    start,
                    end,
                }
            })
            .collect()
    } else {
        tokens
            .iter()
            .enumerate()
            .map(|(i, tok)| {
                let rel_start = timestamps.get(i).copied().unwrap_or(0.0) as f64;
                let rel_end = if i + 1 < timestamps.len() {
                    (timestamps[i + 1] as f64).max(rel_start)
                } else {
                    slice_duration_sec.max(rel_start)
                };
                let start = (slice_start_sec + rel_start).max(segment_start);
                let end = (slice_start_sec + rel_end).min(slice_end_sec).max(start);
                TranscriptTimingUnit {
                    text: tok.clone(),
                    start,
                    end,
                }
            })
            .collect()
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

    #[test]
    fn test_project_tokens_to_timing_units_multi_token_words() {
        // Multi-token character tokens: "hello" = h, e, l, l, o; "world" = w, o, r, l, d
        let tokens = vec![
            "h".to_string(),
            "e".to_string(),
            "l".to_string(),
            "l".to_string(),
            "o".to_string(),
            " ".to_string(),
            "w".to_string(),
            "o".to_string(),
            "r".to_string(),
            "l".to_string(),
            "d".to_string(),
        ];
        let timestamps = vec![
            0.10, 0.15, 0.20, 0.25, 0.30, 0.35, 0.40, 0.45, 0.50, 0.55, 0.60,
        ];
        let slice_start = 1.0;
        let slice_duration = 0.70;
        let segment_start = 1.0;

        let units = project_tokens_to_timing_units(
            "hello world",
            &tokens,
            &timestamps,
            slice_start,
            slice_duration,
            segment_start,
        );

        assert_eq!(units.len(), 2);
        assert_eq!(units[0].text, "hello");
        // "hello" should span from 'h' (0.10s) up to ' ' (0.35s), with offset 1.0 -> [1.10, 1.35]
        assert!((units[0].start - 1.10).abs() < 1e-4);
        assert!((units[0].end - 1.35).abs() < 1e-4);

        assert_eq!(units[1].text, "world");
        // "world" should span from 'w' (0.40s) up to end of slice (0.70s), with offset 1.0 -> [1.40, 1.70]
        assert!((units[1].start - 1.40).abs() < 1e-4);
        assert!((units[1].end - 1.70).abs() < 1e-4);
    }
}
