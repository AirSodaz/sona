use crate::runners::VadModelRunner;
use crate::state_machine::UniversalVadStateMachine;
use sona_core::ports::vad::{StreamingVadPort, VadDetectionOptions};

/// Native ONNX streaming VAD adapter implementing [`StreamingVadPort`].
///
/// Buffers incoming audio samples, executes incremental recurrent inference on complete
/// windows via [`VadModelRunner::step`], and feeds probabilities into [`UniversalVadStateMachine`].
pub struct NativeStreamingVad {
    runner: Box<dyn VadModelRunner>,
    state_machine: UniversalVadStateMachine,
    buffer: Vec<f32>,
    window_size: usize,
}

impl NativeStreamingVad {
    pub fn new(
        runner: Box<dyn VadModelRunner>,
        options: &VadDetectionOptions,
        sample_rate: u32,
    ) -> Self {
        let window_size = runner.window_size();
        let state_machine = UniversalVadStateMachine::new(options, sample_rate);
        Self {
            runner,
            state_machine,
            buffer: Vec::with_capacity(window_size * 2),
            window_size,
        }
    }
}
impl StreamingVadPort for NativeStreamingVad {
    fn accept_samples(&mut self, samples: &[f32]) {
        self.buffer.extend_from_slice(samples);

        while self.buffer.len() >= self.window_size {
            let chunk = &self.buffer[..self.window_size];
            match self.runner.step(chunk) {
                Ok(prob) => {
                    self.state_machine.accept_prob(prob, self.window_size);
                }
                Err(err) => {
                    log::warn!("VAD streaming step inference failed: {err}");
                }
            }
            self.buffer.drain(..self.window_size);
        }
    }

    fn is_speech_detected(&self) -> bool {
        self.state_machine.is_speech_active()
    }

    fn reset(&mut self) {
        self.buffer.clear();
        self.runner.reset_state();
        self.state_machine.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sona_core::ports::asr::AsrPortError;

    struct MockRunner {
        window_size: usize,
        prob_sequence: Vec<f32>,
        idx: usize,
    }

    impl VadModelRunner for MockRunner {
        fn window_size(&self) -> usize {
            self.window_size
        }
        fn step(&mut self, _chunk: &[f32]) -> Result<f32, AsrPortError> {
            let p = self.prob_sequence.get(self.idx).copied().unwrap_or(0.0);
            self.idx += 1;
            Ok(p)
        }
        fn predict_probs_batch(&mut self, _samples: &[f32]) -> Result<Vec<f32>, AsrPortError> {
            Ok(self.prob_sequence.clone())
        }
        fn reset_state(&mut self) {
            self.idx = 0;
        }
    }

    #[test]
    fn streaming_buffers_and_tracks_speech_state() {
        let opts = VadDetectionOptions::streaming_defaults("mock.onnx");
        let runner = Box::new(MockRunner {
            window_size: 512,
            prob_sequence: vec![0.1, 0.1, 0.9, 0.9, 0.9, 0.9, 0.9, 0.1, 0.1],
            idx: 0,
        });

        let mut stream = NativeStreamingVad::new(runner, &opts, 16_000);
        assert!(!stream.is_speech_detected());

        // Feed 1024 samples (2 frames of 0.1)
        stream.accept_samples(&vec![0.0; 1024]);
        assert!(!stream.is_speech_detected());

        // Feed 1024 samples (2 frames of 0.9 -> reaches min_speech 150ms -> speech detected!)
        // Feed 2560 samples (5 frames of 0.9 -> 2560 samples >= 2400 samples (150ms) -> speech detected!)
        stream.accept_samples(&vec![0.0; 2560]);
        assert!(stream.is_speech_detected());

        // Reset clears speech detection
        stream.reset();
        assert!(!stream.is_speech_detected());
    }
}
