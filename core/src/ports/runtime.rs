use std::path::Path;

use async_trait::async_trait;

use crate::models::preset_models::ModelCatalogSnapshot;
use crate::transcription::runtime::{BatchTranscribeOptions, BatchTranscribePlan};

#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum RuntimeCapabilityError {
    #[error("Model catalog discovery failed: {reason}")]
    ModelCatalog { reason: String },
    #[error("Batch transcription plan resolution failed: {reason}")]
    BatchPlan { reason: String },
}

#[async_trait]
pub trait MediaFileValidator: Send + Sync {
    async fn is_valid_media_file(&self, path: &Path) -> bool;
}

#[async_trait]
pub trait GpuAvailabilityProvider: Send + Sync {
    async fn is_gpu_available(&self) -> bool;
}

pub trait ModelCatalogProvider: Send + Sync {
    fn build_model_catalog_snapshot(
        &self,
        models_dir: &Path,
    ) -> Result<ModelCatalogSnapshot, RuntimeCapabilityError>;
}

pub trait BatchTranscribePlanResolver: Send + Sync {
    fn resolve_batch_transcribe_plan(
        &self,
        options: BatchTranscribeOptions,
    ) -> Result<BatchTranscribePlan, RuntimeCapabilityError>;
}
