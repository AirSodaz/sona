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
    pub total_samples: Mutex<usize>,
    pub segment_start_time: Mutex<f64>,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegment {
    pub id: String,
    pub text: String,
    pub start: f64,
    pub end: f64,
    pub is_final: bool,
    pub tokens: Option<Vec<String>>,
    pub timestamps: Option<Vec<f32>>,
    pub durations: Option<Vec<f32>>,
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
    *state.total_samples.lock().await = 0;
    *state.segment_start_time.lock().await = 0.0;
    Ok(())
}

#[tauri::command]
pub async fn stop_recognizer(state: State<'_, SherpaState>) -> Result<(), String> {
    *state.recognizer.lock().await = None;
    *state.stream.lock().await = None;
    *state.total_samples.lock().await = 0;
    *state.segment_start_time.lock().await = 0.0;
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
    let mut total_samples = state.total_samples.lock().await;
    let mut segment_start = state.segment_start_time.lock().await;

    if let (Some(Recognizer { inner: RecognizerInner::Online(r) }), Some(st)) = (rec_guard.as_ref(), stream_guard.as_ref()) {
        st.0.accept_waveform(16000, &samples);
        *total_samples += samples.len();

        while r.is_ready(&st.0) {
            r.decode(&st.0);
        }
        
        let current_time = *total_samples as f64 / 16000.0;
        
        if let Some(result) = r.get_result(&st.0) {
            if !result.text.trim().is_empty() {
                let segment = TranscriptSegment {
                    id: uuid::Uuid::new_v4().to_string(), // Requires uuid crate
                    text: result.text.clone(),
                    start: *segment_start,
                    end: current_time,
                    is_final: false,
                    tokens: Some(result.tokens.clone()),
                    timestamps: result.timestamps.clone(),
                    durations: None,
                };
                let _ = app.emit("recognizer-output", &segment); 
            }
        }
        
        if r.is_endpoint(&st.0) {
            // Re-get final result
            if let Some(result) = r.get_result(&st.0) {
                if !result.text.trim().is_empty() {
                    let segment = TranscriptSegment {
                        id: uuid::Uuid::new_v4().to_string(),
                        text: result.text.clone(),
                        start: *segment_start,
                        end: current_time,
                        is_final: true,
                        tokens: Some(result.tokens),
                        timestamps: result.timestamps,
                        durations: None,
                    };
                    let _ = app.emit("recognizer-output", &segment); 
                }
            }

            r.reset(&st.0);
            *segment_start = current_time;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn process_batch_file<R: tauri::Runtime>(
    _app: AppHandle<R>,
    state: State<'_, SherpaState>,
    file_path: String,
    save_to_path: Option<String>,
) -> Result<Vec<TranscriptSegment>, String> {
    let samples = crate::pipeline::extract_and_resample_audio(&file_path, 16000)?;
    
    if let Some(path) = save_to_path {
        crate::pipeline::save_wav_file(&samples, 16000, &path).map_err(|e| e.to_string())?;
    }

    let rec_guard = state.recognizer.lock().await;
    let total_duration = samples.len() as f64 / 16000.0;
    
    if let Some(Recognizer { inner: RecognizerInner::Offline(r) }) = rec_guard.as_ref() {
        let stream = r.create_stream();
        stream.accept_waveform(16000, &samples);
        r.decode(&stream);
        
        if let Some(result) = stream.get_result() {
            let segment = TranscriptSegment {
                id: uuid::Uuid::new_v4().to_string(),
                text: result.text,
                start: 0.0,
                end: total_duration,
                is_final: true,
                tokens: Some(result.tokens),
                timestamps: result.timestamps,
                durations: None,
            };
            return Ok(vec![segment]);
        }
    }
    // Alternatively fallback to online recognizer for batch
    if let Some(Recognizer { inner: RecognizerInner::Online(r) }) = rec_guard.as_ref() {
        let stream = r.create_stream();
        stream.accept_waveform(16000, &samples);
        let mut segments = Vec::new();
        let mut segment_start = 0.0;
        let mut current_samples = 0;
        
        // Simulating decoding by chunks
        let chunk_size = 16000; // 1s
        for chunk in samples.chunks(chunk_size) {
            stream.accept_waveform(16000, chunk);
            current_samples += chunk.len();
            while r.is_ready(&stream) {
                r.decode(&stream);
            }
            if r.is_endpoint(&stream) {
                let current_time = current_samples as f64 / 16000.0;
                if let Some(result) = r.get_result(&stream) {
                    if !result.text.trim().is_empty() {
                        segments.push(TranscriptSegment {
                            id: uuid::Uuid::new_v4().to_string(),
                            text: result.text,
                            start: segment_start,
                            end: current_time,
                            is_final: true,
                            tokens: Some(result.tokens),
                            timestamps: result.timestamps,
                            durations: None,
                        });
                    }
                }
                r.reset(&stream);
                segment_start = current_time;
            }
        }
        
        // Finalize remaining
        if let Some(result) = r.get_result(&stream) {
            if !result.text.trim().is_empty() {
                segments.push(TranscriptSegment {
                    id: uuid::Uuid::new_v4().to_string(),
                    text: result.text,
                    start: segment_start,
                    end: total_duration,
                    is_final: true,
                    tokens: Some(result.tokens),
                    timestamps: result.timestamps,
                    durations: None,
                });
            }
        }
        
        return Ok(segments);
    }
    
    Err("Recognizer not initialized or failed".to_string())
}
