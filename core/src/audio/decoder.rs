use std::fs::File;
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use thiserror::Error;

use super::resampler::resample_mono_to_target;

#[derive(Debug, Error)]
pub enum AudioDecodeError {
    #[error("File I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Unsupported audio format: {0}")]
    UnsupportedFormat(String),
    #[error("Audio decode error: {0}")]
    Decode(String),
    #[error("Resampling error: {0}")]
    Resample(String),
    #[error("Audio file contains no audio tracks")]
    NoAudioTrack,
    #[error("Decoded audio contains no samples")]
    EmptyAudio,
}

pub struct BuiltinAudioDecoder;

pub fn decode_audio_file(
    path: &Path,
    target_sample_rate: u32,
) -> Result<Vec<f32>, AudioDecodeError> {
    BuiltinAudioDecoder::decode_file(path, target_sample_rate)
}

pub fn decode_audio_slice(
    path: &Path,
    start_seconds: f64,
    duration_seconds: f64,
    target_sample_rate: u32,
) -> Result<Vec<f32>, AudioDecodeError> {
    BuiltinAudioDecoder::decode_slice(path, start_seconds, duration_seconds, target_sample_rate)
}

impl BuiltinAudioDecoder {
    /// Decodes an audio file at `path` into 16kHz (or `target_sample_rate`) mono PCM f32 samples.
    pub fn decode_file(path: &Path, target_sample_rate: u32) -> Result<Vec<f32>, AudioDecodeError> {
        if target_sample_rate == 0 {
            return Err(AudioDecodeError::Resample(
                "Target sample rate cannot be zero".into(),
            ));
        }

        let file = File::open(path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let format_opts = FormatOptions {
            enable_gapless: true,
            ..Default::default()
        };
        let metadata_opts = MetadataOptions::default();

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &format_opts, &metadata_opts)
            .map_err(|e| match e {
                SymphoniaError::Unsupported(_) => AudioDecodeError::UnsupportedFormat(format!(
                    "Unsupported format or container: {e}"
                )),
                SymphoniaError::IoError(io_err) => AudioDecodeError::Io(io_err),
                other => AudioDecodeError::Decode(other.to_string()),
            })?;

        let mut format = probed.format;

        // Find the first audio track
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or(AudioDecodeError::NoAudioTrack)?;

        let track_id = track.id;
        let track_rate = track
            .codec_params
            .sample_rate
            .ok_or_else(|| AudioDecodeError::Decode("Track sample rate is unknown".into()))?;

        let decoder_opts = DecoderOptions::default();
        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &decoder_opts)
            .map_err(|e| match e {
                SymphoniaError::Unsupported(_) => {
                    AudioDecodeError::UnsupportedFormat(format!("Unsupported audio codec: {e}"))
                }
                other => AudioDecodeError::Decode(other.to_string()),
            })?;

        let mut sample_buf: Option<SampleBuffer<f32>> = None;
        let mut mono_samples: Vec<f32> = Vec::new();

        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(SymphoniaError::IoError(err))
                    if err.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(SymphoniaError::ResetRequired) => {
                    decoder.reset();
                    continue;
                }
                Err(err) => {
                    log::debug!("End of packets or stream error: {err}");
                    break;
                }
            };

            if packet.track_id() != track_id {
                continue;
            }

            match decoder.decode(&packet) {
                Ok(decoded) => {
                    let spec = *decoded.spec();
                    let channels = spec.channels.count();
                    if channels == 0 {
                        continue;
                    }

                    let required_samples = decoded.capacity() * channels;
                    if sample_buf
                        .as_ref()
                        .is_none_or(|b| b.capacity() < required_samples)
                    {
                        sample_buf =
                            Some(SampleBuffer::<f32>::new(decoded.capacity() as u64, spec));
                    }

                    if let Some(buf) = sample_buf.as_mut() {
                        buf.copy_interleaved_ref(decoded);
                        let interleaved = buf.samples();
                        for frame in interleaved.chunks_exact(channels) {
                            let sum: f32 = frame.iter().sum();
                            mono_samples.push(sum / (channels as f32));
                        }
                    }
                }
                Err(SymphoniaError::DecodeError(err)) => {
                    log::warn!("Recoverable audio decode error: {err}");
                    continue;
                }
                Err(SymphoniaError::ResetRequired) => {
                    decoder.reset();
                    continue;
                }
                Err(err) => {
                    log::warn!("Fatal packet decode error: {err}");
                    break;
                }
            }
        }

        if mono_samples.is_empty() {
            return Err(AudioDecodeError::EmptyAudio);
        }

        // Resample to target_sample_rate if needed
        if track_rate == target_sample_rate {
            Ok(mono_samples)
        } else {
            resample_mono_to_target(&mono_samples, track_rate, target_sample_rate)
                .map_err(AudioDecodeError::Resample)
        }
    }

    /// Decodes a time-slice of the audio file into mono PCM f32 samples.
    pub fn decode_slice(
        path: &Path,
        start_seconds: f64,
        duration_seconds: f64,
        target_sample_rate: u32,
    ) -> Result<Vec<f32>, AudioDecodeError> {
        let all_samples = Self::decode_file(path, target_sample_rate)?;
        let start_sample = (start_seconds.max(0.0) * target_sample_rate as f64).round() as usize;
        let slice_samples =
            (duration_seconds.max(0.0) * target_sample_rate as f64).round() as usize;

        if start_sample >= all_samples.len() {
            return Ok(Vec::new());
        }

        let end_sample = (start_sample + slice_samples).min(all_samples.len());
        Ok(all_samples[start_sample..end_sample].to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_wav_file(sample_rate: u32, channels: u16, samples: &[i16]) -> NamedTempFile {
        let mut file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        let chunk_size = 36 + samples.len() * 2;
        bytes.extend_from_slice(&(chunk_size as u32).to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes()); // PCM
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        let byte_rate = sample_rate * channels as u32 * 2;
        bytes.extend_from_slice(&byte_rate.to_le_bytes());
        let block_align = channels * 2;
        bytes.extend_from_slice(&block_align.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        let subchunk2_size = samples.len() * 2;
        bytes.extend_from_slice(&(subchunk2_size as u32).to_le_bytes());
        for s in samples {
            bytes.extend_from_slice(&s.to_le_bytes());
        }
        file.write_all(&bytes).unwrap();
        file.flush().unwrap();
        file
    }

    #[test]
    fn test_decode_mono_wav_16k() {
        // 16000 samples of 16kHz mono = 1 second
        let pcm_samples: Vec<i16> = (0..16000)
            .map(|i| (i as f32 / 16000.0 * 1000.0) as i16)
            .collect();
        let temp_file = create_test_wav_file(16000, 1, &pcm_samples);

        let decoded = decode_audio_file(temp_file.path(), 16000).unwrap();
        assert_eq!(decoded.len(), 16000);
        // Verify first sample close to 0.0
        assert!((decoded[0] - (pcm_samples[0] as f32 / 32768.0)).abs() < 0.01);
    }

    #[test]
    fn test_decode_stereo_wav_44k1_resampled_to_16k() {
        // 1 second of 44100Hz stereo = 44100 * 2 = 88200 samples
        let pcm_samples: Vec<i16> = (0..88200).map(|i| (i % 500) as i16).collect();
        let temp_file = create_test_wav_file(44100, 2, &pcm_samples);

        let decoded = decode_audio_file(temp_file.path(), 16000).unwrap();
        // Expected output ~16000 mono samples
        assert_eq!(decoded.len(), 16000);
    }

    #[test]
    fn test_decode_audio_slice() {
        // 2 seconds of 16kHz mono = 32000 samples
        let pcm_samples: Vec<i16> = (0..32000).map(|i| (i % 1000) as i16).collect();
        let temp_file = create_test_wav_file(16000, 1, &pcm_samples);

        // Extract 0.5s to 1.5s (duration 1.0s = 16000 samples)
        let slice = decode_audio_slice(temp_file.path(), 0.5, 1.0, 16000).unwrap();
        assert_eq!(slice.len(), 16000);
    }

    #[test]
    fn test_decode_multichannel_5_1_surround() {
        // 6-channel 48kHz audio (5.1 surround), 0.5s = 24000 frames = 144000 samples
        let pcm_samples: Vec<i16> = (0..144000).map(|i| ((i % 1000) - 500) as i16).collect();
        let temp_file = create_test_wav_file(48000, 6, &pcm_samples);

        let decoded = decode_audio_file(temp_file.path(), 16000).unwrap();
        // 0.5s at 16kHz = 8000 mono samples
        assert_eq!(decoded.len(), 8000);
    }

    #[test]
    fn test_decode_nonexistent_file() {
        let invalid_path = Path::new("nonexistent_audio_file.wav");
        assert!(matches!(
            decode_audio_file(invalid_path, 16000),
            Err(AudioDecodeError::Io(_))
        ));
    }
}
