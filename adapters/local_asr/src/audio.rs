use hound::{SampleFormat, WavSpec, WavWriter};
use sherpa_onnx::{SileroVadModelConfig, VadModelConfig, VoiceActivityDetector};
use sona_core::ports::asr::BatchSegmentationMode;
use std::path::{Path, PathBuf};

pub(crate) type VadConfig = VadModelConfig;
pub(crate) type VadDetector = VoiceActivityDetector;

pub struct SafeVad(VadDetector);
unsafe impl Send for SafeVad {}
unsafe impl Sync for SafeVad {}

#[derive(Debug, Clone, Copy)]
pub struct VadDetectorOptions {
    pub threshold: f32,
    pub min_silence_duration: f32,
    pub min_speech_duration: f32,
    pub window_size: i32,
    pub sample_rate: i32,
    pub num_threads: i32,
}

impl Default for VadDetectorOptions {
    fn default() -> Self {
        Self {
            threshold: 0.30,
            min_silence_duration: 0.5,
            min_speech_duration: 0.25,
            window_size: 512,
            sample_rate: 16000,
            num_threads: 1,
        }
    }
}

impl VadDetectorOptions {
    pub fn batch_segmentation() -> Self {
        Self {
            threshold: 0.35,
            min_silence_duration: 1.0,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioSegment {
    pub samples: Vec<f32>,
    pub start_time: f32,
    pub duration: f32,
}

impl AudioSegment {
    pub fn end_time(&self) -> f32 {
        self.start_time + self.duration
    }
}

pub fn resolve_ffmpeg_sidecar_path_from_exe(exe_path: &Path) -> Result<PathBuf, String> {
    let exe_dir = exe_path
        .parent()
        .ok_or("Failed to get parent directory of executable")?;

    #[cfg(windows)]
    let ffmpeg_filename = "ffmpeg.exe";
    #[cfg(not(windows))]
    let ffmpeg_filename = "ffmpeg";

    Ok(exe_dir.join(ffmpeg_filename))
}

pub fn resolve_ffmpeg_sidecar_path() -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe()
        .map_err(|error| format!("Failed to get current executable path: {error}"))?;
    resolve_ffmpeg_sidecar_path_from_exe(&exe_path)
}

pub fn resolve_model_onnx_path(path: &Path) -> Result<PathBuf, String> {
    if !path.exists() {
        return Err(format!("Model path does not exist: {}", path.display()));
    }

    if path.is_file() {
        return Ok(path.to_path_buf());
    }

    let entries = std::fs::read_dir(path)
        .map_err(|error| format!("Failed to read model directory {}: {error}", path.display()))?;
    entries
        .flatten()
        .find(|entry| entry.path().extension().is_some_and(|ext| ext == "onnx"))
        .map(|entry| entry.path())
        .ok_or_else(|| format!("No .onnx file found in model directory {}", path.display()))
}

pub fn pcm_i16_to_f32(data: &[i16]) -> Vec<f32> {
    data.iter().map(|&sample| sample as f32 / 32768.0).collect()
}

fn pcm_bytes_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|chunk| {
            let lo = chunk[0] as i16;
            let hi = (chunk[1] as i16) << 8;
            let sample = lo | hi;
            sample as f32 / 32768.0
        })
        .collect()
}

pub fn save_wav_file(data: &[f32], sample_rate: u32, filepath: &Path) -> hound::Result<()> {
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut writer = WavWriter::create(filepath, spec)?;
    for &sample in data {
        let amplitude = i16::MAX as f32;
        writer.write_sample((sample.clamp(-1.0, 1.0) * amplitude) as i16)?;
    }
    writer.finalize()
}

pub async fn extract_and_resample_audio(
    filepath: &Path,
    target_sample_rate: u32,
) -> Result<Vec<f32>, String> {
    let ffmpeg_path = resolve_ffmpeg_sidecar_path()?;
    let mut command = tokio::process::Command::new(ffmpeg_path);

    #[cfg(target_os = "windows")]
    {
        #[allow(unused_imports)]
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }

    let output = command
        .arg("-loglevel")
        .arg("error")
        .arg("-i")
        .arg(filepath)
        .arg("-f")
        .arg("s16le")
        .arg("-acodec")
        .arg("pcm_s16le")
        .arg("-ar")
        .arg(target_sample_rate.to_string())
        .arg("-ac")
        .arg("1")
        .arg("-")
        .output()
        .await
        .map_err(|error| format!("Failed to run ffmpeg command: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("FFmpeg exited with {:?}: {stderr}", output.status));
    }

    Ok(pcm_bytes_to_f32(&output.stdout))
}

pub fn fixed_chunk_audio(
    samples: &[f32],
    sample_rate: u32,
    chunk_duration: f32,
) -> Vec<AudioSegment> {
    let chunk_size = (sample_rate as f32 * chunk_duration) as usize;
    if chunk_size == 0 {
        return Vec::new();
    }

    samples
        .chunks(chunk_size)
        .enumerate()
        .map(|(index, chunk)| {
            let start_sample = index * chunk_size;
            AudioSegment {
                samples: chunk.to_vec(),
                start_time: start_sample as f32 / sample_rate as f32,
                duration: chunk.len() as f32 / sample_rate as f32,
            }
        })
        .collect()
}

pub fn whole_audio_segment(samples: &[f32], sample_rate: u32) -> Vec<AudioSegment> {
    if samples.is_empty() {
        return Vec::new();
    }

    vec![AudioSegment {
        samples: samples.to_vec(),
        start_time: 0.0,
        duration: samples.len() as f32 / sample_rate as f32,
    }]
}

pub fn segment_batch_audio(
    samples: &[f32],
    vad_model: Option<&Path>,
    vad_buffer: f32,
    batch_segmentation_mode: BatchSegmentationMode,
) -> Vec<AudioSegment> {
    if batch_segmentation_mode == BatchSegmentationMode::Whole {
        return whole_audio_segment(samples, 16000);
    }

    let Some(vad_model) = vad_model else {
        return fixed_chunk_audio(samples, 16000, 30.0);
    };

    if vad_model.as_os_str().is_empty() || !vad_model.exists() {
        return fixed_chunk_audio(samples, 16000, 30.0);
    }

    let vad_config = match create_vad_config(vad_model, VadDetectorOptions::batch_segmentation()) {
        Ok(config) => config,
        Err(_) => return fixed_chunk_audio(samples, 16000, 30.0),
    };

    vad_segment_audio(samples, 16000, &vad_config, vad_buffer)
        .unwrap_or_else(|_| fixed_chunk_audio(samples, 16000, 30.0))
}

pub(crate) fn create_vad_config(
    vad_model: &Path,
    options: VadDetectorOptions,
) -> Result<VadConfig, String> {
    let model_path = resolve_model_onnx_path(vad_model)?;
    let silero_vad = SileroVadModelConfig {
        model: Some(model_path.to_string_lossy().to_string()),
        threshold: options.threshold,
        min_silence_duration: options.min_silence_duration,
        min_speech_duration: options.min_speech_duration,
        window_size: options.window_size,
        ..Default::default()
    };
    Ok(VadConfig {
        silero_vad,
        sample_rate: options.sample_rate,
        num_threads: options.num_threads,
        ..Default::default()
    })
}

pub(crate) fn create_vad_detector(
    vad_model: &Path,
    detector_capacity_seconds: f32,
) -> Result<VadDetector, String> {
    let vad_config = create_vad_config(vad_model, VadDetectorOptions::default())?;
    let detector_capacity_seconds = if detector_capacity_seconds > 0.0 {
        detector_capacity_seconds
    } else {
        60.0
    };
    VoiceActivityDetector::create(&vad_config, detector_capacity_seconds)
        .ok_or("Failed to create VoiceActivityDetector".to_string())
}

pub fn load_vad(vad_model: Option<String>) -> Option<SafeVad> {
    let v_path = vad_model?;

    if v_path.is_empty() {
        println!(
            "[Sherpa] load_vad: Path is empty or does not exist: {}",
            v_path
        );
        return None;
    }

    match create_vad_detector(Path::new(&v_path), 60.0) {
        Ok(vad) => {
            println!("[Sherpa] load_vad: VAD successfully created!");
            Some(SafeVad(vad))
        }
        Err(error) => {
            println!("[Sherpa] load_vad: {error}");
            None
        }
    }
}

pub fn reset_vad(vad: &mut SafeVad) {
    vad.0.reset();
    vad.0.clear();
}

pub fn accept_vad_samples(vad: &SafeVad, samples: &[f32]) {
    vad.0.accept_waveform(samples);
}

pub fn vad_detected(vad: &SafeVad) -> bool {
    vad.0.detected()
}

pub fn vad_segment_audio(
    samples: &[f32],
    sample_rate: u32,
    vad_config: &VadConfig,
    buffer_size_seconds: f32,
) -> Result<Vec<AudioSegment>, String> {
    let detector_capacity_seconds = if buffer_size_seconds > 0.0 {
        buffer_size_seconds
    } else {
        60.0
    };
    vad_segment_audio_with_capacity(samples, sample_rate, vad_config, detector_capacity_seconds)
}

pub(crate) fn vad_segment_audio_with_capacity(
    samples: &[f32],
    sample_rate: u32,
    vad_config: &VadConfig,
    detector_capacity_seconds: f32,
) -> Result<Vec<AudioSegment>, String> {
    let detector_capacity_seconds = if detector_capacity_seconds > 0.0 {
        detector_capacity_seconds
    } else {
        60.0
    };
    let mut vad = VoiceActivityDetector::create(vad_config, detector_capacity_seconds)
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
    use super::{
        resolve_ffmpeg_sidecar_path_from_exe, resolve_model_onnx_path, segment_batch_audio,
    };
    use crate::audio::pcm_i16_to_f32;
    use sona_core::ports::asr::BatchSegmentationMode;
    use std::fs;
    use std::path::Path;

    #[test]
    fn resolves_ffmpeg_sidecar_path_next_to_cli_executable() {
        let exe = Path::new("/tmp/sona-cli");
        let ffmpeg = resolve_ffmpeg_sidecar_path_from_exe(exe).unwrap();

        #[cfg(windows)]
        assert!(ffmpeg.ends_with("ffmpeg.exe"));

        #[cfg(not(windows))]
        assert!(ffmpeg.ends_with("ffmpeg"));
    }

    #[test]
    fn converts_pcm_i16_to_f32_samples() {
        let samples = pcm_i16_to_f32(&[0, 16384, -32768]);
        assert_eq!(samples[0], 0.0);
        assert!((samples[1] - 0.5).abs() < 0.01);
        assert_eq!(samples[2], -1.0);
    }

    #[test]
    fn batch_whole_segmentation_uses_one_full_audio_segment() {
        let samples = vec![0.0; 16000 * 65];
        let segments = segment_batch_audio(&samples, None, 5.0, BatchSegmentationMode::Whole);

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].start_time, 0.0);
        assert_eq!(segments[0].duration, 65.0);
        assert_eq!(segments[0].samples.len(), samples.len());
    }

    #[test]
    fn batch_vad_segmentation_falls_back_to_fixed_chunks_without_vad_model() {
        let samples = vec![0.0; 16000 * 65];
        let segments = segment_batch_audio(&samples, None, 5.0, BatchSegmentationMode::Vad);

        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].start_time, 0.0);
        assert_eq!(segments[0].duration, 30.0);
        assert_eq!(segments[1].start_time, 30.0);
        assert_eq!(segments[1].duration, 30.0);
        assert_eq!(segments[2].start_time, 60.0);
        assert_eq!(segments[2].duration, 5.0);
    }

    #[test]
    fn resolves_model_onnx_path_from_file_or_directory() {
        let root = std::env::temp_dir().join(format!("sona-model-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let onnx_path = root.join("model.onnx");
        fs::write(&onnx_path, "onnx").unwrap();

        assert_eq!(resolve_model_onnx_path(&onnx_path).unwrap(), onnx_path);
        assert_eq!(
            resolve_model_onnx_path(&root).unwrap(),
            root.join("model.onnx")
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resolves_model_onnx_path_rejects_missing_path() {
        let missing = std::env::temp_dir().join(format!("sona-missing-{}", uuid::Uuid::new_v4()));

        let error = resolve_model_onnx_path(&missing).unwrap_err();

        assert!(error.contains("Model path does not exist"));
    }
}
