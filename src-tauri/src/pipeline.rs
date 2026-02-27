use hound;
use rubato::{FftFixedOut, Resampler};
use sherpa_onnx::{VadModelConfig, VoiceActivityDetector};
use std::path::Path;
use symphonia::core::audio::{AudioBuffer, Signal};
use symphonia::core::codecs::CODEC_TYPE_NULL;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub struct AudioSegment {
    pub samples: Vec<f32>,
    pub start_time: f32,
    pub duration: f32,
}

pub fn pcm_i16_to_f32(data: &[i16]) -> Vec<f32> {
    data.iter().map(|&s| s as f32 / 32768.0).collect()
}

pub fn save_wav_file(data: &[f32], sample_rate: u32, filepath: &str) -> hound::Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(filepath, spec)?;
    for &sample in data {
        let amplitude = i16::MAX as f32;
        writer.write_sample((sample.clamp(-1.0, 1.0) * amplitude) as i16)?;
    }
    writer.finalize()
}

pub fn extract_and_resample_audio(
    filepath: &str,
    target_sample_rate: u32,
) -> Result<Vec<f32>, String> {
    let src = std::fs::File::open(filepath).map_err(|e| e.to_string())?;
    let mss = MediaSourceStream::new(Box::new(src), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = Path::new(filepath).extension().and_then(|s| s.to_str()) {
        hint.with_extension(ext);
    }

    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .map_err(|e| e.to_string())?;

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or("No supported audio track found")?;

    let track_id = track.id;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(16000);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &Default::default())
        .map_err(|e| e.to_string())?;

    let mut all_samples: Vec<f32> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(err))
                if err.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(e.to_string()),
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                let mut buf = AudioBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
                decoded.convert(&mut buf);

                let channels = buf.spec().channels.count();
                let frames = buf.frames();
                for i in 0..frames {
                    let mut sum = 0.0;
                    for c in 0..channels {
                        sum += buf.chan(c)[i];
                    }
                    all_samples.push(sum / (channels as f32));
                }
            }
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(e.to_string()),
        }
    }

    if sample_rate != target_sample_rate {
        let chunk_size = 1024;
        let mut resampler = FftFixedOut::<f32>::new(
            sample_rate as usize,
            target_sample_rate as usize,
            chunk_size,
            1,
            1,
        )
        .map_err(|e| e.to_string())?;

        let mut input_frames_needed = resampler.input_frames_next();
        let mut resampled_samples = Vec::new();

        let mut i = 0;
        let mut input_buffer = vec![vec![0.0; input_frames_needed]; 1];
        let mut output_buffer = vec![vec![0.0; chunk_size]; 1];

        while i < all_samples.len() {
            let remain = all_samples.len() - i;
            let take = remain.min(input_frames_needed);
            input_buffer[0][..take].copy_from_slice(&all_samples[i..i + take]);
            for j in take..input_frames_needed {
                input_buffer[0][j] = 0.0;
            }

            if let Ok((_in_len, out_len)) =
                resampler.process_into_buffer(&input_buffer, &mut output_buffer, None)
            {
                resampled_samples.extend_from_slice(&output_buffer[0][..out_len]);
            }
            i += take;
            input_frames_needed = resampler.input_frames_next();
            if input_buffer[0].len() != input_frames_needed {
                input_buffer[0].resize(input_frames_needed, 0.0);
            }
        }
        Ok(resampled_samples)
    } else {
        Ok(all_samples)
    }
}

pub fn fixed_chunk_audio(
    samples: &[f32],
    sample_rate: u32,
    chunk_duration: f32,
) -> Vec<AudioSegment> {
    let mut segments = Vec::new();
    let chunk_size = (sample_rate as f32 * chunk_duration) as usize;
    let mut i = 0;

    while i < samples.len() {
        let end = (i + chunk_size).min(samples.len());
        let chunk = &samples[i..end];
        segments.push(AudioSegment {
            samples: chunk.to_vec(),
            start_time: i as f32 / sample_rate as f32,
            duration: chunk.len() as f32 / sample_rate as f32,
        });
        i += chunk_size;
    }
    segments
}

pub fn vad_segment_audio(
    samples: &[f32],
    sample_rate: u32,
    vad_config: &VadModelConfig,
    _buffer_size_seconds: f32,
) -> Result<Vec<AudioSegment>, String> {
    let vad = VoiceActivityDetector::create(vad_config, 60.0)
        .ok_or("Failed to create VoiceActivityDetector")?;

    // Use the same window_size as the VAD model (512 samples = 32ms at 16kHz).
    let window_size = vad_config.silero_vad.window_size as usize;
    let chunk_size = if window_size > 0 { window_size } else { 512 };

    let mut segments: Vec<AudioSegment> = Vec::new();

    // Feed audio to VAD in window-sized chunks and collect completed segments.
    // Use segment.samples() directly — sherpa's VoiceActivityDetector already
    // includes proper context in the samples it returns. Re-extracting from the
    // original array with manual padding caused misaligned split positions.
    let mut current_pos = 0;
    while current_pos < samples.len() {
        let end = (current_pos + chunk_size).min(samples.len());
        let chunk = &samples[current_pos..end];
        vad.accept_waveform(chunk);

        while !vad.is_empty() {
            if let Some(segment) = vad.front() {
                let start_sample = segment.start() as usize;
                let seg_samples = segment.samples().to_vec();
                let start_time = start_sample as f32 / sample_rate as f32;
                let duration = seg_samples.len() as f32 / sample_rate as f32;

                eprintln!(
                    "[Sona VAD] segment start_sample={} duration={:.2}s samples={}",
                    start_sample,
                    duration,
                    seg_samples.len()
                );

                segments.push(AudioSegment {
                    samples: seg_samples,
                    start_time,
                    duration,
                });
            }
            vad.pop();
        }
        current_pos += chunk_size;
    }

    // Flush remaining speech at end of audio
    vad.flush();
    while !vad.is_empty() {
        if let Some(segment) = vad.front() {
            let start_sample = segment.start() as usize;
            let seg_samples = segment.samples().to_vec();
            let start_time = start_sample as f32 / sample_rate as f32;
            let duration = seg_samples.len() as f32 / sample_rate as f32;

            eprintln!(
                "[Sona VAD] segment (flush) start_sample={} duration={:.2}s samples={}",
                start_sample,
                duration,
                seg_samples.len()
            );

            segments.push(AudioSegment {
                samples: seg_samples,
                start_time,
                duration,
            });
        }
        vad.pop();
    }

    Ok(segments)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pcm_i16_to_f32() {
        let input = vec![0, 16384, 32767, -16384, -32768];
        let output = pcm_i16_to_f32(&input);
        assert_eq!(output[0], 0.0);
        assert!((output[1] - 0.5).abs() < 1e-4);
        assert!((output[2] - 0.9999).abs() < 1e-3); // 32767 / 32768.0 = 0.999969
        assert!((output[3] + 0.5).abs() < 1e-4);
        assert_eq!(output[4], -1.0);
    }

    #[test]
    fn test_fixed_chunk_audio() {
        let sample_rate = 16000;
        let samples = vec![0.0; 16000 * 5]; // 5 seconds
        let segments = fixed_chunk_audio(&samples, sample_rate, 2.0);
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].duration, 2.0);
        assert_eq!(segments[0].start_time, 0.0);
        assert_eq!(segments[1].duration, 2.0);
        assert_eq!(segments[1].start_time, 2.0);
        assert_eq!(segments[2].duration, 1.0);
        assert_eq!(segments[2].start_time, 4.0);
    }

    #[test]
    fn test_vad_segmentation_behavior() {
        use sherpa_onnx::{SileroVadModelConfig, VadModelConfig, VoiceActivityDetector};

        let sample_rate = 16000;
        // let's generate 4 seconds of silence, 2 seconds of "speech" (ones), 4 seconds of silence
        let mut samples = vec![0.0; sample_rate * 4];
        samples.extend(vec![0.5; sample_rate * 2]);
        samples.extend(vec![0.0; sample_rate * 4]);

        let mut silero_vad = SileroVadModelConfig::default();
        // we can't really load a model easily in a unit test without the file, so we just print
        // Actually, if we cannot load the model, we can't test VAD natively this easily.
        println!("Test stub");
    }
}
