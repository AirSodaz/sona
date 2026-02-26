use sherpa_onnx::{
    OfflineRecognizer, OfflineRecognizerConfig, OnlineRecognizer, OnlineRecognizerConfig,
};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum ModelType {
    OnlineTransducer {
        encoder: PathBuf,
        decoder: PathBuf,
        joiner: PathBuf,
        tokens: PathBuf,
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
        // According to sherpa-onnx, whisper has encoder and decoder
        encoder: PathBuf,
        decoder: PathBuf,
        tokens: PathBuf,
        language: String,
    },
}

pub fn find_model_config<P: AsRef<Path>>(
    model_path: P,
    enable_itn: bool,
    language: &str,
) -> Option<ModelType> {
    let model_path = model_path.as_ref();
    if !model_path.exists() || !model_path.is_dir() {
        return None;
    }

    let tokens_path = model_path.join("tokens.txt");
    if !tokens_path.exists() {
        return None;
    }

    // Helper to find a file containing a specific string
    let find_file = |substring: &str| -> Option<PathBuf> {
        let entries = fs::read_dir(model_path).ok()?;
        let mut candidates = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.contains(substring) && name.ends_with(".onnx") {
                    candidates.push(path);
                }
            }
        }

        if candidates.is_empty() {
            return None;
        }

        // Prefer int8
        if let Some(int8_path) = candidates
            .iter()
            .find(|p| p.to_string_lossy().contains("int8"))
        {
            return Some(int8_path.clone());
        }

        Some(candidates[0].clone())
    };

    let encoder = find_file("encoder");
    let decoder = find_file("decoder");
    let joiner = find_file("joiner");
    let model = find_file("model"); // For SenseVoice

    if let (Some(enc), Some(dec)) = (&encoder, &decoder) {
        // Could be transducer, paraformer, or whisper.
        // Let's check if it's whisper (usually has whisper in the path or name)
        let is_whisper = model_path
            .to_string_lossy()
            .to_lowercase()
            .contains("whisper")
            || enc.to_string_lossy().to_lowercase().contains("whisper");

        if is_whisper {
            return Some(ModelType::OfflineWhisper {
                encoder: enc.clone(),
                decoder: dec.clone(),
                tokens: tokens_path,
                language: language.to_string(),
            });
        }

        if let Some(join) = joiner {
            return Some(ModelType::OnlineTransducer {
                encoder: enc.clone(),
                decoder: dec.clone(),
                joiner: join,
                tokens: tokens_path,
            });
        } else {
            return Some(ModelType::OnlineParaformer {
                encoder: enc.clone(),
                decoder: dec.clone(),
                tokens: tokens_path,
            });
        }
    }

    if let Some(mod_path) = model {
        // SenseVoice
        return Some(ModelType::OfflineSenseVoice {
            model: mod_path,
            tokens: tokens_path,
            language: language.to_string(),
            use_itn: enable_itn,
        });
    }

    None
}

pub enum RecognizerInner {
    Online(OnlineRecognizer),
    Offline(OfflineRecognizer),
}

pub struct Recognizer {
    pub inner: RecognizerInner,
}

impl Recognizer {
    pub fn new(model_type: ModelType, num_threads: i32, itn_model: Option<String>) -> Result<Self, String> {
        let rec = match model_type {
            ModelType::OnlineTransducer {
                encoder,
                decoder,
                joiner,
                tokens,
            } => {
                let mut config = OnlineRecognizerConfig::default();
                config.rule_fsts = itn_model.clone();
                config.model_config.transducer.encoder =
                    Some(encoder.to_string_lossy().to_string());
                config.model_config.transducer.decoder =
                    Some(decoder.to_string_lossy().to_string());
                config.model_config.transducer.joiner = Some(joiner.to_string_lossy().to_string());
                config.model_config.tokens = Some(tokens.to_string_lossy().to_string());
                config.model_config.num_threads = num_threads;
                config.model_config.provider = Some("cpu".to_string());
                config.feat_config.sample_rate = 16000;
                config.feat_config.feature_dim = 80;
                config.enable_endpoint = true;

                let recognizer =
                    OnlineRecognizer::create(&config).ok_or("Failed to create OnlineRecognizer")?;
                RecognizerInner::Online(recognizer)
            }
            ModelType::OnlineParaformer {
                encoder,
                decoder,
                tokens,
            } => {
                let mut config = OnlineRecognizerConfig::default();
                config.rule_fsts = itn_model.clone();
                config.model_config.paraformer.encoder =
                    Some(encoder.to_string_lossy().to_string());
                config.model_config.paraformer.decoder =
                    Some(decoder.to_string_lossy().to_string());
                config.model_config.tokens = Some(tokens.to_string_lossy().to_string());
                config.model_config.num_threads = num_threads;
                config.model_config.provider = Some("cpu".to_string());
                config.feat_config.sample_rate = 16000;
                config.feat_config.feature_dim = 80;

                let recognizer =
                    OnlineRecognizer::create(&config).ok_or("Failed to create OnlineRecognizer")?;
                RecognizerInner::Online(recognizer)
            }
            ModelType::OfflineSenseVoice {
                model,
                tokens,
                language,
                use_itn,
            } => {
                let mut config = OfflineRecognizerConfig::default();
                config.rule_fsts = itn_model.clone();
                config.model_config.sense_voice.model = Some(model.to_string_lossy().to_string());
                config.model_config.sense_voice.language = Some(language);
                config.model_config.sense_voice.use_itn = use_itn;
                config.model_config.tokens = Some(tokens.to_string_lossy().to_string());
                config.model_config.num_threads = num_threads;
                config.model_config.provider = Some("cpu".to_string());
                config.feat_config.sample_rate = 16000;
                config.feat_config.feature_dim = 80;

                let recognizer = OfflineRecognizer::create(&config)
                    .ok_or("Failed to create OfflineRecognizer")?;
                RecognizerInner::Offline(recognizer)
            }
            ModelType::OfflineWhisper {
                encoder,
                decoder,
                tokens,
                language,
            } => {
                let mut config = OfflineRecognizerConfig::default();
                config.rule_fsts = itn_model.clone();
                config.model_config.whisper.encoder = Some(encoder.to_string_lossy().to_string());
                config.model_config.whisper.decoder = Some(decoder.to_string_lossy().to_string());
                config.model_config.whisper.language = Some(language);
                config.model_config.tokens = Some(tokens.to_string_lossy().to_string());
                config.model_config.num_threads = num_threads;
                config.model_config.provider = Some("cpu".to_string());
                config.feat_config.sample_rate = 16000;
                config.feat_config.feature_dim = 80;

                let recognizer = OfflineRecognizer::create(&config)
                    .ok_or("Failed to create OfflineRecognizer")?;
                RecognizerInner::Offline(recognizer)
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

use tauri::{State, AppHandle, Emitter};
use tokio::sync::Mutex;

pub struct SherpaState {
    pub recognizer: Mutex<Option<Recognizer>>,
    pub stream: Mutex<Option<SafeStream>>,
}

#[tauri::command]
pub async fn start_recognizer(
    state: State<'_, SherpaState>,
    model_path: String,
    num_threads: i32,
    enable_itn: bool,
    language: String,
    itn_model: Option<String>,
) -> Result<(), String> {
    let model_type = find_model_config(&model_path, enable_itn, &language)
        .ok_or_else(|| "Could not find valid model configuration".to_string())?;

    let recognizer = Recognizer::new(model_type, num_threads, itn_model)?;
    let stream = match &recognizer.inner {
        RecognizerInner::Online(r) => Some(SafeStream(r.create_stream())),
        _ => None,
    };
    
    *state.recognizer.lock().await = Some(recognizer);
    *state.stream.lock().await = stream;
    Ok(())
}

#[tauri::command]
pub async fn stop_recognizer(state: State<'_, SherpaState>) -> Result<(), String> {
    *state.recognizer.lock().await = None;
    *state.stream.lock().await = None;
    Ok(())
}

#[tauri::command]
pub async fn feed_audio_chunk<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, SherpaState>,
    samples: Vec<f32>,
) -> Result<(), String> {
    let rec_guard = state.recognizer.lock().await;
    let stream_guard = state.stream.lock().await;

    if let (Some(Recognizer { inner: RecognizerInner::Online(r) }), Some(st)) = (rec_guard.as_ref(), stream_guard.as_ref()) {
        st.0.accept_waveform(16000, &samples);
        while r.is_ready(&st.0) {
            r.decode(&st.0);
        }
        
        if let Some(result) = r.get_result(&st.0) {
            if !result.text.trim().is_empty() {
                // Emit final and partial results, for simple testing emit raw text
                let _ = app.emit("recognizer-event", &result.text); 
            }
        }
        
        if r.is_endpoint(&st.0) {
            r.reset(&st.0);
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn process_batch_file<R: tauri::Runtime>(
    _app: AppHandle<R>,
    state: State<'_, SherpaState>,
    file_path: String,
) -> Result<String, String> {
    let samples = crate::pipeline::extract_and_resample_audio(&file_path, 16000)?;
    let rec_guard = state.recognizer.lock().await;
    
    if let Some(Recognizer { inner: RecognizerInner::Offline(r) }) = rec_guard.as_ref() {
        let stream = r.create_stream();
        stream.accept_waveform(16000, &samples);
        r.decode(&stream);
        
        if let Some(result) = stream.get_result() {
            return Ok(result.text);
        }
    }
    // Alternatively fallback to online recognizer for batch
    if let Some(Recognizer { inner: RecognizerInner::Online(r) }) = rec_guard.as_ref() {
        let stream = r.create_stream();
        stream.accept_waveform(16000, &samples);
        while r.is_ready(&stream) {
            r.decode(&stream);
        }
        if let Some(result) = r.get_result(&stream) {
            return Ok(result.text);
        }
    }
    
    Err("Recognizer not initialized or failed".to_string())
}
