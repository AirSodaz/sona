use log::{debug, info};
use serde::{Deserialize, Serialize};
use sherpa_onnx::{
    OfflineRecognizer, OfflineRecognizerConfig, OnlineRecognizer, OnlineRecognizerConfig,
    SileroVadModelConfig, VadModelConfig, VoiceActivityDetector,
};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelFileConfig {
    pub encoder: Option<String>,
    pub decoder: Option<String>,
    pub model: Option<String>,
    pub joiner: Option<String>,
    pub tokens: Option<String>,
    pub conv_frontend: Option<String>,
    pub encoder_adaptor: Option<String>,
    pub llm: Option<String>,
    pub embedding: Option<String>,
    pub tokenizer: Option<String>,
}

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
            // Sherpa treats an empty language as "auto/default behavior" for
            // Whisper-style models, so the UI's `auto` value is normalized
            // before we hand the config to the backend.
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
            // FunASR Nano uses an empty string to keep multilingual inference
            // enabled instead of forcing one explicit language code.
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

pub struct SafeOnlineRecognizer(pub OnlineRecognizer);
unsafe impl Send for SafeOnlineRecognizer {}
unsafe impl Sync for SafeOnlineRecognizer {}

pub struct SafeOfflineRecognizer(pub OfflineRecognizer);
unsafe impl Send for SafeOfflineRecognizer {}
unsafe impl Sync for SafeOfflineRecognizer {}

pub enum RecognizerInner {
    Online(SafeOnlineRecognizer),
    Offline(SafeOfflineRecognizer),
}

pub struct Recognizer {
    pub inner: RecognizerInner,
}

pub(crate) struct RecognizerCreateResult {
    pub(crate) recognizer: Recognizer,
    pub(crate) provider: Option<String>,
    pub(crate) fallback_notice: Option<crate::app::hardware::GpuFallbackNotice>,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct TestRecognizerCreateResult {
    provider: Option<String>,
    fallback_notice: Option<crate::app::hardware::GpuFallbackNotice>,
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
        // This builds the heavy recognizer object for one concrete model
        // configuration. Pooling/reuse happens one level up in `AsrState`.
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

                if let Some(_hw) = hotwords {
                    // Hotwords require the beam-search path; the default greedy
                    // decoder does not consult them.
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

                if let Some(hw) = hotwords {
                    config.model_config.qwen3_asr.hotwords = Some(hw);
                }

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

pub(crate) fn create_recognizer_with_gpu_plan(
    model_type: ModelType,
    num_threads: i32,
    plan: crate::app::hardware::GpuAccelerationPlan,
) -> Result<RecognizerCreateResult, String> {
    let (recognizer, provider, fallback_notice) =
        create_recognizer_with_gpu_plan_impl(plan, |provider| {
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

fn create_recognizer_with_gpu_plan_impl<T, F>(
    plan: crate::app::hardware::GpuAccelerationPlan,
    mut create: F,
) -> Result<
    (
        T,
        Option<String>,
        Option<crate::app::hardware::GpuFallbackNotice>,
    ),
    String,
>
where
    F: FnMut(Option<&str>) -> Result<T, String>,
{
    let mut last_error = None;

    for provider in plan.provider_options() {
        let provider_name = provider.as_deref();
        match create(provider_name) {
            Ok(value) => {
                let fallback_notice = last_error
                    .take()
                    .map(crate::app::hardware::GpuFallbackNotice::directml_retry);
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

#[cfg(test)]
fn create_recognizer_with_gpu_plan_for_test<F>(
    plan: crate::app::hardware::GpuAccelerationPlan,
    create: F,
) -> Result<TestRecognizerCreateResult, String>
where
    F: FnMut(Option<&str>) -> Result<String, String>,
{
    let (_value, provider, fallback_notice) = create_recognizer_with_gpu_plan_impl(plan, create)?;
    Ok(TestRecognizerCreateResult {
        provider,
        fallback_notice,
    })
}

unsafe impl Send for Recognizer {}
unsafe impl Sync for Recognizer {}

pub struct SafeStream(pub sherpa_onnx::OnlineStream);
unsafe impl Send for SafeStream {}
unsafe impl Sync for SafeStream {}

pub struct SafeVad(pub sherpa_onnx::VoiceActivityDetector);
unsafe impl Send for SafeVad {}
unsafe impl Sync for SafeVad {}

// -----------------------------------------------------------------------------------------
// Punctuation
// -----------------------------------------------------------------------------------------
pub struct Punctuation {
    inner: sherpa_onnx::OfflinePunctuation,
}

impl Punctuation {
    pub fn new(model_path: &str, num_threads: i32) -> Result<Self, String> {
        let config = sherpa_onnx::OfflinePunctuationConfig {
            model: sherpa_onnx::OfflinePunctuationModelConfig {
                ct_transformer: Some(model_path.to_string()),
                num_threads,
                debug: false,
                provider: Some("cpu".to_string()),
            },
        };

        let inner = sherpa_onnx::OfflinePunctuation::create(&config)
            .ok_or("Failed to create OfflinePunctuation")?;

        Ok(Self { inner })
    }

    pub fn add_punct(&self, text: &str) -> String {
        self.inner
            .add_punctuation(text)
            .unwrap_or_else(|| text.to_string())
    }
}

unsafe impl Send for Punctuation {}
unsafe impl Sync for Punctuation {}

pub fn load_punctuation(punctuation_model: Option<String>) -> Option<Punctuation> {
    let p_path = punctuation_model?;

    if p_path.is_empty() || !Path::new(&p_path).exists() {
        return None;
    }

    let entries = std::fs::read_dir(&p_path).ok()?;
    let onnx_file = entries
        .flatten()
        .find(|e: &std::fs::DirEntry| e.path().extension().is_some_and(|ext| ext == "onnx"))?;

    Punctuation::new(&onnx_file.path().to_string_lossy(), 1).ok()
}

pub fn load_vad(vad_model: Option<String>) -> Option<SafeVad> {
    let v_path = vad_model?;

    if v_path.is_empty() || !Path::new(&v_path).exists() {
        println!(
            "[Sherpa] load_vad: Path is empty or does not exist: {}",
            v_path
        );
        return None;
    }

    let model_file_path = if Path::new(&v_path).is_file() {
        v_path
    } else {
        let entries = match std::fs::read_dir(&v_path) {
            Ok(e) => e,
            Err(err) => {
                println!("[Sherpa] load_vad: Failed to read dir {}: {}", v_path, err);
                return None;
            }
        };

        let onnx_file = entries
            .flatten()
            .find(|e: &std::fs::DirEntry| e.path().extension().is_some_and(|ext| ext == "onnx"));

        match onnx_file {
            Some(file) => file.path().to_string_lossy().into_owned(),
            None => {
                println!(
                    "[Sherpa] load_vad: No .onnx file found in directory {}",
                    v_path
                );
                return None;
            }
        }
    };

    println!(
        "[Sherpa] load_vad: Found VAD model file: {}",
        model_file_path
    );

    let silero_vad = SileroVadModelConfig {
        model: Some(model_file_path),
        threshold: 0.30,
        min_silence_duration: 0.5,
        min_speech_duration: 0.25,
        window_size: 512,
        ..Default::default()
    };

    let vad_config = VadModelConfig {
        silero_vad,
        sample_rate: 16000,
        num_threads: 1,
        ..Default::default()
    };

    let result = VoiceActivityDetector::create(&vad_config, 60.0).map(SafeVad);
    if result.is_none() {
        println!("[Sherpa] load_vad: VoiceActivityDetector::create FAILED!");
    } else {
        println!("[Sherpa] load_vad: VAD successfully created!");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn build_model_config_supports_qwen3_asr_without_tokens() {
        let model_path = Path::new("C:/models/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25");
        let file_config = Some(ModelFileConfig {
            conv_frontend: Some("conv_frontend.onnx".to_string()),
            encoder: Some("encoder.int8.onnx".to_string()),
            decoder: Some("decoder.int8.onnx".to_string()),
            tokenizer: Some("tokenizer".to_string()),
            ..Default::default()
        });

        let model = build_model_config(model_path, "qwen3-asr", &file_config, false, "auto", None)
            .expect("qwen3-asr model should build");

        match model {
            ModelType::OfflineQwen3Asr {
                conv_frontend,
                encoder,
                decoder,
                tokenizer,
                ..
            } => {
                assert_eq!(conv_frontend, model_path.join("conv_frontend.onnx"));
                assert_eq!(encoder, model_path.join("encoder.int8.onnx"));
                assert_eq!(decoder, model_path.join("decoder.int8.onnx"));
                assert_eq!(tokenizer, model_path.join("tokenizer"));
            }
            other => panic!("expected OfflineQwen3Asr, got {other:?}"),
        }
    }

    #[test]
    fn build_model_config_still_requires_tokens_for_sensevoice() {
        let model_path = Path::new("C:/models/sensevoice");
        let file_config = Some(ModelFileConfig {
            model: Some("model.int8.onnx".to_string()),
            ..Default::default()
        });

        let error = build_model_config(model_path, "sensevoice", &file_config, true, "auto", None)
            .expect_err("sensevoice should still require tokens.txt");

        assert!(
            error.contains("Required file name not specified in config"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn build_model_config_supports_funasr_nano_without_tokens() {
        let model_path = Path::new("C:/models/funasr-nano");
        let file_config = Some(ModelFileConfig {
            encoder_adaptor: Some("encoder_adaptor.int8.onnx".to_string()),
            llm: Some("llm.int8.onnx".to_string()),
            embedding: Some("embedding.int8.onnx".to_string()),
            tokenizer: Some("Qwen3-0.6B".to_string()),
            ..Default::default()
        });

        let model =
            build_model_config(model_path, "funasr-nano", &file_config, false, "auto", None)
                .expect("funasr-nano should build without tokens");

        match model {
            ModelType::OfflineFunASRNano { tokens, .. } => {
                assert!(tokens.is_none());
            }
            other => panic!("expected OfflineFunASRNano, got {other:?}"),
        }
    }

    #[test]
    fn recognizer_factory_retries_auto_directml_failure_on_cpu() {
        let plan = crate::app::hardware::GpuAccelerationPlan::for_platform(
            Some("auto"),
            true,
            false,
            true,
        );
        let mut attempted = Vec::new();
        let result = create_recognizer_with_gpu_plan_for_test(plan, |provider| {
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
        let plan = crate::app::hardware::GpuAccelerationPlan::for_platform(
            Some("directml"),
            true,
            false,
            true,
        );
        let mut attempted = Vec::new();
        let error = create_recognizer_with_gpu_plan_for_test(plan, |provider| {
            attempted.push(provider.map(str::to_string));
            Err("directml init failed".to_string())
        })
        .expect_err("explicit directml should fail without cpu retry");

        assert_eq!(error, "directml init failed");
        assert_eq!(attempted, vec![Some("directml".to_string())]);
    }

    #[test]
    fn recognizer_factory_skips_unavailable_directml_runtime() {
        let plan = crate::app::hardware::GpuAccelerationPlan::for_platform(
            Some("auto"),
            true,
            false,
            false,
        );
        let mut attempted = Vec::new();
        let result = create_recognizer_with_gpu_plan_for_test(plan, |provider| {
            attempted.push(provider.map(str::to_string));
            Ok(provider.unwrap_or("none").to_string())
        })
        .expect("cpu provider should be used directly");

        assert_eq!(result.provider.as_deref(), Some("cpu"));
        assert!(result.fallback_notice.is_none());
        assert_eq!(attempted, vec![Some("cpu".to_string())]);
    }
}
