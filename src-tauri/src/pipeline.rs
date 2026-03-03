use hound;
use sherpa_onnx::{VadModelConfig, VoiceActivityDetector};

pub struct AudioSegment {
    pub samples: Vec<f32>,
    pub start_time: f32,
    pub duration: f32,
}

pub fn pcm_i16_to_f32(data: &[i16]) -> Vec<f32> {
    data.iter().map(|&s| s as f32 / 32768.0).collect()
}

/// Convert raw PCM bytes (Int16LE) to Vec<f32> samples.
fn pcm_bytes_to_f32(bytes: &[u8]) -> Vec<f32> {
    let num_samples = bytes.len() / 2;
    let mut samples = Vec::with_capacity(num_samples);
    for i in 0..num_samples {
        let lo = bytes[i * 2] as i16;
        let hi = (bytes[i * 2 + 1] as i16) << 8;
        let sample_i16 = lo | hi;
        samples.push(sample_i16 as f32 / 32768.0);
    }
    samples
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

/// Extract audio from any file and resample to the target sample rate using FFmpeg.
/// This mirrors the JS sidecar's `convertToWav` function exactly:
///   ffmpeg -i <file> -f s16le -acodec pcm_s16le -ar <rate> -ac 1 -
pub async fn extract_and_resample_audio<R: tauri::Runtime>(
    _app: &tauri::AppHandle<R>,
    filepath: &str,
    target_sample_rate: u32,
) -> Result<Vec<f32>, String> {
    // Locate the current executable path
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get current executable path: {}", e))?;

    // Get the directory of the current executable
    let exe_dir = exe_path
        .parent()
        .ok_or("Failed to get parent directory of executable")?;

    // Build the target triple specific sidecar name
    let _target = tauri::utils::platform::target_triple()
        .map_err(|e| format!("Failed to get target triple: {}", e))?;

    #[cfg(windows)]
    let ffmpeg_filename = "ffmpeg.exe".to_string();
    #[cfg(not(windows))]
    let ffmpeg_filename = "ffmpeg".to_string();

    // Construct the absolute path to the sidecar
    let ffmpeg_path = exe_dir.join(ffmpeg_filename);

    // Run ffmpeg using tokio::process::Command
    let mut command = tokio::process::Command::new(ffmpeg_path);

    #[cfg(target_os = "windows")]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW

    let output = command
        .args([
            "-loglevel",
            "error",
            "-i",
            filepath,
            "-f",
            "s16le",
            "-acodec",
            "pcm_s16le",
            "-ar",
            &target_sample_rate.to_string(),
            "-ac",
            "1",
            "-",
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to run ffmpeg command: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "FFmpeg exited with {:?}: {}",
            output.status, stderr
        ));
    }

    let samples = pcm_bytes_to_f32(&output.stdout);
    Ok(samples)
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
}
