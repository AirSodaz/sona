use std::path::{Path as StdPath, PathBuf};
use std::sync::Arc;

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use sona_core::ports::asr::BatchTranscriberPort;
use sona_core::ports::runtime::{BatchTranscribePlanPort, RuntimeCapabilityError};
use sona_core::transcription::runtime::BatchTranscribeOptions;
use tokio::sync::mpsc;

use crate::jobs::{JobManager, JobStatus, TranscriptionJob};
use crate::platform::{ApiServerPlatform, ApiServerTranscriptionDefaults, OnlineBatchRequest};

type HmacSha256 = Hmac<Sha256>;

pub(crate) async fn send_webhook(job: &TranscriptionJob, status: &JobStatus) {
    let Some(webhook_url) = &job.webhook_url else {
        return;
    };
    if webhook_url.is_empty() {
        return;
    }

    let mut payload = serde_json::Map::new();
    payload.insert(
        "job_id".to_string(),
        serde_json::Value::String(job.job_id.clone()),
    );

    match status {
        JobStatus::Completed(segments) => {
            payload.insert(
                "status".to_string(),
                serde_json::Value::String("Completed".to_string()),
            );
            payload.insert(
                "segments".to_string(),
                serde_json::to_value(segments).unwrap_or_default(),
            );
        }
        JobStatus::Failed(error) => {
            payload.insert(
                "status".to_string(),
                serde_json::Value::String("Failed".to_string()),
            );
            payload.insert(
                "error".to_string(),
                serde_json::Value::String(error.clone()),
            );
        }
        _ => return,
    }

    let payload_str = serde_json::to_string(&payload).unwrap_or_default();

    static WEBHOOK_CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    let client = WEBHOOK_CLIENT.get_or_init(reqwest::Client::new);

    let mut request = client
        .post(webhook_url)
        .header("Content-Type", "application/json");

    if let Some(secret) = &job.webhook_secret
        && !secret.is_empty()
        && let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes())
    {
        mac.update(payload_str.as_bytes());
        let result = mac.finalize().into_bytes();
        let hex_signature = hex::encode(result);
        request = request.header("X-Sona-Signature", format!("sha256={}", hex_signature));
    }

    match request.body(payload_str).send().await {
        Ok(response) => {
            if !response.status().is_success() {
                log::warn!(
                    "[Server] webhook delivery failed: job_id={} url={} status={}",
                    job.job_id,
                    webhook_url,
                    response.status()
                );
            }
        }
        Err(error) => {
            log::warn!(
                "[Server] webhook delivery error: job_id={} url={} error={}",
                job.job_id,
                webhook_url,
                error
            );
        }
    }
}

/// Everything the transcription worker loop needs besides its job channel.
/// Bundled so the loop keeps one dependency parameter instead of seven.
pub(crate) struct TranscriptionWorkerDeps {
    pub job_manager: JobManager,
    pub models_dir: PathBuf,
    pub max_concurrent: usize,
    pub transcription_defaults: ApiServerTranscriptionDefaults,
    pub batch_transcriber: Arc<dyn BatchTranscriberPort>,
    pub batch_plan_resolver: Arc<dyn BatchTranscribePlanPort>,
    pub platform: Arc<dyn ApiServerPlatform>,
}

pub(crate) async fn start_worker_loop(
    mut receiver: mpsc::Receiver<TranscriptionJob>,
    deps: TranscriptionWorkerDeps,
) {
    let TranscriptionWorkerDeps {
        job_manager: shared_job_manager,
        models_dir: shared_models_dir,
        max_concurrent,
        transcription_defaults,
        batch_transcriber: shared_batch_transcriber,
        batch_plan_resolver: shared_batch_plan_resolver,
        platform: shared_platform,
    } = deps;
    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));

    while let Some(job) = receiver.recv().await {
        let job_manager = shared_job_manager.clone();
        let models_dir = shared_models_dir.clone();
        let semaphore = semaphore.clone();
        let defaults = transcription_defaults.clone();
        let batch_transcriber = shared_batch_transcriber.clone();
        let batch_plan_resolver = shared_batch_plan_resolver.clone();
        let platform = shared_platform.clone();

        tokio::spawn(async move {
            let _permit = match semaphore.acquire().await {
                Ok(permit) => permit,
                Err(_) => {
                    log::error!("[Server] semaphore closed, job {} abandoned", job.job_id);
                    job_manager
                        .update_job(
                            &job.job_id,
                            JobStatus::Failed("Internal: worker pool closed".to_string()),
                        )
                        .await;
                    return;
                }
            };
            job_manager
                .update_job(&job.job_id, JobStatus::Processing)
                .await;

            let final_status = if job.engine == "Online" {
                if let Some(provider_id) = job.online_provider_id.clone() {
                    let request = OnlineBatchRequest {
                        file_path: job.file_path.clone(),
                        provider_id,
                        profile_id: job.model_id.clone(),
                        config: job.online_provider_config.clone().unwrap_or_default(),
                        language: if job.language == "auto" {
                            "".to_string()
                        } else {
                            job.language.clone()
                        },
                        hotwords: job.hotwords.clone(),
                    };
                    match platform.transcribe_online_batch(request).await {
                        Ok(segments) => JobStatus::Completed(segments),
                        Err(error) => JobStatus::Failed(error.to_string()),
                    }
                } else {
                    JobStatus::Failed("Missing online provider ID".to_string())
                }
            } else {
                let options = build_local_transcribe_options(&job, &models_dir, &defaults);
                match batch_plan_resolver.resolve_batch_transcribe_plan(options) {
                    Ok(plan) => match batch_transcriber.transcribe(plan).await {
                        Ok(segments) => JobStatus::Completed(segments),
                        Err(e) => JobStatus::Failed(e.to_string()),
                    },
                    Err(RuntimeCapabilityError::BatchPlan { reason }) => JobStatus::Failed(reason),
                    Err(error) => JobStatus::Failed(error.to_string()),
                }
            };

            job_manager
                .update_job(&job.job_id, final_status.clone())
                .await;

            if job.webhook_url.is_some() {
                let job_clone = job.clone();
                let status_clone = final_status.clone();
                tokio::spawn(async move {
                    send_webhook(&job_clone, &status_clone).await;
                });
            }

            let _ = tokio::fs::remove_file(&job.file_path).await;
        });
    }
}

pub fn build_local_transcribe_options(
    job: &TranscriptionJob,
    models_dir: &StdPath,
    defaults: &ApiServerTranscriptionDefaults,
) -> BatchTranscribeOptions {
    let (vad_model_id, punctuation_model_id) =
        companion_defaults_for_model(&job.model_id, defaults);
    BatchTranscribeOptions {
        input: job.file_path.clone(),
        output: None,
        format: None,
        language: if job.language == "auto" {
            None
        } else {
            Some(job.language.clone())
        },
        model_id: Some(job.model_id.clone()),
        models_dir: Some(models_dir.to_path_buf()),
        default_models_dir: None,
        vad_model_id,
        punctuation_model_id,
        threads: None,
        enable_itn: None,
        hotwords: job.hotwords.clone(),
        gpu_acceleration: defaults.gpu_acceleration.clone(),
        vad_buffer: None,
        save_wav: None,
        quiet: true,
        force: true,
    }
}

fn companion_defaults_for_model(
    model_id: &str,
    defaults: &ApiServerTranscriptionDefaults,
) -> (Option<String>, Option<String>) {
    let rules = sona_core::models::preset_models::find_preset_model(model_id)
        .map(|model| model.resolved_rules());

    let vad_model_id = match defaults.vad_model_id.as_deref() {
        Some(id)
            if rules.map(|rules| rules.requires_vad).unwrap_or(true)
                || id != sona_core::models::preset_models::DEFAULT_SILERO_VAD_MODEL_ID =>
        {
            Some(id.to_string())
        }
        _ => None,
    };

    let punctuation_model_id = match defaults.punctuation_model_id.as_deref() {
        Some(id)
            if rules
                .map(|rules| rules.requires_punctuation)
                .unwrap_or(true)
                || id != sona_core::models::preset_models::DEFAULT_PUNCTUATION_MODEL_ID =>
        {
            Some(id.to_string())
        }
        _ => None,
    };

    (vad_model_id, punctuation_model_id)
}
