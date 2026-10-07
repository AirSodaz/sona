use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};

/// Resample mono audio samples from `input_sample_rate` to `target_sample_rate`.
///
/// If `input_sample_rate == target_sample_rate`, samples are returned as-is.
pub fn resample_mono_to_target(
    samples: &[f32],
    input_sample_rate: u32,
    target_sample_rate: u32,
) -> Result<Vec<f32>, String> {
    if input_sample_rate == 0 || target_sample_rate == 0 {
        return Err("Sample rate cannot be zero".to_string());
    }
    if input_sample_rate == target_sample_rate || samples.is_empty() {
        return Ok(samples.to_vec());
    }

    let chunk_size_in = 1024;
    let mut resampler = Fft::<f32>::new(
        input_sample_rate as usize,
        target_sample_rate as usize,
        chunk_size_in,
        1,
        FixedSync::Input,
    )
    .map_err(|e| format!("Failed to create resampler: {e}"))?;

    let delay = resampler.output_delay();
    let expected_target_len = ((samples.len() as f64)
        * (target_sample_rate as f64 / input_sample_rate as f64))
        .round() as usize;

    let mut input_buffer = vec![0.0_f32; resampler.input_frames_max()];
    let mut output_buffer = vec![0.0_f32; resampler.output_frames_max()];

    let mut output = Vec::with_capacity(expected_target_len + delay);

    let mut pos = 0;
    while pos < samples.len() {
        let needed = resampler.input_frames_next();
        let remaining = samples.len() - pos;
        let frames = needed.min(remaining);

        input_buffer[..frames].copy_from_slice(&samples[pos..pos + frames]);
        if frames < needed {
            input_buffer[frames..needed].fill(0.0);
        }

        let indexing = (frames < needed).then(|| rubato::Indexing::new().partial_len(frames));
        let in_adapter = InterleavedSlice::new(&input_buffer[..needed], 1, needed)
            .map_err(|e| format!("Input adapter error: {e}"))?;
        let out_capacity = output_buffer.len();
        let mut out_adapter = InterleavedSlice::new_mut(&mut output_buffer, 1, out_capacity)
            .map_err(|e| format!("Output adapter error: {e}"))?;

        let (_consumed, written) = resampler
            .process_into_buffer(&in_adapter, &mut out_adapter, indexing.as_ref())
            .map_err(|e| format!("Resampling process error: {e}"))?;

        if written > 0 {
            output.extend_from_slice(&output_buffer[..written]);
        }
        pos += frames;
    }

    // Flush filter delay to capture trailing signal
    let tail_delay = (delay as f64 / resampler.resample_ratio()).ceil() as usize + chunk_size_in;
    let mut tail_remaining = tail_delay;
    while tail_remaining > 0 {
        let needed = resampler.input_frames_next();
        let frames = needed.min(tail_remaining);
        input_buffer[..needed].fill(0.0);

        let indexing = (frames < needed).then(|| rubato::Indexing::new().partial_len(frames));
        let in_adapter = InterleavedSlice::new(&input_buffer[..needed], 1, needed)
            .map_err(|e| format!("Input adapter error: {e}"))?;
        let out_capacity = output_buffer.len();
        let mut out_adapter = InterleavedSlice::new_mut(&mut output_buffer, 1, out_capacity)
            .map_err(|e| format!("Output adapter error: {e}"))?;

        let (_consumed, written) = resampler
            .process_into_buffer(&in_adapter, &mut out_adapter, indexing.as_ref())
            .map_err(|e| format!("Resampling flush error: {e}"))?;

        if written > 0 {
            output.extend_from_slice(&output_buffer[..written]);
        }
        tail_remaining = tail_remaining.saturating_sub(frames);
    }

    // Discard the initial algorithmic filter delay so the resampled audio aligns with the original timeline
    let aligned = if output.len() > delay {
        &output[delay..]
    } else {
        &output[..]
    };

    // Trim to the exact expected length (removing tail silence padding)
    let final_len = expected_target_len.min(aligned.len());
    Ok(aligned[..final_len].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resample_passthrough_when_same_rate() {
        let input = vec![0.1, 0.2, -0.3, 0.5];
        let resampled = resample_mono_to_target(&input, 16000, 16000).unwrap();
        assert_eq!(input, resampled);
    }

    #[test]
    fn test_resample_empty_input() {
        let input: Vec<f32> = Vec::new();
        let resampled = resample_mono_to_target(&input, 48000, 16000).unwrap();
        assert!(resampled.is_empty());
    }

    #[test]
    fn test_resample_zero_rate_fails() {
        let input = vec![0.1, 0.2];
        assert!(resample_mono_to_target(&input, 0, 16000).is_err());
        assert!(resample_mono_to_target(&input, 16000, 0).is_err());
    }

    #[test]
    fn test_resample_48k_to_16k() {
        // 1 second of 48kHz audio = 48000 samples
        let input = vec![0.5_f32; 48000];
        let resampled = resample_mono_to_target(&input, 48000, 16000).unwrap();
        assert_eq!(resampled.len(), 16000);
    }

    #[test]
    fn test_resample_44k1_to_16k() {
        // 1 second of 44100Hz audio
        let input = vec![0.2_f32; 44100];
        let resampled = resample_mono_to_target(&input, 44100, 16000).unwrap();
        assert_eq!(resampled.len(), 16000);
    }
}
