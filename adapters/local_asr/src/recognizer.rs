use crate::gpu::{GpuAccelerationPlan, GpuFallbackNotice};
use log::{debug, info};
use sherpa_onnx::{
    OfflineRecognizer, OfflineRecognizerConfig, OnlineRecognizer, OnlineRecognizerConfig,
};
use sona_core::models::config::ModelFileConfig;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum ModelType {
    OnlineTransducer {
        encoder: PathBuf,
        decoder: PathBuf,
        joiner: PathBuf,
        tokens: PathBuf,
        hotwords: Option<String>,
    },
    OnlineParaformer {
        encoder: PathBuf,
        decoder: PathBuf,
        tokens: PathBuf,
    },
    OfflineSenseVoice {
        model: PathBuf,
        tokens: PathBuf,
        language: String,
        use_itn: bool,
    },
    OfflineWhisper {
        encoder: PathBuf,
        decoder: PathBuf,
        tokens: PathBuf,
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
        tokens: PathBuf,
    },
    OfflineDolphin {
        model: PathBuf,
        tokens: PathBuf,
    },
    OfflineQwen3Asr {
        conv_frontend: PathBuf,
        encoder: PathBuf,
        decoder: PathBuf,
        tokenizer: PathBuf,
        hotwords: Option<String>,
    },
}

impl ModelType {
    fn model_type_name(&self) -> &'static str {
        match self {
            ModelType::OnlineTransducer { .. } => "zipformer",
            ModelType::OnlineParaformer { .. } => "paraformer",
            ModelType::OfflineSenseVoice { .. } => "sensevoice",
            ModelType::OfflineWhisper { .. } => "whisper",
            ModelType::OfflineFunASRNano { .. } => "funasr-nano",
            ModelType::OfflineFireRedAsr { .. } => "fire-red-asr",
            ModelType::OfflineDolphin { .. } => "dolphin",
            ModelType::OfflineQwen3Asr { .. } => "qwen3-asr",
        }
    }

    fn is_offline(&self) -> bool {
        matches!(
            self,
            ModelType::OfflineSenseVoice { .. }
                | ModelType::OfflineWhisper { .. }
                | ModelType::OfflineFunASRNano { .. }
                | ModelType::OfflineFireRedAsr { .. }
                | ModelType::OfflineDolphin { .. }
                | ModelType::OfflineQwen3Asr { .. }
        )
    }
}

fn is_offline_model_type(model_type: &str) -> bool {
    matches!(
        model_type,
        "sensevoice" | "whisper" | "funasr-nano" | "fire-red-asr" | "dolphin" | "qwen3-asr"
    )
}

/// Resolves one installed model directory plus its file manifest into the
/// concrete Sherpa recognizer variant needed by the runtime.
pub fn build_model_config(
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
        "zipformer" => {
            let encoder = get_path(&fc.encoder)?;
            let decoder = get_path(&fc.decoder)?;
            let joiner = get_path(&fc.joiner)?;
            let tokens = get_path(&fc.tokens)?;
            Ok(ModelType::OnlineTransducer {
                encoder,
                decoder,
                joiner,
                tokens,
                hotwords,
            })
        }
        "paraformer" => {
            let encoder = get_path(&fc.encoder)?;
            let decoder = get_path(&fc.decoder)?;
            let tokens = get_path(&fc.tokens)?;
            Ok(ModelType::OnlineParaformer {
                encoder,
                decoder,
                tokens,
            })
        }
        "sensevoice" => {
            let model = get_path(&fc.model)?;
            let tokens = get_path(&fc.tokens)?;
            Ok(ModelType::OfflineSenseVoice {
                model,
                tokens,
                language: language.to_string(),
                use_itn: enable_itn,
            })
        }
        "whisper" => {
            let encoder = get_path(&fc.encoder)?;
            let decoder = get_path(&fc.decoder)?;
            let tokens = get_path(&fc.tokens)?;
            let language = if language == "auto" { "" } else { language }.to_string();
            Ok(ModelType::OfflineWhisper {
                encoder,
                decoder,
                tokens,
                language,
            })
        }
        "funasr-nano" => {
            let encoder_adaptor = get_path(&fc.encoder_adaptor)?;
            let llm = get_path(&fc.llm)?;
            let embedding = get_path(&fc.embedding)?;
            let tokenizer = get_path(&fc.tokenizer)?;
            let tokens = fc
                .tokens
                .as_ref()
                .map(|_| get_path(&fc.tokens))
                .transpose()?;
            let language = if language == "multilingual" {
                ""
            } else {
                language
            }
            .to_string();
            Ok(ModelType::OfflineFunASRNano {
                encoder_adaptor,
                llm,
                embedding,
                tokenizer,
                tokens,
                language,
            })
        }
        "fire-red-asr" => {
            let encoder = get_path(&fc.encoder)?;
            let decoder = get_path(&fc.decoder)?;
            let tokens = get_path(&fc.tokens)?;
            Ok(ModelType::OfflineFireRedAsr {
                encoder,
                decoder,
                tokens,
            })
        }
        "dolphin" => {
            let model = get_path(&fc.model)?;
            let tokens = get_path(&fc.tokens)?;
            Ok(ModelType::OfflineDolphin { model, tokens })
        }
        "qwen3-asr" => {
            let conv_frontend = get_path(&fc.conv_frontend)?;
            let encoder = get_path(&fc.encoder)?;
            let decoder = get_path(&fc.decoder)?;
            let tokenizer = get_path(&fc.tokenizer)?;
            Ok(ModelType::OfflineQwen3Asr {
                conv_frontend,
                encoder,
                decoder,
                tokenizer,
                hotwords,
            })
        }
        _ => Err(format!("Unsupported model type: {}", model_type)),
    }
}

pub fn build_offline_model_config(
    model_path: &Path,
    model_type: &str,
    file_config: &Option<ModelFileConfig>,
    enable_itn: bool,
    language: &str,
    hotwords: Option<String>,
) -> Result<ModelType, String> {
    if !is_offline_model_type(model_type) {
        return Err(format!("Unsupported offline model type: {model_type}"));
    }

    build_model_config(
        model_path,
        model_type,
        file_config,
        enable_itn,
        language,
        hotwords,
    )
}

pub struct SafeOnlineRecognizer(OnlineRecognizer);
unsafe impl Send for SafeOnlineRecognizer {}
unsafe impl Sync for SafeOnlineRecognizer {}

pub struct SafeOfflineRecognizer(OfflineRecognizer);
unsafe impl Send for SafeOfflineRecognizer {}
unsafe impl Sync for SafeOfflineRecognizer {}

#[derive(Debug, Clone, PartialEq)]
pub struct OfflineDecodeResult {
    pub text: String,
    pub tokens: Vec<String>,
    pub timestamps: Option<Vec<f32>>,
}

impl OfflineDecodeResult {
    pub fn is_empty_text(&self) -> bool {
        self.text.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OnlineDecodeResult {
    pub text: String,
    pub tokens: Vec<String>,
    pub timestamps: Option<Vec<f32>>,
}

pub enum RecognizerInner {
    Online(SafeOnlineRecognizer),
    Offline(SafeOfflineRecognizer),
}

pub struct Recognizer {
    pub inner: RecognizerInner,
}

pub struct RecognizerCreateResult {
    pub recognizer: Recognizer,
    pub provider: Option<String>,
    pub fallback_notice: Option<GpuFallbackNotice>,
}

fn get_base_online_config(
    num_threads: i32,
    tokens: &Path,
    provider: Option<String>,
) -> OnlineRecognizerConfig {
    let mut config = OnlineRecognizerConfig {
        rule1_min_trailing_silence: 1.2,
        rule2_min_trailing_silence: 1.2,
        rule3_min_utterance_length: 300.0,
        ..Default::default()
    };
    config.model_config.tokens = Some(tokens.to_string_lossy().to_string());
    config.model_config.num_threads = num_threads;
    config.model_config.provider = Some(provider.unwrap_or_else(|| "cpu".to_string()));
    config.model_config.model_type = Some("paraformer".to_string());
    config.feat_config.sample_rate = 16000;
    config.feat_config.feature_dim = 80;
    config.enable_endpoint = true;
    config
}

fn get_base_offline_config(
    num_threads: i32,
    tokens: Option<&Path>,
    provider: Option<String>,
) -> OfflineRecognizerConfig {
    let mut config = OfflineRecognizerConfig::default();
    config.model_config.tokens = tokens.map(|path| path.to_string_lossy().to_string());
    config.model_config.num_threads = num_threads;
    config.model_config.provider = Some(provider.unwrap_or_else(|| "cpu".to_string()));
    config.feat_config.sample_rate = 16000;
    config.feat_config.feature_dim = 80;
    config
}

impl Recognizer {
    pub fn kind_label(&self) -> &'static str {
        match &self.inner {
            RecognizerInner::Online(_) => "online",
            RecognizerInner::Offline(_) => "offline",
        }
    }

    pub fn new(
        model_type: ModelType,
        num_threads: i32,
        provider: Option<String>,
    ) -> Result<Self, String> {
        info!(
            "[Recognizer::new] start model_type={:?} num_threads={num_threads}",
            model_type
        );
        let rec = match model_type {
            ModelType::OnlineTransducer {
                encoder,
                decoder,
                joiner,
                tokens,
                hotwords,
            } => {
                info!("[Recognizer::new] branch=OnlineTransducer");
                let mut config = get_base_online_config(num_threads, &tokens, provider.clone());
                config.model_config.model_type = Some("transducer".to_string());
                config.model_config.transducer.encoder =
                    Some(encoder.to_string_lossy().to_string());
                config.model_config.transducer.decoder =
                    Some(decoder.to_string_lossy().to_string());
                config.model_config.transducer.joiner = Some(joiner.to_string_lossy().to_string());

                if hotwords.is_some() {
                    config.decoding_method = Some("modified_beam_search".to_string());
                }

                debug!("Calling OnlineRecognizer::create from sherpa_onnx (OnlineTransducer)");
                let recognizer =
                    OnlineRecognizer::create(&config).ok_or("Failed to create OnlineRecognizer")?;
                debug!("Successfully created OnlineRecognizer (OnlineTransducer)");
                RecognizerInner::Online(SafeOnlineRecognizer(recognizer))
            }
            ModelType::OnlineParaformer {
                encoder,
                decoder,
                tokens,
            } => {
                info!("[Recognizer::new] branch=OnlineParaformer");
                let mut config = get_base_online_config(num_threads, &tokens, provider.clone());
                config.model_config.paraformer.encoder =
                    Some(encoder.to_string_lossy().to_string());
                config.model_config.paraformer.decoder =
                    Some(decoder.to_string_lossy().to_string());

                debug!("Calling OnlineRecognizer::create from sherpa_onnx (OnlineParaformer)");
                let recognizer =
                    OnlineRecognizer::create(&config).ok_or("Failed to create OnlineRecognizer")?;
                debug!("Successfully created OnlineRecognizer (OnlineParaformer)");
                RecognizerInner::Online(SafeOnlineRecognizer(recognizer))
            }
            ModelType::OfflineSenseVoice {
                model,
                tokens,
                language,
                use_itn,
            } => {
                info!("[Recognizer::new] branch=OfflineSenseVoice");
                let mut config =
                    get_base_offline_config(num_threads, Some(&tokens), provider.clone());
                config.model_config.sense_voice.model = Some(model.to_string_lossy().to_string());
                config.model_config.sense_voice.language = Some(language);
                config.model_config.sense_voice.use_itn = use_itn;

                debug!("Calling OfflineRecognizer::create from sherpa_onnx (OfflineSenseVoice)");
                let recognizer = OfflineRecognizer::create(&config)
                    .ok_or("Failed to create OfflineRecognizer")?;
                debug!("Successfully created OfflineRecognizer (OfflineSenseVoice)");
                RecognizerInner::Offline(SafeOfflineRecognizer(recognizer))
            }
            ModelType::OfflineWhisper {
                encoder,
                decoder,
                tokens,
                language,
            } => {
                info!("[Recognizer::new] branch=OfflineWhisper");
                let mut config =
                    get_base_offline_config(num_threads, Some(&tokens), provider.clone());
                config.model_config.whisper.encoder = Some(encoder.to_string_lossy().to_string());
                config.model_config.whisper.decoder = Some(decoder.to_string_lossy().to_string());
                config.model_config.whisper.language = Some(language);

                debug!("Calling OfflineRecognizer::create from sherpa_onnx (OfflineWhisper)");
                let recognizer = OfflineRecognizer::create(&config)
                    .ok_or("Failed to create OfflineRecognizer")?;
                debug!("Successfully created OfflineRecognizer (OfflineWhisper)");
                RecognizerInner::Offline(SafeOfflineRecognizer(recognizer))
            }
            ModelType::OfflineFunASRNano {
                encoder_adaptor,
                llm,
                embedding,
                tokenizer,
                tokens,
                language,
            } => {
                info!("[Recognizer::new] branch=OfflineFunASRNano");
                let mut config =
                    get_base_offline_config(num_threads, tokens.as_deref(), provider.clone());
                config.model_config.funasr_nano.encoder_adaptor =
                    Some(encoder_adaptor.to_string_lossy().to_string());
                config.model_config.funasr_nano.llm = Some(llm.to_string_lossy().to_string());
                config.model_config.funasr_nano.embedding =
                    Some(embedding.to_string_lossy().to_string());
                config.model_config.funasr_nano.tokenizer =
                    Some(tokenizer.to_string_lossy().to_string());
                config.model_config.funasr_nano.language = Some(language);

                debug!("Calling OfflineRecognizer::create from sherpa_onnx (OfflineFunASRNano)");
                let recognizer = OfflineRecognizer::create(&config)
                    .ok_or("Failed to create OfflineRecognizer")?;
                debug!("Successfully created OfflineRecognizer (OfflineFunASRNano)");
                RecognizerInner::Offline(SafeOfflineRecognizer(recognizer))
            }
            ModelType::OfflineFireRedAsr {
                encoder,
                decoder,
                tokens,
            } => {
                info!("[Recognizer::new] branch=OfflineFireRedAsr");
                let mut config =
                    get_base_offline_config(num_threads, Some(&tokens), provider.clone());
                config.model_config.fire_red_asr.encoder =
                    Some(encoder.to_string_lossy().to_string());
                config.model_config.fire_red_asr.decoder =
                    Some(decoder.to_string_lossy().to_string());

                debug!("Calling OfflineRecognizer::create from sherpa_onnx (OfflineFireRedAsr)");
                let recognizer = OfflineRecognizer::create(&config)
                    .ok_or("Failed to create OfflineRecognizer")?;
                debug!("Successfully created OfflineRecognizer (OfflineFireRedAsr)");
                RecognizerInner::Offline(SafeOfflineRecognizer(recognizer))
            }
            ModelType::OfflineDolphin { model, tokens } => {
                info!("[Recognizer::new] branch=OfflineDolphin");
                let mut config =
                    get_base_offline_config(num_threads, Some(&tokens), provider.clone());
                config.model_config.dolphin.model = Some(model.to_string_lossy().to_string());

                debug!("Calling OfflineRecognizer::create from sherpa_onnx (OfflineDolphin)");
                let recognizer = OfflineRecognizer::create(&config)
                    .ok_or("Failed to create OfflineRecognizer")?;
                debug!("Successfully created OfflineRecognizer (OfflineDolphin)");
                RecognizerInner::Offline(SafeOfflineRecognizer(recognizer))
            }
            ModelType::OfflineQwen3Asr {
                conv_frontend,
                encoder,
                decoder,
                tokenizer,
                hotwords,
            } => {
                info!("[Recognizer::new] branch=OfflineQwen3Asr");
                let mut config = get_base_offline_config(num_threads, None, provider.clone());
                config.model_config.qwen3_asr.conv_frontend =
                    Some(conv_frontend.to_string_lossy().to_string());
                config.model_config.qwen3_asr.encoder = Some(encoder.to_string_lossy().to_string());
                config.model_config.qwen3_asr.decoder = Some(decoder.to_string_lossy().to_string());
                config.model_config.qwen3_asr.tokenizer =
                    Some(tokenizer.to_string_lossy().to_string());
                config.model_config.qwen3_asr.hotwords = hotwords;

                debug!("Calling OfflineRecognizer::create from sherpa_onnx (OfflineQwen3Asr)");
                let recognizer = OfflineRecognizer::create(&config)
                    .ok_or("Failed to create OfflineRecognizer")?;
                debug!("Successfully created OfflineRecognizer (OfflineQwen3Asr)");
                RecognizerInner::Offline(SafeOfflineRecognizer(recognizer))
            }
        };
        Ok(Self { inner: rec })
    }
}

pub fn create_recognizer_with_gpu_plan(
    model_type: ModelType,
    num_threads: i32,
    plan: GpuAccelerationPlan,
) -> Result<RecognizerCreateResult, String> {
    let (recognizer, provider, fallback_notice) = create_value_with_gpu_plan(plan, |provider| {
        Recognizer::new(
            model_type.clone(),
            num_threads,
            provider.map(str::to_string),
        )
    })?;

    Ok(RecognizerCreateResult {
        recognizer,
        provider,
        fallback_notice,
    })
}

pub fn create_offline_recognizer(
    model_type: ModelType,
    num_threads: i32,
    provider: Option<&str>,
) -> Result<SafeOfflineRecognizer, String> {
    if !model_type.is_offline() {
        return Err(format!(
            "Unsupported offline model type: {}",
            model_type.model_type_name()
        ));
    }

    match Recognizer::new(
        model_type,
        num_threads,
        provider.map(std::string::ToString::to_string),
    )?
    .inner
    {
        RecognizerInner::Offline(recognizer) => Ok(recognizer),
        RecognizerInner::Online(_) => Err("Unsupported offline model type".to_string()),
    }
}

pub fn decode_offline_samples(
    recognizer: &SafeOfflineRecognizer,
    samples: &[f32],
) -> Option<OfflineDecodeResult> {
    let stream = recognizer.0.create_stream();
    stream.accept_waveform(16000, samples);
    recognizer.0.decode(&stream);

    stream.get_result().map(|result| OfflineDecodeResult {
        text: result.text,
        tokens: result.tokens,
        timestamps: result.timestamps,
    })
}

pub fn create_online_stream(recognizer: &SafeOnlineRecognizer) -> SafeStream {
    SafeStream(recognizer.0.create_stream())
}

pub fn accept_online_samples(stream: &SafeStream, samples: &[f32]) {
    stream.0.accept_waveform(16000, samples);
}

pub fn decode_online_ready(recognizer: &SafeOnlineRecognizer, stream: &SafeStream) {
    while recognizer.0.is_ready(&stream.0) {
        recognizer.0.decode(&stream.0);
    }
}

pub fn is_online_endpoint(recognizer: &SafeOnlineRecognizer, stream: &SafeStream) -> bool {
    recognizer.0.is_endpoint(&stream.0)
}

pub fn online_stream_result(
    recognizer: &SafeOnlineRecognizer,
    stream: &SafeStream,
) -> Option<OnlineDecodeResult> {
    recognizer
        .0
        .get_result(&stream.0)
        .map(|result| OnlineDecodeResult {
            text: result.text,
            tokens: result.tokens,
            timestamps: result.timestamps,
        })
}

pub fn reset_online_stream(recognizer: &SafeOnlineRecognizer, stream: &SafeStream) {
    recognizer.0.reset(&stream.0);
}

fn create_value_with_gpu_plan<T, F>(
    plan: GpuAccelerationPlan,
    mut create: F,
) -> Result<(T, Option<String>, Option<GpuFallbackNotice>), String>
where
    F: FnMut(Option<&str>) -> Result<T, String>,
{
    let mut last_error = None;

    for provider in plan.provider_options() {
        let provider_name = provider.as_deref();
        match create(provider_name) {
            Ok(value) => {
                let fallback_notice = last_error.take().map(GpuFallbackNotice::directml_retry);
                return Ok((value, provider, fallback_notice));
            }
            Err(error)
                if provider_name
                    .map(|provider| plan.should_retry_after_failure(provider))
                    .unwrap_or(false) =>
            {
                last_error = Some(error);
            }
            Err(error) => return Err(error),
        }
    }

    Err(last_error.unwrap_or_else(|| "Recognizer creation failed.".to_string()))
}

unsafe impl Send for Recognizer {}
unsafe impl Sync for Recognizer {}

pub struct SafeStream(sherpa_onnx::OnlineStream);
unsafe impl Send for SafeStream {}
unsafe impl Sync for SafeStream {}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestCreateResult {
        provider: Option<String>,
        fallback_notice: Option<GpuFallbackNotice>,
    }

    fn create_value_with_gpu_plan_for_test<F>(
        plan: GpuAccelerationPlan,
        create: F,
    ) -> Result<TestCreateResult, String>
    where
        F: FnMut(Option<&str>) -> Result<String, String>,
    {
        let (_value, provider, fallback_notice) = create_value_with_gpu_plan(plan, create)?;
        Ok(TestCreateResult {
            provider,
            fallback_notice,
        })
    }

    #[test]
    fn recognizer_factory_retries_auto_directml_failure_on_cpu() {
        let plan = GpuAccelerationPlan::for_platform(Some("auto"), true, false, true);
        let mut attempted = Vec::new();
        let result = create_value_with_gpu_plan_for_test(plan, |provider| {
            attempted.push(provider.map(str::to_string));
            match provider {
                Some("directml") => Err("directml init failed".to_string()),
                Some("cpu") => Ok("cpu".to_string()),
                other => Err(format!("unexpected provider: {other:?}")),
            }
        })
        .expect("cpu retry should succeed");

        assert_eq!(result.provider.as_deref(), Some("cpu"));
        assert_eq!(
            result.fallback_notice.unwrap().error,
            "directml init failed"
        );
        assert_eq!(
            attempted,
            vec![Some("directml".to_string()), Some("cpu".to_string())]
        );
    }

    #[test]
    fn recognizer_factory_does_not_retry_explicit_directml_failure() {
        let plan = GpuAccelerationPlan::for_platform(Some("directml"), true, false, true);
        let mut attempted = Vec::new();
        let error = create_value_with_gpu_plan_for_test(plan, |provider| {
            attempted.push(provider.map(str::to_string));
            Err("directml init failed".to_string())
        })
        .expect_err("explicit directml should fail without cpu retry");

        assert_eq!(error, "directml init failed");
        assert_eq!(attempted, vec![Some("directml".to_string())]);
    }

    #[test]
    fn recognizer_factory_skips_unavailable_directml_runtime() {
        let plan = GpuAccelerationPlan::for_platform(Some("auto"), true, false, false);
        let mut attempted = Vec::new();
        let result = create_value_with_gpu_plan_for_test(plan, |provider| {
            attempted.push(provider.map(str::to_string));
            Ok(provider.unwrap_or("none").to_string())
        })
        .expect("cpu provider should be used directly");

        assert_eq!(result.provider.as_deref(), Some("cpu"));
        assert!(result.fallback_notice.is_none());
        assert_eq!(attempted, vec![Some("cpu".to_string())]);
    }
}
