pub mod fsmn;
pub mod silero;

use sona_core::ports::asr::AsrPortError;

/// Trait implemented by VAD neural network model runners (Silero, FSMN).
pub trait VadModelRunner: Send {
    /// Window size in PCM audio samples expected by [`step`].
    /// For Silero this is typically 512 samples (32ms @ 16kHz).
    /// For FSMN this is typically 160 samples (10ms hop @ 16kHz).
    fn window_size(&self) -> usize;

    /// Evaluates one audio chunk in a streaming session, advancing internal recurrent state
    /// and returning the speech probability scalar for the chunk.
    fn step(&mut self, chunk: &[f32]) -> Result<f32, AsrPortError>;

    /// Evaluates an entire audio slice in batch, returning a vector of speech probabilities
    /// corresponding to each window.
    fn predict_probs_batch(&mut self, samples: &[f32]) -> Result<Vec<f32>, AsrPortError>;

    /// Clears internal recurrent/cache states back to zeros for a new stream.
    fn reset_state(&mut self);
}
