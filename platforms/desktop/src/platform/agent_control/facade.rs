use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::services::DesktopServices;
use sona_application::live_transcription::{LiveInputTransform, LiveSourceEpoch};
use sona_core::export::{ExportFormat, ExportMode};
use sona_core::history::store::HistoryStore;
use sona_core::history::{
    HistorySummaryPayload, HistoryWorkspaceQueryResult, TranscriptSummaryRecordPayload,
};
use sona_core::ports::asr::{
    AsrEngineConfig, AsrMode, AsrRuntimeObserver, AsrTranscriptUpdateEvent,
    AsrTranscriptionRequest, OnlineAsrProviderRequest,
};
use sona_core::project::{ProjectCreateInput, ProjectRecord, ProjectUpdateInput};
use sona_core::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
use sona_core::transcription::transcript::TranscriptSegment;

pub(crate) struct AgentAsrRuntimeObserver {
    inner: crate::integrations::asr::TauriAsrRuntimeObserver,
    segments: Arc<std::sync::Mutex<Vec<TranscriptSegment>>>,
}

impl AgentAsrRuntimeObserver {
    pub(crate) fn new(
        inner: crate::integrations::asr::TauriAsrRuntimeObserver,
        segments: Arc<std::sync::Mutex<Vec<TranscriptSegment>>>,
    ) -> Self {
        Self { inner, segments }
    }
}

impl AsrRuntimeObserver for AgentAsrRuntimeObserver {
    fn on_transcript_update(&self, event: &AsrTranscriptUpdateEvent) {
        self.inner.on_transcript_update(event);

        if let Ok(mut segs) = self.segments.lock() {
            if !event.update.remove_ids.is_empty() {
                segs.retain(|s| !event.update.remove_ids.contains(&s.id));
            }
            for upsert in &event.update.upsert_segments {
                if let Some(pos) = segs.iter().position(|s| s.id == upsert.id) {
                    segs[pos] = upsert.clone();
                } else {
                    segs.push(upsert.clone());
                }
            }
        }
    }

    fn on_model_load(&self, metric: &AsrModelLoadMetric) {
        self.inner.on_model_load(metric);
    }

    fn on_live_inference(&self, metric: &AsrInferenceMetric) {
        self.inner.on_live_inference(metric);
    }
}

#[derive(Clone)]
pub struct ActiveRecordingSession {
    pub history_id: String,
    pub consumer_id: String,
    pub started_at_epoch: u64,
    pub started_at_instant: std::time::Instant,
    pub segments: Arc<std::sync::Mutex<Vec<TranscriptSegment>>>,
    pub has_coordinator_consumer: bool,
    pub coordinator_released: bool,
    pub audio_stopped: bool,
    pub frozen_duration_seconds: Option<f64>,
}

#[derive(Clone)]
pub struct AgentControlFacade {
    pub(crate) services: DesktopServices,
    app_handle: Option<tauri::AppHandle>,
    active_session: Arc<Mutex<Option<ActiveRecordingSession>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientStateResult {
    pub online: bool,
    pub is_recording: bool,
    pub active_project_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StartRecordingRequest {
    #[serde(default, alias = "projectId", alias = "project_id")]
    pub project_id: Option<String>,
    #[serde(default, alias = "deviceName", alias = "device_name")]
    pub device_name: Option<String>,
    #[serde(default, alias = "asrRequest", alias = "asr_request")]
    pub asr_request: Option<AsrTranscriptionRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRecordingResult {
    pub history_id: String,
    pub started_at: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StopRecordingRequest {
    #[serde(default)]
    pub discard: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StopRecordingResult {
    pub history_id: String,
    pub duration_seconds: f64,
    pub segment_count: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryHistoryRequest {
    #[serde(default)]
    pub query: String,
    #[serde(default, alias = "projectId", alias = "project_id")]
    pub project_id: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItemSummary {
    pub id: String,
    pub title: String,
    pub preview_text: String,
    pub timestamp: u64,
    pub duration: f64,
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadTranscriptResult {
    pub history_id: String,
    pub segments: Vec<TranscriptSegment>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditTranscriptRequest {
    #[serde(alias = "historyId", alias = "history_id")]
    pub history_id: String,
    pub segments: Vec<TranscriptSegment>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditTranscriptResult {
    pub success: bool,
    pub snapshot_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteHistoryRequest {
    #[serde(alias = "historyId", alias = "history_id")]
    pub history_id: String,
    #[serde(default)]
    pub permanent: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SetActiveProjectRequest {
    #[serde(default, alias = "projectId", alias = "project_id")]
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GetSettingsRequest {
    pub key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSettingRequest {
    pub key: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TranscribeFileRequest {
    #[serde(alias = "filePath", alias = "file_path")]
    pub file_path: String,
    #[serde(default, alias = "projectId", alias = "project_id")]
    pub project_id: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default, alias = "saveToPath", alias = "save_to_path")]
    pub save_to_path: Option<String>,
    #[serde(default, alias = "instanceId", alias = "instance_id")]
    pub instance_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscribeFileResult {
    pub history_id: String,
    pub duration_seconds: f64,
    pub segment_count: usize,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExportTranscriptRequest {
    #[serde(alias = "historyId", alias = "history_id")]
    pub history_id: String,
    pub format: String,
    #[serde(alias = "outputPath", alias = "output_path")]
    pub output_path: String,
    #[serde(default)]
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportTranscriptResult {
    pub success: bool,
    pub output_path: String,
    pub format: String,
    pub segment_count: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SaveSummaryRequest {
    #[serde(alias = "historyId", alias = "history_id")]
    pub history_id: String,
    pub content: String,
    #[serde(default, alias = "templateId", alias = "template_id")]
    pub template_id: Option<String>,
    #[serde(default)]
    pub thought: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DownloadPresetModelRequest {
    #[serde(alias = "modelId", alias = "model_id")]
    pub model_id: String,
    #[serde(default)]
    pub mirror: Option<String>,
    #[serde(default, alias = "downloadId", alias = "download_id")]
    pub download_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CreateProjectRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct UpdateProjectRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct QueryTrashRequest {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
}

pub fn resolve_batch_asr_request_from_config(
    config: &serde_json::Value,
    language_override: Option<&str>,
) -> Result<AsrTranscriptionRequest, String> {
    let batch_selection = config
        .pointer("/asr/selections/batch")
        .or_else(|| config.pointer("/asr/batch"));

    if let Some(batch) = batch_selection {
        let engine = batch
            .get("engine")
            .and_then(|e| e.as_str())
            .unwrap_or("local");
        if engine == "online" {
            let provider_id = batch
                .get("providerId")
                .or_else(|| batch.get("provider_id"))
                .and_then(|p| p.as_str())
                .unwrap_or("volcengine-doubao");

            let profile_id = batch
                .get("profileId")
                .or_else(|| batch.get("profile_id"))
                .and_then(|p| p.as_str())
                .unwrap_or("default");

            let provider_config = config
                .pointer(&format!("/asr/providers/online/{provider_id}"))
                .or_else(|| config.pointer(&format!("/asr/providers/{provider_id}")))
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));

            let language = language_override
                .map(ToString::to_string)
                .or_else(|| {
                    batch
                        .get("language")
                        .or_else(|| config.get("language"))
                        .and_then(|l| l.as_str())
                        .map(ToString::to_string)
                })
                .unwrap_or_else(|| "auto".to_string());

            let enable_itn = config
                .get("enableITN")
                .or_else(|| config.get("enable_itn"))
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            return Ok(AsrTranscriptionRequest {
                mode: AsrMode::Batch,
                language,
                enable_itn,
                normalization_options: Default::default(),
                postprocess_options: Default::default(),
                hotwords: None,
                speaker_processing: None,
                engine_config: AsrEngineConfig::Online {
                    provider: OnlineAsrProviderRequest {
                        provider_id: provider_id.to_string(),
                        profile_id: profile_id.to_string(),
                        config: provider_config,
                    },
                },
            });
        }

        let model_path = batch
            .get("modelPath")
            .or_else(|| batch.get("model_path"))
            .and_then(|p| p.as_str())
            .unwrap_or("");

        if !model_path.trim().is_empty() {
            let language = language_override
                .map(ToString::to_string)
                .or_else(|| {
                    batch
                        .get("language")
                        .or_else(|| config.get("language"))
                        .and_then(|l| l.as_str())
                        .map(ToString::to_string)
                })
                .unwrap_or_else(|| "auto".to_string());

            let enable_itn = config
                .get("enableITN")
                .or_else(|| config.get("enable_itn"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let vad_model = batch
                .get("vadModel")
                .or_else(|| batch.get("vad_model"))
                .and_then(|v| v.as_str())
                .map(ToString::to_string);

            let punctuation_model = batch
                .get("punctuationModel")
                .or_else(|| batch.get("punctuation_model"))
                .and_then(|v| v.as_str())
                .map(ToString::to_string);

            let num_threads = batch
                .get("numThreads")
                .or_else(|| batch.get("num_threads"))
                .and_then(|t| t.as_i64())
                .unwrap_or(4) as i32;

            return Ok(AsrTranscriptionRequest::local_sherpa(
                AsrMode::Batch,
                model_path.to_string(),
                num_threads,
                enable_itn,
                language,
                punctuation_model,
                vad_model,
                0.5,
                String::new(),
                None,
                None,
                Default::default(),
                Default::default(),
                None,
                None,
            ));
        }
    }

    let legacy_model_path = config
        .get("batchModelPath")
        .or_else(|| config.get("batch_model_path"))
        .and_then(|p| p.as_str())
        .unwrap_or("");

    if !legacy_model_path.trim().is_empty() {
        let language = language_override
            .map(ToString::to_string)
            .or_else(|| {
                config
                    .get("language")
                    .and_then(|l| l.as_str())
                    .map(ToString::to_string)
            })
            .unwrap_or_else(|| "auto".to_string());

        let enable_itn = config
            .get("enableITN")
            .or_else(|| config.get("enable_itn"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let vad_model = config
            .get("vadModel")
            .or_else(|| config.get("vad_model"))
            .and_then(|v| v.as_str())
            .map(ToString::to_string);

        let punctuation_model = config
            .get("punctuationModel")
            .or_else(|| config.get("punctuation_model"))
            .and_then(|v| v.as_str())
            .map(ToString::to_string);

        return Ok(AsrTranscriptionRequest::local_sherpa(
            AsrMode::Batch,
            legacy_model_path.to_string(),
            4,
            enable_itn,
            language,
            punctuation_model,
            vad_model,
            0.5,
            String::new(),
            None,
            None,
            Default::default(),
            Default::default(),
            None,
            None,
        ));
    }

    match resolve_live_asr_request_from_config(config) {
        Ok(mut req) => {
            req.mode = AsrMode::Batch;
            if let Some(lang) = language_override {
                req.language = lang.to_string();
            }
            Ok(req)
        }
        Err(_) => Err(
            "No batch or live ASR model or provider is configured in Sona client settings"
                .to_string(),
        ),
    }
}

pub fn resolve_live_asr_request_from_config(
    config: &serde_json::Value,
) -> Result<AsrTranscriptionRequest, String> {
    let live_selection = config
        .pointer("/asr/selections/live")
        .or_else(|| config.pointer("/asr/live"));

    if let Some(live) = live_selection {
        let engine = live
            .get("engine")
            .and_then(|e| e.as_str())
            .unwrap_or("local");
        if engine == "online" {
            let provider_id = live
                .get("providerId")
                .or_else(|| live.get("provider_id"))
                .and_then(|p| p.as_str())
                .unwrap_or("volcengine-doubao");

            let profile_id = live
                .get("profileId")
                .or_else(|| live.get("profile_id"))
                .and_then(|p| p.as_str())
                .unwrap_or("default");

            let provider_config = config
                .pointer(&format!("/asr/providers/online/{provider_id}"))
                .or_else(|| config.pointer(&format!("/asr/providers/{provider_id}")))
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));

            let language = live
                .get("language")
                .or_else(|| config.get("language"))
                .and_then(|l| l.as_str())
                .unwrap_or("auto")
                .to_string();

            let enable_itn = config
                .get("enableITN")
                .or_else(|| config.get("enable_itn"))
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            return Ok(AsrTranscriptionRequest {
                mode: AsrMode::Streaming,
                language,
                enable_itn,
                normalization_options: Default::default(),
                postprocess_options: Default::default(),
                hotwords: None,
                speaker_processing: None,
                engine_config: AsrEngineConfig::Online {
                    provider: OnlineAsrProviderRequest {
                        provider_id: provider_id.to_string(),
                        profile_id: profile_id.to_string(),
                        config: provider_config,
                    },
                },
            });
        }

        let model_path = live
            .get("modelPath")
            .or_else(|| live.get("model_path"))
            .and_then(|p| p.as_str())
            .unwrap_or("");

        if !model_path.trim().is_empty() {
            let language = live
                .get("language")
                .or_else(|| config.get("language"))
                .and_then(|l| l.as_str())
                .unwrap_or("auto")
                .to_string();

            let enable_itn = config
                .get("enableITN")
                .or_else(|| config.get("enable_itn"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let vad_model = live
                .get("vadModel")
                .or_else(|| live.get("vad_model"))
                .and_then(|v| v.as_str())
                .map(ToString::to_string);

            let punctuation_model = live
                .get("punctuationModel")
                .or_else(|| live.get("punctuation_model"))
                .and_then(|p| p.as_str())
                .map(ToString::to_string);

            let num_threads = live
                .get("numThreads")
                .or_else(|| live.get("num_threads"))
                .and_then(|t| t.as_i64())
                .unwrap_or(4) as i32;

            return Ok(AsrTranscriptionRequest::local_sherpa(
                AsrMode::Streaming,
                model_path.to_string(),
                num_threads,
                enable_itn,
                language,
                punctuation_model,
                vad_model,
                0.5,
                String::new(),
                None,
                None,
                Default::default(),
                Default::default(),
                None,
                None,
            ));
        }
    }

    let legacy_model_path = config
        .get("streamingModelPath")
        .or_else(|| config.get("streaming_model_path"))
        .or_else(|| config.get("modelPath"))
        .or_else(|| config.get("model_path"))
        .and_then(|p| p.as_str())
        .unwrap_or("");

    if !legacy_model_path.trim().is_empty() {
        let language = config
            .get("language")
            .and_then(|l| l.as_str())
            .unwrap_or("auto")
            .to_string();

        let enable_itn = config
            .get("enableITN")
            .or_else(|| config.get("enable_itn"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let vad_model = config
            .get("liveVadModelPath")
            .or_else(|| config.get("live_vad_model_path"))
            .and_then(|v| v.as_str())
            .map(ToString::to_string);

        let punctuation_model = config
            .get("livePunctuationModelPath")
            .or_else(|| config.get("live_punctuation_model_path"))
            .and_then(|p| p.as_str())
            .map(ToString::to_string);

        return Ok(AsrTranscriptionRequest::local_sherpa(
            AsrMode::Streaming,
            legacy_model_path.to_string(),
            4,
            enable_itn,
            language,
            punctuation_model,
            vad_model,
            0.5,
            String::new(),
            None,
            None,
            Default::default(),
            Default::default(),
            None,
            None,
        ));
    }

    Err("No live ASR model or provider is configured in Sona client settings".to_string())
}

impl AgentControlFacade {
    pub fn new(services: DesktopServices, app_handle: Option<tauri::AppHandle>) -> Self {
        Self {
            services,
            app_handle,
            active_session: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn get_client_state(&self) -> Result<ClientStateResult, String> {
        let is_recording =
            self.active_session.lock().await.is_some() || self.services.audio.has_active_captures();
        let active_project_id = self
            .services
            .projects
            .get_active_tag_id()
            .await
            .ok()
            .flatten();
        Ok(ClientStateResult {
            online: true,
            is_recording,
            active_project_id,
        })
    }

    pub fn focus_window(&self) -> Result<bool, String> {
        if let Some(app) = &self.app_handle {
            use tauri::Manager;
            if let Some(window) = app.get_webview_window(crate::app::window::MAIN_WINDOW_LABEL) {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub async fn start_recording(
        &self,
        req: StartRecordingRequest,
    ) -> Result<StartRecordingResult, String> {
        if self.services.audio.has_active_captures() || self.services.asr.is_busy().await {
            return Err("DEVICE_BUSY: Audio capture or ASR engine is currently in use".to_string());
        }

        let mut session_guard = self.active_session.lock().await;
        if session_guard.is_some() {
            return Err(
                "RECORDING_ALREADY_ACTIVE: A recording session is already in progress".to_string(),
            );
        }

        // 1. Resolve ASR request FIRST before creating draft or starting hardware capture
        // Prevents any orphaned draft or audio capture if config I/O fails.
        let resolved_asr_req = match req.asr_request {
            Some(explicit) => Some(explicit),
            None => {
                let cfg_opt = self
                    .services
                    .config
                    .load()
                    .map_err(|e| format!("Failed to load config: {e}"))?;
                match &cfg_opt {
                    Some(cfg) => resolve_live_asr_request_from_config(cfg).ok(),
                    None => None,
                }
            }
        };

        let history_id = uuid::Uuid::new_v4().to_string();
        let now_epoch_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs();

        let draft_req = crate::platform::history_repository::HistoryCreateLiveDraftRequest {
            id: Some(history_id.clone()),
            audio_extension: "wav".to_string(),
            tag_ids: req.project_id.clone().into_iter().collect(),
            project_id: req.project_id.clone(),
            icon: None,
        };

        let draft = self
            .services
            .history
            .mutation_file(move |service| service.create_live_draft(draft_req))
            .await
            .map_err(|e| e.to_string())?;

        let audio_state = self.services.audio.clone();
        let asr_state = self.services.asr.clone();
        let emitter = self.services.emitter.clone();
        let app_data_dir = self
            .services
            .sqlite
            .current_context()
            .map_err(|e| e.to_string())?
            .app_data_dir()
            .to_path_buf();

        let consumer_id = format!("agent-recording-{}", draft.item.id);
        let output_path = Some(draft.audio_absolute_path.clone());
        let device_name = req.device_name.clone();
        let consumer_id_clone = consumer_id.clone();

        let lease = match crate::platform::blocking::spawn_blocking_map(move || {
            crate::integrations::audio::start_native_live_capture(
                audio_state,
                asr_state,
                emitter,
                app_data_dir,
                "microphone",
                device_name,
                consumer_id_clone,
                output_path,
            )
        })
        .await
        {
            Ok(lease) => lease,
            Err(e) => {
                let hid = draft.item.id.clone();
                let _ = self
                    .services
                    .history
                    .mutation_file(move |service| {
                        service.purge_items(
                            sona_core::history::mutation_repository::HistoryDeleteItemsRequest {
                                ids: vec![hid],
                            },
                        )
                    })
                    .await;
                return Err(format!("Failed to start native live capture: {e}"));
            }
        };

        let segments = Arc::new(std::sync::Mutex::new(Vec::new()));
        let inner_observer = crate::integrations::asr::TauriAsrRuntimeObserver::new(
            self.services.emitter.clone(),
            self.services.asr.metrics_store(),
        );
        let observer = Arc::new(AgentAsrRuntimeObserver::new(
            inner_observer,
            segments.clone(),
        )) as Arc<dyn AsrRuntimeObserver>;

        let mut has_coordinator_consumer = false;
        if let Some(asr_req) = resolved_asr_req {
            if self
                .services
                .asr
                .live_coordinator()
                .has_consumer(&consumer_id)
                .await
            {
                let _ = self
                    .services
                    .asr
                    .live_coordinator()
                    .release(&consumer_id)
                    .await;
            }
            match self
                .services
                .asr
                .live_coordinator()
                .acquire(
                    consumer_id.clone(),
                    LiveSourceEpoch::new(lease.source_id.clone(), lease.source_generation),
                    lease.source_cursor,
                    LiveInputTransform { gain: 1.0 },
                    asr_req,
                    observer,
                )
                .await
            {
                Ok(_) => {
                    has_coordinator_consumer = true;
                }
                Err(e) => {
                    // Rollback native live capture and draft on acquire failure
                    let _ = crate::integrations::audio::stop_native_live_capture(
                        &self.services.audio,
                        "microphone",
                        consumer_id.clone(),
                    )
                    .await;
                    let hid = draft.item.id.clone();
                    let _ = self.services.history.mutation_file(move |service| {
                        service.purge_items(sona_core::history::mutation_repository::HistoryDeleteItemsRequest {
                            ids: vec![hid],
                        })
                    }).await;
                    return Err(format!(
                        "ASR_ACQUIRE_FAILED: Failed to acquire ASR coordinator for live transcription: {e}"
                    ));
                }
            }
        }

        *session_guard = Some(ActiveRecordingSession {
            history_id: draft.item.id.clone(),
            consumer_id,
            started_at_epoch: now_epoch_secs,
            started_at_instant: std::time::Instant::now(),
            segments,
            has_coordinator_consumer,
            coordinator_released: false,
            audio_stopped: false,
            frozen_duration_seconds: None,
        });

        let _ = self.services.emitter.emit(
            "agent-control-recording-status",
            serde_json::json!({ "active": true, "historyId": &draft.item.id }),
        );

        Ok(StartRecordingResult {
            history_id: draft.item.id,
            started_at: now_epoch_secs,
        })
    }

    pub async fn stop_recording(
        &self,
        req: StopRecordingRequest,
    ) -> Result<StopRecordingResult, String> {
        let (session_snapshot, duration_seconds) = {
            let mut session_guard = self.active_session.lock().await;
            let session = session_guard.as_mut().ok_or_else(|| {
                "NO_ACTIVE_RECORDING: No recording is currently in progress".to_string()
            })?;

            let dur = session
                .frozen_duration_seconds
                .unwrap_or_else(|| session.started_at_instant.elapsed().as_secs_f64());
            session.frozen_duration_seconds = Some(dur);
            (session.clone(), dur)
        };

        // 1. Release coordinator if active and not already released
        if session_snapshot.has_coordinator_consumer && !session_snapshot.coordinator_released {
            self.services
                .asr
                .live_coordinator()
                .release(&session_snapshot.consumer_id)
                .await
                .map_err(|e| {
                    format!("ASR_RELEASE_FAILED: Failed to release ASR coordinator: {e}")
                })?;
            if let Some(s) = self.active_session.lock().await.as_mut() {
                s.coordinator_released = true;
            }
        }

        // 2. Stop audio capture if not already stopped
        if !session_snapshot.audio_stopped {
            crate::integrations::audio::stop_native_live_capture(
                &self.services.audio,
                "microphone",
                session_snapshot.consumer_id.clone(),
            )
            .await
            .map_err(|e| format!("AUDIO_STOP_FAILED: Failed to stop native audio capture: {e}"))?;
            if let Some(s) = self.active_session.lock().await.as_mut() {
                s.audio_stopped = true;
            }
        }

        // 3. Finalize or discard draft with strict error checking
        let segment_count = if req.discard {
            let deleted_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_millis() as u64;
            let hid = session_snapshot.history_id.clone();
            self.services
                .history
                .mutation_file(move |service| {
                    service.trash_items(
                        sona_core::history::mutation_repository::HistoryTrashItemsRequest {
                            ids: vec![hid],
                            deleted_at,
                        },
                    )
                })
                .await
                .map_err(|e| {
                    format!("TRASH_DRAFT_FAILED: Failed to discard recording draft: {e}")
                })?;
            0
        } else {
            let collected = session_snapshot
                .segments
                .lock()
                .map(|s| s.clone())
                .unwrap_or_default();
            let segments = if !collected.is_empty() {
                collected
            } else {
                let hid = session_snapshot.history_id.clone();
                self.services
                    .history
                    .query_db(move |service| service.load_transcript(&hid))
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or_default()
            };
            let count = segments.len();

            let complete_req =
                sona_core::history::mutation_repository::HistoryCompleteLiveDraftRequest {
                    history_id: session_snapshot.history_id.clone(),
                    segments,
                    duration: duration_seconds,
                };

            self.services
                .history
                .mutation_file(move |service| service.complete_live_draft(complete_req))
                .await
                .map_err(|e| {
                    format!("COMPLETE_DRAFT_FAILED: Failed to complete recording draft: {e}")
                })?;
            count
        };

        // All steps succeeded; clear active session
        {
            let mut session_guard = self.active_session.lock().await;
            *session_guard = None;
        }

        let _ = self.services.emitter.emit(
            "agent-control-recording-status",
            serde_json::json!({ "active": false }),
        );

        Ok(StopRecordingResult {
            history_id: session_snapshot.history_id,
            duration_seconds,
            segment_count,
        })
    }

    pub async fn query_history(
        &self,
        req: QueryHistoryRequest,
    ) -> Result<Vec<HistoryItemSummary>, String> {
        let scope = if let Some(pid) = req.project_id {
            sona_core::history::HistoryWorkspaceScope::Project { project_id: pid }
        } else {
            sona_core::history::HistoryWorkspaceScope::All
        };

        let query_req = sona_core::history::HistoryWorkspaceQueryRequest {
            scope,
            query: req.query,
            filter_type: sona_core::history::HistoryWorkspaceFilterType::All,
            date_filter: sona_core::history::HistoryWorkspaceDateFilter::All,
            sort_order: sona_core::history::HistoryWorkspaceSortOrder::Newest,
            limit: req.limit.unwrap_or(20),
            offset: req.offset.unwrap_or(0),
        };

        let result: HistoryWorkspaceQueryResult = self
            .services
            .history
            .query_db(move |service| service.query_workspace(query_req))
            .await
            .map_err(|e| e.to_string())?;

        let summaries = result
            .filtered_items
            .into_iter()
            .map(|item| HistoryItemSummary {
                id: item.id,
                title: item.title,
                preview_text: item.preview_text,
                timestamp: item.timestamp,
                duration: item.duration,
                project_id: item.project_id,
            })
            .collect();

        Ok(summaries)
    }

    pub async fn read_transcript(
        &self,
        history_id: String,
    ) -> Result<ReadTranscriptResult, String> {
        let hid = history_id.clone();
        let segments = self
            .services
            .history
            .query_db(move |service| service.load_transcript(&hid))
            .await
            .map_err(|e| e.to_string())?
            .unwrap_or_default();

        let text = segments
            .iter()
            .map(|s| s.text.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        Ok(ReadTranscriptResult {
            history_id,
            segments,
            text,
        })
    }

    pub async fn edit_transcript(
        &self,
        req: EditTranscriptRequest,
    ) -> Result<EditTranscriptResult, String> {
        let hid = req.history_id.clone();
        let base_segments = self
            .services
            .history
            .query_db(move |service| service.load_transcript(&hid))
            .await
            .map_err(|e| e.to_string())?
            .unwrap_or_default();

        let edit_session_id = uuid::Uuid::new_v4().to_string();
        let commit_req =
            sona_core::history::mutation_repository::HistoryCommitTranscriptEditRequest {
                history_id: req.history_id.clone(),
                edit_session_id,
                base_segments,
                edited_segments: req.segments,
            };

        let result = self
            .services
            .history
            .mutation_db(move |service| service.commit_transcript_edit(commit_req))
            .await
            .map_err(|e| e.to_string())?;

        let snapshot_id = match result {
            sona_core::history::mutation_repository::HistoryCommitTranscriptEditResult::Committed {
                snapshot,
                ..
            } => snapshot.id,
            sona_core::history::mutation_repository::HistoryCommitTranscriptEditResult::Unchanged => {
                format!("unchanged-{}", req.history_id)
            }
            sona_core::history::mutation_repository::HistoryCommitTranscriptEditResult::Conflict {
                ..
            } => {
                return Err("CONFLICT: Transcript was concurrently modified".to_string());
            }
        };

        let _ = self.services.emitter.emit(
            "transcript-updated",
            serde_json::json!({ "historyId": &req.history_id }),
        );

        Ok(EditTranscriptResult {
            success: true,
            snapshot_id,
        })
    }

    pub async fn delete_history(
        &self,
        history_id: String,
        permanent: bool,
    ) -> Result<bool, String> {
        let hid = history_id.clone();
        if permanent {
            self.services
                .history
                .mutation_file(move |service| {
                    service.purge_items(
                        sona_core::history::mutation_repository::HistoryDeleteItemsRequest {
                            ids: vec![hid],
                        },
                    )
                })
                .await
                .map_err(|e| e.to_string())?;
        } else {
            let deleted_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_millis() as u64;
            self.services
                .history
                .mutation_file(move |service| {
                    service.trash_items(
                        sona_core::history::mutation_repository::HistoryTrashItemsRequest {
                            ids: vec![hid],
                            deleted_at,
                        },
                    )
                })
                .await
                .map_err(|e| e.to_string())?;
        }

        let _ = self.services.emitter.emit(
            "history-item-deleted",
            serde_json::json!({ "historyId": history_id, "permanent": permanent }),
        );

        Ok(true)
    }

    pub async fn list_projects(&self) -> Result<Vec<ProjectRecord>, String> {
        self.services.projects.list().await
    }

    pub async fn set_active_project(&self, project_id: Option<String>) -> Result<bool, String> {
        self.services.projects.set_active_tag_id(project_id).await?;
        Ok(true)
    }

    pub fn get_settings(&self, key: Option<String>) -> Result<serde_json::Value, String> {
        if let Some(k) = key {
            self.services
                .config
                .get_setting(&k)
                .map(|v| v.unwrap_or(serde_json::Value::Null))
        } else {
            self.services
                .config
                .load()
                .map(|v| v.unwrap_or(serde_json::Value::Null))
        }
    }

    pub fn update_setting(&self, key: String, value: serde_json::Value) -> Result<bool, String> {
        self.services.config.set_setting(&key, &value)?;
        Ok(true)
    }

    pub async fn transcribe_file(
        &self,
        req: TranscribeFileRequest,
    ) -> Result<TranscribeFileResult, String> {
        let path = std::path::PathBuf::from(&req.file_path);
        if !path.exists() {
            return Err(format!("File not found: {}", req.file_path));
        }

        let cfg_val = self
            .services
            .config
            .load()
            .map_err(|e| format!("Failed to load config: {e}"))?
            .unwrap_or_else(|| serde_json::json!({}));
        let asr_req = resolve_batch_asr_request_from_config(&cfg_val, req.language.as_deref())?;

        let save_to = req.save_to_path.as_ref().map(std::path::PathBuf::from);
        let instance_id = req
            .instance_id
            .unwrap_or_else(|| format!("mcp-transcribe-{}", uuid::Uuid::new_v4()));
        let segments = crate::integrations::asr::process_batch_file(
            self.services.emitter.clone(),
            &self.services.asr,
            path,
            save_to,
            asr_req,
            None,
            Some(instance_id),
        )
        .await
        .map_err(|e| e.to_string())?;

        let duration_seconds = segments.iter().map(|s| s.end).fold(0.0f64, f64::max);
        let segment_count = segments.len();
        let text = segments
            .iter()
            .map(|s| s.text.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        let save_req = sona_core::history::HistorySaveImportedFileRequest {
            id: None,
            source_path: req.file_path.clone(),
            segments,
            duration: duration_seconds,
            tag_ids: Vec::new(),
            project_id: req.project_id,
            converted_source_path: None,
        };
        let history_item = self
            .services
            .history
            .mutation_file(move |s| s.save_imported_file(save_req))
            .await
            .map_err(|e| e.to_string())?;
        let history_id = history_item.id;

        let _ = self.services.emitter.emit(
            "transcript-updated",
            serde_json::json!({ "historyId": &history_id }),
        );

        Ok(TranscribeFileResult {
            history_id,
            duration_seconds,
            segment_count,
            text,
        })
    }

    pub async fn cancel_batch_task(&self, instance_id: String) -> Result<bool, String> {
        let cancelled = self.services.asr.batch_cancel.cancel(&instance_id).await;
        Ok(cancelled)
    }

    pub async fn export_transcript(
        &self,
        req: ExportTranscriptRequest,
    ) -> Result<ExportTranscriptResult, String> {
        let format = match req.format.trim().to_ascii_lowercase().as_str() {
            "markdown" | "md" => ExportFormat::Md,
            "json" => ExportFormat::Json,
            "txt" => ExportFormat::Txt,
            "srt" => ExportFormat::Srt,
            "vtt" => ExportFormat::Vtt,
            other => return Err(format!("Unsupported export format: {other}")),
        };

        let mode = match req
            .mode
            .as_deref()
            .unwrap_or("original")
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "translation" => ExportMode::Translation,
            "bilingual" => ExportMode::Bilingual,
            "original" | "clean" | "" => ExportMode::Original,
            other => return Err(format!("Unsupported export mode: {other}")),
        };

        let hid = req.history_id.clone();
        let segments = self
            .services
            .history
            .query_db(move |repo| repo.load_transcript(&hid))
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Transcript not found for history ID: {}", req.history_id))?;

        let segment_count = segments.len();
        let export_req = sona_core::export::ExportTranscriptFileRequest {
            segments,
            format,
            mode,
            output_path: req.output_path.clone(),
        };

        tokio::task::spawn_blocking(move || {
            sona_export::export_transcript_file(export_req).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())??;

        Ok(ExportTranscriptResult {
            success: true,
            output_path: req.output_path,
            format: req.format,
            segment_count,
        })
    }

    pub async fn save_summary(&self, req: SaveSummaryRequest) -> Result<bool, String> {
        let hid = req.history_id.clone();
        let segments_opt = self
            .services
            .history
            .query_db(move |repo| repo.load_transcript(&hid))
            .await
            .map_err(|e| e.to_string())?;

        let source_fingerprint = match &segments_opt {
            Some(segs) => sona_core::llm::jobs::compute_summary_source_fingerprint(segs),
            None => String::new(),
        };
        let template_id = req.template_id.unwrap_or_else(|| "default".to_string());
        let generated_at = chrono::Utc::now().to_rfc3339();

        let payload = HistorySummaryPayload {
            active_template_id: template_id.clone(),
            record: Some(TranscriptSummaryRecordPayload {
                template_id,
                content: req.content,
                thought: req.thought,
                generated_at,
                source_fingerprint,
            }),
        };

        let target_hid = req.history_id.clone();
        self.services
            .history
            .db_task(move |repo| repo.save_summary(&target_hid, payload))
            .await
            .map_err(|e| e.to_string())?;

        let _ = self.services.emitter.emit(
            "summary-updated",
            serde_json::json!({ "historyId": &req.history_id }),
        );

        Ok(true)
    }

    pub async fn load_summary(
        &self,
        history_id: String,
    ) -> Result<Option<HistorySummaryPayload>, String> {
        self.services
            .history
            .db_task(move |repo| repo.load_summary(&history_id))
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn get_model_catalog(&self) -> Result<serde_json::Value, String> {
        if let Some(app) = &self.app_handle {
            let snapshot =
                crate::platform::models::preset::get_model_catalog_snapshot_for_app(app).await?;
            serde_json::to_value(&snapshot).map_err(|e| e.to_string())
        } else {
            let models_dir = self
                .services
                .sqlite
                .current_context()
                .map_err(|e| e.to_string())?
                .app_data_dir()
                .join("models");
            let snapshot = tokio::task::spawn_blocking(move || {
                let _ = sona_runtime_fs::ensure_directory_exists(&models_dir);
                sona_runtime_fs::build_model_catalog_snapshot(&models_dir)
            })
            .await
            .map_err(|e| e.to_string())?;
            serde_json::to_value(&snapshot).map_err(|e| e.to_string())
        }
    }

    pub async fn download_preset_model(
        &self,
        req: DownloadPresetModelRequest,
    ) -> Result<String, String> {
        let app = self.app_handle.as_ref().ok_or_else(|| {
            "APP_HANDLE_UNAVAILABLE: Desktop UI handle is required to initiate downloads"
                .to_string()
        })?;
        let download_id = req
            .download_id
            .unwrap_or_else(|| format!("mcp-{}", uuid::Uuid::new_v4()));
        let app_clone = app.clone();
        let downloads_service = self.services.downloads.clone();
        let dl_id_clone = download_id.clone();
        let model_id = req.model_id;
        let mirror = req.mirror;

        tauri::async_runtime::spawn(async move {
            if let Err(e) = crate::platform::model_downloads::download_preset_model(
                app_clone,
                &downloads_service,
                model_id,
                dl_id_clone,
                mirror,
            )
            .await
            {
                log::error!("[AgentControl] Background model download failed: {e}");
            }
        });

        Ok(download_id)
    }

    pub async fn cancel_download(&self, download_id: String) -> Result<bool, String> {
        self.services.downloads.notify_download(&download_id).await;
        Ok(true)
    }

    pub async fn get_sync_status(&self) -> Result<serde_json::Value, String> {
        let status = self.services.sync.get_status().await?;
        serde_json::to_value(&status).map_err(|e| e.to_string())
    }

    pub async fn trigger_sync(&self) -> Result<serde_json::Value, String> {
        let result = self.services.sync.run_now().await?;
        serde_json::to_value(&result).map_err(|e| e.to_string())
    }

    pub async fn create_project(&self, req: CreateProjectRequest) -> Result<ProjectRecord, String> {
        let input = ProjectCreateInput {
            name: req.name,
            description: req.description,
            icon: req.icon,
            color: req.color,
            pipeline: None,
        };
        self.services.projects.create(input).await
    }

    pub async fn update_project(
        &self,
        project_id: String,
        req: UpdateProjectRequest,
    ) -> Result<Option<ProjectRecord>, String> {
        let input = ProjectUpdateInput {
            name: req.name,
            description: req.description,
            icon: req.icon,
            color: req.color,
            pipeline: None,
        };
        self.services.projects.update(project_id, input).await
    }

    pub async fn delete_project(
        &self,
        project_id: String,
        cascade_action: Option<String>,
    ) -> Result<bool, String> {
        let action = match cascade_action.as_deref() {
            Some("trash") | Some("deleteItems") => "deleteItems",
            _ => "moveToInbox",
        };
        self.services
            .projects
            .delete_with_cascade(project_id, action.to_string())
            .await?;
        Ok(true)
    }

    pub async fn query_trash(
        &self,
        req: QueryTrashRequest,
    ) -> Result<Vec<HistoryItemSummary>, String> {
        let query_req = sona_core::history::HistoryWorkspaceQueryRequest {
            scope: sona_core::history::HistoryWorkspaceScope::Trash,
            query: req.query,
            filter_type: sona_core::history::HistoryWorkspaceFilterType::All,
            date_filter: sona_core::history::HistoryWorkspaceDateFilter::All,
            sort_order: sona_core::history::HistoryWorkspaceSortOrder::Newest,
            limit: req.limit.unwrap_or(20),
            offset: req.offset.unwrap_or(0),
        };

        let result: HistoryWorkspaceQueryResult = self
            .services
            .history
            .query_db(move |service| service.query_workspace(query_req))
            .await
            .map_err(|e| e.to_string())?;

        let summaries = result
            .filtered_items
            .into_iter()
            .map(|item| HistoryItemSummary {
                id: item.id,
                title: item.title,
                preview_text: item.preview_text,
                timestamp: item.timestamp,
                duration: item.duration,
                project_id: item.project_id,
            })
            .collect();

        Ok(summaries)
    }

    pub async fn restore_history(&self, history_id: String) -> Result<bool, String> {
        let hid = history_id.clone();
        let req =
            sona_core::history::mutation_repository::HistoryDeleteItemsRequest { ids: vec![hid] };
        self.services
            .history
            .mutation_file(move |service| service.restore_items(req))
            .await
            .map_err(|e| e.to_string())?;

        let _ = self.services.emitter.emit(
            "history-item-restored",
            serde_json::json!({ "historyId": &history_id }),
        );

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::database::DesktopSqliteState;
    use crate::platform::event::MockEventEmitter;

    async fn create_test_facade() -> (AgentControlFacade, tempfile::TempDir) {
        let temp = tempfile::tempdir().unwrap();
        let ctx = Arc::new(sona_sqlite::SqliteApplicationContext::open(temp.path()).unwrap());
        let sqlite = DesktopSqliteState::new(ctx);
        let emitter = Arc::new(MockEventEmitter::new());

        let services = DesktopServices::builder()
            .sqlite(sqlite)
            .event_emitter(emitter)
            .sync_config_path(temp.path().join("sync.json"))
            .build()
            .unwrap();

        let facade = AgentControlFacade::new(services, None);
        (facade, temp)
    }

    #[tokio::test]
    async fn test_client_state_and_focus() {
        let (facade, _dir) = create_test_facade().await;

        let state = facade.get_client_state().await.unwrap();
        assert!(state.online);
        assert!(!state.is_recording);
        assert_eq!(state.active_project_id, None);

        let focus = facade.focus_window().unwrap();
        assert!(!focus);
    }

    #[tokio::test]
    async fn test_casing_deserialization() {
        // Test snake_case vs camelCase wire format deserialization
        let snake_start: StartRecordingRequest =
            serde_json::from_str(r#"{"project_id":"p-1","device_name":"mic-1"}"#).unwrap();
        assert_eq!(snake_start.project_id.as_deref(), Some("p-1"));
        assert_eq!(snake_start.device_name.as_deref(), Some("mic-1"));

        let camel_start: StartRecordingRequest =
            serde_json::from_str(r#"{"projectId":"p-2","deviceName":"mic-2"}"#).unwrap();
        assert_eq!(camel_start.project_id.as_deref(), Some("p-2"));
        assert_eq!(camel_start.device_name.as_deref(), Some("mic-2"));

        let snake_query: QueryHistoryRequest =
            serde_json::from_str(r#"{"query":"hi","project_id":"p-3"}"#).unwrap();
        assert_eq!(snake_query.project_id.as_deref(), Some("p-3"));

        let snake_edit: EditTranscriptRequest =
            serde_json::from_str(r#"{"history_id":"h-1","segments":[]}"#).unwrap();
        assert_eq!(snake_edit.history_id, "h-1");

        let camel_edit: EditTranscriptRequest =
            serde_json::from_str(r#"{"historyId":"h-2","segments":[]}"#).unwrap();
        assert_eq!(camel_edit.history_id, "h-2");
    }

    #[tokio::test]
    async fn test_resolve_live_asr_config() {
        let local_cfg = serde_json::json!({
            "streamingModelPath": "C:/models/streaming_encoder.onnx",
            "language": "en"
        });
        let req = resolve_live_asr_request_from_config(&local_cfg).unwrap();
        assert_eq!(req.language, "en");
        assert_eq!(req.mode, AsrMode::Streaming);

        let online_cfg = serde_json::json!({
            "asr": {
                "live": {
                    "engine": "online",
                    "providerId": "volcengine-doubao",
                    "apiKey": "test-key"
                }
            }
        });
        let req_online = resolve_live_asr_request_from_config(&online_cfg).unwrap();
        assert_eq!(req_online.mode, AsrMode::Streaming);

        let empty_cfg = serde_json::json!({});
        assert!(resolve_live_asr_request_from_config(&empty_cfg).is_err());
    }

    #[tokio::test]
    async fn test_projects_and_settings() {
        let (facade, _dir) = create_test_facade().await;

        let projects = facade.list_projects().await.unwrap();
        assert!(projects.is_empty());

        facade
            .set_active_project(Some("proj-1".to_string()))
            .await
            .unwrap();
        let state = facade.get_client_state().await.unwrap();
        assert_eq!(state.active_project_id, Some("proj-1".to_string()));

        let initial_cfg = facade.get_settings(None).unwrap();
        assert_eq!(initial_cfg, serde_json::Value::Null);

        facade
            .update_setting("theme".to_string(), serde_json::json!("dark"))
            .unwrap();
        let updated_cfg = facade.get_settings(Some("theme".to_string())).unwrap();
        assert_eq!(updated_cfg, serde_json::json!("dark"));
    }

    #[tokio::test]
    async fn test_query_history_empty() {
        let (facade, _dir) = create_test_facade().await;

        let items = facade
            .query_history(QueryHistoryRequest {
                query: String::new(),
                project_id: None,
                limit: Some(10),
                offset: Some(0),
            })
            .await
            .unwrap();
        assert!(items.is_empty());
    }

    #[tokio::test]
    async fn test_resolve_batch_asr_config() {
        let batch_cfg = serde_json::json!({
            "asr": {
                "selections": {
                    "batch": {
                        "engine": "local",
                        "modelPath": "C:/models/sense-voice.onnx",
                        "language": "zh"
                    }
                }
            }
        });
        let req = resolve_batch_asr_request_from_config(&batch_cfg, None).unwrap();
        assert_eq!(req.language, "zh");
        assert_eq!(req.mode, AsrMode::Batch);

        // Language override
        let req_override = resolve_batch_asr_request_from_config(&batch_cfg, Some("ja")).unwrap();
        assert_eq!(req_override.language, "ja");

        // Fallback to live setting with batch mode
        let live_cfg = serde_json::json!({
            "streamingModelPath": "C:/models/streaming_encoder.onnx",
            "language": "en"
        });
        let req_fallback = resolve_batch_asr_request_from_config(&live_cfg, None).unwrap();
        assert_eq!(req_fallback.language, "en");
        assert_eq!(req_fallback.mode, AsrMode::Batch);
    }

    #[tokio::test]
    async fn test_project_crud_lifecycle() {
        let (facade, _dir) = create_test_facade().await;

        // Create project
        let created = facade
            .create_project(CreateProjectRequest {
                name: "Test Project".to_string(),
                description: Some("Description".to_string()),
                icon: Some("folder".to_string()),
                color: Some("#ff0000".to_string()),
            })
            .await
            .unwrap();
        assert_eq!(created.name, "Test Project");

        // Update project
        let updated = facade
            .update_project(
                created.id.clone(),
                UpdateProjectRequest {
                    name: Some("Updated Project".to_string()),
                    description: None,
                    icon: None,
                    color: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(updated.unwrap().name, "Updated Project");

        // Delete project
        let deleted = facade
            .delete_project(created.id, Some("moveToInbox".to_string()))
            .await
            .unwrap();
        assert!(deleted);
    }

    #[tokio::test]
    async fn test_summary_and_trash() {
        let (facade, _dir) = create_test_facade().await;

        // Save a base history item first
        let dummy_path = _dir.path().join("dummy.wav");
        std::fs::write(&dummy_path, b"dummy audio content").unwrap();
        let history_id = "test-hist-1".to_string();
        let hid_clone = history_id.clone();
        facade
            .services
            .history
            .mutation_file(move |s| {
                s.save_imported_file(sona_core::history::HistorySaveImportedFileRequest {
                    id: Some(hid_clone),
                    source_path: dummy_path.to_string_lossy().to_string(),
                    segments: vec![],
                    duration: 1.0,
                    tag_ids: vec![],
                    project_id: None,
                    converted_source_path: None,
                })
            })
            .await
            .unwrap();
        // Save and load summary
        let saved = facade
            .save_summary(SaveSummaryRequest {
                history_id: history_id.clone(),
                content: "This is a summary".to_string(),
                template_id: Some("meeting".to_string()),
                thought: Some("Thinking process".to_string()),
            })
            .await
            .unwrap();
        assert!(saved);

        let loaded = facade.load_summary(history_id.clone()).await.unwrap();
        assert!(loaded.is_some());
        let payload = loaded.unwrap();
        assert_eq!(payload.active_template_id, "meeting");
        let record = payload.record.unwrap();
        assert_eq!(record.content, "This is a summary");
        assert_eq!(record.thought.as_deref(), Some("Thinking process"));

        // Query trash initially empty
        let trash_items = facade
            .query_trash(QueryTrashRequest {
                query: String::new(),
                limit: Some(10),
                offset: Some(0),
            })
            .await
            .unwrap();
        assert!(trash_items.is_empty());
    }

    #[tokio::test]
    async fn test_model_catalog_and_sync_status() {
        let (facade, _dir) = create_test_facade().await;

        let catalog = facade.get_model_catalog().await.unwrap();
        assert!(catalog.is_object());

        let sync_status = facade.get_sync_status().await.unwrap();
        assert!(sync_status.is_object());
    }
}
