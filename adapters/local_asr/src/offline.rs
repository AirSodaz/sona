use crate::audio::{
    VadDetectorOptions, create_vad_config, extract_and_resample_audio, fixed_chunk_audio,
    save_wav_file, vad_segment_audio,
};
use crate::gpu::{GpuFallbackNotice, resolve_gpu_acceleration_plan};
use crate::punctuation::{Punctuation, load_punctuation_from_path};
use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig, VadModelConfig};
use sona_core::model_config::ModelFileConfig;
use sona_core::transcribe_runtime::OfflineTranscribePlan;
use sona_core::transcript::{TranscriptSegment, ensure_transcript_segment_timing};
use std::path::{Path, PathBuf};

pub async fn run_offline_transcription(
    plan: OfflineTranscribePlan,
) -> Result<Vec<TranscriptSegment>, String> {
    let adapter = LocalOfflineTranscriber::from_plan(plan)?;
    adapter.transcribe().await
}

#[derive(Debug, Clone)]
struct LocalOfflineTranscriber {
    input_path: PathBuf,
    save_to_path: Option<PathBuf>,
    model_path: PathBuf,
    num_threads: i32,
    enable_itn: bool,
    language: String,
    punctuation_model: Option<PathBuf>,
    vad_model: Option<PathBuf>,
    vad_buffer: f32,
    model_type: String,
    file_config: Option<ModelFileConfig>,
    hotwords: Option<String>,
    gpu_acceleration: Option<String>,
    quiet: bool,
}

impl LocalOfflineTranscriber {
    fn from_plan(plan: OfflineTranscribePlan) -> Result<Self, String> {
        if !plan.input_path.is_file() {
            return Err(format!(
                "Input file must be an existing file: {}",
                plan.input_path.display()
            ));
        }

        Ok(Self {
            input_path: plan.input_path,
            save_to_path: plan.save_to_path,
            model_path: PathBuf::from(plan.model_path),
            num_threads: plan.num_threads,
            enable_itn: plan.enable_itn,
            language: plan.language,
            punctuation_model: plan.punctuation_model.map(PathBuf::from),
            vad_model: plan.vad_model.map(PathBuf::from),
            vad_buffer: plan.vad_buffer,
            model_type: plan.model_type,
            file_config: plan.file_config,
            hotwords: plan.hotwords,
            gpu_acceleration: plan.gpu_acceleration,
            quiet: plan.quiet,
        })
    }

    async fn transcribe(self) -> Result<Vec<TranscriptSegment>, String> {
        let gpu_plan = resolve_gpu_acceleration_plan(self.gpu_acceleration.as_deref()).await;
        let mut last_error = None;
        let mut fallback_notice: Option<GpuFallbackNotice> = None;

        for provider in gpu_plan.provider_options() {
            match self.transcribe_with_provider(provider.as_deref()).await {
                Ok(segments) => {
                    if let Some(notice) = fallback_notice.take()
                        && !self.quiet
                    {
                        eprintln!(
                            "DirectML transcription failed, retrying with CPU: {}",
                            notice.error
                        );
                    }
                    return Ok(segments);
                }
                Err(error)
                    if provider
                        .as_deref()
                        .map(|provider| gpu_plan.should_retry_after_failure(provider))
                        .unwrap_or(false) =>
                {
                    fallback_notice = Some(GpuFallbackNotice::directml_retry(error.clone()));
                    last_error = Some(error);
                }
                Err(error) => return Err(error),
            }
        }

        Err(last_error.unwrap_or_else(|| "Recognizer creation failed.".to_string()))
    }

    async fn transcribe_with_provider(
        &self,
        provider: Option<&str>,
    ) -> Result<Vec<TranscriptSegment>, String> {
        let model_type = build_model_config(
            &self.model_path,
            &self.model_type,
            &self.file_config,
            self.enable_itn,
            &self.language,
            self.hotwords.clone(),
        )?;

        let recognizer =
            create_recognizer(model_type, self.num_threads, provider, &self.file_config)?;
        let punctuation = load_punctuation_from_path(self.punctuation_model.as_deref())?;
        let vad_config = load_vad_config(self.vad_model.as_deref())?;

        let samples = extract_and_resample_audio(&self.input_path, 16000).await?;
        if let Some(path) = self.save_to_path.as_ref() {
            save_wav_file(&samples, 16000, path).map_err(|error| error.to_string())?;
        }

        transcribe_samples(
            &samples,
            &recognizer,
            punctuation.as_ref(),
            vad_config.as_ref(),
            self.vad_buffer,
        )
    }
}

#[derive(Debug, Clone)]
enum ModelType {
    OfflineSenseVoice {
        model: PathBuf,
        language: String,
        use_itn: bool,
    },
    OfflineWhisper {
        encoder: PathBuf,
        decoder: PathBuf,
        language: String,
    },
    OfflineFunASRNano {
        encoder_adaptor: PathBuf,
        llm: PathBuf,
        embedding: PathBuf,
        tokenizer: PathBuf,
        tokens: Option<PathBuf>,
        language: String,
    },
    OfflineFireRedAsr {
        encoder: PathBuf,
        decoder: PathBuf,
    },
    OfflineDolphin {
        model: PathBuf,
    },
    OfflineQwen3Asr {
        conv_frontend: PathBuf,
        encoder: PathBuf,
        decoder: PathBuf,
        tokenizer: PathBuf,
        hotwords: Option<String>,
    },
}

fn build_model_config(
    model_path: &Path,
    model_type: &str,
    file_config: &Option<ModelFileConfig>,
    enable_itn: bool,
    language: &str,
    hotwords: Option<String>,
) -> Result<ModelType, String> {
    let fc = file_config
        .as_ref()
        .ok_or("File configuration is missing for this model.")?;

    let get_path = |filename: &Option<String>| -> Result<PathBuf, String> {
        let name = filename
            .as_ref()
            .ok_or("Required file name not specified in config")?;
        Ok(model_path.join(name))
    };

    match model_type {
        "sensevoice" => Ok(ModelType::OfflineSenseVoice {
            model: get_path(&fc.model)?,
            language: language.to_string(),
            use_itn: enable_itn,
        }),
        "whisper" => Ok(ModelType::OfflineWhisper {
            encoder: get_path(&fc.encoder)?,
            decoder: get_path(&fc.decoder)?,
            language: if language == "auto" {
                String::new()
            } else {
                language.to_string()
            },
        }),
        "funasr-nano" => Ok(ModelType::OfflineFunASRNano {
            encoder_adaptor: get_path(&fc.encoder_adaptor)?,
            llm: get_path(&fc.llm)?,
            embedding: get_path(&fc.embedding)?,
            tokenizer: get_path(&fc.tokenizer)?,
            tokens: fc
                .tokens
                .as_ref()
                .map(|_| get_path(&fc.tokens))
                .transpose()?,
            language: if language == "multilingual" {
                String::new()
            } else {
                language.to_string()
            },
        }),
        "fire-red-asr" => Ok(ModelType::OfflineFireRedAsr {
            encoder: get_path(&fc.encoder)?,
            decoder: get_path(&fc.decoder)?,
        }),
        "dolphin" => Ok(ModelType::OfflineDolphin {
            model: get_path(&fc.model)?,
        }),
        "qwen3-asr" => Ok(ModelType::OfflineQwen3Asr {
            conv_frontend: get_path(&fc.conv_frontend)?,
            encoder: get_path(&fc.encoder)?,
            decoder: get_path(&fc.decoder)?,
            tokenizer: get_path(&fc.tokenizer)?,
            hotwords,
        }),
        other => Err(format!("Unsupported offline model type: {other}")),
    }
}

fn file_tokens(file_config: &Option<ModelFileConfig>) -> Option<String> {
    file_config
        .as_ref()
        .and_then(|config| config.tokens.as_ref())
        .map(|path| path.to_string())
}

fn create_recognizer(
    model_type: ModelType,
    num_threads: i32,
    provider: Option<&str>,
    file_config: &Option<ModelFileConfig>,
) -> Result<OfflineRecognizer, String> {
    let mut config = OfflineRecognizerConfig::default();
    config.model_config.num_threads = num_threads;
    config.model_config.provider = Some(provider.unwrap_or("cpu").to_string());
    config.feat_config.sample_rate = 16000;
    config.feat_config.feature_dim = 80;

    match model_type {
        ModelType::OfflineSenseVoice {
            model,
            language,
            use_itn,
        } => {
            config.model_config.tokens = file_tokens(file_config);
            config.model_config.sense_voice.model = Some(model.to_string_lossy().to_string());
            config.model_config.sense_voice.language = Some(language);
            config.model_config.sense_voice.use_itn = use_itn;
        }
        ModelType::OfflineWhisper {
            encoder,
            decoder,
            language,
        } => {
            config.model_config.tokens = file_tokens(file_config);
            config.model_config.whisper.encoder = Some(encoder.to_string_lossy().to_string());
            config.model_config.whisper.decoder = Some(decoder.to_string_lossy().to_string());
            config.model_config.whisper.language = Some(language);
        }
        ModelType::OfflineFunASRNano {
            encoder_adaptor,
            llm,
            embedding,
            tokenizer,
            tokens,
            language,
        } => {
            config.model_config.tokens = tokens
                .as_ref()
                .map(|path| path.to_string_lossy().to_string())
                .or_else(|| file_tokens(file_config));
            config.model_config.funasr_nano.encoder_adaptor =
                Some(encoder_adaptor.to_string_lossy().to_string());
            config.model_config.funasr_nano.llm = Some(llm.to_string_lossy().to_string());
            config.model_config.funasr_nano.embedding =
                Some(embedding.to_string_lossy().to_string());
            config.model_config.funasr_nano.tokenizer =
                Some(tokenizer.to_string_lossy().to_string());
            config.model_config.funasr_nano.language = Some(language);
        }
        ModelType::OfflineFireRedAsr { encoder, decoder } => {
            config.model_config.tokens = file_tokens(file_config);
            config.model_config.fire_red_asr.encoder = Some(encoder.to_string_lossy().to_string());
            config.model_config.fire_red_asr.decoder = Some(decoder.to_string_lossy().to_string());
        }
        ModelType::OfflineDolphin { model } => {
            config.model_config.tokens = file_tokens(file_config);
            config.model_config.dolphin.model = Some(model.to_string_lossy().to_string());
        }
        ModelType::OfflineQwen3Asr {
            conv_frontend,
            encoder,
            decoder,
            tokenizer,
            hotwords,
        } => {
            config.model_config.qwen3_asr.conv_frontend =
                Some(conv_frontend.to_string_lossy().to_string());
            config.model_config.qwen3_asr.encoder = Some(encoder.to_string_lossy().to_string());
            config.model_config.qwen3_asr.decoder = Some(decoder.to_string_lossy().to_string());
            config.model_config.qwen3_asr.tokenizer = Some(tokenizer.to_string_lossy().to_string());
            config.model_config.qwen3_asr.hotwords = hotwords;
        }
    }

    OfflineRecognizer::create(&config)
        .ok_or_else(|| "Failed to create OfflineRecognizer".to_string())
}

fn load_vad_config(vad_model: Option<&Path>) -> Result<Option<VadModelConfig>, String> {
    let Some(path) = vad_model else {
        return Ok(None);
    };

    create_vad_config(path, VadDetectorOptions::default()).map(Some)
}

fn transcribe_samples(
    samples: &[f32],
    recognizer: &OfflineRecognizer,
    punctuation: Option<&Punctuation>,
    vad_config: Option<&VadModelConfig>,
    vad_buffer: f32,
) -> Result<Vec<TranscriptSegment>, String> {
    let audio_segments = if let Some(vad_config) = vad_config {
        vad_segment_audio(samples, 16000, vad_config, vad_buffer)
            .unwrap_or_else(|_| fixed_chunk_audio(samples, 16000, 30.0))
    } else {
        fixed_chunk_audio(samples, 16000, 30.0)
    };

    let mut results = Vec::new();
    for segment in audio_segments {
        let stream = recognizer.create_stream();
        stream.accept_waveform(16000, &segment.samples);
        recognizer.decode(&stream);

        if let Some(result) = stream.get_result() {
            let cleaned_text = normalize_recognizer_text(&result.text);
            if cleaned_text.is_empty() {
                continue;
            }

            let text = finalize_transcript_text(&cleaned_text, punctuation);
            if text.is_empty() {
                continue;
            }

            let timestamps_abs = result.timestamps.as_ref().map(|timestamps| {
                timestamps
                    .iter()
                    .map(|timestamp| *timestamp + segment.start_time)
                    .collect::<Vec<_>>()
            });
            let durations = timestamps_abs
                .as_ref()
                .and_then(|timestamps| synthesize_durations(timestamps, segment.end_time()));

            let mut transcript_segment = TranscriptSegment {
                id: uuid::Uuid::new_v4().to_string(),
                text,
                start: segment.start_time as f64,
                end: segment.end_time() as f64,
                is_final: true,
                timing: None,
                tokens: Some(result.tokens),
                timestamps: timestamps_abs,
                durations,
                translation: None,
                speaker: None,
                speaker_attribution: None,
            };

            ensure_transcript_segment_timing(&mut transcript_segment);
            results.push(transcript_segment);
        }
    }

    Ok(results)
}

fn normalize_recognizer_text(text: &str) -> String {
    let mut result = text.trim();

    while result.starts_with("<|") && result.contains("|>") {
        let Some(tag_end) = result.find("|>") else {
            break;
        };
        result = result[tag_end + 2..].trim();
    }

    result.trim().to_string()
}

fn finalize_transcript_text(cleaned_text: &str, punctuation: Option<&Punctuation>) -> String {
    let mut result = cleaned_text.trim().to_string();
    if result.is_empty() {
        return result;
    }

    if let Some(punctuation) = punctuation {
        result = punctuation.add_punct(&result);
    }

    result
}

fn synthesize_durations(timestamps: &[f32], end_time: f32) -> Option<Vec<f32>> {
    if timestamps.is_empty() {
        return None;
    }

    let mut durations = Vec::with_capacity(timestamps.len());
    for index in 0..timestamps.len() {
        let next_time = if index + 1 < timestamps.len() {
            timestamps[index + 1]
        } else {
            end_time
        };
        durations.push(next_time - timestamps[index]);
    }

    Some(durations)
}

#[cfg(test)]
mod tests {
    use super::run_offline_transcription;
    use sona_core::export::ExportFormat;
    use sona_core::transcribe_runtime::{OfflineTranscribePlan, OutputTarget};
    use std::path::PathBuf;

    #[tokio::test]
    async fn offline_transcription_rejects_missing_input_file() {
        let plan = OfflineTranscribePlan {
            input_path: PathBuf::from("missing.wav"),
            save_to_path: None,
            model_path: "C:/models/demo".to_string(),
            num_threads: 4,
            enable_itn: false,
            language: "auto".to_string(),
            punctuation_model: None,
            vad_model: None,
            vad_buffer: 5.0,
            model_type: "whisper".to_string(),
            file_config: None,
            hotwords: None,
            gpu_acceleration: Some("cpu".to_string()),
            export_format: ExportFormat::Json,
            output_target: OutputTarget::Stdout,
            quiet: true,
        };

        let error = run_offline_transcription(plan).await.unwrap_err();
        assert!(error.contains("existing file"));
    }
}
