use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use sona_core::transcription::transcript::TranscriptSegment;
use tokio::sync::{RwLock, mpsc};

use crate::ApiServerJobError;

#[derive(Debug, Clone, serde::Serialize)]
pub enum JobStatus {
    Pending,
    Processing,
    Completed(Vec<TranscriptSegment>),
    Failed(String),
}

#[derive(Clone)]
pub struct TranscriptionJob {
    pub job_id: String,
    pub file_path: PathBuf,
    pub model_id: String,
    pub language: String,
    pub hotwords: Option<String>,
    pub webhook_url: Option<String>,
    pub webhook_secret: Option<String>,
    pub engine: String,
    pub online_provider_id: Option<String>,
    pub online_provider_config: Option<serde_json::Value>,
}

#[derive(Clone)]
pub struct JobEntry {
    pub status: JobStatus,
    pub completed_at: Option<std::time::Instant>,
}

#[derive(Clone)]
pub struct JobManager {
    /// Crate-visible so unit tests can seed finished/pending entries for TTL cleanup.
    pub(crate) jobs: Arc<RwLock<HashMap<String, JobEntry>>>,
    sender: mpsc::Sender<TranscriptionJob>,
}

impl JobManager {
    pub fn new(sender: mpsc::Sender<TranscriptionJob>) -> Self {
        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
            sender,
        }
    }

    pub async fn submit_job(&self, job: TranscriptionJob) -> Result<(), ApiServerJobError> {
        let job_id = job.job_id.clone();
        self.jobs.write().await.insert(
            job_id.clone(),
            JobEntry {
                status: JobStatus::Pending,
                completed_at: None,
            },
        );
        if self.sender.send(job).await.is_err() {
            self.jobs.write().await.remove(&job_id);
            return Err(ApiServerJobError::QueueClosed { job_id });
        }
        Ok(())
    }

    pub async fn update_job(&self, job_id: &str, status: JobStatus) {
        if let Some(job) = self.jobs.write().await.get_mut(job_id) {
            let is_finished = matches!(status, JobStatus::Completed(_) | JobStatus::Failed(_));
            job.status = status;
            if is_finished {
                job.completed_at = Some(std::time::Instant::now());
            }
        }
    }

    pub async fn get_job(&self, job_id: &str) -> Option<JobStatus> {
        self.jobs
            .read()
            .await
            .get(job_id)
            .map(|entry| entry.status.clone())
    }

    pub async fn list_jobs(&self) -> HashMap<String, JobStatus> {
        self.jobs
            .read()
            .await
            .iter()
            .map(|(k, v)| (k.clone(), v.status.clone()))
            .collect()
    }

    pub async fn clean_expired_jobs(&self, ttl_duration: std::time::Duration) {
        self.jobs.write().await.retain(|_, entry| {
            if let Some(completed_at) = entry.completed_at {
                completed_at.elapsed() <= ttl_duration
            } else {
                true
            }
        });
    }
}
