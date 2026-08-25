use crate::models::config::ModelFileConfig;
use crate::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
use crate::transcription::postprocess::TranscriptPostprocessor;
pub use crate::transcription::postprocess::{
    TranscriptNormalizationOptions, TranscriptPostprocessOptions, TranscriptTextReplacementRule,
    TranscriptTextReplacementRuleSet,
};
use crate::transcription::runtime::BatchTranscribePlan;
use crate::transcription::transcript::{TranscriptSegment, TranscriptUpdate};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "specta")]
use specta::Type;
use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

const ONLINE_ASR_PROVIDERS_JSON: &str = include_str!("online-asr-providers.json");

pub const LOCAL_SHERPA_ONNX_PROVIDER_ID: &str = "local_sherpa_onnx";
pub const LOCAL_LLAMA_CPP_PROVIDER_ID: &str = "local_llama_cpp";
pub const VOLCENGINE_DOUBAO_PROVIDER_ID: &str = "volcengine-doubao";
pub const VOLCENGINE_DOUBAO_LEGACY_PROVIDER_KEY: &str = "volcengineDoubao";
pub const GROQ_WHISPER_PROVIDER_ID: &str = "groq-whisper";
pub const MISTRAL_VOXTRAL_PROVIDER_ID: &str = "mistral-voxtral";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "kebab-case")]
pub enum LocalAsrEngine {
    #[default]
    SherpaOnnx,
    LlamaCpp,
}

impl LocalAsrEngine {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SherpaOnnx => "sherpa-onnx",
            Self::LlamaCpp => "llama-cpp",
        }
    }

    pub const fn provider_id(self) -> &'static str {
        match self {
            Self::SherpaOnnx => LOCAL_SHERPA_ONNX_PROVIDER_ID,
            Self::LlamaCpp => LOCAL_LLAMA_CPP_PROVIDER_ID,
        }
    }

    pub fn from_name(value: &str) -> Option<Self> {
        match value {
            "sherpa-onnx" => Some(Self::SherpaOnnx),
            "llama-cpp" => Some(Self::LlamaCpp),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "kebab-case")]
pub enum AsrEngine {
    /// Local offline transcription.
    #[serde(rename = "local", alias = "local-sherpa")]
    Local,
    Online,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "lowercase")]
pub enum AsrMode {
    Streaming,
    #[serde(alias = "offline")]
    Batch,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "lowercase")]
pub enum BatchSegmentationMode {
    #[default]
    Vad,
    Whole,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AsrPortErrorKind {
    InvalidRequest,
    FileSystem,
    Model,
    Authentication,
    RateLimited,
    Timeout,
    Network,
    Protocol,
    Unsupported,
    Unavailable,
    Runtime,
}

impl AsrPortErrorKind {
    /// Returns a stable SCREAMING_SNAKE_CASE code for this kind.
    pub fn as_code(self) -> &'static str {
        match self {
            Self::InvalidRequest => "INVALID_REQUEST",
            Self::FileSystem => "FILE_SYSTEM",
            Self::Model => "MODEL_ERROR",
            Self::Authentication => "AUTHENTICATION",
            Self::RateLimited => "RATE_LIMITED",
            Self::Timeout => "TIMEOUT",
            Self::Network => "NETWORK",
            Self::Protocol => "PROTOCOL",
            Self::Unsupported => "UNSUPPORTED",
            Self::Unavailable => "UNAVAILABLE",
            Self::Runtime => "RUNTIME",
        }
    }
}

/// A typed error returned by Core ASR ports.
///
/// `stable_code` is an optional override used to preserve backward-compatible
/// public codes (e.g. FFI or HTTP) when an adapter-specific error is mapped to
/// this type. Callers should use [`AsrPortError::code`] rather than reading
/// `kind` directly when they need the wire-level code string.
#[derive(Clone, Debug, thiserror::Error)]
#[error("{message}")]
pub struct AsrPortError {
    pub kind: AsrPortErrorKind,
    pub message: String,
    stable_code: Option<String>,
}

impl PartialEq for AsrPortError {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.message == other.message
    }
}

impl Eq for AsrPortError {}

impl AsrPortError {
    pub fn new(kind: AsrPortErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            stable_code: None,
        }
    }

    /// Attach a stable public code that overrides the kind-derived code.
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.stable_code = Some(code.into());
        self
    }

    /// Returns the stable code for use in public-facing contracts.
    /// Returns the override if one was set, otherwise falls back to
    /// `kind.as_code()`.
    pub fn code(&self) -> &str {
        self.stable_code
            .as_deref()
            .unwrap_or_else(|| self.kind.as_code())
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(AsrPortErrorKind::InvalidRequest, message)
    }

    pub fn runtime(message: impl Into<String>) -> Self {
        Self::new(AsrPortErrorKind::Runtime, message)
    }
}

impl Serialize for AsrPortError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("AsrPortError", 2)?;
        s.serialize_field("code", self.code())?;
        s.serialize_field("message", &self.message)?;
        s.end()
    }
}

impl From<String> for AsrPortError {
    fn from(message: String) -> Self {
        Self::runtime(message)
    }
}

impl From<&str> for AsrPortError {
    fn from(message: &str) -> Self {
        Self::runtime(message)
    }
}

#[async_trait]
pub trait BatchTranscriberPort: Send + Sync {
    async fn transcribe(
        &self,
        plan: BatchTranscribePlan,
    ) -> Result<Vec<TranscriptSegment>, AsrPortError>;

    async fn transcribe_with_observer(
        &self,
        plan: BatchTranscribePlan,
        observer: Arc<dyn BatchTranscriptionObserver>,
    ) -> Result<Vec<TranscriptSegment>, AsrPortError> {
        let segments = self.transcribe(plan).await?;
        observer.on_transcript_update(&TranscriptUpdate {
            remove_ids: Vec::new(),
            upsert_segments: segments.clone(),
        });
        observer.on_progress(100.0);
        Ok(segments)
    }
}

pub trait BatchTranscriptionObserver: Send + Sync {
    fn on_progress(&self, progress: f32);
    fn on_transcript_update(&self, update: &TranscriptUpdate);
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopBatchTranscriptionObserver;

impl BatchTranscriptionObserver for NoopBatchTranscriptionObserver {
    fn on_progress(&self, _progress: f32) {}

    fn on_transcript_update(&self, _update: &TranscriptUpdate) {}
}

#[derive(Clone, Debug, PartialEq)]
pub struct AsrTranscriptUpdateEvent {
    pub instance_id: String,
    pub stage: String,
    pub update: TranscriptUpdate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AsrStreamingErrorEvent {
    pub instance_id: String,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AsrStreamBoundaryEvent {
    pub instance_id: String,
    pub sequence: u64,
    pub end_sample: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AsrAudioFrame {
    pub sequence: u64,
    pub start_sample: u64,
    pub samples: Arc<[f32]>,
}

/// Assigns a monotonic sequence and sample cursor to frames produced by a
/// single input source. Hosts use this small pure helper when their capture
/// API only exposes raw PCM chunks.
#[derive(Debug, Default, Clone, Copy)]
pub struct StreamingAudioFrameCursor {
    next_sequence: u64,
    next_sample: u64,
}

impl StreamingAudioFrameCursor {
    pub fn next_samples(&mut self, samples: impl Into<Arc<[f32]>>) -> AsrAudioFrame {
        let samples = samples.into();
        let frame = AsrAudioFrame::new(self.next_sequence, self.next_sample, samples);
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.next_sample = frame.end_sample();
        frame
    }

    pub fn next_pcm_s16le(&mut self, bytes: &[u8]) -> Result<AsrAudioFrame, AsrPortError> {
        let frame = AsrAudioFrame::from_pcm_s16le(self.next_sequence, self.next_sample, bytes)?;
        self.observe(&frame);
        Ok(frame)
    }

    pub fn observe(&mut self, frame: &AsrAudioFrame) {
        self.next_sequence = self.next_sequence.max(frame.sequence.saturating_add(1));
        self.next_sample = self.next_sample.max(frame.end_sample());
    }
}

impl AsrAudioFrame {
    pub fn new(sequence: u64, start_sample: u64, samples: impl Into<Arc<[f32]>>) -> Self {
        Self {
            sequence,
            start_sample,
            samples: samples.into(),
        }
    }

    pub fn end_sample(&self) -> u64 {
        self.start_sample.saturating_add(self.samples.len() as u64)
    }

    pub fn from_pcm_s16le(
        sequence: u64,
        start_sample: u64,
        bytes: &[u8],
    ) -> Result<Self, AsrPortError> {
        if !bytes.len().is_multiple_of(2) {
            return Err(AsrPortError::invalid_request(
                "PCM16 payload must contain complete samples",
            ));
        }
        let samples = bytes
            .chunks_exact(2)
            .map(|sample| i16::from_le_bytes([sample[0], sample[1]]) as f32 / 32768.0)
            .collect::<Vec<_>>();
        Ok(Self::new(sequence, start_sample, samples))
    }
}

pub trait AsrRuntimeObserver: Send + Sync {
    fn on_transcript_update(&self, event: &AsrTranscriptUpdateEvent);
    fn on_model_load(&self, metric: &AsrModelLoadMetric);
    fn on_live_inference(&self, metric: &AsrInferenceMetric);

    fn on_streaming_error(&self, _event: &AsrStreamingErrorEvent) {}

    fn on_stream_boundary(&self, _event: &AsrStreamBoundaryEvent) {}
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopAsrRuntimeObserver;

impl AsrRuntimeObserver for NoopAsrRuntimeObserver {
    fn on_transcript_update(&self, _event: &AsrTranscriptUpdateEvent) {}

    fn on_model_load(&self, _metric: &AsrModelLoadMetric) {}

    fn on_live_inference(&self, _metric: &AsrInferenceMetric) {}
}

#[async_trait]
pub trait AsrStreamingSession: Send + Sync {
    async fn start(&self) -> Result<(), AsrPortError>;
    async fn stop(&self) -> Result<(), AsrPortError>;
    async fn flush(&self) -> Result<(), AsrPortError>;
    async fn feed_audio_frame(&self, frame: AsrAudioFrame) -> Result<(), AsrPortError>;
}

#[derive(Debug, Clone)]
pub struct BatchTranscriptionRequest {
    pub instance_id: Option<String>,
    pub file_path: PathBuf,
    pub save_to_path: Option<PathBuf>,
    pub model_path: String,
    pub num_threads: i32,
    pub enable_itn: bool,
    pub language: String,
    pub punctuation_model: Option<String>,
    pub vad_model: Option<String>,
    pub vad_buffer: f32,
    pub batch_segmentation_mode: BatchSegmentationMode,
    pub model_type: String,
    pub file_config: Option<ModelFileConfig>,
    pub hotwords: Option<String>,
    pub speaker_processing: Option<crate::transcription::speaker::SpeakerProcessingConfig>,
    pub normalization_options: TranscriptNormalizationOptions,
    pub postprocessor: TranscriptPostprocessor,
    pub gpu_acceleration: Option<String>,
    pub engine: LocalAsrEngine,
}

impl BatchTranscriptionRequest {
    pub fn from_local_asr_request(
        file_path: PathBuf,
        save_to_path: Option<PathBuf>,
        request: AsrTranscriptionRequest,
        speaker_processing: Option<crate::transcription::speaker::SpeakerProcessingConfig>,
        instance_id: Option<String>,
    ) -> Result<Self, AsrPortError> {
        let AsrTranscriptionRequest {
            language,
            enable_itn,
            normalization_options,
            postprocess_options,
            hotwords,
            engine_config,
            ..
        } = request;

        match engine_config {
            AsrEngineConfig::Local {
                local_engine,
                model_path,
                num_threads,
                punctuation_model,
                vad_model,
                vad_buffer,
                batch_segmentation_mode,
                model_type,
                file_config,
                gpu_acceleration,
                ..
            } => Ok(Self {
                instance_id,
                file_path,
                save_to_path,
                model_path,
                num_threads,
                enable_itn,
                language,
                punctuation_model,
                vad_model,
                vad_buffer,
                batch_segmentation_mode,
                model_type,
                file_config: *file_config,
                hotwords,
                speaker_processing,
                normalization_options,
                postprocessor: TranscriptPostprocessor::compile(postprocess_options)
                    .map_err(|error| AsrPortError::invalid_request(error.to_string()))?,
                gpu_acceleration,
                engine: local_engine,
            }),
            _ => Err(AsrPortError::invalid_request(
                "Expected local ASR engine config",
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalSherpaStreamingRequest {
    pub instance_id: String,
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
    pub normalization_options: TranscriptNormalizationOptions,
    pub postprocess_options: TranscriptPostprocessOptions,
    pub gpu_acceleration: Option<String>,
}

impl LocalSherpaStreamingRequest {
    pub fn from_local_sherpa_request(
        instance_id: String,
        request: AsrTranscriptionRequest,
    ) -> Result<Self, AsrPortError> {
        validate_local_asr_mode(&request, AsrMode::Streaming)?;

        let AsrTranscriptionRequest {
            language,
            enable_itn,
            normalization_options,
            postprocess_options,
            hotwords,
            engine_config,
            ..
        } = request;

        match engine_config {
            AsrEngineConfig::Local {
                local_engine,
                model_path,
                num_threads,
                punctuation_model,
                vad_model,
                vad_buffer,
                model_type,
                file_config,
                gpu_acceleration,
                ..
            } => {
                if local_engine != LocalAsrEngine::SherpaOnnx {
                    return Err(local_asr_engine_mismatch(
                        LocalAsrEngine::SherpaOnnx,
                        local_engine,
                    ));
                }
                Ok(Self {
                    instance_id,
                    model_path,
                    num_threads,
                    enable_itn,
                    language,
                    punctuation_model,
                    vad_model,
                    vad_buffer,
                    model_type,
                    file_config: *file_config,
                    hotwords,
                    normalization_options,
                    postprocess_options,
                    gpu_acceleration,
                })
            }
            _ => Err(AsrPortError::invalid_request(
                "Expected LocalSherpa engine config",
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct OnlineBatchTranscriptionRequest {
    pub file_path: PathBuf,
    pub request: AsrTranscriptionRequest,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OnlineBatchTranscriptionOutput {
    pub segments: Vec<TranscriptSegment>,
    pub audio_duration_ms: f64,
    pub buffered_samples: usize,
    pub stage: String,
}

#[async_trait]
pub trait OnlineBatchTranscriberPort: Send + Sync {
    async fn transcribe(
        &self,
        request: OnlineBatchTranscriptionRequest,
    ) -> Result<OnlineBatchTranscriptionOutput, AsrPortError>;
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AsrTranscriptionRequest {
    pub mode: AsrMode,
    pub language: String,
    pub enable_itn: bool,
    pub normalization_options: TranscriptNormalizationOptions,
    pub postprocess_options: TranscriptPostprocessOptions,
    pub hotwords: Option<String>,
    pub speaker_processing: Option<crate::transcription::speaker::SpeakerProcessingConfig>,

    #[serde(flatten)]
    pub engine_config: AsrEngineConfig,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(tag = "engine")]
pub enum AsrEngineConfig {
    /// Local offline transcription through a provider-crate engine.
    ///
    /// The `local-sherpa` alias keeps configs persisted by older versions
    /// deserializable; every write path emits the neutral `local` tag.
    #[serde(rename = "local", alias = "local-sherpa", rename_all = "camelCase")]
    Local {
        #[serde(default)]
        local_engine: LocalAsrEngine,
        #[serde(default)]
        model_id: Option<String>,
        model_path: String,
        num_threads: i32,
        #[serde(default)]
        punctuation_model: Option<String>,
        #[serde(default)]
        vad_model: Option<String>,
        #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
        vad_buffer: f32,
        #[serde(default)]
        batch_segmentation_mode: BatchSegmentationMode,
        model_type: String,
        #[serde(default)]
        file_config: Box<Option<ModelFileConfig>>,
        #[serde(default)]
        gpu_acceleration: Option<String>,
    },
    #[serde(rename = "online", rename_all = "camelCase")]
    Online {
        #[serde(rename = "onlineProvider")]
        provider: OnlineAsrProviderRequest,
    },
}

impl AsrEngineConfig {
    /// The selected local engine, or `None` for online configs.
    pub fn local_engine(&self) -> Option<LocalAsrEngine> {
        match self {
            AsrEngineConfig::Local { local_engine, .. } => Some(*local_engine),
            AsrEngineConfig::Online { .. } => None,
        }
    }
}

impl AsrTranscriptionRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn local_sherpa(
        mode: AsrMode,
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
        normalization_options: TranscriptNormalizationOptions,
        postprocess_options: TranscriptPostprocessOptions,
        speaker_processing: Option<crate::transcription::speaker::SpeakerProcessingConfig>,
        gpu_acceleration: Option<String>,
    ) -> Self {
        Self {
            mode,
            language,
            enable_itn,
            normalization_options,
            postprocess_options,
            hotwords,
            speaker_processing,
            engine_config: AsrEngineConfig::Local {
                local_engine: LocalAsrEngine::SherpaOnnx,
                model_id: None,
                model_path,
                num_threads,
                punctuation_model,
                vad_model,
                vad_buffer,
                batch_segmentation_mode: BatchSegmentationMode::Vad,
                model_type,
                file_config: Box::new(file_config),
                gpu_acceleration,
            },
        }
    }

    pub fn engine(&self) -> AsrEngine {
        match &self.engine_config {
            AsrEngineConfig::Local { .. } => AsrEngine::Local,
            AsrEngineConfig::Online { .. } => AsrEngine::Online,
        }
    }

    pub fn provider_id(&self) -> &str {
        match &self.engine_config {
            AsrEngineConfig::Local { local_engine, .. } => local_engine.provider_id(),
            AsrEngineConfig::Online { provider } => provider.provider_id.as_str(),
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct StreamingInferenceSpec {
    request: AsrTranscriptionRequest,
}

impl StreamingInferenceSpec {
    pub fn from_request(request: &AsrTranscriptionRequest) -> Result<Self, AsrPortError> {
        if request.mode != AsrMode::Streaming {
            return Err(AsrPortError::invalid_request(
                "Streaming inference requires ASR mode streaming",
            ));
        }

        let mut request = request.clone();
        request.normalization_options = TranscriptNormalizationOptions::default();
        request.postprocess_options = TranscriptPostprocessOptions::default();
        request.speaker_processing = None;
        Ok(Self { request })
    }

    pub fn engine_request(&self) -> AsrTranscriptionRequest {
        self.request.clone()
    }

    pub fn engine(&self) -> AsrEngine {
        self.request.engine()
    }

    pub fn provider_id(&self) -> &str {
        self.request.provider_id()
    }
}

impl fmt::Debug for StreamingInferenceSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StreamingInferenceSpec")
            .field("engine", &self.engine())
            .field("provider_id", &self.provider_id())
            .field("mode", &self.request.mode)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
pub struct StreamingOutputPolicy {
    normalization_options: TranscriptNormalizationOptions,
    postprocessor: TranscriptPostprocessor,
    next_timeline_id: Arc<AtomicU64>,
}

impl StreamingOutputPolicy {
    pub fn from_request(request: &AsrTranscriptionRequest) -> Result<Self, AsrPortError> {
        let postprocessor = TranscriptPostprocessor::compile(request.postprocess_options.clone())
            .map_err(|error| AsrPortError::invalid_request(error.to_string()))?;
        Ok(Self {
            normalization_options: request.normalization_options,
            postprocessor,
            next_timeline_id: Arc::new(AtomicU64::new(0)),
        })
    }

    pub fn process_update(&self, update: TranscriptUpdate) -> TranscriptUpdate {
        if !self.normalization_options.enable_timeline {
            return self.postprocessor.process_update(update);
        }

        let mut normalized = TranscriptUpdate {
            remove_ids: update.remove_ids,
            upsert_segments: Vec::new(),
        };
        for segment in update.upsert_segments {
            let segment_update =
                crate::transcription::transcript::build_transcript_update_with_id_generator(
                    segment,
                    self.normalization_options,
                    || {
                        let next = self.next_timeline_id.fetch_add(1, Ordering::Relaxed);
                        format!("timeline-{next}")
                    },
                );
            for remove_id in segment_update.remove_ids {
                if !normalized.remove_ids.contains(&remove_id) {
                    normalized.remove_ids.push(remove_id);
                }
            }
            normalized
                .upsert_segments
                .extend(segment_update.upsert_segments);
        }
        self.postprocessor.process_update(normalized)
    }
}

#[async_trait]
pub trait StreamingAsrFactoryPort: Send + Sync {
    async fn prepare(&self, spec: &StreamingInferenceSpec) -> Result<(), AsrPortError>;

    async fn create(
        &self,
        pipeline_id: &str,
        spec: &StreamingInferenceSpec,
        observer: Arc<dyn AsrRuntimeObserver>,
    ) -> Result<Arc<dyn AsrStreamingSession>, AsrPortError>;
}

pub fn validate_local_asr_mode(
    request: &AsrTranscriptionRequest,
    expected: AsrMode,
) -> Result<(), AsrPortError> {
    if request.engine() != AsrEngine::Local {
        return Err(AsrPortError::new(
            AsrPortErrorKind::Unsupported,
            "Unsupported ASR engine for local ASR adapter",
        ));
    }
    if request.mode != expected {
        return Err(AsrPortError::invalid_request(format!(
            "ASR request mode mismatch: expected {:?}, got {:?}",
            expected, request.mode
        )));
    }
    Ok(())
}

pub fn local_asr_engine_mismatch(
    adapter: LocalAsrEngine,
    requested: LocalAsrEngine,
) -> AsrPortError {
    AsrPortError::new(
        AsrPortErrorKind::Unsupported,
        format!(
            "Local ASR adapter '{}' cannot execute engine '{}'.",
            adapter.as_str(),
            requested.as_str()
        ),
    )
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct OnlineAsrProviderRequest {
    pub provider_id: String,
    pub profile_id: String,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Unknown))]
    pub config: Value,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct VolcengineDoubaoAsrConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub streaming_endpoint: String,
    #[serde(default)]
    pub streaming_resource_id: String,
    #[serde(default)]
    pub batch_endpoint: String,
    #[serde(default)]
    pub batch_resource_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
struct OnlineAsrProviderManifest {
    schema_version: u32,
    providers: Vec<OnlineAsrProvider>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct OnlineAsrProvider {
    pub id: String,
    pub profile_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Unknown))]
    pub defaults: Value,
    pub streaming: OnlineAsrCapability,
    pub batch: OnlineAsrBatchCapability,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct OnlineAsrCapability {
    pub supported: Option<bool>,
    pub requires_api_key: bool,
    pub required_config_fields: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct OnlineAsrBatchCapability {
    pub requires_api_key: bool,
    pub required_config_fields: Vec<String>,
    pub local_file_mode: OnlineAsrLocalFileBatchMode,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct OnlineAsrLocalFileBatchMode {
    pub supported: bool,
    pub endpoint: String,
    pub resource_id: String,
    pub unsupported_message: String,
}

static ONLINE_ASR_PROVIDER_MANIFEST: OnceLock<OnlineAsrProviderManifest> = OnceLock::new();

fn online_asr_provider_manifest() -> &'static OnlineAsrProviderManifest {
    ONLINE_ASR_PROVIDER_MANIFEST.get_or_init(|| {
        let manifest: OnlineAsrProviderManifest = serde_json::from_str(ONLINE_ASR_PROVIDERS_JSON)
            .expect("shared online ASR providers JSON should be valid");
        assert_eq!(
            manifest.schema_version, 1,
            "shared online ASR providers schema version should be supported"
        );
        assert!(
            manifest
                .providers
                .iter()
                .any(|provider| provider.id == VOLCENGINE_DOUBAO_PROVIDER_ID),
            "shared online ASR providers JSON should include Volcengine Doubao"
        );
        for provider in &manifest.providers {
            assert!(
                !provider.profile_id.trim().is_empty(),
                "online ASR provider profile id should not be empty"
            );
            validate_capability_config_fields(
                &provider.id,
                "streaming",
                &provider.streaming.required_config_fields,
            );
            validate_capability_config_fields(
                &provider.id,
                "batch",
                &provider.batch.required_config_fields,
            );
            if provider.streaming.requires_api_key || provider.batch.requires_api_key {
                assert!(
                    provider.defaults.get("apiKey").is_some()
                        || provider
                            .streaming
                            .required_config_fields
                            .iter()
                            .chain(provider.batch.required_config_fields.iter())
                            .any(|field| field == "apiKey"),
                    "online ASR provider requiring an API key should declare apiKey"
                );
            }
            if provider.batch.local_file_mode.supported {
                assert!(
                    !provider.batch.local_file_mode.endpoint.trim().is_empty(),
                    "online ASR local file mode endpoint should not be empty"
                );
            } else {
                assert!(
                    !provider
                        .batch
                        .local_file_mode
                        .unsupported_message
                        .trim()
                        .is_empty(),
                    "online ASR local file mode unsupported message should not be empty"
                );
            }
        }
        manifest
    })
}

fn validate_capability_config_fields(provider_id: &str, label: &str, fields: &[String]) {
    for field in fields {
        assert!(
            !field.trim().is_empty(),
            "online ASR provider {provider_id} {label} config field should not be empty"
        );
    }
}

pub fn online_asr_providers() -> &'static [OnlineAsrProvider] {
    online_asr_provider_manifest().providers.as_slice()
}

pub fn find_online_asr_provider(provider_id: &str) -> Option<&'static OnlineAsrProvider> {
    online_asr_providers()
        .iter()
        .find(|provider| provider.id == provider_id)
}

bitflags::bitflags! {
    /// Declares which engine-neutral Core ports a local ASR engine can back.
    ///
    /// Capability bits are the single source of truth for feature gating:
    /// hosts derive UI availability and run-time routing decisions from this
    /// set instead of special-casing individual engines.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct EngineCapabilities: u16 {
        /// Can transcribe complete audio files through [`BatchTranscriberPort`].
        const BATCH = 1 << 0;
        /// Can create live sessions through [`StreamingAsrFactoryPort`].
        const STREAMING = 1 << 1;
        /// Can attribute transcript segments to speakers.
        const SPEAKER = 1 << 2;
        /// Can restore punctuation on transcripts.
        const PUNCTUATION = 1 << 3;
        /// Can bias recognition with user-provided hotwords.
        const HOTWORDS = 1 << 4;
        /// Can offload inference to a GPU when one is available.
        const GPU = 1 << 5;
    }
}

/// Engine-owned facade exposing one local ASR engine behind the
/// engine-neutral Core ports.
///
/// Implementations live in provider crates. Application and platform code
/// must depend only on this trait; hosts compose concrete adapters into an
/// application-level registry so adding an engine never requires touching
/// Core or shared wiring.
pub trait LocalAsrAdapter: Send + Sync {
    /// The local engine implemented by this adapter.
    fn engine(&self) -> LocalAsrEngine;

    /// The ports this adapter can back. Must agree with the accessors below:
    /// `BATCH` implies [`Self::batch_transcriber`] always succeeds, and the
    /// `STREAMING` bit implies [`Self::streaming_factory`] returns `Some`.
    fn capabilities(&self) -> EngineCapabilities;

    /// Batch transcription for this engine. Only meaningful when
    /// `capabilities()` contains `BATCH`.
    fn batch_transcriber(&self) -> Arc<dyn BatchTranscriberPort>;

    /// Streaming session factory for this engine, or `None` when the engine
    /// does not support live transcription.
    fn streaming_factory(&self) -> Option<Arc<dyn StreamingAsrFactoryPort>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_sherpa_request_builder_sets_defaults() {
        let request = AsrTranscriptionRequest::local_sherpa(
            AsrMode::Batch,
            "model".to_string(),
            4,
            true,
            "auto".to_string(),
            None,
            None,
            5.0,
            "whisper".to_string(),
            None,
            None,
            TranscriptNormalizationOptions::default(),
            TranscriptPostprocessOptions::default(),
            None,
            None,
        );

        assert_eq!(request.engine(), AsrEngine::Local);
        assert!(matches!(
            request.engine_config,
            AsrEngineConfig::Local {
                batch_segmentation_mode: BatchSegmentationMode::Vad,
                ..
            }
        ));
    }

    #[test]
    fn online_request_serializes_in_camel_case() {
        let request = AsrTranscriptionRequest {
            mode: AsrMode::Streaming,
            language: "auto".to_string(),
            enable_itn: false,
            normalization_options: TranscriptNormalizationOptions::default(),
            postprocess_options: TranscriptPostprocessOptions::default(),
            hotwords: None,
            speaker_processing: None,
            engine_config: AsrEngineConfig::Online {
                provider: OnlineAsrProviderRequest {
                    provider_id: "volcengine".to_string(),
                    profile_id: "default".to_string(),
                    config: serde_json::json!({"apiKey":"secret"}),
                },
            },
        };

        let json = serde_json::to_value(request).unwrap();
        assert_eq!(json["engine"], "online");
        assert_eq!(json["mode"], "streaming");
        assert_eq!(json["onlineProvider"]["providerId"], "volcengine");
    }
}
