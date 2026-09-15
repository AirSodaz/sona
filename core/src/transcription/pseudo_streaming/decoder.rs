use crate::ports::asr::AsrPortError;

/// Inference stage for pseudo-streaming transcription passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeStage {
    /// In-progress partial hypothesis during an active speech utterance.
    Partial,
    /// Final hypothesis at the boundary of a detected speech utterance or stream flush.
    Final,
}

impl DecodeStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Partial => "partial",
            Self::Final => "final",
        }
    }

    pub fn is_final(self) -> bool {
        matches!(self, Self::Final)
    }
}

/// Raw recognition output returned by a [`PseudoStreamDecoder`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PseudoStreamDecodeResult {
    /// The decoded raw text hypothesis.
    pub text: String,
    /// Optional decoded token sequence.
    pub tokens: Option<Vec<String>>,
    /// Optional relative timestamp offsets in seconds for tokens/words.
    pub timestamps: Option<Vec<f32>>,
}

/// Engine-neutral decoder trait for pseudo-streaming ASR.
///
/// Any offline/batch ASR backend (e.g. llama.cpp, Sherpa-onnx offline recognizers,
/// Whisper, Candle, MLX, etc.) implements this trait to plug into
/// [`super::session::PseudoStreamingSession`].
pub trait PseudoStreamDecoder: Send + Sync + 'static {
    /// Decodes mono 16 kHz PCM `audio_samples` for the given `stage`.
    ///
    /// Implementations may return `Ok(None)` or an empty string to indicate
    /// no audible speech could be recognized from the chunk.
    fn decode(
        &self,
        audio_samples: &[f32],
        stage: DecodeStage,
    ) -> Result<Option<PseudoStreamDecodeResult>, AsrPortError>;
}
