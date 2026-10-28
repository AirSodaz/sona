use hound::{WavSpec, WavWriter};
use sona_core::ports::asr::{
    AsrPortError, AsrPortErrorKind, pcm_s16le_bytes_to_f32, resolve_ffmpeg_path,
};
use std::path::{Path, PathBuf};
use tokio::io::AsyncReadExt;

pub struct AudioStreamSourceConfig<'a> {
    pub url: &'a str,
    pub target_sample_rate: u32,
    pub ffmpeg_enabled: bool,
    pub custom_ffmpeg_path: Option<&'a Path>,
    pub save_to_path: Option<PathBuf>,
}

const MAX_IN_MEMORY_STREAM_BYTES: usize = 100 * 1024 * 1024;

pub enum AudioStreamReader {
    Ffmpeg {
        child: Box<tokio::process::Child>,
        stdout: tokio::io::BufReader<tokio::process::ChildStdout>,
        stderr_task: Option<tokio::task::JoinHandle<String>>,
        wav_writer: Option<WavWriter<std::io::BufWriter<std::fs::File>>>,
        carryover_byte: Option<u8>,
    },
    InMemory {
        samples: Vec<f32>,
        position: usize,
        wav_writer: Option<WavWriter<std::io::BufWriter<std::fs::File>>>,
    },
}

impl Drop for AudioStreamReader {
    fn drop(&mut self) {
        match self {
            AudioStreamReader::Ffmpeg {
                child,
                wav_writer,
                stderr_task,
                ..
            } => {
                if let Some(writer) = wav_writer.take() {
                    let _ = writer.finalize();
                }
                if let Some(task) = stderr_task.take() {
                    task.abort();
                }
                let _ = child.start_kill();
            }
            AudioStreamReader::InMemory { wav_writer, .. } => {
                if let Some(writer) = wav_writer.take() {
                    let _ = writer.finalize();
                }
            }
        }
    }
}

pub fn is_video_or_playlist_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    let path = lower
        .split('?')
        .next()
        .unwrap_or(&lower)
        .split('#')
        .next()
        .unwrap_or(&lower);
    path.ends_with(".m3u8")
        || path.ends_with(".mp4")
        || path.ends_with(".mkv")
        || path.ends_with(".avi")
        || path.ends_with(".mov")
        || path.ends_with(".flv")
        || path.ends_with(".webm")
        || path.ends_with(".ts")
}

fn create_wav_writer(
    path: &Path,
    sample_rate: u32,
) -> Result<WavWriter<std::io::BufWriter<std::fs::File>>, AsrPortError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            AsrPortError::new(
                AsrPortErrorKind::FileSystem,
                format!("Failed to create audio directory {}: {e}", parent.display()),
            )
        })?;
    }
    let file = std::fs::File::create(path).map_err(|e| {
        AsrPortError::new(
            AsrPortErrorKind::FileSystem,
            format!("Failed to create audio file {}: {e}", path.display()),
        )
    })?;
    let buf_writer = std::io::BufWriter::new(file);
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    WavWriter::new(buf_writer, spec).map_err(|e| {
        AsrPortError::new(
            AsrPortErrorKind::FileSystem,
            format!("Failed to initialize wav writer: {e}"),
        )
    })
}

impl AudioStreamReader {
    pub async fn open(config: AudioStreamSourceConfig<'_>) -> Result<Self, AsrPortError> {
        let sample_rate = config.target_sample_rate;
        let wav_writer = if let Some(save_path) = &config.save_to_path {
            Some(create_wav_writer(save_path, sample_rate)?)
        } else {
            None
        };

        if config.ffmpeg_enabled {
            let ffmpeg_path = resolve_ffmpeg_path(config.custom_ffmpeg_path).map_err(|e| {
                AsrPortError::new(
                    AsrPortErrorKind::InvalidRequest,
                    format!("FFmpeg is required for streaming but could not be resolved: {e}"),
                )
            })?;

            let mut command = tokio::process::Command::new(&ffmpeg_path);
            command.kill_on_drop(true);
            #[cfg(target_os = "windows")]
            {
                #[allow(unused_imports)]
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x0800_0000);
            }

            command.stdin(std::process::Stdio::null());
            command.arg("-nostdin");
            command.arg("-loglevel").arg("error");
            let trimmed_url = config.url.trim();
            let is_http = trimmed_url
                .get(..7)
                .is_some_and(|s| s.eq_ignore_ascii_case("http://"))
                || trimmed_url
                    .get(..8)
                    .is_some_and(|s| s.eq_ignore_ascii_case("https://"));
            if is_http {
                command
                    .arg("-reconnect")
                    .arg("1")
                    .arg("-reconnect_streamed")
                    .arg("1")
                    .arg("-reconnect_on_network_error")
                    .arg("1")
                    .arg("-reconnect_delay_max")
                    .arg("5");
            }
            if is_http
                || trimmed_url
                    .get(..7)
                    .is_some_and(|s| s.eq_ignore_ascii_case("rtmp://"))
                || trimmed_url
                    .get(..7)
                    .is_some_and(|s| s.eq_ignore_ascii_case("rtsp://"))
            {
                command.arg("-rw_timeout").arg("15000000");
            }
            command
                .arg("-i")
                .arg(trimmed_url)
                .arg("-vn")
                .arg("-f")
                .arg("s16le")
                .arg("-acodec")
                .arg("pcm_s16le")
                .arg("-ar")
                .arg(sample_rate.to_string())
                .arg("-ac")
                .arg("1")
                .arg("-")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());

            let mut child = command.spawn().map_err(|e| {
                AsrPortError::new(
                    AsrPortErrorKind::FileSystem,
                    format!("Failed to spawn ffmpeg stream process: {e}"),
                )
            })?;

            let stdout = child.stdout.take().ok_or_else(|| {
                AsrPortError::runtime("Failed to open child process stdout for ffmpeg stream")
            })?;

            let stderr_task = child.stderr.take().map(|mut stderr| {
                tokio::spawn(async move {
                    use tokio::io::AsyncReadExt;
                    let mut buffer = [0u8; 4096];
                    let mut collected = Vec::new();
                    const MAX_STDERR_BYTES: usize = 64 * 1024;
                    while let Ok(n) = stderr.read(&mut buffer).await {
                        if n == 0 {
                            break;
                        }
                        if collected.len() + n > MAX_STDERR_BYTES {
                            let excess = (collected.len() + n) - MAX_STDERR_BYTES;
                            if excess < collected.len() {
                                collected.drain(0..excess);
                            } else {
                                collected.clear();
                            }
                        }
                        collected.extend_from_slice(&buffer[..n]);
                    }
                    String::from_utf8_lossy(&collected).to_string()
                })
            });

            Ok(AudioStreamReader::Ffmpeg {
                child: Box::new(child),
                stdout: tokio::io::BufReader::new(stdout),
                stderr_task,
                wav_writer,
                carryover_byte: None,
            })
        } else {
            let trimmed_url = config.url.trim();
            let is_http = trimmed_url
                .get(..7)
                .is_some_and(|s| s.eq_ignore_ascii_case("http://"))
                || trimmed_url
                    .get(..8)
                    .is_some_and(|s| s.eq_ignore_ascii_case("https://"));
            if !is_http {
                return Err(AsrPortError::new(
                    AsrPortErrorKind::InvalidRequest,
                    format!(
                        "RTSP and RTMP streams require FFmpeg. Please enable FFmpeg in settings: {trimmed_url}"
                    ),
                ));
            }
            if is_video_or_playlist_url(trimmed_url) {
                return Err(AsrPortError::new(
                    AsrPortErrorKind::InvalidRequest,
                    "Video stream or HLS playlist requires FFmpeg. Please enable FFmpeg in settings.",
                ));
            }

            let client = reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(15))
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .map_err(|e| AsrPortError::runtime(format!("Failed to build HTTP client: {e}")))?;

            let mut response = client.get(trimmed_url).send().await.map_err(|e| {
                AsrPortError::new(
                    AsrPortErrorKind::InvalidRequest,
                    format!("Failed to fetch stream URL: {e}"),
                )
            })?;
            if !response.status().is_success() {
                return Err(AsrPortError::new(
                    AsrPortErrorKind::InvalidRequest,
                    format!(
                        "Stream URL returned non-success status {}: {}",
                        response.status(),
                        config.url
                    ),
                ));
            }

            if let Some(content_length) = response.content_length()
                && content_length > MAX_IN_MEMORY_STREAM_BYTES as u64
            {
                return Err(AsrPortError::new(
                    AsrPortErrorKind::InvalidRequest,
                    format!(
                        "Stream content length ({content_length} bytes) exceeds maximum allowed in-memory limit ({MAX_IN_MEMORY_STREAM_BYTES} bytes)"
                    ),
                ));
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|e| AsrPortError::runtime(format!("Failed to read stream chunk: {e}")))?
            {
                if bytes.len() + chunk.len() > MAX_IN_MEMORY_STREAM_BYTES {
                    return Err(AsrPortError::new(
                        AsrPortErrorKind::InvalidRequest,
                        format!(
                            "Stream size exceeds maximum allowed in-memory limit ({MAX_IN_MEMORY_STREAM_BYTES} bytes)"
                        ),
                    ));
                }
                bytes.extend_from_slice(&chunk);
            }

            let hint_ext = config
                .url
                .split('?')
                .next()
                .and_then(|p| p.split('#').next())
                .and_then(|p| p.rsplit('/').next())
                .and_then(|filename| filename.rsplit_once('.'))
                .map(|(_, ext)| ext.to_string());
            let decoded_samples = tokio::task::spawn_blocking(move || {
                sona_core::audio::decode_audio_bytes(bytes, hint_ext.as_deref(), sample_rate)
            })
            .await
            .map_err(|e| AsrPortError::runtime(format!("Audio decode task join error: {e}")))?
            .map_err(|e| {
                AsrPortError::new(
                    AsrPortErrorKind::InvalidRequest,
                    format!("Failed to decode audio stream: {e}"),
                )
            })?;

            Ok(AudioStreamReader::InMemory {
                samples: decoded_samples,
                position: 0,
                wav_writer,
            })
        }
    }

    pub async fn next_samples(
        &mut self,
        chunk_size: usize,
    ) -> Result<Option<Vec<f32>>, AsrPortError> {
        match self {
            AudioStreamReader::Ffmpeg {
                child,
                stdout,
                stderr_task,
                wav_writer,
                carryover_byte,
            } => {
                let mut buf = vec![0u8; chunk_size * 2];
                let mut offset = 0;
                if let Some(byte) = carryover_byte.take() {
                    buf[0] = byte;
                    offset = 1;
                }

                while offset < buf.len() {
                    let n = stdout.read(&mut buf[offset..]).await.map_err(|e| {
                        AsrPortError::runtime(format!("Error reading from ffmpeg stream: {e}"))
                    })?;
                    if n == 0 {
                        break;
                    }
                    offset += n;
                }

                if offset < 2 {
                    let wait_result =
                        tokio::time::timeout(std::time::Duration::from_secs(2), child.wait()).await;
                    if let Ok(Ok(status)) = wait_result {
                        if !status.success() {
                            let stderr_output = if let Some(task) = stderr_task.take() {
                                task.await.unwrap_or_default()
                            } else {
                                String::new()
                            };
                            let stderr_trimmed = stderr_output.trim();
                            return Err(AsrPortError::new(
                                AsrPortErrorKind::Runtime,
                                if stderr_trimmed.is_empty() {
                                    format!("FFmpeg stream process exited with status {status}")
                                } else {
                                    format!("FFmpeg stream error ({status}): {stderr_trimmed}")
                                },
                            ));
                        }
                    } else if wait_result.is_err() {
                        let _ = child.start_kill();
                    }
                    return Ok(None);
                }
                let total_bytes = offset;
                let usable_bytes = total_bytes - (total_bytes % 2);
                if total_bytes % 2 != 0 {
                    *carryover_byte = Some(buf[total_bytes - 1]);
                }

                let samples = pcm_s16le_bytes_to_f32(&buf[..usable_bytes]);
                if let Some(writer) = wav_writer.as_mut() {
                    for chunk in buf[..usable_bytes].as_chunks::<2>().0 {
                        let sample = i16::from_le_bytes(*chunk);
                        if let Err(e) = writer.write_sample(sample) {
                            log::warn!("Failed to write sample to wav writer: {e}");
                            break;
                        }
                    }
                }
                Ok(Some(samples))
            }
            AudioStreamReader::InMemory {
                samples,
                position,
                wav_writer,
            } => {
                if *position >= samples.len() {
                    return Ok(None);
                }

                let end = (*position + chunk_size).min(samples.len());
                let slice = samples[*position..end].to_vec();
                *position = end;

                if let Some(writer) = wav_writer.as_mut() {
                    for &sample in &slice {
                        let i16_sample = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                        let _ = writer.write_sample(i16_sample);
                    }
                }

                Ok(Some(slice))
            }
        }
    }

    pub async fn take_stderr_output(&mut self) -> Option<String> {
        match self {
            AudioStreamReader::Ffmpeg {
                stderr_task, child, ..
            } => {
                let _ = child.start_kill();
                if let Some(task) = stderr_task.take() {
                    let _ =
                        tokio::time::timeout(std::time::Duration::from_millis(500), child.wait())
                            .await;
                    tokio::time::timeout(std::time::Duration::from_millis(500), task)
                        .await
                        .ok()
                        .and_then(|res| res.ok())
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                } else {
                    None
                }
            }
            AudioStreamReader::InMemory { .. } => None,
        }
    }

    pub fn finish(&mut self) {
        match self {
            AudioStreamReader::Ffmpeg {
                child,
                wav_writer,
                stderr_task,
                ..
            } => {
                let _ = child.start_kill();
                if let Some(writer) = wav_writer.take() {
                    let _ = writer.finalize();
                }
                if let Some(task) = stderr_task.take() {
                    task.abort();
                }
            }
            AudioStreamReader::InMemory { wav_writer, .. } => {
                if let Some(writer) = wav_writer.take() {
                    let _ = writer.finalize();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_video_or_playlist_url() {
        assert!(is_video_or_playlist_url(
            "https://example.com/live/playlist.m3u8?token=xyz"
        ));
        assert!(is_video_or_playlist_url("http://example.com/video.mp4"));
        assert!(is_video_or_playlist_url(
            "http://example.com/video.mkv#t=10"
        ));
        assert!(!is_video_or_playlist_url("https://example.com/audio.mp3"));
        assert!(!is_video_or_playlist_url("https://example.com/stream.wav"));
        assert!(!is_video_or_playlist_url(
            "https://example.com/podcast.flac?auth=1"
        ));
    }

    #[tokio::test]
    async fn test_in_memory_stream_chunking_and_tee() {
        let sample_rate = 16000;
        let samples = vec![0.1f32, 0.2, -0.3, 0.4, 0.5, -0.6, 0.7];
        let temp_dir = tempfile::tempdir().unwrap();
        let save_path = temp_dir.path().join("tee.wav");

        let mut reader = AudioStreamReader::InMemory {
            samples: samples.clone(),
            position: 0,
            wav_writer: Some(create_wav_writer(&save_path, sample_rate).unwrap()),
        };

        let chunk1 = reader.next_samples(4).await.unwrap().unwrap();
        assert_eq!(chunk1, vec![0.1f32, 0.2, -0.3, 0.4]);

        let chunk2 = reader.next_samples(4).await.unwrap().unwrap();
        assert_eq!(chunk2, vec![0.5f32, -0.6, 0.7]);

        let chunk3 = reader.next_samples(4).await.unwrap();
        assert!(chunk3.is_none());

        reader.finish();
        assert!(save_path.exists());
        let mut wav_reader = hound::WavReader::open(&save_path).unwrap();
        let read_samples: Vec<i16> = wav_reader.samples::<i16>().map(|s| s.unwrap()).collect();
        assert_eq!(read_samples.len(), samples.len());
    }
}
