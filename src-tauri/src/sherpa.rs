use log::{debug, info, trace};
use serde::{Deserialize, Serialize};
use sherpa_onnx::{
    OfflineRecognizer, OfflineRecognizerConfig, OnlineRecognizer, OnlineRecognizerConfig,
    SileroVadModelConfig, VadModelConfig, VoiceActivityDetector,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

const BATCH_PROGRESS_EVENT: &str = "batch-progress";

fn recognizer_output_event(instance_id: &str) -> String {
    format!("recognizer-output-{instance_id}")
}

#[derive(Debug, Clone, Default, Deserialize)]
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

fn get_base_online_config(num_threads: i32, tokens: &Path) -> OnlineRecognizerConfig {
    let mut config = OnlineRecognizerConfig {
        rule1_min_trailing_silence: 1.2,
        rule2_min_trailing_silence: 1.2,
        rule3_min_utterance_length: 300.0,
        ..Default::default()
    };
    config.model_config.tokens = Some(tokens.to_string_lossy().to_string());
    config.model_config.num_threads = num_threads;
    config.model_config.provider = Some("cpu".to_string());
    config.model_config.model_type = Some("paraformer".to_string());
    config.feat_config.sample_rate = 16000;
    config.feat_config.feature_dim = 80;
    config.enable_endpoint = true;
    config
}

fn get_base_offline_config(num_threads: i32, tokens: Option<&Path>) -> OfflineRecognizerConfig {
    let mut config = OfflineRecognizerConfig::default();
    config.model_config.tokens = tokens.map(|path| path.to_string_lossy().to_string());
    config.model_config.num_threads = num_threads;
    config.model_config.provider = Some("cpu".to_string());
    config.feat_config.sample_rate = 16000;
    config.feat_config.feature_dim = 80;
    config
}

impl Recognizer {
    pub fn new(model_type: ModelType, num_threads: i32) -> Result<Self, String> {
        info!(
            "[Recognizer::new] start model_type={:?} num_threads={num_threads}",
            model_type
        );
        // This builds the heavy recognizer object for one concrete model
        // configuration. Pooling/reuse happens one level up in `SherpaState`.
        let rec = match model_type {
            ModelType::OnlineTransducer {
                encoder,
                decoder,
                joiner,
                tokens,
                hotwords,
            } => {
                info!("[Recognizer::new] branch=OnlineTransducer");
                let mut config = get_base_online_config(num_threads, &tokens);
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
                let mut config = get_base_online_config(num_threads, &tokens);
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
                let mut config = get_base_offline_config(num_threads, Some(&tokens));
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
                let mut config = get_base_offline_config(num_threads, Some(&tokens));
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
                let mut config = get_base_offline_config(num_threads, tokens.as_deref());
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
                let mut config = get_base_offline_config(num_threads, Some(&tokens));
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
                let mut config = get_base_offline_config(num_threads, Some(&tokens));
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
                let mut config = get_base_offline_config(num_threads, None);
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

pub struct OfflineState {
    pub speech_buffer: Vec<Vec<f32>>,
    pub ring_buffer: std::collections::VecDeque<Vec<f32>>,
    pub is_speaking: bool,
    pub last_inference_time: std::time::Instant,
    pub utterance_start_sample: usize,
}

impl Default for OfflineState {
    fn default() -> Self {
        Self {
            speech_buffer: Vec::new(),
            ring_buffer: std::collections::VecDeque::new(),
            is_speaking: false,
            last_inference_time: std::time::Instant::now(),
            utterance_start_sample: 0,
        }
    }
}

use std::sync::Arc;

use std::collections::HashMap;

#[derive(Debug)]
pub struct RecordDiagnosticsState {
    pub first_sample_logged: bool,
    pub skipped_while_stopped_logged: bool,
    pub first_segment_emitted: Arc<AtomicBool>,
}

impl Default for RecordDiagnosticsState {
    fn default() -> Self {
        Self {
            first_sample_logged: false,
            skipped_while_stopped_logged: false,
            first_segment_emitted: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub struct SherpaInstance {
    pub recognizer: Option<Arc<Recognizer>>,
    pub stream: Option<SafeStream>,
    pub vad: Option<SafeVad>,
    pub punctuation: Option<Arc<Punctuation>>,
    pub total_samples: usize,
    pub segment_start_time: f64,
    pub offline_state: OfflineState,
    pub vad_model: Option<String>,
    pub vad_buffer: f32,
    pub current_segment_id: Option<String>,
    pub is_running: bool,
    pub record_diagnostics: RecordDiagnosticsState,
    pub normalization_options: TranscriptNormalizationOptions,
}

impl Default for SherpaInstance {
    fn default() -> Self {
        Self {
            recognizer: None,
            stream: None,
            vad: None,
            punctuation: None,
            total_samples: 0,
            segment_start_time: 0.0,
            offline_state: OfflineState::default(),
            vad_model: None,
            vad_buffer: 5.0,
            current_segment_id: None,
            is_running: false,
            record_diagnostics: RecordDiagnosticsState::default(),
            normalization_options: TranscriptNormalizationOptions::default(),
        }
    }
}

fn diagnostics_instance_label(instance_id: &str) -> Option<&'static str> {
    match instance_id {
        "record" => Some("record"),
        "caption" => Some("caption"),
        "voice-typing" => Some("voice-typing"),
        _ => None,
    }
}

fn buffered_sample_count(chunks: &[Vec<f32>]) -> usize {
    chunks.iter().map(|chunk| chunk.len()).sum()
}

fn reset_instance_runtime_state(instance: &mut SherpaInstance) {
    // Reset only per-run counters and buffers. The shared recognizer/VAD/model
    // attachments stay on the instance so later starts can reuse them.
    instance.total_samples = 0;
    instance.segment_start_time = 0.0;
    instance.offline_state = OfflineState::default();
    instance.current_segment_id = None;
    instance.record_diagnostics = RecordDiagnosticsState::default();
}

fn start_instance_runtime(instance: &mut SherpaInstance, stream: Option<SafeStream>) {
    // Online recognizers get a fresh stream per run; offline recognizers leave
    // this as `None` and rebuild utterances from buffered audio chunks.
    instance.stream = stream;
    reset_instance_runtime_state(instance);
    instance.is_running = true;
}

fn stop_instance_runtime(instance: &mut SherpaInstance) {
    // Stopping a run clears volatile state only; the instance still keeps its
    // recognizer and optional VAD/punctuation attachments for the next start.
    instance.stream = None;
    reset_instance_runtime_state(instance);
    instance.is_running = false;
}

fn log_segment_emit_diagnostics(
    instance_id: &str,
    first_segment_emitted: Option<&Arc<AtomicBool>>,
    segment: &TranscriptSegment,
    stage: &str,
) {
    // These logs are intentionally scoped to the long-lived live instances we
    // debug most often (`record`, `caption`, `voice-typing`), not to every
    // possible recognizer consumer.
    let Some(label) = diagnostics_instance_label(instance_id) else {
        return;
    };

    let text_len = segment.text.chars().count();
    if let Some(first_segment_emitted) = first_segment_emitted {
        if first_segment_emitted
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            info!(
                "[Sherpa] {label} first segment emitted. stage={} segment_id={} final={} text_len={}",
                stage, segment.id, segment.is_final, text_len
            );
        }
    }

    info!(
        "[Sherpa] {label} emit. stage={} segment_id={} final={} text_len={}",
        stage, segment.id, segment.is_final, text_len
    );
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelConfigKey {
    pub model_path: String,
    pub model_type: String,
    pub num_threads: i32,
    pub enable_itn: bool,
    pub language: String,
    pub hotwords: Option<String>,
}

pub struct SherpaState {
    // Each logical instance keeps its own runtime buffers and stream state,
    // while recognizers are pooled separately by configuration.
    pub instances: Mutex<HashMap<String, SherpaInstance>>,
    pub recognizer_pool: Mutex<HashMap<ModelConfigKey, Arc<Recognizer>>>,
}

impl Default for SherpaState {
    fn default() -> Self {
        Self::new()
    }
}

impl SherpaState {
    pub fn new() -> Self {
        Self {
            instances: Mutex::new(HashMap::new()),
            recognizer_pool: Mutex::new(HashMap::new()),
        }
    }
}

/// Batch transcription request shared by the GUI Tauri command and the CLI.
#[derive(Debug, Clone)]
pub struct BatchTranscriptionRequest {
    pub file_path: String,
    pub save_to_path: Option<String>,
    pub model_path: String,
    pub num_threads: i32,
    pub enable_itn: bool,
    pub language: String,
    pub punctuation_model: Option<String>,
    pub vad_model: Option<String>,
    pub vad_buffer: f32,
    pub model_type: String,
    pub file_config: Option<ModelFileConfig>,
    pub hotwords: Option<String>,
    pub speaker_processing: Option<crate::speaker::SpeakerProcessingConfig>,
    pub normalization_options: TranscriptNormalizationOptions,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptTimingLevel {
    Token,
    Segment,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptTimingSource {
    Model,
    Derived,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptTimingUnit {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptTiming {
    pub level: TranscriptTimingLevel,
    pub source: TranscriptTimingSource,
    pub units: Vec<TranscriptTimingUnit>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptNormalizationOptions {
    pub enable_timeline: bool,
}

impl Default for TranscriptNormalizationOptions {
    fn default() -> Self {
        Self {
            enable_timeline: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptUpdate {
    pub remove_ids: Vec<String>,
    pub upsert_segments: Vec<TranscriptSegment>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegment {
    pub id: String,
    pub text: String,
    pub start: f64,
    pub end: f64,
    pub is_final: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timing: Option<TranscriptTiming>,
    // Legacy raw fields are still written for compatibility with older
    // persisted transcript records and upgrade paths.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamps: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub durations: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker: Option<crate::speaker::SpeakerTag>,
}

const MAX_SEGMENT_LENGTH_CJK: usize = 36;
const MAX_SEGMENT_LENGTH_WESTERN: usize = 84;
const ABBREVIATIONS: &[&str] = &[
    "mr", "mrs", "ms", "dr", "prof", "sr", "jr", "st", "vs", "etc", "no", "op", "vol", "fig",
    "inc", "ltd", "co", "dept",
];

#[derive(Clone, Debug)]
struct TimingTextUnit {
    text: String,
    normalized: String,
}

#[derive(Clone, Debug)]
struct TokenMap {
    start_indices: Vec<usize>,
    end_indices: Vec<usize>,
    timestamps: Vec<f32>,
}

#[derive(Clone, Debug)]
struct SplitterState {
    current_text: String,
    current_start: f64,
    current_segment_start: f64,
    char_index: usize,
    effective_char_index: usize,
    last_token_index: usize,
    next_token_slice_start: usize,
}

fn normalize_text_search_key(text: &str) -> String {
    text.chars()
        .flat_map(|ch| ch.to_lowercase())
        .filter(|ch| ch.is_alphanumeric())
        .collect()
}

fn normalize_timing_units(
    units: Vec<TranscriptTimingUnit>,
    segment_start: f64,
    segment_end: f64,
) -> Vec<TranscriptTimingUnit> {
    let safe_start = segment_start.max(0.0);
    let safe_end = segment_end.max(safe_start);

    let unit_count = units.len();

    units
        .into_iter()
        .enumerate()
        .filter_map(|(index, unit)| {
            if unit.text.is_empty() {
                return None;
            }

            let start = unit.start.max(safe_start).min(safe_end);
            let fallback_end = if index + 1 == unit_count {
                safe_end
            } else {
                start
            };
            let end = unit.end.max(fallback_end).min(safe_end).max(start);

            Some(TranscriptTimingUnit {
                text: unit.text,
                start,
                end,
            })
        })
        .collect()
}

fn build_segment_level_timing(
    segment: &TranscriptSegment,
    source: TranscriptTimingSource,
) -> TranscriptTiming {
    TranscriptTiming {
        level: TranscriptTimingLevel::Segment,
        source,
        units: vec![TranscriptTimingUnit {
            text: segment.text.clone(),
            start: segment.start,
            end: segment.end,
        }],
    }
}

fn build_token_windows(
    timestamps: &[f32],
    durations: Option<&[f32]>,
    segment_end: f64,
) -> Vec<(f64, f64)> {
    timestamps
        .iter()
        .enumerate()
        .map(|(index, timestamp)| {
            let start = *timestamp as f64;
            let explicit_end = durations
                .and_then(|values| values.get(index))
                .map(|value| start + (*value as f64).max(0.0));
            let next_start = timestamps.get(index + 1).map(|value| *value as f64);
            let end = explicit_end
                .or(next_start)
                .unwrap_or(segment_end)
                .max(start);
            (start, end)
        })
        .collect()
}

fn fallback_token_index(char_pos: usize, char_to_token_index: &[usize]) -> usize {
    if char_to_token_index.is_empty() {
        return 0;
    }

    if char_pos >= char_to_token_index.len() {
        char_to_token_index[char_to_token_index.len() - 1]
    } else {
        char_to_token_index[char_pos]
    }
}

fn find_subsequence(haystack: &[char], needle: &[char]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }

    if needle.len() > haystack.len() {
        return None;
    }

    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn is_cjk_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{3040}'..='\u{309F}'
            | '\u{30A0}'..='\u{30FF}'
            | '\u{AC00}'..='\u{D7AF}'
    )
}

fn lex_timing_text_units(text: &str) -> Vec<TimingTextUnit> {
    let mut units = Vec::new();
    let chars = text.chars().collect::<Vec<_>>();
    let mut index = 0usize;

    while index < chars.len() {
        let ch = chars[index];

        if ch.is_whitespace() {
            let start = index;
            index += 1;
            while index < chars.len() && chars[index].is_whitespace() {
                index += 1;
            }
            units.push(TimingTextUnit {
                text: chars[start..index].iter().collect(),
                normalized: String::new(),
            });
            continue;
        }

        if is_cjk_char(ch) {
            let text = ch.to_string();
            units.push(TimingTextUnit {
                normalized: normalize_text_search_key(&text),
                text,
            });
            index += 1;
            continue;
        }

        let start = index;
        index += 1;
        while index < chars.len() && !chars[index].is_whitespace() && !is_cjk_char(chars[index]) {
            index += 1;
        }

        let text = chars[start..index].iter().collect::<String>();
        let normalized = normalize_text_search_key(&text);

        if normalized.is_empty() {
            if let Some(previous) = units.last_mut() {
                if !previous.normalized.is_empty() {
                    previous.text.push_str(&text);
                    continue;
                }
            }
        }

        units.push(TimingTextUnit { text, normalized });
    }

    units
}

fn build_aligned_timing_units(
    text: &str,
    tokens: &[String],
    timestamps: &[f32],
    durations: Option<&[f32]>,
    segment_end: f64,
) -> Option<Vec<TranscriptTimingUnit>> {
    if tokens.is_empty() || tokens.len() != timestamps.len() {
        return None;
    }

    let normalized_tokens = tokens
        .iter()
        .map(|token| normalize_text_search_key(token))
        .collect::<Vec<_>>();
    let windows = build_token_windows(timestamps, durations, segment_end);
    let units = lex_timing_text_units(text);

    let mut joined_token_chars = Vec::new();
    let mut char_to_token_index = Vec::new();
    for (token_index, token) in normalized_tokens.iter().enumerate() {
        for ch in token.chars() {
            joined_token_chars.push(ch);
            char_to_token_index.push(token_index);
        }
    }

    if char_to_token_index.is_empty() {
        return None;
    }

    let mut char_pos = 0usize;
    let mut result = Vec::new();

    for unit in units {
        if unit.text.is_empty() {
            continue;
        }

        let token_index = if unit.normalized.is_empty() {
            fallback_token_index(char_pos, &char_to_token_index)
        } else {
            let needle = unit.normalized.chars().collect::<Vec<_>>();
            let search_limit = needle.len().saturating_mul(2).max(20);
            let window_end = (char_pos + search_limit).min(joined_token_chars.len());
            let local_index = find_subsequence(&joined_token_chars[char_pos..window_end], &needle);

            if let Some(local_index) = local_index {
                let match_pos = char_pos + local_index;
                char_pos = (match_pos + needle.len()).min(joined_token_chars.len());
                fallback_token_index(match_pos, &char_to_token_index)
            } else {
                let fallback = fallback_token_index(char_pos, &char_to_token_index);
                char_pos = (char_pos + needle.len().max(1)).min(joined_token_chars.len());
                fallback
            }
        };

        let (start, end) = windows
            .get(token_index)
            .copied()
            .unwrap_or((segment_end, segment_end));
        result.push(TranscriptTimingUnit {
            text: unit.text,
            start,
            end,
        });
    }

    Some(result)
}

fn build_timing_from_legacy(segment: &TranscriptSegment) -> Option<TranscriptTiming> {
    let tokens = segment.tokens.as_ref()?;
    let timestamps = segment.timestamps.as_ref()?;
    if tokens.is_empty() || tokens.len() != timestamps.len() {
        return None;
    }

    let durations = segment
        .durations
        .as_ref()
        .filter(|values| values.len() == tokens.len())
        .map(|values| values.as_slice());
    let units =
        build_aligned_timing_units(&segment.text, tokens, timestamps, durations, segment.end)?;
    let units = normalize_timing_units(units, segment.start, segment.end);
    if units.is_empty() {
        return None;
    }

    Some(TranscriptTiming {
        level: TranscriptTimingLevel::Token,
        source: TranscriptTimingSource::Model,
        units,
    })
}

pub(crate) fn ensure_transcript_segment_timing(segment: &mut TranscriptSegment) {
    segment.start = segment.start.max(0.0);
    segment.end = segment.end.max(segment.start);

    let timing = segment
        .timing
        .clone()
        .map(|timing| TranscriptTiming {
            level: timing.level,
            source: timing.source,
            units: normalize_timing_units(timing.units, segment.start, segment.end),
        })
        .filter(|timing| !timing.units.is_empty())
        .or_else(|| build_timing_from_legacy(segment))
        .unwrap_or_else(|| build_segment_level_timing(segment, TranscriptTimingSource::Derived));

    segment.timing = Some(timing);
}

fn normalize_transcript_segments(mut segments: Vec<TranscriptSegment>) -> Vec<TranscriptSegment> {
    for segment in &mut segments {
        ensure_transcript_segment_timing(segment);
    }
    segments
}

fn is_meaningful_segment_char(ch: char) -> bool {
    ch.is_alphanumeric()
}

fn effective_length(text: &str) -> usize {
    text.chars()
        .filter(|ch| is_meaningful_segment_char(*ch))
        .count()
}

fn ends_with_abbreviation(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }

    let last_word = trimmed
        .split_whitespace()
        .last()
        .unwrap_or_default()
        .trim_matches(|ch: char| !ch.is_alphanumeric())
        .to_ascii_lowercase();
    ABBREVIATIONS.iter().any(|value| *value == last_word)
}

fn contains_cjk(text: &str) -> bool {
    text.chars().any(is_cjk_char)
}

fn is_strong_split_char(ch: char) -> bool {
    matches!(ch, '.' | '?' | '!' | '。' | '？' | '！')
}

fn is_weak_split_char(ch: char) -> bool {
    matches!(ch, ',' | '，' | ';' | '；' | ':' | '：')
}

fn split_text_parts<F>(text: &str, is_delimiter: F) -> Vec<String>
where
    F: Fn(char) -> bool,
{
    let chars = text.chars().collect::<Vec<_>>();
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut index = 0usize;

    while index < chars.len() {
        if is_delimiter(chars[index]) {
            if !current.is_empty() {
                parts.push(current.clone());
                current.clear();
            }

            let start = index;
            index += 1;
            while index < chars.len() && is_delimiter(chars[index]) {
                index += 1;
            }
            parts.push(chars[start..index].iter().collect());
            continue;
        }

        current.push(chars[index]);
        index += 1;
    }

    if !current.is_empty() {
        parts.push(current);
    }

    parts
}

fn segment_has_token_timestamps(segment: &TranscriptSegment) -> bool {
    matches!(
        (segment.tokens.as_ref(), segment.timestamps.as_ref()),
        (Some(tokens), Some(timestamps)) if !tokens.is_empty() && tokens.len() == timestamps.len()
    )
}

fn build_token_map(segment: &TranscriptSegment) -> Option<TokenMap> {
    let tokens = segment.tokens.as_ref()?;
    let timestamps = segment.timestamps.as_ref()?;
    if tokens.is_empty() || tokens.len() != timestamps.len() {
        return None;
    }

    let mut start_indices = Vec::with_capacity(tokens.len());
    let mut end_indices = Vec::with_capacity(tokens.len());
    let mut current_len = 0usize;

    for token in tokens {
        let token_len = effective_length(token);
        start_indices.push(current_len);
        current_len += token_len;
        end_indices.push(current_len);
    }

    Some(TokenMap {
        start_indices,
        end_indices,
        timestamps: timestamps.clone(),
    })
}

fn find_timestamp_from_map(
    map: &TokenMap,
    effective_index: usize,
    hint_index: usize,
) -> Option<(f32, usize)> {
    if hint_index < map.start_indices.len()
        && map.start_indices[hint_index] <= effective_index
        && effective_index < map.end_indices[hint_index]
    {
        return Some((map.timestamps[hint_index], hint_index));
    }

    let next = hint_index + 1;
    if next < map.start_indices.len()
        && map.start_indices[next] <= effective_index
        && effective_index < map.end_indices[next]
    {
        return Some((map.timestamps[next], next));
    }

    let mut left = if hint_index < map.start_indices.len()
        && map.start_indices[hint_index] <= effective_index
    {
        hint_index
    } else {
        0
    };
    let mut right = map.start_indices.len().saturating_sub(1);
    let mut index = None;

    while left <= right {
        let mid = (left + right) / 2;
        if map.start_indices[mid] <= effective_index {
            index = Some(mid);
            left = mid + 1;
        } else if mid == 0 {
            break;
        } else {
            right = mid - 1;
        }
    }

    let found_index = index?;
    if effective_index >= map.end_indices[found_index] {
        return None;
    }

    Some((map.timestamps[found_index], found_index))
}

fn finalize_split_segment(
    mut segment: TranscriptSegment,
    is_first: bool,
    original_id: &str,
) -> TranscriptSegment {
    if !is_first {
        segment.id = uuid::Uuid::new_v4().to_string();
    } else {
        segment.id = original_id.to_string();
    }
    segment
}

fn split_segment_by_parts<F>(
    segment: &TranscriptSegment,
    is_delimiter: F,
    check_abbreviations: bool,
) -> Vec<TranscriptSegment>
where
    F: Fn(char) -> bool + Copy,
{
    let parts = split_text_parts(&segment.text, is_delimiter);
    if parts.len() <= 1 {
        return vec![segment.clone()];
    }

    let has_timestamps = segment_has_token_timestamps(segment);
    let token_map = if has_timestamps {
        build_token_map(segment)
    } else {
        None
    };
    let total_duration = segment.end - segment.start;
    let total_char_len = segment.text.chars().count().max(1);

    let mut state = SplitterState {
        current_text: String::new(),
        current_start: segment.start,
        current_segment_start: token_map
            .as_ref()
            .and_then(|map| map.timestamps.first().copied())
            .map(|value| value as f64)
            .unwrap_or(segment.start),
        char_index: 0,
        effective_char_index: 0,
        last_token_index: 0,
        next_token_slice_start: 0,
    };

    let mut results = Vec::new();
    let original_id = segment.id.clone();

    for part in parts {
        let part_effective_len = effective_length(&part);
        let is_delimiter_part = part.chars().next().map(is_delimiter).unwrap_or(false);

        if is_delimiter_part {
            let should_merge = check_abbreviations
                && part.contains('.')
                && ends_with_abbreviation(&state.current_text);
            state.current_text.push_str(&part);
            state.char_index += part.chars().count();
            state.effective_char_index += part_effective_len;

            if should_merge {
                continue;
            }

            let fallback_ratio = state.current_text.chars().count() as f64 / total_char_len as f64;
            let fallback_segment_end = state.current_start + fallback_ratio * total_duration;

            let mut segment_end = fallback_segment_end;
            let mut current_tokens = None;
            let mut current_timestamps = None;
            let mut current_durations = None;

            if let Some(map) = token_map.as_ref() {
                let mut slice_end = map.timestamps.len();
                if let Some((timestamp, found_index)) =
                    find_timestamp_from_map(map, state.effective_char_index, state.last_token_index)
                {
                    slice_end = found_index;
                    segment_end = timestamp as f64;
                    state.last_token_index = found_index;
                }

                if slice_end > state.next_token_slice_start {
                    if let Some(tokens) = segment.tokens.as_ref() {
                        current_tokens =
                            Some(tokens[state.next_token_slice_start..slice_end].to_vec());
                    }
                    if let Some(timestamps) = segment.timestamps.as_ref() {
                        current_timestamps =
                            Some(timestamps[state.next_token_slice_start..slice_end].to_vec());
                    }
                    if let Some(durations) = segment
                        .durations
                        .as_ref()
                        .filter(|values| values.len() == map.timestamps.len())
                    {
                        current_durations =
                            Some(durations[state.next_token_slice_start..slice_end].to_vec());
                    }
                    state.next_token_slice_start = slice_end;
                }

                if let Some(timestamps) = current_timestamps
                    .as_ref()
                    .filter(|values| !values.is_empty())
                {
                    state.current_segment_start = timestamps[0] as f64;
                }
            }

            let child = TranscriptSegment {
                id: original_id.clone(),
                text: state.current_text.trim().to_string(),
                start: state.current_segment_start,
                end: segment_end.max(state.current_segment_start),
                is_final: true,
                timing: None,
                tokens: current_tokens,
                timestamps: current_timestamps,
                durations: current_durations,
                translation: segment.translation.clone(),
                speaker: segment.speaker.clone(),
            };

            if !child.text.is_empty() {
                results.push(finalize_split_segment(
                    child,
                    results.is_empty(),
                    &original_id,
                ));
            }

            state.current_start = segment_end;
            state.current_segment_start = segment_end;
            state.current_text.clear();

            if let Some(map) = token_map.as_ref() {
                if state.last_token_index == state.next_token_slice_start
                    && state.next_token_slice_start < map.timestamps.len()
                {
                    state.current_segment_start =
                        map.timestamps[state.next_token_slice_start] as f64;
                    state.current_start = state.current_segment_start;
                }
            }

            continue;
        }

        state.current_text.push_str(&part);
        state.char_index += part.chars().count();
        state.effective_char_index += part_effective_len;
    }

    if !state.current_text.trim().is_empty() {
        let mut current_tokens = None;
        let mut current_timestamps = None;
        let mut current_durations = None;

        if let Some(map) = token_map.as_ref() {
            if let Some(tokens) = segment.tokens.as_ref() {
                current_tokens = Some(tokens[state.next_token_slice_start..].to_vec());
            }
            if let Some(timestamps) = segment.timestamps.as_ref() {
                current_timestamps = Some(timestamps[state.next_token_slice_start..].to_vec());
            }
            if let Some(durations) = segment
                .durations
                .as_ref()
                .filter(|values| values.len() == map.timestamps.len())
            {
                current_durations = Some(durations[state.next_token_slice_start..].to_vec());
            }
            if let Some(timestamps) = current_timestamps
                .as_ref()
                .filter(|values| !values.is_empty())
            {
                state.current_segment_start = timestamps[0] as f64;
            }
        }

        let child = TranscriptSegment {
            id: original_id.clone(),
            text: state.current_text.trim().to_string(),
            start: state.current_segment_start,
            end: segment.end,
            is_final: true,
            timing: None,
            tokens: current_tokens,
            timestamps: current_timestamps,
            durations: current_durations,
            translation: segment.translation.clone(),
            speaker: segment.speaker.clone(),
        };

        if !child.text.is_empty() {
            results.push(finalize_split_segment(
                child,
                results.is_empty(),
                &original_id,
            ));
        }
    }

    if results.is_empty() {
        vec![segment.clone()]
    } else {
        results
    }
}

fn split_segment_by_punctuation_rules(segment: &TranscriptSegment) -> Vec<TranscriptSegment> {
    let first_pass = split_segment_by_parts(segment, is_strong_split_char, true);
    let mut final_segments = Vec::new();

    for segment in first_pass {
        let limit = if contains_cjk(&segment.text) {
            MAX_SEGMENT_LENGTH_CJK
        } else {
            MAX_SEGMENT_LENGTH_WESTERN
        };

        if segment.text.chars().count() > limit {
            final_segments.extend(split_segment_by_parts(&segment, is_weak_split_char, false));
        } else {
            final_segments.push(segment);
        }
    }

    final_segments
}

fn apply_timeline_normalization(
    segments: Vec<TranscriptSegment>,
    options: TranscriptNormalizationOptions,
) -> Vec<TranscriptSegment> {
    if !options.enable_timeline {
        return normalize_transcript_segments(segments);
    }

    let mut results = Vec::new();
    for segment in segments {
        if segment.is_final {
            results.extend(split_segment_by_punctuation_rules(&segment));
        } else {
            results.push(segment);
        }
    }

    results.sort_by(|left, right| {
        left.start
            .partial_cmp(&right.start)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                left.end
                    .partial_cmp(&right.end)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    normalize_transcript_segments(results)
}

fn build_transcript_update(
    segment: TranscriptSegment,
    options: TranscriptNormalizationOptions,
) -> TranscriptUpdate {
    let remove_ids = if options.enable_timeline && segment.is_final {
        vec![segment.id.clone()]
    } else {
        Vec::new()
    };

    TranscriptUpdate {
        remove_ids,
        upsert_segments: apply_timeline_normalization(vec![segment], options),
    }
}

fn emit_transcript_update<R: tauri::Runtime>(
    app: &AppHandle<R>,
    instance_id: &str,
    update: &TranscriptUpdate,
    stage: &str,
    first_segment_emitted: Option<&Arc<AtomicBool>>,
) {
    let event_name = recognizer_output_event(instance_id);
    for segment in &update.upsert_segments {
        log_segment_emit_diagnostics(instance_id, first_segment_emitted, segment, stage);
    }
    let _ = app.emit(&event_name, update);
}

fn format_transcript(text: &str, punctuation: Option<&Punctuation>) -> String {
    let mut result = text.trim().to_string();
    if result.is_empty() {
        return result;
    }

    let has_ascii_letters = result.chars().any(|c| c.is_ascii_alphabetic());
    let is_all_caps = has_ascii_letters && result == result.to_uppercase();

    if is_all_caps {
        let mut chars = result.chars();
        if let Some(first) = chars.next() {
            let lower = chars.as_str().to_lowercase();
            result = first.to_uppercase().collect::<String>() + &lower;
        }
    }

    if let Some(p) = punctuation {
        result = p.add_punct(&result);
    }
    result
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

fn is_meaningful_text_char(ch: char) -> bool {
    ch.is_alphanumeric()
}

fn extract_meaningful_text(text: &str) -> String {
    text.chars()
        .filter(|ch| is_meaningful_text_char(*ch))
        .collect()
}

fn extract_ascii_digits(text: &str) -> String {
    text.chars().filter(|ch| ch.is_ascii_digit()).collect()
}

fn is_preservable_trailing_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '。' | '，'
            | '！'
            | '？'
            | '：'
            | '；'
            | '、'
            | '.'
            | ','
            | '!'
            | '?'
            | ':'
            | ';'
            | ')'
            | '）'
            | ']'
            | '】'
            | '}'
            | '」'
            | '』'
            | '》'
            | '〉'
            | '"'
            | '\''
            | '”'
            | '’'
    )
}

fn extract_trailing_punctuation(text: &str) -> String {
    let trimmed = text.trim_end();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut start = trimmed.len();
    for (idx, ch) in trimmed.char_indices().rev() {
        if is_preservable_trailing_punctuation(ch) {
            start = idx;
        } else {
            break;
        }
    }

    if start < trimmed.len() {
        trimmed[start..].to_string()
    } else {
        String::new()
    }
}

fn merge_cleaned_text_with_trailing_punctuation(
    cleaned_text: &str,
    formatted_text: &str,
) -> String {
    let mut result = cleaned_text.trim().to_string();
    let trailing_punctuation = extract_trailing_punctuation(formatted_text);

    if !trailing_punctuation.is_empty() && !result.ends_with(&trailing_punctuation) {
        result.push_str(&trailing_punctuation);
    }

    result
}

fn should_fallback_to_cleaned_text(cleaned_text: &str, formatted_text: &str) -> bool {
    let cleaned_meaningful = extract_meaningful_text(cleaned_text);
    if cleaned_meaningful.is_empty() {
        return false;
    }

    let formatted_meaningful = extract_meaningful_text(formatted_text);
    if formatted_meaningful.is_empty() {
        return true;
    }

    let cleaned_digits = extract_ascii_digits(cleaned_text);
    if !cleaned_digits.is_empty() && extract_ascii_digits(formatted_text) != cleaned_digits {
        return true;
    }

    false
}

fn select_final_transcript_text(cleaned_text: &str, formatted_text: &str) -> String {
    let normalized_formatted = normalize_recognizer_text(formatted_text);
    if should_fallback_to_cleaned_text(cleaned_text, &normalized_formatted) {
        return merge_cleaned_text_with_trailing_punctuation(cleaned_text, &normalized_formatted);
    }

    normalized_formatted
}

fn finalize_transcript_text(cleaned_text: &str, punctuation: Option<&Punctuation>) -> String {
    let formatted_text = format_transcript(cleaned_text, punctuation);
    select_final_transcript_text(cleaned_text, &formatted_text)
}

fn preview_text_for_log(text: &str) -> String {
    const MAX_PREVIEW_CHARS: usize = 24;
    let flattened = text.replace('\r', " ").replace('\n', " ");
    let mut preview = flattened
        .chars()
        .take(MAX_PREVIEW_CHARS)
        .collect::<String>();
    if flattened.chars().count() > MAX_PREVIEW_CHARS {
        preview.push('…');
    }
    preview
}

fn log_text_transform_diagnostics(
    instance_id: &str,
    stage: &str,
    segment_id: &str,
    is_final: bool,
    raw_text: &str,
    cleaned_text: &str,
    final_text: &str,
) {
    let Some(label) = diagnostics_instance_label(instance_id) else {
        return;
    };

    info!(
        "[Sherpa] {label} text transform. stage={} segment_id={} final={} raw_len={} cleaned_len={} final_len={} raw_preview={:?} cleaned_preview={:?} final_preview={:?}",
        stage,
        segment_id,
        is_final,
        raw_text.chars().count(),
        cleaned_text.chars().count(),
        final_text.chars().count(),
        preview_text_for_log(raw_text),
        preview_text_for_log(cleaned_text),
        preview_text_for_log(final_text)
    );
}

fn synthesize_durations(timestamps: &[f32], end_time: f32) -> Option<Vec<f32>> {
    if timestamps.is_empty() {
        return None;
    }
    let mut durations = Vec::with_capacity(timestamps.len());
    for i in 0..timestamps.len() {
        let next_time = if i + 1 < timestamps.len() {
            timestamps[i + 1]
        } else {
            end_time
        };
        durations.push(next_time - timestamps[i]);
    }
    Some(durations)
}

#[allow(clippy::too_many_arguments)]
fn run_offline_inference<R: tauri::Runtime>(
    speech_buffer: &[Vec<f32>],
    app: &AppHandle<R>,
    r: &sherpa_onnx::OfflineRecognizer,
    punctuation: Option<&Punctuation>,
    segment_id: &str,
    global_start: f64,
    is_final: bool,
    instance_id: &str,
    stage: &'static str,
    first_segment_emitted: Option<Arc<AtomicBool>>,
    normalization_options: TranscriptNormalizationOptions,
) {
    if speech_buffer.is_empty() {
        if let Some(label) = diagnostics_instance_label(instance_id) {
            info!(
                "[Sherpa] {label} offline inference skipped because the speech buffer is empty. stage={stage}"
            );
        }
        return;
    }

    // Offline models decode one aggregated utterance at a time, so we flatten
    // the buffered speech chunks into one continuous waveform before calling
    // Sherpa.
    let mut full_audio = Vec::new();
    for chunk in speech_buffer {
        full_audio.extend_from_slice(chunk);
    }
    let stream = r.create_stream();
    debug!(
        "[Offline] FFI: Calling accept_waveform (Offline) with {} samples",
        full_audio.len()
    );
    stream.accept_waveform(16000, &full_audio);

    debug!("[Offline] FFI: Calling decode");
    r.decode(&stream);
    debug!("[Offline] FFI: Decode finished");

    if let Some(label) = diagnostics_instance_label(instance_id) {
        info!(
            "[Sherpa] {label} offline inference finished. stage={} segment_id={} final={} buffered_chunks={} buffered_samples={}",
            stage, segment_id, is_final, speech_buffer.len(), full_audio.len()
        );
    }

    if let Some(result) = stream.get_result() {
        let raw_text = result.text.trim();
        if !raw_text.is_empty() {
            // The offline path emits only meaningful text: raw recognizer output
            // is normalized first, then final-only formatting/punctuation is
            // applied when this segment closes an utterance.
            let cleaned_text = normalize_recognizer_text(&result.text);
            if cleaned_text.is_empty() {
                if let Some(label) = diagnostics_instance_label(instance_id) {
                    info!(
                        "[Sherpa] {label} offline inference produced empty text after normalization. stage={} segment_id={} final={} raw_preview={:?}",
                        stage,
                        segment_id,
                        is_final,
                        preview_text_for_log(raw_text)
                    );
                }
                return;
            }

            let text = if is_final {
                finalize_transcript_text(&cleaned_text, punctuation)
            } else {
                cleaned_text.clone()
            };

            if text.is_empty() {
                if let Some(label) = diagnostics_instance_label(instance_id) {
                    info!(
                        "[Sherpa] {label} offline inference produced empty output text after normalization/formatting. stage={} segment_id={} final={} raw_preview={:?} cleaned_preview={:?}",
                        stage,
                        segment_id,
                        is_final,
                        preview_text_for_log(raw_text),
                        preview_text_for_log(&cleaned_text)
                    );
                }
                return;
            }

            log_text_transform_diagnostics(
                instance_id,
                stage,
                segment_id,
                is_final,
                raw_text,
                &cleaned_text,
                &text,
            );

            let global_end = global_start + (full_audio.len() as f64 / 16000.0);
            // Sherpa timestamps are relative to the decoded utterance, so we
            // shift them into the global recording timeline before emitting.
            let timestamps_abs: Option<Vec<f32>> = result
                .timestamps
                .as_ref()
                .map(|ts| ts.iter().map(|t| *t + global_start as f32).collect());
            let durations = timestamps_abs
                .as_ref()
                .and_then(|ts| synthesize_durations(ts, global_end as f32));

            let segment = TranscriptSegment {
                id: segment_id.to_string(),
                text,
                start: global_start,
                end: global_end,
                is_final,
                timing: None,
                tokens: Some(result.tokens),
                timestamps: timestamps_abs,
                durations,
                translation: None,
                speaker: None,
            };
            let update = build_transcript_update(segment, normalization_options);
            emit_transcript_update(
                app,
                instance_id,
                &update,
                stage,
                first_segment_emitted.as_ref(),
            );
        } else if let Some(label) = diagnostics_instance_label(instance_id) {
            info!(
                "[Sherpa] {label} offline inference produced empty text after formatting. stage={} segment_id={} final={}",
                stage, segment_id, is_final
            );
        }
    } else if let Some(label) = diagnostics_instance_label(instance_id) {
        info!(
            "[Sherpa] {label} offline inference produced no recognizer result. stage={} segment_id={} final={}",
            stage, segment_id, is_final
        );
    }
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn init_recognizer(
    state: State<'_, SherpaState>,
    instance_id: String,
    model_path: String,
    num_threads: i32,
    enable_itn: bool,
    language: String,
    punctuation_model: Option<String>,
    vad_model: Option<String>,
    vad_buffer: f32,
    model_type: String,
    file_config: Option<ModelFileConfig>,
    hotwords: Option<String>,
    normalization_options: Option<TranscriptNormalizationOptions>,
) -> Result<(), String> {
    info!(
        "[init_recognizer] start instance_id={instance_id} model_path={model_path} model_type={model_type} num_threads={num_threads} enable_itn={enable_itn} language={language} punctuation_model={:?} vad_model={:?} vad_buffer={vad_buffer} hotwords={:?}",
        punctuation_model,
        vad_model,
        hotwords
    );

    let config_key = ModelConfigKey {
        model_path: model_path.clone(),
        model_type: model_type.clone(),
        num_threads,
        enable_itn,
        language: language.clone(),
        hotwords: hotwords.clone(),
    };

    let recognizer = {
        let mut pool = state.recognizer_pool.lock().await;
        if let Some(r) = pool.get(&config_key) {
            // Heavy recognizers are reused across logical instances when their
            // model path and runtime knobs match exactly.
            info!("[init_recognizer] Reusing existing recognizer from pool");
            r.clone()
        } else {
            info!("[init_recognizer] Creating new recognizer and adding to pool");
            let config_type = build_model_config(
                Path::new(&model_path),
                &model_type,
                &file_config,
                enable_itn,
                &language,
                hotwords,
            )?;
            let r = Arc::new(Recognizer::new(config_type, num_threads)?);
            pool.insert(config_key, r.clone());
            r
        }
    };

    let punctuation = load_punctuation(punctuation_model);
    let vad = load_vad(vad_model.clone());

    let mut instances = state.instances.lock().await;
    let instance = instances
        .entry(instance_id)
        .or_insert_with(SherpaInstance::default);

    // Instance-local attachments can differ even when the core recognizer is
    // shared, so VAD/punctuation/runtime settings are refreshed here.
    instance.recognizer = Some(recognizer);
    instance.vad = vad;
    instance.punctuation = punctuation.map(Arc::new);
    instance.vad_model = vad_model.clone();
    instance.vad_buffer = vad_buffer;
    instance.normalization_options = normalization_options.unwrap_or_default();

    Ok(())
}

#[tauri::command]
pub async fn start_recognizer(
    state: State<'_, SherpaState>,
    instance_id: String,
) -> Result<(), String> {
    let mut instances = state.instances.lock().await;
    let instance = instances
        .get_mut(&instance_id)
        .ok_or("Instance not found")?;

    let Some(recognizer) = instance.recognizer.as_ref() else {
        return Err("Recognizer not initialized".to_string());
    };
    let recognizer_kind = match &recognizer.inner {
        RecognizerInner::Offline(_) => "offline",
        RecognizerInner::Online(_) => "online",
    };

    let stream = match &recognizer.inner {
        RecognizerInner::Online(r) => Some(SafeStream(r.0.create_stream())),
        _ => None,
    };

    // Starting a run resets transient buffers and, for online models, creates a
    // fresh Sherpa stream that will accumulate new incremental state.
    start_instance_runtime(instance, stream);

    if instance.vad_model.is_some() {
        // Reload VAD per start so any prior run-specific detector state cannot
        // bleed into the next recording/caption session.
        instance.vad = load_vad(instance.vad_model.clone());
    }

    if let Some(label) = diagnostics_instance_label(&instance_id) {
        info!(
            "[Sherpa] start_recognizer({label}): is_running=true recognizer_kind={} vad_configured={} punctuation_loaded={}",
            recognizer_kind,
            instance.vad_model.is_some(),
            instance.punctuation.is_some()
        );
    }

    Ok(())
}

#[tauri::command]
pub async fn stop_recognizer(
    state: State<'_, SherpaState>,
    instance_id: String,
) -> Result<(), String> {
    let mut instances = state.instances.lock().await;
    if let Some(instance) = instances.get_mut(&instance_id) {
        if let Some(label) = diagnostics_instance_label(&instance_id) {
            info!(
                "[Sherpa] stop_recognizer({label}): was_running={} total_samples={} buffered_chunks={} buffered_samples={} current_segment={} emitted_any={}",
                instance.is_running,
                instance.total_samples,
                instance.offline_state.speech_buffer.len(),
                buffered_sample_count(&instance.offline_state.speech_buffer),
                instance.current_segment_id.as_deref().unwrap_or("none"),
                instance
                    .record_diagnostics
                    .first_segment_emitted
                    .load(Ordering::SeqCst)
            );
        }
        stop_instance_runtime(instance);
    }
    Ok(())
}

#[tauri::command]
pub async fn flush_recognizer<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, SherpaState>,
    instance_id: String,
) -> Result<(), String> {
    info!("Flushing recognizer for instance id: {}", instance_id);
    let mut instances = state.instances.lock().await;
    let instance = instances
        .get_mut(&instance_id)
        .ok_or("Instance not found")?;

    if let Some(label) = diagnostics_instance_label(&instance_id) {
        info!(
            "[Sherpa] flush_recognizer({label}): is_running={} total_samples={} buffered_chunks={} buffered_samples={} current_segment={} speaking={}",
            instance.is_running,
            instance.total_samples,
            instance.offline_state.speech_buffer.len(),
            buffered_sample_count(&instance.offline_state.speech_buffer),
            instance.current_segment_id.as_deref().unwrap_or("none"),
            instance.offline_state.is_speaking
        );
    }

    if let Some(recognizer) = instance.recognizer.clone() {
        if let RecognizerInner::Offline(_) = &recognizer.inner {
            if !instance.offline_state.speech_buffer.is_empty() {
                let seg_id = instance
                    .current_segment_id
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                let global_start = instance.offline_state.utterance_start_sample as f64 / 16000.0;

                let offline_copy = instance.offline_state.speech_buffer.clone();
                let app_copy = app.clone();
                let recognizer_copy = recognizer.clone();
                let punct_copy = instance.punctuation.clone();
                let seg_id_copy = seg_id.clone();
                let instance_id_copy = instance_id.clone();
                let first_segment_emitted = diagnostics_instance_label(&instance_id)
                    .is_some()
                    .then(|| instance.record_diagnostics.first_segment_emitted.clone());
                let normalization_options = instance.normalization_options;

                if let Some(label) = diagnostics_instance_label(&instance_id) {
                    info!(
                        "[Sherpa] {label} flush triggering offline inference. segment_id={} buffered_chunks={} buffered_samples={}",
                        seg_id, offline_copy.len(), buffered_sample_count(&offline_copy)
                    );
                }

                // Offline decoding can be CPU-heavy, so the final utterance pass
                // runs on a blocking worker and then emits one final segment.
                tauri::async_runtime::spawn_blocking(move || {
                    if let RecognizerInner::Offline(safe_r) = &recognizer_copy.inner {
                        run_offline_inference(
                            &offline_copy,
                            &app_copy,
                            &safe_r.0,
                            punct_copy.as_deref(),
                            &seg_id_copy,
                            global_start,
                            true,
                            &instance_id_copy,
                            "flush_offline",
                            first_segment_emitted,
                            normalization_options,
                        );
                    }
                })
                .await
                .map_err(|e| e.to_string())?;

                instance.offline_state.speech_buffer.clear();
                instance.offline_state.is_speaking = false;
            } else if let Some(label) = diagnostics_instance_label(&instance_id) {
                info!("[Sherpa] {label} flush found no pending offline speech buffer.");
            }
            instance.current_segment_id = None;
            instance.offline_state = OfflineState::default();
            if let Some(label) = diagnostics_instance_label(&instance_id) {
                info!("[Sherpa] flush_recognizer({label}) complete. mode=offline");
            }
            return Ok(());
        }
    }

    if let (Some(recognizer), Some(st)) = (instance.recognizer.as_deref(), instance.stream.as_ref())
    {
        if let RecognizerInner::Online(r) = &recognizer.inner {
            let current_time = instance.total_samples as f64 / 16000.0;

            // Online models need a short tail of silence to finalize the last
            // partial hypothesis before we reset the stream.
            let tail_padding = vec![0.0; (16000.0 * 0.8) as usize];
            debug!("FFI: Calling accept_waveform (Online, tail_padding)");
            st.0.accept_waveform(16000, &tail_padding);
            debug!("FFI: Successfully returned from accept_waveform (Online, tail_padding)");
            while r.0.is_ready(&st.0) {
                r.0.decode(&st.0);
            }

            if let Some(result) = r.0.get_result(&st.0) {
                if !result.text.trim().is_empty() {
                    let text = format_transcript(&result.text, instance.punctuation.as_deref());
                    let timestamps_abs = result.timestamps.as_ref().map(|ts| {
                        ts.iter()
                            .map(|t| *t + instance.segment_start_time as f32)
                            .collect::<Vec<_>>()
                    });
                    let durations = timestamps_abs
                        .as_ref()
                        .and_then(|ts| synthesize_durations(ts, current_time as f32));

                    let id = instance
                        .current_segment_id
                        .take()
                        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

                    let segment = TranscriptSegment {
                        id,
                        text,
                        start: instance.segment_start_time,
                        end: current_time,
                        is_final: true,
                        timing: None,
                        tokens: Some(result.tokens),
                        timestamps: timestamps_abs,
                        durations,
                        translation: None,
                        speaker: None,
                    };
                    let update = build_transcript_update(segment, instance.normalization_options);
                    emit_transcript_update(
                        &app,
                        &instance_id,
                        &update,
                        "flush_online",
                        Some(&instance.record_diagnostics.first_segment_emitted),
                    );
                }
            }

            instance.current_segment_id = None;
            r.0.reset(&st.0);
            instance.segment_start_time = current_time;
            if let Some(label) = diagnostics_instance_label(&instance_id) {
                info!("[Sherpa] flush_recognizer({label}) complete. mode=online");
            }
        }
    }

    Ok(())
}

pub async fn feed_audio_samples<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &SherpaState,
    instance_id: &str,
    samples: &[f32],
) -> Result<(), String> {
    let mut instances = state.instances.lock().await;
    let instance = instances.get_mut(instance_id).ok_or("Instance not found")?;

    if !instance.is_running {
        if let Some(label) = diagnostics_instance_label(instance_id) {
            if !instance.record_diagnostics.skipped_while_stopped_logged {
                println!(
                    "[Sherpa] {label} audio chunk skipped because recognizer is not running. samples={} total_samples={}",
                    samples.len(),
                    instance.total_samples
                );
                instance.record_diagnostics.skipped_while_stopped_logged = true;
            }
        }
        return Ok(());
    }

    if let Some(label) = diagnostics_instance_label(instance_id) {
        if !instance.record_diagnostics.first_sample_logged {
            println!(
                "[Sherpa] {label} first sample received. samples={} total_samples_before={} current_segment={}",
                samples.len(),
                instance.total_samples,
                instance.current_segment_id.as_deref().unwrap_or("none")
            );
            instance.record_diagnostics.first_sample_logged = true;
        }
    }

    let recognizer = instance
        .recognizer
        .clone()
        .ok_or("Recognizer not initialized")?;

    match &recognizer.inner {
        RecognizerInner::Offline(_) => {
            let Some(SafeVad(vad)) = instance.vad.as_ref() else {
                println!(
                    "[Sherpa] feed_audio_samples: VAD model is missing for instance {}",
                    instance_id
                );
                return Err("VAD model is missing or not configured. This model requires VAD for live transcription. Please download the Silero VAD model in Settings -> Model Center.".to_string());
            };

            // Offline live transcription is VAD-driven: we keep feeding audio to
            // the detector, grow/trim utterance buffers, and only run full
            // recognizer inference when a speech segment boundary is reached.
            vad.accept_waveform(samples);
            let currently_speaking = vad.detected();

            if let Some(label) = diagnostics_instance_label(instance_id) {
                if instance.total_samples % 160000 < 2000 {
                    // Print once every ~10 seconds
                    println!(
                        "[Sherpa] instance '{label}' running, total_samples: {}, currently_speaking: {}, emitted_any: {}",
                        instance.total_samples,
                        currently_speaking,
                        instance
                            .record_diagnostics
                            .first_segment_emitted
                            .load(Ordering::SeqCst)
                    );
                }
            }

            if instance.current_segment_id.is_none() {
                instance.current_segment_id = Some(uuid::Uuid::new_v4().to_string());
            }
            let seg_id = instance.current_segment_id.as_ref().unwrap().clone();

            if currently_speaking && !instance.offline_state.is_speaking {
                if let Some(label) = diagnostics_instance_label(instance_id) {
                    let ring_buffer_samples: usize = instance
                        .offline_state
                        .ring_buffer
                        .iter()
                        .map(|chunk| chunk.len())
                        .sum();
                    println!(
                        "[Sherpa] {label} detected speech start. segment_id={} total_samples={} ring_buffer_samples={}",
                        seg_id, instance.total_samples, ring_buffer_samples
                    );
                } else {
                    println!("[Sherpa] Instance {} detected speech start.", instance_id);
                }
                instance.offline_state.is_speaking = true;

                let samples_to_keep = (16000.0 * 0.3) as usize;
                let mut context_len = 0;

                if !instance.offline_state.ring_buffer.is_empty() {
                    let ring_flat: Vec<f32> = instance
                        .offline_state
                        .ring_buffer
                        .iter()
                        .flatten()
                        .copied()
                        .collect();

                    let keep_start = ring_flat.len().saturating_sub(samples_to_keep);

                    let context = ring_flat[keep_start..].to_vec();
                    context_len = context.len();
                    instance.offline_state.speech_buffer.push(context);
                }

                instance.offline_state.utterance_start_sample =
                    instance.total_samples - context_len;
                instance.offline_state.ring_buffer.clear();
            }

            if currently_speaking {
                instance.offline_state.speech_buffer.push(samples.to_vec());

                let now = std::time::Instant::now();
                if now
                    .duration_since(instance.offline_state.last_inference_time)
                    .as_millis()
                    > 200
                {
                    let global_start =
                        instance.offline_state.utterance_start_sample as f64 / 16000.0;

                    let offline_copy = instance.offline_state.speech_buffer.clone();
                    let app_copy = app.clone();
                    let punct_copy = instance.punctuation.clone();
                    let seg_id_copy = seg_id.clone();
                    let instance_id_copy = instance_id.to_string();
                    let recognizer_copy = recognizer.clone();
                    let first_segment_emitted = diagnostics_instance_label(instance_id)
                        .is_some()
                        .then(|| instance.record_diagnostics.first_segment_emitted.clone());
                    let normalization_options = instance.normalization_options;

                    if let Some(label) = diagnostics_instance_label(instance_id) {
                        println!(
                            "[Sherpa] {label} triggering offline inference. stage=partial segment_id={} buffered_chunks={} buffered_samples={} global_start={:.3}",
                            seg_id,
                            offline_copy.len(),
                            buffered_sample_count(&offline_copy),
                            global_start
                        );
                    }

                    tauri::async_runtime::spawn_blocking(move || {
                        if let RecognizerInner::Offline(safe_r) = &recognizer_copy.inner {
                            run_offline_inference(
                                &offline_copy,
                                &app_copy,
                                &safe_r.0,
                                punct_copy.as_deref(),
                                &seg_id_copy,
                                global_start,
                                false,
                                &instance_id_copy,
                                "partial",
                                first_segment_emitted,
                                normalization_options,
                            );
                        }
                    });
                    instance.offline_state.last_inference_time = now;
                }
            }

            if !currently_speaking {
                if instance.offline_state.is_speaking {
                    if let Some(label) = diagnostics_instance_label(instance_id) {
                        println!(
                            "[Sherpa] {label} detected speech end. segment_id={} total_samples={} buffered_chunks={} buffered_samples={}",
                            seg_id,
                            instance.total_samples,
                            instance.offline_state.speech_buffer.len() + 1,
                            buffered_sample_count(&instance.offline_state.speech_buffer)
                                + samples.len()
                        );
                    } else {
                        println!("[Sherpa] Instance {} detected speech end.", instance_id);
                    }
                    instance.offline_state.is_speaking = false;
                    instance.offline_state.speech_buffer.push(samples.to_vec());

                    let global_start =
                        instance.offline_state.utterance_start_sample as f64 / 16000.0;

                    let offline_copy = instance.offline_state.speech_buffer.clone();
                    let app_copy = app.clone();
                    let punct_copy = instance.punctuation.clone();
                    let seg_id_copy = seg_id.clone();
                    let instance_id_copy = instance_id.to_string();
                    let recognizer_copy = recognizer.clone();
                    let first_segment_emitted = diagnostics_instance_label(instance_id)
                        .is_some()
                        .then(|| instance.record_diagnostics.first_segment_emitted.clone());
                    let normalization_options = instance.normalization_options;

                    if let Some(label) = diagnostics_instance_label(instance_id) {
                        println!(
                            "[Sherpa] {label} triggering offline inference. stage=final segment_id={} buffered_chunks={} buffered_samples={} global_start={:.3}",
                            seg_id,
                            offline_copy.len(),
                            buffered_sample_count(&offline_copy),
                            global_start
                        );
                    }

                    tauri::async_runtime::spawn_blocking(move || {
                        if let RecognizerInner::Offline(safe_r) = &recognizer_copy.inner {
                            run_offline_inference(
                                &offline_copy,
                                &app_copy,
                                &safe_r.0,
                                punct_copy.as_deref(),
                                &seg_id_copy,
                                global_start,
                                true,
                                &instance_id_copy,
                                "final",
                                first_segment_emitted,
                                normalization_options,
                            );
                        }
                    });

                    instance.offline_state.speech_buffer.clear();
                    instance.current_segment_id = Some(uuid::Uuid::new_v4().to_string());
                }

                instance
                    .offline_state
                    .ring_buffer
                    .push_back(samples.to_vec());
                let max_ring_samples = (16000.0 * 0.3) as usize;

                let mut ring_len: usize = instance
                    .offline_state
                    .ring_buffer
                    .iter()
                    .map(|v| v.len())
                    .sum();
                while ring_len > max_ring_samples + 4000 {
                    if let Some(first) = instance.offline_state.ring_buffer.front() {
                        let first_len = first.len();
                        if ring_len - first_len >= max_ring_samples {
                            instance.offline_state.ring_buffer.pop_front();
                            ring_len -= first_len;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }

            instance.total_samples += samples.len();
            Ok(())
        }
        RecognizerInner::Online(r) => {
            let st = instance
                .stream
                .as_ref()
                .ok_or("Stream not initialized for online model")?;

            st.0.accept_waveform(16000, samples);
            instance.total_samples += samples.len();

            while r.0.is_ready(&st.0) {
                r.0.decode(&st.0);
            }

            let current_time = instance.total_samples as f64 / 16000.0;

            if let Some(result) = r.0.get_result(&st.0) {
                let has_text = !result.text.trim().is_empty();
                if has_text || instance.current_segment_id.is_some() {
                    let id = instance
                        .current_segment_id
                        .get_or_insert_with(|| uuid::Uuid::new_v4().to_string())
                        .clone();
                    let timestamps_abs = result.timestamps.as_ref().map(|ts| {
                        ts.iter()
                            .map(|t| *t + instance.segment_start_time as f32)
                            .collect::<Vec<_>>()
                    });
                    let durations = timestamps_abs
                        .as_ref()
                        .and_then(|ts| synthesize_durations(ts, current_time as f32));

                    let segment = TranscriptSegment {
                        id,
                        text: result.text.clone(),
                        start: instance.segment_start_time,
                        end: current_time,
                        is_final: false,
                        timing: None,
                        tokens: Some(result.tokens.clone()),
                        timestamps: timestamps_abs,
                        durations,
                        translation: None,
                        speaker: None,
                    };
                    let update = build_transcript_update(segment, instance.normalization_options);
                    emit_transcript_update(
                        app,
                        instance_id,
                        &update,
                        "online_partial",
                        Some(&instance.record_diagnostics.first_segment_emitted),
                    );
                }
            }

            if r.0.is_endpoint(&st.0) {
                let tail_padding = vec![0.0; (16000.0 * 0.8) as usize];
                debug!("FFI: Calling accept_waveform (Online, tail_padding)");
                st.0.accept_waveform(16000, &tail_padding);
                debug!("FFI: Successfully returned from accept_waveform (Online, tail_padding)");
                while r.0.is_ready(&st.0) {
                    r.0.decode(&st.0);
                }

                if let Some(result) = r.0.get_result(&st.0) {
                    if !result.text.trim().is_empty() {
                        let text = format_transcript(&result.text, instance.punctuation.as_deref());

                        let timestamps_abs = result.timestamps.as_ref().map(|ts| {
                            ts.iter()
                                .map(|t| *t + instance.segment_start_time as f32)
                                .collect::<Vec<_>>()
                        });
                        let durations = timestamps_abs
                            .as_ref()
                            .and_then(|ts| synthesize_durations(ts, current_time as f32));

                        let id = instance
                            .current_segment_id
                            .take()
                            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

                        let segment = TranscriptSegment {
                            id,
                            text,
                            start: instance.segment_start_time,
                            end: current_time,
                            is_final: true,
                            timing: None,
                            tokens: Some(result.tokens),
                            timestamps: timestamps_abs,
                            durations,
                            translation: None,
                            speaker: None,
                        };
                        let update =
                            build_transcript_update(segment, instance.normalization_options);
                        emit_transcript_update(
                            app,
                            instance_id,
                            &update,
                            "online_final",
                            Some(&instance.record_diagnostics.first_segment_emitted),
                        );
                    }
                }

                instance.current_segment_id = None;
                r.0.reset(&st.0);
                instance.segment_start_time = current_time;
            }

            Ok(())
        }
    }
}

#[tauri::command]
pub async fn feed_audio_chunk<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, SherpaState>,
    instance_id: String,
    samples: Vec<u8>,
) -> Result<(), String> {
    trace!(
        "feed_audio_chunk called with id: {}, samples bytes: {}",
        instance_id,
        samples.len()
    );
    let mut float_samples = Vec::with_capacity(samples.len() / 2);
    for chunk in samples.chunks_exact(2) {
        let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
        float_samples.push(sample as f32 / 32768.0);
    }
    feed_audio_samples(&app, &state, &instance_id, &float_samples).await
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn process_batch_file<R: tauri::Runtime>(
    app: AppHandle<R>,
    file_path: String,
    save_to_path: Option<String>,
    model_path: String,
    num_threads: i32,
    enable_itn: bool,
    language: String,
    punctuation_model: Option<String>,
    vad_model: Option<String>,
    vad_buffer: f32,
    model_type: String,
    file_config: Option<ModelFileConfig>,
    hotwords: Option<String>,
    speaker_processing: Option<crate::speaker::SpeakerProcessingConfig>,
    normalization_options: Option<TranscriptNormalizationOptions>,
) -> Result<Vec<TranscriptSegment>, String> {
    let request = BatchTranscriptionRequest {
        file_path,
        save_to_path,
        model_path,
        num_threads,
        enable_itn,
        language,
        punctuation_model,
        vad_model,
        vad_buffer,
        model_type,
        file_config,
        hotwords,
        speaker_processing,
        normalization_options: normalization_options.unwrap_or_default(),
    };
    let progress_file_path = request.file_path.clone();

    transcribe_batch_with_progress(&request, |progress| {
        let _ = app.emit(BATCH_PROGRESS_EVENT, &(progress_file_path.as_str(), progress));
    })
    .await
}

pub async fn transcribe_batch_with_progress<F>(
    request: &BatchTranscriptionRequest,
    mut on_progress: F,
) -> Result<Vec<TranscriptSegment>, String>
where
    F: FnMut(f32),
{
    let samples = crate::pipeline::extract_and_resample_audio(&request.file_path, 16000).await?;

    if let Some(path) = request.save_to_path.as_ref() {
        crate::pipeline::save_wav_file(&samples, 16000, path).map_err(|e| e.to_string())?;
    }

    let config_type = build_model_config(
        Path::new(&request.model_path),
        &request.model_type,
        &request.file_config,
        request.enable_itn,
        &request.language,
        request.hotwords.clone(),
    )?;
    let recognizer = Recognizer::new(config_type, request.num_threads)?;
    let punctuation = load_punctuation(request.punctuation_model.clone());

    let segments = match &recognizer.inner {
        RecognizerInner::Offline(r) => {
            process_batch_offline(
                r,
                &samples,
                request.vad_model.clone(),
                request.vad_buffer,
                punctuation.as_ref(),
                &mut on_progress,
            )
            .await?
        }
        RecognizerInner::Online(r) => {
            process_batch_online(r, &samples, punctuation.as_ref(), &mut on_progress).await?
        }
    };

    let annotated_segments = crate::speaker::annotate_segments_with_speakers(
        &samples,
        &segments,
        request.speaker_processing.as_ref(),
    )?;

    Ok(apply_timeline_normalization(
        annotated_segments,
        request.normalization_options,
    ))
}

async fn process_batch_offline<F>(
    r: &SafeOfflineRecognizer,
    samples: &[f32],
    vad_model: Option<String>,
    vad_buffer: f32,
    punctuation: Option<&Punctuation>,
    on_progress: &mut F,
) -> Result<Vec<TranscriptSegment>, String>
where
    F: FnMut(f32),
{
    let segments = if let Some(v_path) = vad_model {
        if !v_path.is_empty() && Path::new(&v_path).exists() {
            let silero_vad = sherpa_onnx::SileroVadModelConfig {
                model: Some(v_path),
                threshold: 0.35,
                min_silence_duration: 1.0,
                min_speech_duration: 0.25,
                window_size: 512,
                ..Default::default()
            };

            let vad_config = sherpa_onnx::VadModelConfig {
                silero_vad,
                sample_rate: 16000,
                num_threads: 1,
                ..Default::default()
            };

            crate::pipeline::vad_segment_audio(samples, 16000, &vad_config, vad_buffer)
                .unwrap_or_else(|_| crate::pipeline::fixed_chunk_audio(samples, 16000, 30.0))
        } else {
            crate::pipeline::fixed_chunk_audio(samples, 16000, 30.0)
        }
    } else {
        crate::pipeline::fixed_chunk_audio(samples, 16000, 30.0)
    };

    let mut results = Vec::new();
    let total_segments = segments.len();
    if total_segments == 0 {
        on_progress(100.0);
        return Ok(results);
    }

    for (i, seg) in segments.into_iter().enumerate() {
        {
            let stream = r.0.create_stream();
            debug!("FFI: Calling accept_waveform (Offline segment)");
            stream.accept_waveform(16000, &seg.samples);
            debug!("FFI: Successfully returned from accept_waveform (Offline segment)");
            r.0.decode(&stream);

            if let Some(res) = stream.get_result() {
                if !res.text.trim().is_empty() {
                    let text = format_transcript(&res.text, punctuation);
                    let timestamps_abs = res
                        .timestamps
                        .as_ref()
                        .map(|ts| ts.iter().map(|t| *t + seg.start_time).collect::<Vec<_>>());
                    let durations = timestamps_abs
                        .as_ref()
                        .and_then(|ts| synthesize_durations(ts, seg.start_time + seg.duration));

                    results.push(TranscriptSegment {
                        id: uuid::Uuid::new_v4().to_string(),
                        text,
                        start: seg.start_time as f64,
                        end: (seg.start_time + seg.duration) as f64,
                        is_final: true,
                        timing: None,
                        tokens: Some(res.tokens),
                        timestamps: timestamps_abs,
                        durations,
                        translation: None,
                        speaker: None,
                    });
                }
            }
        }
        let progress = ((i + 1) as f32 / total_segments as f32) * 100.0;
        on_progress(progress);
        tokio::task::yield_now().await;
    }
    Ok(results)
}

async fn process_batch_online<F>(
    r: &SafeOnlineRecognizer,
    samples: &[f32],
    punctuation: Option<&Punctuation>,
    on_progress: &mut F,
) -> Result<Vec<TranscriptSegment>, String>
where
    F: FnMut(f32),
{
    let stream = SafeStream(r.0.create_stream());
    let mut segments = Vec::new();
    let mut segment_start = 0.0;
    let mut current_samples = 0;

    let chunk_size = 8000;
    let total_samples = samples.len();
    if total_samples == 0 {
        on_progress(100.0);
        return Ok(segments);
    }

    for chunk in samples.chunks(chunk_size) {
        stream.0.accept_waveform(16000, chunk);
        current_samples += chunk.len();
        while r.0.is_ready(&stream.0) {
            r.0.decode(&stream.0);
        }
        if r.0.is_endpoint(&stream.0) {
            let current_time = current_samples as f64 / 16000.0;
            if let Some(result) = r.0.get_result(&stream.0) {
                if !result.text.trim().is_empty() {
                    let text = format_transcript(&result.text, punctuation);
                    let timestamps_abs = result.timestamps.as_ref().map(|ts| {
                        ts.iter()
                            .map(|t| *t + segment_start as f32)
                            .collect::<Vec<_>>()
                    });
                    let durations = timestamps_abs
                        .as_ref()
                        .and_then(|ts| synthesize_durations(ts, current_time as f32));

                    segments.push(TranscriptSegment {
                        id: uuid::Uuid::new_v4().to_string(),
                        text,
                        start: segment_start,
                        end: current_time,
                        is_final: true,
                        timing: None,
                        tokens: Some(result.tokens),
                        timestamps: timestamps_abs,
                        durations,
                        translation: None,
                        speaker: None,
                    });
                }
            }
            r.0.reset(&stream.0);
            segment_start = current_time;
        }
        let progress = (current_samples as f32 / total_samples as f32) * 100.0;
        on_progress(progress);
        tokio::task::yield_now().await;
    }

    let tail_padding = vec![0.0; (16000.0 * 0.8) as usize];
    stream.0.accept_waveform(16000, &tail_padding);
    while r.0.is_ready(&stream.0) {
        r.0.decode(&stream.0);
    }

    if let Some(result) = r.0.get_result(&stream.0) {
        if !result.text.trim().is_empty() {
            let text = format_transcript(&result.text, punctuation);
            let current_time = samples.len() as f64 / 16000.0;
            let timestamps_abs = result.timestamps.as_ref().map(|ts| {
                ts.iter()
                    .map(|t| *t + segment_start as f32)
                    .collect::<Vec<_>>()
            });
            let durations = timestamps_abs
                .as_ref()
                .and_then(|ts| synthesize_durations(ts, current_time as f32));

            segments.push(TranscriptSegment {
                id: uuid::Uuid::new_v4().to_string(),
                text,
                start: segment_start,
                end: current_time,
                is_final: true,
                timing: None,
                tokens: Some(result.tokens),
                timestamps: timestamps_abs,
                durations,
                translation: None,
                speaker: None,
            });
        }
    }
    on_progress(100.0);
    Ok(segments)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_recognizer_text_strips_multiple_leading_tags() {
        assert_eq!(
            normalize_recognizer_text("  <|zh|><|withitn|><|noise|> 123。 "),
            "123。"
        );
    }

    #[test]
    fn select_final_transcript_text_falls_back_to_cleaned_digits_when_formatting_drops_them() {
        assert_eq!(select_final_transcript_text("123", "。"), "123。");
    }

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

    fn sample_segment(text: &str, start: f64, end: f64) -> TranscriptSegment {
        TranscriptSegment {
            id: "segment-1".to_string(),
            text: text.to_string(),
            start,
            end,
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            speaker: None,
        }
    }

    #[test]
    fn ensure_transcript_segment_timing_builds_token_level_units_from_legacy_fields() {
        let mut segment = TranscriptSegment {
            text: "你好世界".to_string(),
            tokens: Some(vec![
                "你".to_string(),
                "好".to_string(),
                "世".to_string(),
                "界".to_string(),
            ]),
            timestamps: Some(vec![0.0, 0.25, 0.5, 0.75]),
            durations: Some(vec![0.25, 0.25, 0.25, 0.25]),
            ..sample_segment("你好世界", 0.0, 1.0)
        };

        ensure_transcript_segment_timing(&mut segment);

        let timing = segment.timing.expect("timing should exist");
        assert_eq!(timing.level, TranscriptTimingLevel::Token);
        assert_eq!(timing.source, TranscriptTimingSource::Model);
        assert_eq!(timing.units.len(), 4);
        assert_eq!(timing.units[0].text, "你");
        assert_eq!(timing.units[0].start, 0.0);
        assert_eq!(timing.units[3].text, "界");
        assert_eq!(timing.units[3].end, 1.0);
    }

    #[test]
    fn ensure_transcript_segment_timing_falls_back_to_segment_level_without_token_timestamps() {
        let mut segment = TranscriptSegment {
            tokens: Some(vec!["Hello".to_string(), "world".to_string()]),
            ..sample_segment("Hello world", 1.0, 3.0)
        };

        ensure_transcript_segment_timing(&mut segment);

        let timing = segment.timing.expect("timing should exist");
        assert_eq!(timing.level, TranscriptTimingLevel::Segment);
        assert_eq!(timing.source, TranscriptTimingSource::Derived);
        assert_eq!(
            timing.units,
            vec![TranscriptTimingUnit {
                text: "Hello world".to_string(),
                start: 1.0,
                end: 3.0,
            }]
        );
    }

    #[test]
    fn apply_timeline_normalization_splits_token_level_segments_with_model_timing() {
        let segment = TranscriptSegment {
            text: "你好。世界。".to_string(),
            tokens: Some(vec![
                "你".to_string(),
                "好".to_string(),
                "。".to_string(),
                "世".to_string(),
                "界".to_string(),
                "。".to_string(),
            ]),
            timestamps: Some(vec![0.0, 0.2, 0.4, 0.6, 0.8, 1.0]),
            durations: Some(vec![0.2; 6]),
            ..sample_segment("你好。世界。", 0.0, 1.2)
        };

        let results = apply_timeline_normalization(
            vec![segment],
            TranscriptNormalizationOptions {
                enable_timeline: true,
            },
        );

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].text, "你好。");
        assert_eq!(results[1].text, "世界。");
        assert_eq!(
            results[0].timing.as_ref().map(|timing| timing.level),
            Some(TranscriptTimingLevel::Token)
        );
        assert_eq!(
            results[0].timing.as_ref().map(|timing| timing.source),
            Some(TranscriptTimingSource::Model)
        );
        assert!(results[1].start >= results[0].end);
    }

    #[test]
    fn apply_timeline_normalization_marks_segment_level_splits_as_derived() {
        let results = apply_timeline_normalization(
            vec![sample_segment("Hello. World.", 0.0, 2.0)],
            TranscriptNormalizationOptions {
                enable_timeline: true,
            },
        );

        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0].timing.as_ref().map(|timing| timing.level),
            Some(TranscriptTimingLevel::Segment)
        );
        assert_eq!(
            results[0].timing.as_ref().map(|timing| timing.source),
            Some(TranscriptTimingSource::Derived)
        );
        assert_eq!(
            results[1].timing.as_ref().map(|timing| timing.source),
            Some(TranscriptTimingSource::Derived)
        );
    }

    #[test]
    fn build_transcript_update_replaces_final_segments_atomically_when_timeline_enabled() {
        let update = build_transcript_update(
            sample_segment("Hello. World.", 0.0, 2.0),
            TranscriptNormalizationOptions {
                enable_timeline: true,
            },
        );

        assert_eq!(update.remove_ids, vec!["segment-1".to_string()]);
        assert_eq!(update.upsert_segments.len(), 2);
        assert_eq!(update.upsert_segments[0].id, "segment-1");
    }

    #[test]
    fn start_instance_runtime_resets_progress_and_enables_running() {
        let mut instance = SherpaInstance::default();
        instance.total_samples = 42;
        instance.segment_start_time = 3.5;
        instance.current_segment_id = Some("segment-1".to_string());
        instance
            .offline_state
            .speech_buffer
            .push(vec![0.1, 0.2, 0.3]);
        instance.offline_state.is_speaking = true;
        instance.record_diagnostics.first_sample_logged = true;
        instance.record_diagnostics.skipped_while_stopped_logged = true;
        instance
            .record_diagnostics
            .first_segment_emitted
            .store(true, Ordering::SeqCst);

        start_instance_runtime(&mut instance, None);

        assert!(instance.is_running);
        assert!(instance.stream.is_none());
        assert_eq!(instance.total_samples, 0);
        assert_eq!(instance.segment_start_time, 0.0);
        assert!(instance.current_segment_id.is_none());
        assert!(instance.offline_state.speech_buffer.is_empty());
        assert!(!instance.offline_state.is_speaking);
        assert!(!instance.record_diagnostics.first_sample_logged);
        assert!(!instance.record_diagnostics.skipped_while_stopped_logged);
        assert!(!instance
            .record_diagnostics
            .first_segment_emitted
            .load(Ordering::SeqCst));
    }

    #[test]
    fn stop_instance_runtime_clears_progress_and_disables_running() {
        let mut instance = SherpaInstance::default();
        instance.is_running = true;
        instance.total_samples = 128;
        instance.segment_start_time = 1.25;
        instance.current_segment_id = Some("segment-2".to_string());
        instance.offline_state.speech_buffer.push(vec![0.4, 0.5]);
        instance.offline_state.is_speaking = true;
        instance.record_diagnostics.first_sample_logged = true;
        instance
            .record_diagnostics
            .first_segment_emitted
            .store(true, Ordering::SeqCst);

        stop_instance_runtime(&mut instance);

        assert!(!instance.is_running);
        assert!(instance.stream.is_none());
        assert_eq!(instance.total_samples, 0);
        assert_eq!(instance.segment_start_time, 0.0);
        assert!(instance.current_segment_id.is_none());
        assert!(instance.offline_state.speech_buffer.is_empty());
        assert!(!instance.offline_state.is_speaking);
        assert!(!instance.record_diagnostics.first_sample_logged);
        assert!(!instance
            .record_diagnostics
            .first_segment_emitted
            .load(Ordering::SeqCst));
    }
}
