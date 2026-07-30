use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use ipnet::IpNet;
use sona_core::ports::runtime::{
    BatchTranscribePlanPort, GpuAvailabilityPort, MediaValidatorPort, ModelCatalogPort,
};
use tokio::sync::RwLock;

use crate::jobs::JobManager;
use crate::platform::{ApiServerPlatform, ApiServerTranscriptionDefaults};

#[derive(Clone)]
pub struct ServerState {
    pub job_manager: JobManager,
    pub temp_dir: PathBuf,
    pub models_dir: PathBuf,
    pub start_time: std::time::Instant,
    pub api_key: String,
    pub streaming_semaphore: Arc<tokio::sync::Semaphore>,
    pub ip_whitelist: Arc<Vec<IpNet>>,
    pub online_asr_config: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    pub transcription_defaults: ApiServerTranscriptionDefaults,
    pub media_validator: Arc<dyn MediaValidatorPort>,
    pub gpu_availability: Arc<dyn GpuAvailabilityPort>,
    pub model_catalog: Arc<dyn ModelCatalogPort>,
    pub batch_plan_resolver: Arc<dyn BatchTranscribePlanPort>,
    pub platform: Arc<dyn ApiServerPlatform>,
}
