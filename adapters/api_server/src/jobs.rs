use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use sona_core::transcription::transcript::TranscriptSegment;
use tokio::sync::{RwLock, mpsc};

use crate::ApiServerJobError;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
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
    pub created_at: std::time::Instant,
    pub completed_at: Option<std::time::Instant>,
    pub file_path: Option<PathBuf>,
    pub abort_handle: Option<tokio::task::AbortHandle>,
}

impl JobEntry {
    pub fn new(status: JobStatus, file_path: Option<PathBuf>) -> Self {
        Self {
            status,
            created_at: std::time::Instant::now(),
            completed_at: None,
            file_path,
            abort_handle: None,
        }
    }
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
        let file_path = job.file_path.clone();
        match self.sender.try_send(job) {
            Ok(()) => {
                self.jobs.write().await.insert(
                    job_id,
                    JobEntry {
                        status: JobStatus::Pending,
                        created_at: std::time::Instant::now(),
                        completed_at: None,
                        file_path: Some(file_path),
                        abort_handle: None,
                    },
                );
                Ok(())
            }
            Err(mpsc::error::TrySendError::Full(_)) => Err(ApiServerJobError::QueueFull { job_id }),
            Err(mpsc::error::TrySendError::Closed(_)) => {
                Err(ApiServerJobError::QueueClosed { job_id })
            }
        }
    }
    pub async fn set_abort_handle(&self, job_id: &str, abort_handle: tokio::task::AbortHandle) {
        if let Some(entry) = self.jobs.write().await.get_mut(job_id) {
            entry.abort_handle = Some(abort_handle);
        }
    }

    pub async fn update_job(&self, job_id: &str, status: JobStatus) {
        if let Some(job) = self.jobs.write().await.get_mut(job_id) {
            let is_finished = matches!(status, JobStatus::Completed(_) | JobStatus::Failed(_));
            job.status = status;
            if is_finished {
                job.completed_at = Some(std::time::Instant::now());
                job.abort_handle = None;
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
    pub async fn get_job_file_path(&self, job_id: &str) -> Option<PathBuf> {
        self.jobs
            .read()
            .await
            .get(job_id)
            .and_then(|entry| entry.file_path.clone())
    }

    pub async fn remove_job(&self, job_id: &str) -> Option<Option<PathBuf>> {
        let removed = self.jobs.write().await.remove(job_id);
        if let Some(entry) = &removed
            && let Some(abort) = &entry.abort_handle
        {
            abort.abort();
        }
        removed.map(|entry| entry.file_path)
    }
    pub async fn list_jobs(&self) -> HashMap<String, JobStatus> {
        self.jobs
            .read()
            .await
            .iter()
            .map(|(k, v)| (k.clone(), v.status.clone()))
            .collect()
    }
    pub async fn list_jobs_ordered(&self) -> Vec<(String, JobStatus)> {
        let jobs = self.jobs.read().await;
        let mut entries: Vec<_> = jobs
            .iter()
            .map(|(k, v)| (k.clone(), v.status.clone(), v.created_at))
            .collect();
        entries.sort_by(|a, b| a.2.cmp(&b.2).then_with(|| a.0.cmp(&b.0)));
        entries
            .into_iter()
            .map(|(id, status, _)| (id, status))
            .collect()
    }
    pub async fn active_job_count(&self) -> (usize, usize) {
        let jobs = self.jobs.read().await;
        let processing = jobs
            .values()
            .filter(|entry| matches!(entry.status, JobStatus::Processing))
            .count();
        let pending = jobs
            .values()
            .filter(|entry| matches!(entry.status, JobStatus::Pending))
            .count();
        (processing, pending)
    }

    pub async fn has_active_jobs(&self) -> bool {
        let (processing, pending) = self.active_job_count().await;
        processing > 0 || pending > 0
    }

    pub async fn abort_all_active(&self) -> usize {
        let mut count = 0;
        let mut jobs = self.jobs.write().await;
        for entry in jobs.values_mut() {
            if matches!(entry.status, JobStatus::Processing | JobStatus::Pending) {
                if let Some(abort) = entry.abort_handle.take() {
                    abort.abort();
                    count += 1;
                }
                entry.status = JobStatus::Failed("Server shutdown aborted the task".to_string());
                entry.completed_at = Some(std::time::Instant::now());
            }
        }
        count
    }

    pub async fn clean_expired_jobs(&self, ttl_duration: std::time::Duration) {
        let mut to_delete = Vec::new();
        self.jobs.write().await.retain(|_, entry| {
            if let Some(completed_at) = entry.completed_at {
                let expired = completed_at.elapsed() > ttl_duration;
                if expired {
                    if let Some(path) = &entry.file_path {
                        to_delete.push(path.clone());
                    }
                    false
                } else {
                    true
                }
            } else {
                true
            }
        });
        for path in to_delete {
            let _ = tokio::fs::remove_file(path).await;
        }
    }
}
