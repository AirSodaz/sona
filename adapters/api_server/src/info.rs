use std::collections::HashMap;
use std::path::Path as StdPath;
use std::sync::Arc;

use sona_core::ports::asr::online_asr_providers;
use sona_core::ports::runtime::{GpuAvailabilityPort, ModelCatalogPort};

use crate::ApiServerPlatformError;
use crate::jobs::JobStatus;
use crate::state::ServerState;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: String,
    pub uptime: u64,
    pub active_jobs: usize,
    pub pending_jobs: usize,
    pub cache_space_bytes: u64,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineAsrProviderInfo {
    pub id: String,
    pub languages: Vec<String>,
    pub language_mode: sona_core::models::preset_models::LanguageMode,
    pub configured: bool,
    pub supports_batch: bool,
    pub supports_streaming: bool,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoResponse {
    pub platform: String,
    pub gpu_available: bool,
    pub models: Vec<String>,
    pub vad_installed: bool,
    pub punctuation_installed: bool,
    pub online_asr_providers: Vec<OnlineAsrProviderInfo>,
}

pub async fn build_info_response(
    gpu_availability: Arc<dyn GpuAvailabilityPort>,
    model_catalog: Arc<dyn ModelCatalogPort>,
    models_dir: &StdPath,
    online_asr_config: &HashMap<String, serde_json::Value>,
) -> Result<InfoResponse, ApiServerPlatformError> {
    let gpu_available = gpu_availability.is_gpu_available().await;
    let models_dir = models_dir.to_path_buf();
    let snapshot = tokio::task::spawn_blocking(move || {
        model_catalog.build_model_catalog_snapshot(&models_dir)
    })
    .await
    .map_err(|error| {
        ApiServerPlatformError::information(format!("Failed to build model snapshot: {error}"))
    })?
    .map_err(|error| ApiServerPlatformError::information(error.to_string()))?;

    let installed_models = snapshot
        .models
        .iter()
        .filter(|m| m.is_installed)
        .map(|m| m.id.clone())
        .collect::<Vec<_>>();
    let vad_installed = snapshot.models.iter().any(|m| {
        m.id == sona_core::models::preset_models::DEFAULT_SILERO_VAD_MODEL_ID && m.is_installed
    });
    let punctuation_installed = snapshot.models.iter().any(|m| {
        m.id == sona_core::models::preset_models::DEFAULT_PUNCTUATION_MODEL_ID && m.is_installed
    });

    let online_asr_providers = online_asr_providers()
        .iter()
        .map(|provider| {
            let configured = online_asr_config
                .get(&provider.id)
                .and_then(|config| config.get("apiKey"))
                .and_then(serde_json::Value::as_str)
                .is_some_and(|api_key| !api_key.is_empty());
            OnlineAsrProviderInfo {
                id: provider.id.clone(),
                languages: provider.languages.clone(),
                language_mode: provider.language_mode,
                configured,
                supports_batch: provider.batch.local_file_mode.supported,
                supports_streaming: provider.streaming.supported.unwrap_or(false),
            }
        })
        .collect();

    Ok(InfoResponse {
        platform: std::env::consts::OS.to_string(),
        gpu_available,
        models: installed_models,
        vad_installed,
        punctuation_installed,
        online_asr_providers,
    })
}

pub(crate) async fn build_health_response(state: &ServerState) -> HealthResponse {
    let uptime = state.start_time.elapsed().as_secs();

    let cache_space_bytes = tokio::task::spawn_blocking({
        let temp_dir = state.temp_dir.clone();
        move || {
            walkdir::WalkDir::new(&temp_dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter_map(|e| e.metadata().ok())
                .filter(|m| m.is_file())
                .map(|m| m.len())
                .sum()
        }
    })
    .await
    .unwrap_or(0);

    let jobs = state.job_manager.list_jobs().await;
    let mut active_jobs = 0;
    let mut pending_jobs = 0;
    for status in jobs.values() {
        match status {
            JobStatus::Pending => pending_jobs += 1,
            JobStatus::Processing => active_jobs += 1,
            _ => {}
        }
    }

    HealthResponse {
        status: "ok".to_string(),
        uptime,
        active_jobs,
        pending_jobs,
        cache_space_bytes,
    }
}
