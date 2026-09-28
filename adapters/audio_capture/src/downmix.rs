use ringbuf::traits::Producer;
use std::sync::atomic::{AtomicBool, Ordering};

/// Downmix multi-channel f32 samples to mono and push directly into an SPSC ring buffer.
///
/// Guaranteed ZERO heap allocations, safe for real-time audio driver callbacks (e.g. CoreAudio HAL).
#[inline]
pub fn push_downmixed_f32(data: &[f32], channels: usize, producer: &mut impl Producer<Item = f32>) {
    if channels <= 1 {
        for &sample in data {
            let _ = producer.try_push(sample);
        }
    } else {
        let inv_channels = 1.0 / channels as f32;
        for frame in data.chunks(channels) {
            let sum: f32 = frame.iter().sum();
            let _ = producer.try_push(sum * inv_channels);
        }
    }
}

/// Downmix multi-channel i16 samples to normalized f32 mono and push directly into ring buffer.
#[inline]
pub fn push_downmixed_i16(data: &[i16], channels: usize, producer: &mut impl Producer<Item = f32>) {
    const NORM: f32 = 1.0 / 32768.0;
    if channels <= 1 {
        for &sample in data {
            let _ = producer.try_push(sample as f32 * NORM);
        }
    } else {
        let inv_channels = 1.0 / channels as f32;
        for frame in data.chunks(channels) {
            let mut sum = 0.0_f32;
            for &sample in frame {
                sum += sample as f32 * NORM;
            }
            let _ = producer.try_push(sum * inv_channels);
        }
    }
}

/// Downmix multi-channel u16 samples to normalized f32 mono and push directly into ring buffer.
#[inline]
pub fn push_downmixed_u16(data: &[u16], channels: usize, producer: &mut impl Producer<Item = f32>) {
    if channels <= 1 {
        for &sample in data {
            let _ = producer.try_push((sample as f32 - 32768.0) / 32768.0);
        }
    } else {
        let inv_channels = 1.0 / channels as f32;
        for frame in data.chunks(channels) {
            let mut sum = 0.0_f32;
            for &sample in frame {
                sum += (sample as f32 - 32768.0) / 32768.0;
            }
            let _ = producer.try_push(sum * inv_channels);
        }
    }
}

/// Same as `push_downmixed_f32`, but records an overflow flag if the ring buffer fills up.
#[inline]
pub fn push_downmixed_f32_checked(
    data: &[f32],
    channels: usize,
    producer: &mut impl Producer<Item = f32>,
    overflow: &AtomicBool,
) {
    if channels <= 1 {
        for &sample in data {
            if producer.try_push(sample).is_err() {
                overflow.store(true, Ordering::Release);
                break;
            }
        }
    } else {
        let inv_channels = 1.0 / channels as f32;
        for frame in data.chunks(channels) {
            let sum: f32 = frame.iter().sum();
            if producer.try_push(sum * inv_channels).is_err() {
                overflow.store(true, Ordering::Release);
                break;
            }
        }
    }
}

/// Same as `push_downmixed_i16`, but records an overflow flag if the ring buffer fills up.
#[inline]
pub fn push_downmixed_i16_checked(
    data: &[i16],
    channels: usize,
    producer: &mut impl Producer<Item = f32>,
    overflow: &AtomicBool,
) {
    const NORM: f32 = 1.0 / 32768.0;
    if channels <= 1 {
        for &sample in data {
            if producer.try_push(sample as f32 * NORM).is_err() {
                overflow.store(true, Ordering::Release);
                break;
            }
        }
    } else {
        let inv_channels = 1.0 / channels as f32;
        for frame in data.chunks(channels) {
            let mut sum = 0.0_f32;
            for &sample in frame {
                sum += sample as f32 * NORM;
            }
            if producer.try_push(sum * inv_channels).is_err() {
                overflow.store(true, Ordering::Release);
                break;
            }
        }
    }
}

/// Same as `push_downmixed_u16`, but records an overflow flag if the ring buffer fills up.
#[inline]
pub fn push_downmixed_u16_checked(
    data: &[u16],
    channels: usize,
    producer: &mut impl Producer<Item = f32>,
    overflow: &AtomicBool,
) {
    if channels <= 1 {
        for &sample in data {
            if producer
                .try_push((sample as f32 - 32768.0) / 32768.0)
                .is_err()
            {
                overflow.store(true, Ordering::Release);
                break;
            }
        }
    } else {
        let inv_channels = 1.0 / channels as f32;
        for frame in data.chunks(channels) {
            let mut sum = 0.0_f32;
            for &sample in frame {
                sum += (sample as f32 - 32768.0) / 32768.0;
            }
            if producer.try_push(sum * inv_channels).is_err() {
                overflow.store(true, Ordering::Release);
                break;
            }
        }
    }
}

pub fn downmix_f32(samples: &[f32], channels: usize) -> crate::error::AudioCaptureResult<Vec<f32>> {
    if channels == 0 {
        return Err(crate::error::AudioCaptureError::ConfigFailed(
            "audio input reported zero channels".to_string(),
        ));
    }
    if channels == 1 {
        return Ok(samples.to_vec());
    }
    let inv_channels = 1.0 / channels as f32;
    Ok(samples
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() * inv_channels)
        .collect())
}

pub fn downmix_i16(samples: &[i16], channels: usize) -> crate::error::AudioCaptureResult<Vec<f32>> {
    if channels == 0 {
        return Err(crate::error::AudioCaptureError::ConfigFailed(
            "audio input reported zero channels".to_string(),
        ));
    }
    const NORM: f32 = 1.0 / 32768.0;
    let inv_channels = 1.0 / channels as f32;
    Ok(samples
        .chunks(channels)
        .map(|frame| frame.iter().map(|&s| s as f32 * NORM).sum::<f32>() * inv_channels)
        .collect())
}

pub fn downmix_u16(samples: &[u16], channels: usize) -> crate::error::AudioCaptureResult<Vec<f32>> {
    if channels == 0 {
        return Err(crate::error::AudioCaptureError::ConfigFailed(
            "audio input reported zero channels".to_string(),
        ));
    }
    let inv_channels = 1.0 / channels as f32;
    Ok(samples
        .chunks(channels)
        .map(|frame| {
            frame
                .iter()
                .map(|&s| (s as f32 - 32767.5) / 32767.5)
                .sum::<f32>()
                * inv_channels
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringbuf::HeapRb;
    use ringbuf::traits::{Consumer, Observer, Split};

    #[test]
    fn test_push_downmixed_f32_mono() {
        let rb = HeapRb::<f32>::new(8);
        let (mut prod, mut cons) = rb.split();
        let data = [0.1, 0.2, 0.3];
        push_downmixed_f32(&data, 1, &mut prod);
        assert_eq!(cons.occupied_len(), 3);
        let mut out = [0.0; 3];
        cons.pop_slice(&mut out);
        assert_eq!(out, [0.1, 0.2, 0.3]);
    }

    #[test]
    fn test_push_downmixed_f32_stereo() {
        let rb = HeapRb::<f32>::new(8);
        let (mut prod, mut cons) = rb.split();
        let data = [0.2, 0.4, 0.6, 0.8]; // 2 frames of stereo
        push_downmixed_f32(&data, 2, &mut prod);
        assert_eq!(cons.occupied_len(), 2);
        let mut out = [0.0; 2];
        cons.pop_slice(&mut out);
        assert!((out[0] - 0.3).abs() < 1e-6);
        assert!((out[1] - 0.7).abs() < 1e-6);
    }

    #[test]
    fn test_push_downmixed_i16() {
        let rb = HeapRb::<f32>::new(8);
        let (mut prod, mut cons) = rb.split();
        let data = [0, 16384, -16384];
        push_downmixed_i16(&data, 1, &mut prod);
        assert_eq!(cons.occupied_len(), 3);
        let mut out = [0.0; 3];
        cons.pop_slice(&mut out);
        assert_eq!(out[0], 0.0);
        assert!((out[1] - 0.5).abs() < 1e-4);
        assert!((out[2] - (-0.5)).abs() < 1e-4);
    }
}
