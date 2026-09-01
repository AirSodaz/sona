use hound::{SampleFormat, WavSpec, WavWriter};
use sherpa_onnx::{SileroVadModelConfig, VadModelConfig, VoiceActivityDetector};
use sona_core::ports::asr::{AsrPortError, AsrPortErrorKind};
pub use sona_core::ports::asr::{
    pcm_i16_to_f32, pcm_s16le_bytes_to_f32, resolve_ffmpeg_path, resolve_ffmpeg_path_from_exe,
    resolve_ffmpeg_sidecar_path, resolve_ffmpeg_sidecar_path_from_exe,
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
    let ffmpeg_path = resolve_ffmpeg_path(custom_ffmpeg_path)?;
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
}
