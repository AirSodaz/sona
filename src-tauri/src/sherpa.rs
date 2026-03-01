use sherpa_onnx::{
    OfflineRecognizer, OfflineRecognizerConfig, OnlineRecognizer, OnlineRecognizerConfig,
    SileroVadModelConfig, VadModelConfig, VoiceActivityDetector,
};
use std::ffi::{c_char, CStr, CString};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

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
        return Some(ModelType::OfflineSenseVoice {
            model: mod_path,
            tokens: tokens_path,
            language: language.to_string(),
            use_itn: enable_itn,
        });
    }

    None
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

impl Recognizer {
    pub fn new(
        model_type: ModelType,
        num_threads: i32,
        itn_model: Option<String>,
    ) -> Result<Self, String> {
        let rec = match model_type {
            ModelType::OnlineTransducer {
                encoder,
                decoder,
                joiner,
                tokens,
            } => {
                let mut config = OnlineRecognizerConfig::default();
                config.rule_fsts = itn_model.clone();
                config.rule1_min_trailing_silence = 2.4;
                config.rule2_min_trailing_silence = 2.4;
                config.rule3_min_utterance_length = 300.0;
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
                RecognizerInner::Online(SafeOnlineRecognizer(recognizer))
            }
            ModelType::OnlineParaformer {
                encoder,
                decoder,
                tokens,
            } => {
                let mut config = OnlineRecognizerConfig::default();
                config.rule_fsts = itn_model;
                config.rule1_min_trailing_silence = 2.4;
                config.rule2_min_trailing_silence = 2.4;
                config.rule3_min_utterance_length = 300.0;
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
                RecognizerInner::Online(SafeOnlineRecognizer(recognizer))
            }
            ModelType::OfflineSenseVoice {
                model,
                tokens,
                language,
                use_itn,
            } => {
                let mut config = OfflineRecognizerConfig::default();
                config.rule_fsts = itn_model;
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
                RecognizerInner::Offline(SafeOfflineRecognizer(recognizer))
            }
            ModelType::OfflineWhisper {
                encoder,
                decoder,
                tokens,
                language,
            } => {
                let mut config = OfflineRecognizerConfig::default();
                config.rule_fsts = itn_model;
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
// Punctuation FFI
// -----------------------------------------------------------------------------------------
#[repr(C)]
pub struct SherpaOnnxOfflinePunctuationModelConfig {
    pub ct_transformer: *const c_char,
    pub num_threads: i32,
    pub debug: i32,
    pub provider: *const c_char,
}

#[repr(C)]
pub struct SherpaOnnxOfflinePunctuationConfig {
    pub model: SherpaOnnxOfflinePunctuationModelConfig,
}

pub enum SherpaOnnxOfflinePunctuation {}

#[link(name = "sherpa-onnx-c-api")]
extern "C" {
    pub fn SherpaOnnxCreateOfflinePunctuation(
        config: *const SherpaOnnxOfflinePunctuationConfig,
    ) -> *const SherpaOnnxOfflinePunctuation;

    pub fn SherpaOnnxDestroyOfflinePunctuation(punct: *const SherpaOnnxOfflinePunctuation);

    pub fn SherpaOfflinePunctuationAddPunct(
        punct: *const SherpaOnnxOfflinePunctuation,
        text: *const c_char,
    ) -> *const c_char;

    pub fn SherpaOfflinePunctuationFreeText(text: *const c_char);
}

pub struct Punctuation {
    ptr: *const SherpaOnnxOfflinePunctuation,
}

impl Punctuation {
    pub fn new(model_path: &str, num_threads: i32) -> Result<Self, String> {
        let ct_transformer = CString::new(model_path).map_err(|e| e.to_string())?;
        let provider = CString::new("cpu").map_err(|e| e.to_string())?;

        let model_config = SherpaOnnxOfflinePunctuationModelConfig {
            ct_transformer: ct_transformer.as_ptr(),
            num_threads,
            debug: 0,
            provider: provider.as_ptr(),
        };

        let config = SherpaOnnxOfflinePunctuationConfig {
            model: model_config,
        };

        let ptr = unsafe { SherpaOnnxCreateOfflinePunctuation(&config) };
        if ptr.is_null() {
            Err("Failed to create OfflinePunctuation".to_string())
        } else {
            Ok(Self { ptr })
        }
    }

    pub fn add_punct(&self, text: &str) -> String {
        let c_text = CString::new(text).unwrap_or_default();
        unsafe {
            let res_ptr = SherpaOfflinePunctuationAddPunct(self.ptr, c_text.as_ptr());
            if res_ptr.is_null() {
                return text.to_string();
            }
            let res_str = CStr::from_ptr(res_ptr).to_string_lossy().into_owned();
            SherpaOfflinePunctuationFreeText(res_ptr);
            res_str
        }
    }
}

impl Drop for Punctuation {
    fn drop(&mut self) {
        unsafe {
            SherpaOnnxDestroyOfflinePunctuation(self.ptr);
        }
    }
}
unsafe impl Send for Punctuation {}
unsafe impl Sync for Punctuation {}

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
        }
    }
}

pub struct SherpaState {
    pub instances: Mutex<HashMap<String, SherpaInstance>>,
}

impl SherpaState {
    pub fn new() -> Self {
        Self {
            instances: Mutex::new(HashMap::new()),
        }
    }
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

fn get_valid_itn_paths(itn_model: Option<String>) -> Option<String> {
    itn_model.and_then(|m| {
        let valid_paths: Vec<&str> = m
            .split(',')
            .map(|p| p.trim())
            .filter(|p| !p.is_empty() && Path::new(p).exists())
            .collect();
        if valid_paths.is_empty() {
            None
        } else {
            Some(valid_paths.join(","))
        }
    })
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

fn run_offline_inference<R: tauri::Runtime>(
    speech_buffer: &[Vec<f32>],
    app: &AppHandle<R>,
    r: &sherpa_onnx::OfflineRecognizer,
    punctuation: Option<&Punctuation>,
    segment_id: &str,
    global_start: f64,
    is_final: bool,
    _instance_id: &str,
) {
    if speech_buffer.is_empty() {
        return;
    }
    let mut full_audio = Vec::new();
    for chunk in speech_buffer {
        full_audio.extend_from_slice(chunk);
    }
    let stream = r.create_stream();
    stream.accept_waveform(16000, &full_audio);
    r.decode(&stream);

    if let Some(result) = stream.get_result() {
        if !result.text.trim().is_empty() {
            let text = if is_final {
                format_transcript(&result.text, punctuation)
            } else {
                result.text.clone()
            };

            let global_end = global_start + (full_audio.len() as f64 / 16000.0);
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
                tokens: Some(result.tokens),
                timestamps: timestamps_abs,
                durations,
            };
            let _ = app.emit("recognizer-output", &segment);
        }
    }
}

#[tauri::command]
pub async fn init_recognizer(
    state: State<'_, SherpaState>,
    instance_id: String,
    model_path: String,
    num_threads: i32,
    enable_itn: bool,
    language: String,
    itn_model: Option<String>,
    punctuation_model: Option<String>,
    vad_model: Option<String>,
    vad_buffer: f32,
) -> Result<(), String> {
    let valid_itn = get_valid_itn_paths(itn_model);

    let model_type = find_model_config(&model_path, enable_itn, &language)
        .ok_or_else(|| "Could not find valid model configuration".to_string())?;

    let recognizer = Recognizer::new(model_type, num_threads, valid_itn)?;

    // Initialize Punctuation
    let mut punctuation = None;
    if let Some(p_path) = punctuation_model {
        if Path::new(&p_path).exists() {
            let entries = fs::read_dir(&p_path).map_err(|e| e.to_string())?;
            let onnx_file = entries
                .flatten()
                .find(|e| e.path().extension().map_or(false, |ext| ext == "onnx"));
            if let Some(e) = onnx_file {
                punctuation = Punctuation::new(&e.path().to_string_lossy(), 1).ok();
            }
        }
    }

    // Initialize VAD
    let mut vad = None;
    if let Some(v_path) = &vad_model {
        if Path::new(v_path).exists() {
            let mut silero_vad = SileroVadModelConfig::default();
            silero_vad.model = Some(v_path.clone());
            silero_vad.threshold = 0.35;
            silero_vad.min_silence_duration = 0.5;
            silero_vad.min_speech_duration = 0.25;
            silero_vad.window_size = 512;

            let mut vad_config = VadModelConfig::default();
            vad_config.silero_vad = silero_vad;
            vad_config.sample_rate = 16000;
            vad_config.num_threads = 1;

            vad = VoiceActivityDetector::create(&vad_config, 60.0).map(SafeVad);
        }
    }

    let mut instances = state.instances.lock().await;
    let instance = instances.entry(instance_id).or_insert_with(SherpaInstance::default);

    instance.recognizer = Some(Arc::new(recognizer));
    instance.vad = vad;
    instance.punctuation = punctuation.map(Arc::new);
    instance.vad_model = vad_model.clone();
    instance.vad_buffer = vad_buffer;

    Ok(())
}

#[tauri::command]
pub async fn start_recognizer(
    state: State<'_, SherpaState>,
    instance_id: String,
) -> Result<(), String> {
    let mut instances = state.instances.lock().await;
    let instance = instances.get_mut(&instance_id).ok_or("Instance not found")?;

    let Some(recognizer) = instance.recognizer.as_ref() else {
        return Err("Recognizer not initialized".to_string());
    };

    let stream = match &recognizer.inner {
        RecognizerInner::Online(r) => Some(SafeStream(r.0.create_stream())),
        _ => None,
    };

    instance.stream = stream;
    instance.total_samples = 0;
    instance.segment_start_time = 0.0;
    instance.offline_state = OfflineState::default();
    instance.current_segment_id = None;
    Ok(())
}

#[tauri::command]
pub async fn stop_recognizer(
    state: State<'_, SherpaState>,
    instance_id: String,
) -> Result<(), String> {
    let mut instances = state.instances.lock().await;
    if let Some(instance) = instances.get_mut(&instance_id) {
        // Keep models loaded, but clear active recording state
        instance.stream = None;
        instance.total_samples = 0;
        instance.segment_start_time = 0.0;
        instance.offline_state = OfflineState::default();
        instance.current_segment_id = None;
    }
    Ok(())
}

#[tauri::command]
pub async fn flush_recognizer<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, SherpaState>,
    instance_id: String,
) -> Result<(), String> {
    let mut instances = state.instances.lock().await;
    let instance = instances.get_mut(&instance_id).ok_or("Instance not found")?;

    // Flush offline speech buffer
    if let Some(recognizer) = instance.recognizer.clone() {
        if let RecognizerInner::Offline(_) = &recognizer.inner {
            if !instance.offline_state.speech_buffer.is_empty() {
                let seg_id = instance.current_segment_id
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

                tauri::async_runtime::spawn(async move {
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
                            );
                        }
                    }).await
                });

                instance.offline_state.speech_buffer.clear();
                instance.offline_state.is_speaking = false;
            }
            instance.current_segment_id = None;
            instance.offline_state = OfflineState::default();
            return Ok(());
        }
    }

    // Flush online stream
    if let (
        Some(recognizer),
        Some(st),
    ) = (instance.recognizer.as_deref(), instance.stream.as_ref())
    {
        if let RecognizerInner::Online(r) = &recognizer.inner {
            let current_time = instance.total_samples as f64 / 16000.0;

            // Add tail padding to flush the decoder
            let tail_padding = vec![0.0; (16000.0 * 0.8) as usize];
            st.0.accept_waveform(16000, &tail_padding);
            while r.0.is_ready(&st.0) {
                r.0.decode(&st.0);
            }

            if let Some(result) = r.0.get_result(&st.0) {
                if !result.text.trim().is_empty() {
                    let text = format_transcript(&result.text, instance.punctuation.as_deref());
                    let timestamps_f32 = result.timestamps;
                    let durations = timestamps_f32
                        .as_ref()
                        .and_then(|ts| synthesize_durations(ts, current_time as f32));

                    let id = if let Some(id) = instance.current_segment_id.take() {
                        id
                    } else {
                        uuid::Uuid::new_v4().to_string()
                    };

                    let segment = TranscriptSegment {
                        id,
                        text,
                        start: instance.segment_start_time,
                        end: current_time,
                        is_final: true,
                        tokens: Some(result.tokens),
                        timestamps: timestamps_f32,
                        durations,
                    };
                    let _ = app.emit("recognizer-output", &segment);
                }
            }

            instance.current_segment_id = None;
            r.0.reset(&st.0);
            instance.segment_start_time = current_time;
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

    // Offline + VAD (pseudo-streaming)
    let is_offline = match instance.recognizer.as_deref() {
        Some(rec) => matches!(rec.inner, RecognizerInner::Offline(_)),
        None => false,
    };

    if is_offline {
        if let (Some(recognizer), Some(SafeVad(vad))) = (instance.recognizer.clone(), instance.vad.as_ref()) {
            if let RecognizerInner::Offline(_) = &recognizer.inner {
                vad.accept_waveform(samples);
                let currently_speaking = vad.detected();

                // Ensure segment ID exists
                if instance.current_segment_id.is_none() {
                    instance.current_segment_id = Some(uuid::Uuid::new_v4().to_string());
                }
                let seg_id = instance.current_segment_id.as_ref().unwrap().clone();

                if currently_speaking && !instance.offline_state.is_speaking {
                    // Rising edge (silence -> speech)
                    instance.offline_state.is_speaking = true;

                    // Prepend Ring Buffer
                    let samples_to_keep = (16000.0 * 0.3) as usize; // 0.3s context
                    let mut context_len = 0;

                    if !instance.offline_state.ring_buffer.is_empty() {
                        let mut ring_flat: Vec<f32> = Vec::new();
                        for rb in &instance.offline_state.ring_buffer {
                            ring_flat.extend_from_slice(rb);
                        }

                        let keep_start = if ring_flat.len() > samples_to_keep {
                            ring_flat.len() - samples_to_keep
                        } else {
                            0
                        };

                        let context = ring_flat[keep_start..].to_vec();
                        context_len = context.len();
                        instance.offline_state.speech_buffer.push(context);
                    }

                    instance.offline_state.utterance_start_sample = instance.total_samples - context_len;
                    instance.offline_state.ring_buffer.clear();
                }

                if currently_speaking {
                    instance.offline_state.speech_buffer.push(samples.to_vec());

                    let now = std::time::Instant::now();
                    if now.duration_since(instance.offline_state.last_inference_time).as_millis() > 200 {
                        let global_start = instance.offline_state.utterance_start_sample as f64 / 16000.0;

                        let offline_copy = instance.offline_state.speech_buffer.clone();
                        let app_copy = app.clone();
                        let recognizer_copy = recognizer.clone();
                        let punct_copy = instance.punctuation.clone();
                        let seg_id_copy = seg_id.clone();
                        let instance_id_copy = instance_id.to_string();

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
                                );
                            }
                        });
                        instance.offline_state.last_inference_time = now;
                    }
                }

                if !currently_speaking {
                    if instance.offline_state.is_speaking {
                        // Falling edge (speech -> silence)
                        instance.offline_state.is_speaking = false;
                        instance.offline_state.speech_buffer.push(samples.to_vec());

                        let global_start = instance.offline_state.utterance_start_sample as f64 / 16000.0;

                        let offline_copy = instance.offline_state.speech_buffer.clone();
                        let app_copy = app.clone();
                        let recognizer_copy = recognizer.clone();
                        let punct_copy = instance.punctuation.clone();
                        let seg_id_copy = seg_id.clone();
                        let instance_id_copy = instance_id.to_string();

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
                                );
                            }
                        });

                        instance.offline_state.speech_buffer.clear();
                        // Generate new segment ID for the next utterance
                        instance.current_segment_id = Some(uuid::Uuid::new_v4().to_string());
                    }

                // Maintain Ring Buffer
                instance.offline_state.ring_buffer.push_back(samples.to_vec());
                let max_ring_samples = (16000.0 * 0.3) as usize;

                let mut ring_len: usize = instance.offline_state.ring_buffer.iter().map(|v| v.len()).sum();
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
                return Ok(());
            }
        }
    }

    // Online model processing
    if let (
        Some(recognizer),
        Some(st),
    ) = (instance.recognizer.as_deref(), instance.stream.as_ref())
    {
        if let RecognizerInner::Online(r) = &recognizer.inner {
        st.0.accept_waveform(16000, samples);
        instance.total_samples += samples.len();

        while r.0.is_ready(&st.0) {
            r.0.decode(&st.0);
        }

        let current_time = instance.total_samples as f64 / 16000.0;

        if let Some(result) = r.0.get_result(&st.0) {
            if !result.text.trim().is_empty() {
                let id = if let Some(id) = instance.current_segment_id.as_ref() {
                    id.clone()
                } else {
                    let new_id = uuid::Uuid::new_v4().to_string();
                    instance.current_segment_id = Some(new_id.clone());
                    new_id
                };

                let segment = TranscriptSegment {
                    id,
                    text: result.text.clone(),
                    start: instance.segment_start_time,
                    end: current_time,
                    is_final: false,
                    tokens: Some(result.tokens.clone()),
                    timestamps: result.timestamps.clone(),
                    durations: None,
                };
                let _ = app.emit("recognizer-output", &segment);
            }
        }

            if r.0.is_endpoint(&st.0) {
                let tail_padding = vec![0.0; (16000.0 * 0.8) as usize];
                st.0.accept_waveform(16000, &tail_padding);
                while r.0.is_ready(&st.0) {
                    r.0.decode(&st.0);
                }

                if let Some(result) = r.0.get_result(&st.0) {
                    if !result.text.trim().is_empty() {
                        let text = format_transcript(&result.text, instance.punctuation.as_deref());

                        let timestamps_f32 = result.timestamps;
                        let durations = timestamps_f32
                            .as_ref()
                            .and_then(|ts| synthesize_durations(ts, current_time as f32));

                        let id = if let Some(id) = instance.current_segment_id.take() {
                            id
                        } else {
                            uuid::Uuid::new_v4().to_string()
                        };

                        let segment = TranscriptSegment {
                            id,
                            text,
                            start: instance.segment_start_time,
                            end: current_time,
                            is_final: true,
                            tokens: Some(result.tokens),
                            timestamps: timestamps_f32,
                            durations,
                        };
                        let _ = app.emit("recognizer-output", &segment);
                    }
                }

                // Always reset state after endpoint, regardless of text content
                instance.current_segment_id = None;
                r.0.reset(&st.0);
                instance.segment_start_time = current_time;
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn feed_audio_chunk<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, SherpaState>,
    instance_id: String,
    samples: Vec<u8>,
) -> Result<(), String> {
    let mut float_samples = Vec::with_capacity(samples.len() / 2);
    for chunk in samples.chunks_exact(2) {
        let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
        float_samples.push(sample as f32 / 32768.0);
    }
    feed_audio_samples(&app, &*state, &instance_id, &float_samples).await
}


#[tauri::command]
pub async fn process_batch_file<R: tauri::Runtime>(
    app: AppHandle<R>,
    file_path: String,
    save_to_path: Option<String>,
    model_path: String,
    num_threads: i32,
    enable_itn: bool,
    language: String,
    itn_model: Option<String>,
    punctuation_model: Option<String>,
    vad_model: Option<String>,
    vad_buffer: f32,
) -> Result<Vec<TranscriptSegment>, String> {
    let samples = crate::pipeline::extract_and_resample_audio(&app, &file_path, 16000).await?;

    if let Some(path) = save_to_path {
        crate::pipeline::save_wav_file(&samples, 16000, &path).map_err(|e| e.to_string())?;
    }

    // Initialize local recognizer, punctuation, and VAD instances
    let valid_itn = get_valid_itn_paths(itn_model);

    let model_type = find_model_config(&model_path, enable_itn, &language)
        .ok_or_else(|| "Could not find valid model configuration".to_string())?;

    let recognizer = Recognizer::new(model_type, num_threads, valid_itn)?;

    // Initialize Punctuation
    let mut punctuation = None;
    if let Some(p_path) = punctuation_model {
        if Path::new(&p_path).exists() {
            let entries = fs::read_dir(&p_path).map_err(|e| e.to_string())?;
            let onnx_file = entries
                .flatten()
                .find(|e| e.path().extension().map_or(false, |ext| ext == "onnx"));
            if let Some(e) = onnx_file {
                punctuation = Punctuation::new(&e.path().to_string_lossy(), 1).ok();
            }
        }
    }

    // Use Offline Recognizer if available
    if let RecognizerInner::Offline(r) = &recognizer.inner {
        let segments = if let Some(v_path) = vad_model {
            if Path::new(&v_path).exists() {
                let mut silero_vad = sherpa_onnx::SileroVadModelConfig::default();
                silero_vad.model = Some(v_path.clone());
                silero_vad.threshold = 0.35;
                silero_vad.min_silence_duration = 1.0;
                silero_vad.min_speech_duration = 0.25;
                silero_vad.window_size = 512;

                let mut vad_config = sherpa_onnx::VadModelConfig::default();
                vad_config.silero_vad = silero_vad;
                vad_config.sample_rate = 16000;
                vad_config.num_threads = 1;

                crate::pipeline::vad_segment_audio(&samples, 16000, &vad_config, vad_buffer)
                    .unwrap_or_else(|_| crate::pipeline::fixed_chunk_audio(&samples, 16000, 30.0))
            } else {
                crate::pipeline::fixed_chunk_audio(&samples, 16000, 30.0)
            }
        } else {
            crate::pipeline::fixed_chunk_audio(&samples, 16000, 30.0)
        };

        let mut results = Vec::new();
        let total_segments = segments.len();
        for (i, seg) in segments.into_iter().enumerate() {
            {
                let stream = r.0.create_stream();
                stream.accept_waveform(16000, &seg.samples);
                r.0.decode(&stream);

                if let Some(res) = stream.get_result() {
                    if !res.text.trim().is_empty() {
                        let text = format_transcript(&res.text, punctuation.as_ref());
                        let timestamps_abs = res.timestamps.as_ref().map(|ts| {
                            ts.iter()
                                .map(|t| *t + seg.start_time as f32)
                                .collect::<Vec<_>>()
                        });
                        let durations = timestamps_abs.as_ref().and_then(|ts| {
                            synthesize_durations(ts, (seg.start_time + seg.duration) as f32)
                        });

                        results.push(TranscriptSegment {
                            id: uuid::Uuid::new_v4().to_string(),
                            text,
                            start: seg.start_time as f64,
                            end: (seg.start_time + seg.duration) as f64,
                            is_final: true,
                            tokens: Some(res.tokens),
                            timestamps: timestamps_abs,
                            durations,
                        });
                    }
                }
            }
            // Emit progress
            let progress = ((i + 1) as f32 / total_segments as f32) * 100.0;
            let _ = app.emit("batch-progress", (&file_path, progress));

            // Yield to Tokio runtime to prevent blocking the async reactor
            tokio::task::yield_now().await;
        }
        return Ok(results);
    }

    if let RecognizerInner::Online(r) = &recognizer.inner {
        let stream = SafeStream(r.0.create_stream());
        let mut segments = Vec::new();
        let mut segment_start = 0.0;
        let mut current_samples = 0;

        let chunk_size = 8000; // 0.5s chunks, matching JS implementation
        let total_samples = samples.len();
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
                        let text = format_transcript(&result.text, punctuation.as_ref());
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
                            tokens: Some(result.tokens),
                            timestamps: timestamps_abs,
                            durations,
                        });
                    }
                }
                r.0.reset(&stream.0);
                segment_start = current_time;
            }

            let progress = (current_samples as f32 / total_samples as f32) * 100.0;
            let _ = app.emit("batch-progress", (&file_path, progress));

            // Yield after every few chunks if possible, but safely we can yield every chunk
            tokio::task::yield_now().await;
        }

        // Add tail padding to flush the decoder, matching feed_audio_chunk behavior
        let tail_padding = vec![0.0; (16000.0 * 0.8) as usize];
        stream.0.accept_waveform(16000, &tail_padding);
        while r.0.is_ready(&stream.0) {
            r.0.decode(&stream.0);
        }

        if let Some(result) = r.0.get_result(&stream.0) {
            if !result.text.trim().is_empty() {
                let text = format_transcript(&result.text, punctuation.as_ref());
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
                    tokens: Some(result.tokens),
                    timestamps: timestamps_abs,
                    durations,
                });
            }
        }

        return Ok(segments);
    }

    Err("Recognizer not initialized or failed".to_string())
}
