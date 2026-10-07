use hound::{SampleFormat, WavSpec, WavWriter};
use sherpa_onnx::{SileroVadModelConfig, VadModelConfig, VoiceActivityDetector};
use sona_core::ports::asr::{AsrPortError, AsrPortErrorKind};
pub use sona_core::ports::asr::{
    find_available_ffmpeg, find_available_ffmpeg_from_exe, pcm_i16_to_f32, pcm_s16le_bytes_to_f32,
    resolve_ffmpeg_path, resolve_ffmpeg_path_from_exe, resolve_ffmpeg_sidecar_path,
    resolve_ffmpeg_sidecar_path_from_exe,
};
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
    pub max_speech_duration: f32,
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
            max_speech_duration: 30.0,
            sample_rate: 16000,
            num_threads: 1,
        }
    }
}

pub fn resolve_model_onnx_path(path: &Path) -> Result<PathBuf, AsrPortError> {
    if !path.exists() {
        return Err(AsrPortError::new(
            AsrPortErrorKind::Model,
            format!("Model path does not exist: {}", path.display()),
        ));
    }

    if path.is_file() {
        return Ok(path.to_path_buf());
    }

    let entries = std::fs::read_dir(path).map_err(|error| {
        AsrPortError::new(
            AsrPortErrorKind::FileSystem,
            format!("Failed to read model directory {}: {error}", path.display()),
        )
    })?;
    entries
        .flatten()
        .find(|entry| entry.path().extension().is_some_and(|ext| ext == "onnx"))
        .map(|entry| entry.path())
        .ok_or_else(|| {
            AsrPortError::new(
                AsrPortErrorKind::Model,
                format!("No .onnx file found in model directory {}", path.display()),
            )
        })
}

fn mono_pcm16_wav_spec(sample_rate: u32) -> WavSpec {
    WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    }
}

fn f32_to_i16_sample(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}

pub struct LiveWavRecorder {
    writer: Option<WavWriter<std::io::BufWriter<std::fs::File>>>,
    filepath: PathBuf,
}

impl LiveWavRecorder {
    pub fn create(filepath: &Path, sample_rate: u32) -> hound::Result<Self> {
        if let Some(parent) = filepath.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(hound::Error::IoError)?;
        }
        let writer = WavWriter::create(filepath, mono_pcm16_wav_spec(sample_rate))?;
        Ok(Self {
            writer: Some(writer),
            filepath: filepath.to_path_buf(),
        })
    }

    pub fn filepath(&self) -> &Path {
        &self.filepath
    }

    pub fn write_samples(&mut self, data: &[f32]) -> hound::Result<()> {
        if let Some(writer) = self.writer.as_mut() {
            for &sample in data {
                writer.write_sample(f32_to_i16_sample(sample))?;
            }
        }

        Ok(())
    }

    pub fn finalize(mut self) -> hound::Result<PathBuf> {
        if let Some(writer) = self.writer.take() {
            writer.finalize()?;
        }

        Ok(self.filepath)
    }
}

pub fn save_wav_file(data: &[f32], sample_rate: u32, filepath: &Path) -> hound::Result<()> {
    let mut writer = WavWriter::create(filepath, mono_pcm16_wav_spec(sample_rate))?;
    for &sample in data {
        writer.write_sample(f32_to_i16_sample(sample))?;
    }
    writer.finalize()
}

pub async fn extract_and_resample_audio(
    filepath: &Path,
    target_sample_rate: u32,
) -> Result<Vec<f32>, AsrPortError> {
    extract_and_resample_audio_with_ffmpeg(filepath, target_sample_rate, None).await
}

pub async fn extract_and_resample_audio_with_ffmpeg(
    filepath: &Path,
    target_sample_rate: u32,
    custom_ffmpeg_path: Option<&Path>,
) -> Result<Vec<f32>, AsrPortError> {
    extract_and_resample_audio_with_options(filepath, target_sample_rate, true, custom_ffmpeg_path)
        .await
}

pub async fn extract_and_resample_audio_with_options(
    filepath: &Path,
    target_sample_rate: u32,
    ffmpeg_enabled: bool,
    custom_ffmpeg_path: Option<&Path>,
) -> Result<Vec<f32>, AsrPortError> {
    // 1. Prioritize built-in pure Rust decoder (Symphonia + Rubato) on blocking task
    let path_buf = filepath.to_path_buf();
    let builtin_result = tokio::task::spawn_blocking(move || {
        sona_core::audio::decode_audio_file(&path_buf, target_sample_rate)
    })
    .await
    .map_err(|join_err| {
        AsrPortError::runtime(format!("Audio decode task join error: {join_err}"))
    })?;

    let builtin_failure_cause = match builtin_result {
        Ok(samples) => return Ok(samples),
        Err(sona_core::audio::AudioDecodeError::Io(io_err)) => {
            return Err(AsrPortError::new(
                AsrPortErrorKind::FileSystem,
                format!("Failed to read audio file {}: {io_err}", filepath.display()),
            ));
        }
        Err(builtin_err) => {
            log::info!(
                "Built-in audio decoder skipped {}: {builtin_err}. Attempting FFmpeg fallback...",
                filepath.display()
            );
            builtin_err.to_string()
        }
    };

    // 2. Fall back to FFmpeg only if enabled
    if !ffmpeg_enabled {
        return Err(AsrPortError::new(
            AsrPortErrorKind::InvalidRequest,
            format!(
                "Failed to decode audio file {}: built-in decoder cannot process this file (built-in decoder error: {builtin_failure_cause}) and FFmpeg is disabled in settings. Enable FFmpeg in settings to support extended formats or use a supported format (MP3, WAV, M4A, AAC, FLAC, OGG).",
                filepath.display()
            ),
        ));
    }

    let ffmpeg_path = match resolve_ffmpeg_path(custom_ffmpeg_path) {
        Ok(path) if path.is_file() => path,
        Ok(path) => {
            let detail = if custom_ffmpeg_path.is_some_and(|p| !p.as_os_str().is_empty()) {
                format!("resolved FFmpeg path does not exist ({})", path.display())
            } else {
                "FFmpeg is not installed. Please install FFmpeg or use a supported format (MP3, WAV, M4A, AAC, FLAC, OGG).".to_string()
            };
            return Err(AsrPortError::new(
                AsrPortErrorKind::InvalidRequest,
                format!(
                    "Failed to decode audio file {}: built-in decoder cannot process this file (built-in decoder error: {builtin_failure_cause}) and {detail}",
                    filepath.display()
                ),
            ));
        }
        Err(err) => {
            let detail = if custom_ffmpeg_path.is_some_and(|p| !p.as_os_str().is_empty()) {
                format!("custom FFmpeg path is invalid: {err}")
            } else {
                "FFmpeg is not installed. Please install FFmpeg or use a supported format (MP3, WAV, M4A, AAC, FLAC, OGG).".to_string()
            };
            return Err(AsrPortError::new(
                AsrPortErrorKind::InvalidRequest,
                format!(
                    "Failed to decode audio file {}: built-in decoder cannot process this file (built-in decoder error: {builtin_failure_cause}) and {detail}",
                    filepath.display()
                ),
            ));
        }
    };

    run_ffmpeg_extract_and_resample(filepath, target_sample_rate, &ffmpeg_path).await
}

async fn run_ffmpeg_extract_and_resample(
    filepath: &Path,
    target_sample_rate: u32,
    ffmpeg_path: &Path,
) -> Result<Vec<f32>, AsrPortError> {
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
        .map_err(|error| {
            AsrPortError::new(
                AsrPortErrorKind::FileSystem,
                format!("Failed to run ffmpeg command: {error}"),
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AsrPortError::runtime(format!(
            "FFmpeg exited with {:?}: {stderr}",
            output.status
        )));
    }

    Ok(pcm_s16le_bytes_to_f32(&output.stdout))
}

pub async fn extract_audio_slice(
    filepath: &Path,
    start_seconds: f64,
    duration_seconds: f64,
    target_sample_rate: u32,
) -> Result<Vec<f32>, AsrPortError> {
    extract_audio_slice_with_ffmpeg(
        filepath,
        start_seconds,
        duration_seconds,
        target_sample_rate,
        None,
    )
    .await
}

pub async fn extract_audio_slice_with_ffmpeg(
    filepath: &Path,
    start_seconds: f64,
    duration_seconds: f64,
    target_sample_rate: u32,
    custom_ffmpeg_path: Option<&Path>,
) -> Result<Vec<f32>, AsrPortError> {
    extract_audio_slice_with_options(
        filepath,
        start_seconds,
        duration_seconds,
        target_sample_rate,
        true,
        custom_ffmpeg_path,
    )
    .await
}

pub async fn extract_audio_slice_with_options(
    filepath: &Path,
    start_seconds: f64,
    duration_seconds: f64,
    target_sample_rate: u32,
    ffmpeg_enabled: bool,
    custom_ffmpeg_path: Option<&Path>,
) -> Result<Vec<f32>, AsrPortError> {
    // 1. Prioritize built-in in-memory slice on blocking task
    let path_buf = filepath.to_path_buf();
    let builtin_result = tokio::task::spawn_blocking(move || {
        sona_core::audio::decode_audio_slice(
            &path_buf,
            start_seconds,
            duration_seconds,
            target_sample_rate,
        )
    })
    .await
    .map_err(|join_err| {
        AsrPortError::runtime(format!("Audio slice task join error: {join_err}"))
    })?;

    let builtin_failure_cause = match builtin_result {
        Ok(samples) => return Ok(samples),
        Err(sona_core::audio::AudioDecodeError::Io(io_err)) => {
            return Err(AsrPortError::new(
                AsrPortErrorKind::FileSystem,
                format!("Failed to read audio file {}: {io_err}", filepath.display()),
            ));
        }
        Err(builtin_err) => {
            log::info!(
                "Built-in slice decoder skipped {}: {builtin_err}. Attempting FFmpeg fallback...",
                filepath.display()
            );
            builtin_err.to_string()
        }
    };

    // 2. Fall back to FFmpeg only if enabled
    if !ffmpeg_enabled {
        return Err(AsrPortError::new(
            AsrPortErrorKind::InvalidRequest,
            format!(
                "Failed to extract audio slice from {}: built-in decoder cannot process this file (built-in decoder error: {builtin_failure_cause}) and FFmpeg is disabled in settings. Enable FFmpeg in settings to support extended formats or use a supported format (MP3, WAV, M4A, AAC, FLAC, OGG).",
                filepath.display()
            ),
        ));
    }

    let ffmpeg_path = match resolve_ffmpeg_path(custom_ffmpeg_path) {
        Ok(path) if path.is_file() => path,
        _ => {
            return Err(AsrPortError::new(
                AsrPortErrorKind::InvalidRequest,
                format!(
                    "Failed to extract audio slice from {}: built-in decoder cannot process this file (built-in decoder error: {builtin_failure_cause}) and FFmpeg is not installed. Please install FFmpeg or use a supported format (MP3, WAV, M4A, AAC, FLAC, OGG).",
                    filepath.display()
                ),
            ));
        }
    };

    run_ffmpeg_extract_slice(
        filepath,
        start_seconds,
        duration_seconds,
        target_sample_rate,
        &ffmpeg_path,
    )
    .await
}

async fn run_ffmpeg_extract_slice(
    filepath: &Path,
    start_seconds: f64,
    duration_seconds: f64,
    target_sample_rate: u32,
    ffmpeg_path: &Path,
) -> Result<Vec<f32>, AsrPortError> {
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
        .arg("-ss")
        .arg(format!("{:.3}", start_seconds.max(0.0)))
        .arg("-i")
        .arg(filepath)
        .arg("-t")
        .arg(format!("{:.3}", duration_seconds.max(0.0)))
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
        .map_err(|error| {
            AsrPortError::new(
                AsrPortErrorKind::FileSystem,
                format!("Failed to run ffmpeg command: {error}"),
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AsrPortError::runtime(format!(
            "FFmpeg exited with {:?}: {stderr}",
            output.status
        )));
    }

    Ok(pcm_s16le_bytes_to_f32(&output.stdout))
}

pub(crate) fn create_vad_config(
    vad_model: &Path,
    options: VadDetectorOptions,
) -> Result<VadConfig, AsrPortError> {
    let model_path = resolve_model_onnx_path(vad_model)?;
    let silero_vad = SileroVadModelConfig {
        model: Some(model_path.to_string_lossy().to_string()),
        threshold: options.threshold,
        min_silence_duration: options.min_silence_duration,
        min_speech_duration: options.min_speech_duration,
        window_size: options.window_size,
        max_speech_duration: options.max_speech_duration,
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
) -> Result<VadDetector, AsrPortError> {
    let vad_config = create_vad_config(vad_model, VadDetectorOptions::default())?;
    let detector_capacity_seconds = if detector_capacity_seconds > 0.0 {
        detector_capacity_seconds
    } else {
        60.0
    };
    VoiceActivityDetector::create(&vad_config, detector_capacity_seconds)
        .ok_or_else(|| AsrPortError::runtime("Failed to create VoiceActivityDetector"))
}

pub fn load_vad(vad_model: Option<String>) -> Option<SafeVad> {
    let v_path = vad_model?;

    if v_path.is_empty() {
        log::warn!(
            "[Sherpa] load_vad: Path is empty or does not exist: {}",
            v_path
        );
        return None;
    }

    match create_vad_detector(Path::new(&v_path), 60.0) {
        Ok(vad) => {
            log::info!("[Sherpa] load_vad: VAD successfully created!");
            Some(SafeVad(vad))
        }
        Err(error) => {
            log::warn!("[Sherpa] load_vad: {error}");
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
    while !vad.0.is_empty() {
        vad.0.pop();
    }
}

pub fn vad_detected(vad: &SafeVad) -> bool {
    vad.0.detected()
}

#[cfg(test)]
mod tests {
    use super::{LiveWavRecorder, resolve_model_onnx_path};
    use std::fs;

    #[test]
    fn live_wav_recorder_writes_clamped_samples_and_reports_path() {
        let filepath =
            std::env::temp_dir().join(format!("sona-live-recorder-{}.wav", uuid::Uuid::new_v4()));
        let mut recorder = LiveWavRecorder::create(&filepath, 16000).unwrap();

        recorder.write_samples(&[-2.0, 0.0, 0.5, 2.0]).unwrap();
        assert_eq!(recorder.filepath(), filepath.as_path());
        let finalized_path = recorder.finalize().unwrap();

        assert_eq!(finalized_path, filepath);
        let mut reader = hound::WavReader::open(&filepath).unwrap();
        let spec = reader.spec();
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.sample_rate, 16000);
        assert_eq!(spec.bits_per_sample, 16);
        assert_eq!(
            reader
                .samples::<i16>()
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            vec![-32767, 0, 16383, 32767]
        );

        fs::remove_file(filepath).unwrap();
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
        use sona_core::ports::asr::AsrPortErrorKind;

        let missing = std::env::temp_dir().join(format!("sona-missing-{}", uuid::Uuid::new_v4()));

        let error = resolve_model_onnx_path(&missing).unwrap_err();

        assert_eq!(error.kind, AsrPortErrorKind::Model);
        assert!(error.message.contains("Model path does not exist"));
    }

    #[test]
    fn vad_detector_options_defaults_max_speech_duration_to_thirty_seconds() {
        use super::VadDetectorOptions;
        let options = VadDetectorOptions::default();
        assert_eq!(options.max_speech_duration, 30.0);
    }

    #[test]
    fn create_vad_config_applies_max_speech_duration() {
        use super::{VadDetectorOptions, create_vad_config};
        let root = std::env::temp_dir().join(format!("sona-vad-model-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let onnx_path = root.join("silero_vad.onnx");
        fs::write(&onnx_path, "onnx").unwrap();

        let options = VadDetectorOptions {
            max_speech_duration: 25.0,
            ..Default::default()
        };
        let config = create_vad_config(&onnx_path, options).unwrap();
        assert_eq!(config.silero_vad.max_speech_duration, 25.0);

        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn test_extract_and_resample_audio_uses_builtin_decoder() {
        use super::{extract_and_resample_audio, save_wav_file};
        let wav_path = std::env::temp_dir().join(format!("sona-test-{}.wav", uuid::Uuid::new_v4()));
        let samples: Vec<f32> = (0..16000).map(|i| i as f32 / 16000.0 * 0.5).collect();
        save_wav_file(&samples, 16000, &wav_path).unwrap();

        let extracted = extract_and_resample_audio(&wav_path, 16000).await.unwrap();
        assert_eq!(extracted.len(), 16000);

        let _ = fs::remove_file(wav_path);
    }

    #[tokio::test]
    async fn test_extract_audio_slice_uses_builtin_decoder() {
        use super::{extract_audio_slice, save_wav_file};
        let wav_path =
            std::env::temp_dir().join(format!("sona-test-slice-{}.wav", uuid::Uuid::new_v4()));
        // 2 seconds of 16kHz
        let samples: Vec<f32> = (0..32000).map(|i| i as f32 / 16000.0 * 0.5).collect();
        save_wav_file(&samples, 16000, &wav_path).unwrap();

        // Extract 0.5s to 1.5s (1.0s duration = 16000 samples)
        let slice = extract_audio_slice(&wav_path, 0.5, 1.0, 16000)
            .await
            .unwrap();
        assert_eq!(slice.len(), 16000);

        let _ = fs::remove_file(wav_path);
    }
}
