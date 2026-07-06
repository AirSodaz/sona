use sherpa_onnx::{VadModelConfig, VoiceActivityDetector};
pub use sona_local_asr::audio::{
    AudioSegment, extract_and_resample_audio, fixed_chunk_audio, pcm_i16_to_f32,
    resolve_ffmpeg_sidecar_path, resolve_ffmpeg_sidecar_path_from_exe, save_wav_file,
    whole_audio_segment,
};

pub fn vad_segment_audio(
    samples: &[f32],
    sample_rate: u32,
    vad_config: &VadModelConfig,
    _buffer_size_seconds: f32,
) -> Result<Vec<AudioSegment>, String> {
    let mut vad = VoiceActivityDetector::create(vad_config, 60.0)
        .ok_or("Failed to create VoiceActivityDetector")?;

    let window_size = vad_config.silero_vad.window_size as usize;
    let chunk_size = if window_size > 0 { window_size } else { 512 };
    let mut segments = Vec::new();

    for chunk in samples.chunks(chunk_size) {
        vad.accept_waveform(chunk);
        extract_vad_segments(&mut vad, sample_rate, &mut segments, false);
    }

    vad.flush();
    extract_vad_segments(&mut vad, sample_rate, &mut segments, true);

    Ok(segments)
}

fn extract_vad_segments(
    vad: &mut VoiceActivityDetector,
    sample_rate: u32,
    segments: &mut Vec<AudioSegment>,
    is_flush: bool,
) {
    while !vad.is_empty() {
        if let Some(segment) = vad.front() {
            let start_sample = segment.start() as usize;
            let seg_samples = segment.samples().to_vec();
            let start_time = start_sample as f32 / sample_rate as f32;
            let duration = seg_samples.len() as f32 / sample_rate as f32;

            let tag = if is_flush { "(flush)" } else { "" };
            log::debug!(
                "[Sona VAD] segment {} start_sample={} duration={:.2}s samples={}",
                tag,
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn resolves_ffmpeg_sidecar_path_from_executable() {
        let exe_path = Path::new("/tmp/sona/sona");
        let ffmpeg_path = resolve_ffmpeg_sidecar_path_from_exe(exe_path).expect("path");

        #[cfg(windows)]
        assert!(ffmpeg_path.ends_with("ffmpeg.exe"));

        #[cfg(not(windows))]
        assert!(ffmpeg_path.ends_with("ffmpeg"));
    }

    #[test]
    fn test_pcm_i16_to_f32() {
        let input = vec![0, 16384, 32767, -16384, -32768];
        let output = pcm_i16_to_f32(&input);
        assert_eq!(output[0], 0.0);
        assert!((output[1] - 0.5).abs() < 1e-4);
        assert!((output[2] - 0.9999).abs() < 1e-3);
        assert!((output[3] + 0.5).abs() < 1e-4);
        assert_eq!(output[4], -1.0);
    }

    #[test]
    fn test_fixed_chunk_audio() {
        let sample_rate = 16000;
        let samples = vec![0.0; 16000 * 5];
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
    fn test_whole_audio_segment() {
        let sample_rate = 16000;
        let samples = vec![0.0; 16000 * 5];
        let segments = whole_audio_segment(&samples, sample_rate);

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].start_time, 0.0);
        assert_eq!(segments[0].duration, 5.0);
        assert_eq!(segments[0].samples.len(), samples.len());
    }

    #[test]
    fn test_whole_audio_segment_omits_empty_audio() {
        let segments = whole_audio_segment(&[], 16000);

        assert!(segments.is_empty());
    }
}
