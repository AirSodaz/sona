use crate::error::{AudioCaptureError, AudioCaptureResult};
use ringbuf::traits::{Consumer, Producer, Split};
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};

pub const TARGET_SAMPLE_RATE: usize = 16_000;
pub const DEFAULT_CHUNK_SIZE_OUT: usize = 1024;

/// Unified audio resampler targeting 16,000 Hz mono.
///
/// Features:
/// 1. Automatic 16kHz passthrough: if input is 16,000 Hz, FFT is completely bypassed (0 CPU / 0 latency).
/// 2. Pre-allocated scratch buffers: zero dynamic heap allocations during streaming.
/// 3. Direct ring buffer integration: pumps between SPSC lock-free queues without intermediate copies.
pub struct AudioResampler {
    input_sample_rate: u32,
    inner: Option<Fft<f32>>,
    input_buffer: Vec<f32>,
    output_buffer: Vec<f32>,
    pending: Vec<f32>,
    chunk_size_out: usize,
    flushed: bool,
}

impl AudioResampler {
    pub fn new(input_sample_rate: u32) -> AudioCaptureResult<Self> {
        Self::with_chunk_size(input_sample_rate, DEFAULT_CHUNK_SIZE_OUT)
    }

    pub fn with_chunk_size(
        input_sample_rate: u32,
        chunk_size_out: usize,
    ) -> AudioCaptureResult<Self> {
        if input_sample_rate == 0 {
            return Err(AudioCaptureError::ResamplerInitFailed(
                "input sample rate cannot be zero".to_string(),
            ));
        }

        if input_sample_rate as usize == TARGET_SAMPLE_RATE {
            return Ok(Self {
                input_sample_rate,
                inner: None,
                input_buffer: Vec::new(),
                output_buffer: Vec::new(),
                pending: Vec::new(),
                chunk_size_out,
                flushed: false,
            });
        }

        let resampler = Fft::<f32>::new(
            input_sample_rate as usize,
            TARGET_SAMPLE_RATE,
            chunk_size_out,
            1,
            FixedSync::Output,
        )
        .map_err(|e| AudioCaptureError::ResamplerInitFailed(e.to_string()))?;

        let input_buffer = vec![0.0_f32; resampler.input_frames_max()];
        let output_buffer = vec![0.0_f32; resampler.output_frames_max()];

        Ok(Self {
            input_sample_rate,
            inner: Some(resampler),
            input_buffer,
            output_buffer,
            pending: Vec::new(),
            chunk_size_out,
            flushed: false,
        })
    }

    #[inline]
    pub fn is_passthrough(&self) -> bool {
        self.inner.is_none()
    }

    #[inline]
    pub fn input_sample_rate(&self) -> u32 {
        self.input_sample_rate
    }

    #[inline]
    pub fn input_frames_next(&self) -> usize {
        match &self.inner {
            Some(resampler) => resampler.input_frames_next(),
            None => DEFAULT_CHUNK_SIZE_OUT,
        }
    }

    /// Read raw audio from `consumer`, resample to 16kHz mono (or pass through if already 16kHz),
    /// and push the resampled samples directly into `producer`.
    ///
    /// Guaranteed ZERO heap allocations per chunk. Returns the total number of 16kHz samples produced.
    pub fn process_from_consumer(
        &mut self,
        consumer: &mut impl Consumer<Item = f32>,
        producer: &mut impl Producer<Item = f32>,
    ) -> usize {
        let Some(resampler) = &mut self.inner else {
            // 16kHz direct passthrough: pump raw samples directly to 16kHz destination
            let occupied = consumer.occupied_len();
            if occupied == 0 {
                return 0;
            }
            let mut total_moved = 0;
            while let Some(sample) = consumer.try_pop() {
                if producer.try_push(sample).is_err() {
                    break;
                }
                total_moved += 1;
            }
            return total_moved;
        };

        let mut total_produced = 0;
        while consumer.occupied_len() >= resampler.input_frames_next() {
            let needed = resampler.input_frames_next();
            let _read = consumer.pop_slice(&mut self.input_buffer[..needed]);

            let input_adapter = match InterleavedSlice::new(&self.input_buffer, 1, needed) {
                Ok(adapter) => adapter,
                Err(error) => {
                    log::error!("[AudioResampler] input adapter error: {error}");
                    continue;
                }
            };

            let out_capacity = self.output_buffer.len();
            let mut output_adapter =
                match InterleavedSlice::new_mut(&mut self.output_buffer, 1, out_capacity) {
                    Ok(adapter) => adapter,
                    Err(error) => {
                        log::error!("[AudioResampler] output adapter error: {error}");
                        continue;
                    }
                };

            match resampler.process_into_buffer(&input_adapter, &mut output_adapter, None) {
                Ok((_in_len, out_len)) => {
                    if out_len > 0 {
                        let out_slice = &self.output_buffer[..out_len];
                        let pushed = producer.push_slice(out_slice);
                        total_produced += pushed;
                    }
                }
                Err(e) => {
                    log::error!("[AudioResampler] resample error: {e}");
                }
            }
        }

        total_produced
    }

    /// Read raw audio from `consumer`, resample (or pass through if 16kHz), and append to `out`.
    /// Returns the number of samples appended.
    pub fn drain_from_consumer(
        &mut self,
        consumer: &mut impl Consumer<Item = f32>,
        out: &mut Vec<f32>,
    ) -> usize {
        let initial_len = out.len();
        let Some(resampler) = &mut self.inner else {
            while let Some(sample) = consumer.try_pop() {
                out.push(sample);
            }
            return out.len() - initial_len;
        };

        while consumer.occupied_len() >= resampler.input_frames_next() {
            let needed = resampler.input_frames_next();
            let _read = consumer.pop_slice(&mut self.input_buffer[..needed]);

            let input_adapter = match InterleavedSlice::new(&self.input_buffer, 1, needed) {
                Ok(adapter) => adapter,
                Err(error) => {
                    log::error!("[AudioResampler] input adapter error: {error}");
                    continue;
                }
            };

            let out_capacity = self.output_buffer.len();
            let mut output_adapter =
                match InterleavedSlice::new_mut(&mut self.output_buffer, 1, out_capacity) {
                    Ok(adapter) => adapter,
                    Err(error) => {
                        log::error!("[AudioResampler] output adapter error: {error}");
                        continue;
                    }
                };

            match resampler.process_into_buffer(&input_adapter, &mut output_adapter, None) {
                Ok((_in_len, out_len)) => {
                    if out_len > 0 {
                        out.extend_from_slice(&self.output_buffer[..out_len]);
                    }
                }
                Err(e) => {
                    log::error!("[AudioResampler] resample error: {e}");
                }
            }
        }

        out.len() - initial_len
    }

    /// Flush any remaining partial samples upon recording stop, padding with silence for the filter delay.
    pub fn drain_finish(
        &mut self,
        consumer: &mut impl Consumer<Item = f32>,
        out: &mut Vec<f32>,
    ) -> usize {
        let initial_len = out.len();
        if self.flushed {
            return 0;
        }
        self.flushed = true;
        self.drain_from_consumer(consumer, out);

        let Some(resampler) = &mut self.inner else {
            return out.len() - initial_len;
        };
        // Drain any remaining samples from the consumer into self.pending
        let occupied = consumer.occupied_len();
        if occupied > 0 {
            let start = self.pending.len();
            self.pending.resize(start + occupied, 0.0);
            let _read = consumer.pop_slice(&mut self.pending[start..]);
        }

        // Add tail silence padding to flush the algorithmic filter delay
        let tail_padding = (resampler.output_delay() as f64 / resampler.resample_ratio()).ceil()
            as usize
            + self.chunk_size_out;
        self.pending.resize(self.pending.len() + tail_padding, 0.0);

        while !self.pending.is_empty() {
            let needed = resampler.input_frames_next();
            let frames = needed.min(self.pending.len());
            self.input_buffer[..frames].copy_from_slice(&self.pending[..frames]);
            if frames < needed {
                self.input_buffer[frames..needed].fill(0.0);
            }

            let indexing = (frames < needed).then(|| rubato::Indexing::new().partial_len(frames));
            let input_adapter = match InterleavedSlice::new(&self.input_buffer[..needed], 1, needed)
            {
                Ok(a) => a,
                Err(e) => {
                    log::error!("[AudioResampler] input adapter error during drain_finish: {e}");
                    self.pending.clear();
                    break;
                }
            };
            let out_capacity = self.output_buffer.len();
            let mut output_adapter =
                match InterleavedSlice::new_mut(&mut self.output_buffer, 1, out_capacity) {
                    Ok(a) => a,
                    Err(e) => {
                        log::error!(
                            "[AudioResampler] output adapter error during drain_finish: {e}"
                        );
                        self.pending.clear();
                        break;
                    }
                };

            match resampler.process_into_buffer(
                &input_adapter,
                &mut output_adapter,
                indexing.as_ref(),
            ) {
                Ok((_consumed, written)) => {
                    if written > 0 {
                        out.extend_from_slice(&self.output_buffer[..written]);
                    }
                    self.pending.drain(..frames);
                }
                Err(e) => {
                    log::error!("[AudioResampler] flush error during drain_finish: {e}");
                    self.pending.clear();
                    break;
                }
            }
        }

        out.len() - initial_len
    }

    /// Resample a single chunk of samples into a newly allocated vector (convenient for testing or batch operations).
    pub fn process_chunk(&mut self, input: &[f32]) -> AudioCaptureResult<Vec<f32>> {
        let Some(resampler) = &mut self.inner else {
            return Ok(input.to_vec());
        };

        self.pending.extend_from_slice(input);
        let mut output = Vec::new();

        while self.pending.len() >= resampler.input_frames_next() {
            let needed = resampler.input_frames_next();
            self.input_buffer[..needed].copy_from_slice(&self.pending[..needed]);

            let input_adapter = match InterleavedSlice::new(&self.input_buffer[..needed], 1, needed)
            {
                Ok(a) => a,
                Err(e) => {
                    self.pending.drain(..needed);
                    return Err(AudioCaptureError::ResamplingFailed(e.to_string()));
                }
            };

            let out_capacity = self.output_buffer.len();
            let mut output_adapter =
                match InterleavedSlice::new_mut(&mut self.output_buffer, 1, out_capacity) {
                    Ok(a) => a,
                    Err(e) => {
                        self.pending.drain(..needed);
                        return Err(AudioCaptureError::ResamplingFailed(e.to_string()));
                    }
                };

            let (_consumed, written) =
                match resampler.process_into_buffer(&input_adapter, &mut output_adapter, None) {
                    Ok(res) => res,
                    Err(e) => {
                        self.pending.drain(..needed);
                        return Err(AudioCaptureError::ResamplingFailed(e.to_string()));
                    }
                };

            output.extend_from_slice(&self.output_buffer[..written]);
            self.pending.drain(..needed);
        }

        Ok(output)
    }

    /// Flush all buffered input audio, padding with silence for the filter delay.
    pub fn finish(mut self) -> AudioCaptureResult<Vec<f32>> {
        let mut output = Vec::new();
        let rb = ringbuf::HeapRb::<f32>::new(1);
        let (_prod, mut cons) = rb.split();
        self.drain_finish(&mut cons, &mut output);
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringbuf::HeapRb;
    use ringbuf::traits::{Observer, Split};

    #[test]
    fn test_passthrough_at_16khz() {
        let mut resampler = AudioResampler::new(16_000).unwrap();
        assert!(resampler.is_passthrough());

        let rb_in = HeapRb::<f32>::new(16);
        let rb_out = HeapRb::<f32>::new(16);
        let (mut prod_in, mut cons_in) = rb_in.split();
        let (mut prod_out, mut cons_out) = rb_out.split();

        prod_in.push_slice(&[0.1, 0.2, 0.3, 0.4]);
        let produced = resampler.process_from_consumer(&mut cons_in, &mut prod_out);
        assert_eq!(produced, 4);

        let mut out = [0.0; 4];
        cons_out.pop_slice(&mut out);
        assert_eq!(out, [0.1, 0.2, 0.3, 0.4]);
    }

    #[test]
    fn test_resampling_from_48khz() {
        let mut resampler = AudioResampler::new(48_000).unwrap();
        assert!(!resampler.is_passthrough());

        let in_needed = resampler.input_frames_next();
        let rb_in = HeapRb::<f32>::new(in_needed * 4);
        let rb_out = HeapRb::<f32>::new(in_needed * 4);
        let (mut prod_in, mut cons_in) = rb_in.split();
        let (mut prod_out, cons_out) = rb_out.split();

        // Feed enough samples for at least 1 output chunk
        let dummy_input = vec![0.1_f32; in_needed * 2];
        prod_in.push_slice(&dummy_input);

        let produced = resampler.process_from_consumer(&mut cons_in, &mut prod_out);
        assert!(produced > 0);
        assert_eq!(cons_out.occupied_len(), produced);
    }

    #[test]
    fn test_drain_from_consumer_passthrough() {
        let mut resampler = AudioResampler::new(16_000).unwrap();
        let rb = HeapRb::<f32>::new(16);
        let (mut prod, mut cons) = rb.split();
        prod.push_slice(&[0.5, 0.6, 0.7]);
        let mut out = Vec::new();
        let count = resampler.drain_from_consumer(&mut cons, &mut out);
        assert_eq!(count, 3);
        assert_eq!(out, vec![0.5, 0.6, 0.7]);
    }

    #[test]
    fn test_drain_from_consumer_resampling_and_finish() {
        let mut resampler = AudioResampler::new(48_000).unwrap();
        let in_needed = resampler.input_frames_next();
        let rb = HeapRb::<f32>::new(in_needed * 4);
        let (mut prod, mut cons) = rb.split();
        prod.push_slice(&vec![0.2; in_needed + 10]);
        let mut out = Vec::new();
        let count = resampler.drain_from_consumer(&mut cons, &mut out);
        assert!(count > 0);
        assert_eq!(cons.occupied_len(), 10);

        let finished_count = resampler.drain_finish(&mut cons, &mut out);
        assert!(finished_count > 0);
        assert_eq!(cons.occupied_len(), 0);
    }

    #[test]
    fn test_drain_finish_flushes_delay_even_when_consumer_empty() {
        let mut resampler = AudioResampler::new(48_000).unwrap();
        let in_needed = resampler.input_frames_next();
        let rb = HeapRb::<f32>::new(in_needed * 4);
        let (mut prod, mut cons) = rb.split();
        prod.push_slice(&vec![0.3; in_needed]);
        let mut out = Vec::new();
        let count = resampler.drain_from_consumer(&mut cons, &mut out);
        assert!(count > 0);
        assert_eq!(cons.occupied_len(), 0);

        // Even though consumer is empty, drain_finish flushes the algorithmic delay
        let flushed = resampler.drain_finish(&mut cons, &mut out);
        assert!(flushed > 0);
        assert!(out.iter().all(|s| s.is_finite()));
        // Second invocation must be a no-op (idempotent, no infinite loop)
        let second = resampler.drain_finish(&mut cons, &mut out);
        assert_eq!(second, 0);
    }
}
