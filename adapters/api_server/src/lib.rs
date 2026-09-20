mod error;
mod handlers;
mod info;
mod ip_whitelist;
mod jobs;
mod platform;
mod runtime;
mod state;
mod streaming;
mod worker;

pub use error::*;
pub use handlers::{
    api_key_auth_middleware, extract_api_key_from_request, handle_health, handle_info,
    handle_job_status, handle_list_jobs, handle_transcribe,
};
pub use info::{
    ApiServerModelInfo, HealthResponse, InfoResponse, OnlineAsrProviderInfo, build_info_response,
};
pub use ip_whitelist::parse_ip_whitelist;
pub use jobs::{JobEntry, JobManager, JobStatus, TranscriptionJob};
pub use platform::{
    ApiServerPlatform, ApiServerTranscriptionDefaults, DefaultApiServerPlatform,
    LLM_POLISH_UNAVAILABLE, LLM_TRANSLATE_UNAVAILABLE, ONLINE_ASR_BATCH_UNAVAILABLE,
    OnlineBatchRequest, online_batch_request_to_core_request,
};
pub use runtime::{
    ApiServerDashboardHandle, ApiServerDashboardSnapshot, ApiServerRuntimeConfig,
    ApiServerRuntimeParts, ApiServerServiceParts, PreparedApiServerRuntime, RunningApiServer,
    format_bind_error, prepare_runtime_config, run_server, start_api_server_runtime,
};
pub use state::ServerState;
pub use streaming::{authorize_streaming_request, build_streaming_router};
pub use worker::build_local_transcribe_options;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handlers::{handle_delete_job, handle_export_job};
    use crate::runtime::startup_channel_closed_error;
    use crate::worker::{TranscriptionWorkerDeps, start_worker_loop};
    use async_trait::async_trait;
    use axum::{
        Router,
        body::Body,
        extract::ConnectInfo,
        http::{Request, StatusCode},
        routing::{get, post},
    };
    use sona_core::models::preset_models::{
        DEFAULT_SILERO_VAD_MODEL_ID, ModelCatalogSnapshot,
        build_model_catalog_snapshot_with_installed_ids,
    };
    use sona_core::ports::asr::BatchTranscriberPort;
    use sona_core::ports::runtime::{
        BatchTranscribePlanPort, GpuAvailabilityPort, MediaValidatorPort, ModelCatalogPort,
        RuntimeCapabilityError,
    };
    use sona_core::transcription::runtime::{
        BatchTranscribeOptions, BatchTranscribePlan, OutputTarget,
    };
    use sona_core::transcription::transcript::TranscriptSegment;
    use std::collections::HashMap;
    use std::fs;
    use std::net::SocketAddr;
    use std::path::{Path as StdPath, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::{RwLock, mpsc};
    use tower::ServiceExt;

    struct RejectingMediaValidator;

    #[async_trait]
    impl MediaValidatorPort for RejectingMediaValidator {
        async fn is_valid_media_file(&self, _path: &StdPath) -> bool {
            false
        }
    }
    struct AcceptingMediaValidator;

    #[async_trait]
    impl MediaValidatorPort for AcceptingMediaValidator {
        async fn is_valid_media_file(&self, _path: &StdPath) -> bool {
            true
        }
    }

    struct FixedGpuAvailability(bool);

    #[async_trait]
    impl GpuAvailabilityPort for FixedGpuAvailability {
        async fn is_gpu_available(&self) -> bool {
            self.0
        }
    }

    struct FixedModelCatalog {
        snapshot: ModelCatalogSnapshot,
    }

    impl ModelCatalogPort for FixedModelCatalog {
        fn build_model_catalog_snapshot(
            &self,
            _models_dir: &StdPath,
        ) -> Result<ModelCatalogSnapshot, RuntimeCapabilityError> {
            Ok(self.snapshot.clone())
        }
    }

    struct FailingModelCatalog;

    impl ModelCatalogPort for FailingModelCatalog {
        fn build_model_catalog_snapshot(
            &self,
            _models_dir: &StdPath,
        ) -> Result<ModelCatalogSnapshot, RuntimeCapabilityError> {
            Err(RuntimeCapabilityError::ModelCatalog {
                reason: "catalog unavailable".to_string(),
            })
        }
    }

    struct RecordingBatchPlanResolver {
        calls: Arc<AtomicUsize>,
        plan: BatchTranscribePlan,
    }

    impl BatchTranscribePlanPort for RecordingBatchPlanResolver {
        fn resolve_batch_transcribe_plan(
            &self,
            _options: BatchTranscribeOptions,
        ) -> Result<BatchTranscribePlan, RuntimeCapabilityError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.plan.clone())
        }
    }

    struct RecordingBatchTranscriber {
        resolver_calls: Arc<AtomicUsize>,
        calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl BatchTranscriberPort for RecordingBatchTranscriber {
        async fn transcribe(
            &self,
            _plan: BatchTranscribePlan,
        ) -> Result<Vec<TranscriptSegment>, sona_core::ports::asr::AsrPortError> {
            assert_eq!(self.resolver_calls.load(Ordering::SeqCst), 1);
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(vec![])
        }
    }

    struct NoopBatchTranscriber;

    #[async_trait]
    impl BatchTranscriberPort for NoopBatchTranscriber {
        async fn transcribe(
            &self,
            _plan: BatchTranscribePlan,
        ) -> Result<Vec<TranscriptSegment>, sona_core::ports::asr::AsrPortError> {
            Ok(vec![])
        }
    }

    fn test_batch_plan(input_path: PathBuf) -> BatchTranscribePlan {
        BatchTranscribePlan {
            input_path,
            save_to_path: None,
            engine: sona_core::ports::asr::LocalAsrEngine::SherpaOnnx,
            model_path: "C:/models/test".to_string(),
            num_threads: 4,
            enable_itn: false,
            language: "auto".to_string(),
            punctuation_model: None,
            alignment_model: None,
            vad_model: None,
            vad_buffer: 5.0,
            batch_segmentation_mode: sona_core::ports::asr::BatchSegmentationMode::Vad,
            model_type: "whisper".to_string(),
            file_config: None,
            hotwords: None,
            speaker_processing: None,
            gpu_acceleration: None,
            export_format: sona_core::export::ExportFormat::Json,
            output_target: OutputTarget::Stdout,
            quiet: true,
            ffmpeg_path: None,
        }
    }

    fn test_batch_transcriber() -> Arc<dyn BatchTranscriberPort> {
        Arc::new(NoopBatchTranscriber)
    }

    fn test_model_catalog() -> Arc<dyn ModelCatalogPort> {
        Arc::new(FixedModelCatalog {
            snapshot: build_model_catalog_snapshot_with_installed_ids(
                StdPath::new("models"),
                &std::collections::HashSet::new(),
            ),
        })
    }

    fn test_batch_plan_resolver() -> Arc<dyn BatchTranscribePlanPort> {
        Arc::new(RecordingBatchPlanResolver {
            calls: Arc::new(AtomicUsize::new(0)),
            plan: test_batch_plan(PathBuf::from("sample.wav")),
        })
    }

    #[tokio::test]
    async fn injected_runtime_capability_rejects_invalid_media_and_removes_upload() {
        let temp = tempfile::tempdir().unwrap();
        let (tx, _rx) = mpsc::channel(1);
        let state = ServerState {
            job_manager: JobManager::new(tx),
            temp_dir: temp.path().to_path_buf(),
            models_dir: PathBuf::from("models"),
            start_time: std::time::Instant::now(),
            api_key: String::new(),
            streaming_semaphore: Arc::new(tokio::sync::Semaphore::new(1)),
            ip_whitelist: Arc::new(vec![]),
            online_asr_config: Arc::new(RwLock::new(HashMap::new())),
            transcription_defaults: ApiServerTranscriptionDefaults::default(),
            media_validator: Arc::new(RejectingMediaValidator),
            gpu_availability: Arc::new(FixedGpuAvailability(false)),
            model_catalog: Arc::new(FixedModelCatalog {
                snapshot: build_model_catalog_snapshot_with_installed_ids(
                    StdPath::new("models"),
                    &std::collections::HashSet::new(),
                ),
            }),
            batch_plan_resolver: Arc::new(RecordingBatchPlanResolver {
                calls: Arc::new(AtomicUsize::new(0)),
                plan: test_batch_plan(PathBuf::from("sample.wav")),
            }),
            platform: Arc::new(DefaultApiServerPlatform),
        };
        let boundary = "sona-test-boundary";
        let body = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"sample.wav\"\r\nContent-Type: audio/wav\r\n\r\ninvalid\r\n--{boundary}--\r\n"
        );
        let response = Router::new()
            .route("/v1/transcriptions", post(handle_transcribe))
            .with_state(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/transcriptions")
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn injected_runtime_capability_builds_info_from_gpu_and_catalog_ports() {
        let mut installed = std::collections::HashSet::new();
        installed.insert("sherpa-onnx-whisper-turbo".to_string());
        installed.insert(DEFAULT_SILERO_VAD_MODEL_ID.to_string());
        let snapshot = build_model_catalog_snapshot_with_installed_ids(
            StdPath::new("injected-models"),
            &installed,
        );
        let mut configs = HashMap::new();
        configs.insert(
            "groq-whisper".to_string(),
            serde_json::json!({"apiKey": "configured"}),
        );

        let info = build_info_response(
            Arc::new(FixedGpuAvailability(true)),
            Arc::new(FixedModelCatalog { snapshot }),
            StdPath::new("ignored-models"),
            &configs,
        )
        .await
        .unwrap();

        assert_eq!(info.platform, std::env::consts::OS);
        assert!(info.gpu_available);
        assert_eq!(info.models.len(), 1);
        assert_eq!(info.models[0].id, "sherpa-onnx-whisper-turbo");
        assert_eq!(info.models[0].name, "Whisper (Large Turbo)");
        assert!(!info.models[0].languages.is_empty());
        assert!(
            !info
                .models
                .iter()
                .any(|m| m.id == DEFAULT_SILERO_VAD_MODEL_ID)
        );
        assert!(info.vad_installed);
        assert!(!info.punctuation_installed);
        assert!(
            info.online_asr_providers
                .iter()
                .any(|provider| provider.id == "groq-whisper" && provider.configured)
        );
    }

    #[tokio::test]
    async fn injected_runtime_capability_maps_catalog_failure() {
        let error = build_info_response(
            Arc::new(FixedGpuAvailability(false)),
            Arc::new(FailingModelCatalog),
            StdPath::new("models"),
            &HashMap::new(),
        )
        .await
        .unwrap_err();

        assert_eq!(
            error,
            ApiServerPlatformError::Information {
                reason: "Model catalog discovery failed: catalog unavailable".to_string(),
            }
        );
    }

    #[tokio::test]
    async fn injected_runtime_capability_resolves_local_plan_before_transcription() {
        let temp = tempfile::tempdir().unwrap();
        let input_path = temp.path().join("sample.wav");
        fs::write(&input_path, b"audio").unwrap();
        let resolver_calls = Arc::new(AtomicUsize::new(0));
        let transcriber_calls = Arc::new(AtomicUsize::new(0));
        let (tx, rx) = mpsc::channel(1);
        let job_manager = JobManager::new(tx);
        let (_shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let worker = tokio::spawn(start_worker_loop(
            rx,
            shutdown_rx,
            TranscriptionWorkerDeps {
                job_manager: job_manager.clone(),
                models_dir: PathBuf::from("models"),
                max_concurrent: 1,
                transcription_defaults: ApiServerTranscriptionDefaults::default(),
                batch_transcriber: Arc::new(RecordingBatchTranscriber {
                    resolver_calls: Arc::clone(&resolver_calls),
                    calls: Arc::clone(&transcriber_calls),
                }),
                batch_plan_resolver: Arc::new(RecordingBatchPlanResolver {
                    calls: Arc::clone(&resolver_calls),
                    plan: test_batch_plan(input_path.clone()),
                }),
                platform: Arc::new(DefaultApiServerPlatform),
            },
        ));
        let job = TranscriptionJob {
            job_id: "injected-plan-job".to_string(),
            file_path: input_path,
            model_id: "model".to_string(),
            language: "auto".to_string(),
            hotwords: None,
            webhook_url: None,
            webhook_secret: None,
            engine: "Local".to_string(),
            online_provider_id: None,
            online_provider_config: None,
        };
        job_manager.submit_job(job).await.unwrap();

        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if matches!(
                    job_manager.get_job("injected-plan-job").await,
                    Some(JobStatus::Completed(_))
                ) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        worker.abort();

        assert_eq!(resolver_calls.load(Ordering::SeqCst), 1);
        assert_eq!(transcriber_calls.load(Ordering::SeqCst), 1);
    }

    fn streaming_authorization_state(
        api_key: &str,
        max_streaming: usize,
        ip_whitelist: &str,
    ) -> ServerState {
        let (tx, _rx) = mpsc::channel(1);
        ServerState {
            job_manager: JobManager::new(tx),
            temp_dir: PathBuf::from("temp"),
            models_dir: PathBuf::from("models"),
            start_time: std::time::Instant::now(),
            api_key: api_key.to_string(),
            streaming_semaphore: Arc::new(tokio::sync::Semaphore::new(max_streaming)),
            ip_whitelist: Arc::new(parse_ip_whitelist(ip_whitelist).unwrap()),
            online_asr_config: Arc::new(RwLock::new(HashMap::new())),
            transcription_defaults: ApiServerTranscriptionDefaults::default(),
            media_validator: Arc::new(AcceptingMediaValidator),
            gpu_availability: Arc::new(FixedGpuAvailability(false)),
            model_catalog: test_model_catalog(),
            batch_plan_resolver: test_batch_plan_resolver(),
            platform: Arc::new(DefaultApiServerPlatform),
        }
    }

    fn test_dashboard_handle(model_catalog: Arc<dyn ModelCatalogPort>) -> ApiServerDashboardHandle {
        let (tx, _rx) = mpsc::channel(1);
        ApiServerDashboardHandle {
            state: ServerState {
                job_manager: JobManager::new(tx),
                temp_dir: PathBuf::from("temp"),
                models_dir: PathBuf::from("models"),
                start_time: std::time::Instant::now(),
                api_key: String::new(),
                streaming_semaphore: Arc::new(tokio::sync::Semaphore::new(1)),
                ip_whitelist: Arc::new(vec![]),
                online_asr_config: Arc::new(RwLock::new(HashMap::new())),
                transcription_defaults: ApiServerTranscriptionDefaults::default(),
                media_validator: Arc::new(AcceptingMediaValidator),
                gpu_availability: Arc::new(FixedGpuAvailability(false)),
                model_catalog,
                batch_plan_resolver: test_batch_plan_resolver(),
                platform: Arc::new(DefaultApiServerPlatform),
            },
        }
    }

    #[tokio::test]
    async fn clean_expired_jobs() {
        let (tx, _rx) = mpsc::channel(1);
        let job_manager = JobManager::new(tx);

        job_manager.jobs.write().await.insert(
            "expired-job".to_string(),
            JobEntry {
                status: JobStatus::Completed(vec![]),
                created_at: std::time::Instant::now(),
                completed_at: Some(std::time::Instant::now() - std::time::Duration::from_secs(120)),
                file_path: None,
                abort_handle: None,
            },
        );
        job_manager.jobs.write().await.insert(
            "fresh-job".to_string(),
            JobEntry {
                status: JobStatus::Completed(vec![]),
                created_at: std::time::Instant::now(),
                completed_at: Some(std::time::Instant::now()),
                file_path: None,
                abort_handle: None,
            },
        );
        job_manager.jobs.write().await.insert(
            "pending-job".to_string(),
            JobEntry {
                status: JobStatus::Pending,
                created_at: std::time::Instant::now(),
                completed_at: None,
                file_path: None,
                abort_handle: None,
            },
        );

        job_manager
            .clean_expired_jobs(std::time::Duration::from_secs(60))
            .await;

        let jobs = job_manager.list_jobs().await;
        assert!(!jobs.contains_key("expired-job"));
        assert!(jobs.contains_key("fresh-job"));
        assert!(jobs.contains_key("pending-job"));
    }

    #[tokio::test]
    async fn submit_job_reports_closed_queue_without_leaving_pending_state() {
        let (tx, rx) = mpsc::channel(1);
        drop(rx);
        let job_manager = JobManager::new(tx);
        let job = TranscriptionJob {
            job_id: "closed-queue-job".to_string(),
            file_path: PathBuf::from("sample.wav"),
            model_id: "model".to_string(),
            language: "auto".to_string(),
            hotwords: None,
            webhook_url: None,
            webhook_secret: None,
            engine: "Local".to_string(),
            online_provider_id: None,
            online_provider_config: None,
        };

        let error = job_manager.submit_job(job).await.unwrap_err();

        assert_eq!(
            error,
            ApiServerJobError::QueueClosed {
                job_id: "closed-queue-job".to_string(),
            }
        );
        assert!(job_manager.get_job("closed-queue-job").await.is_none());
    }

    #[tokio::test]
    async fn dashboard_snapshot_preserves_catalog_failure_category() {
        let dashboard = test_dashboard_handle(Arc::new(FailingModelCatalog));

        let error = match dashboard.snapshot().await {
            Ok(_) => panic!("failing info platform should reject dashboard snapshot"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            ApiServerDashboardError::Platform(ApiServerPlatformError::Information {
                reason: "Model catalog discovery failed: catalog unavailable".to_string(),
            })
        );
    }

    #[tokio::test]
    async fn stop_reports_closed_shutdown_channel() {
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        drop(shutdown_rx);
        let server = RunningApiServer {
            normalized_ip_whitelist: "127.0.0.0/8".to_string(),
            shutdown_tx: Some(shutdown_tx),
            join_handle: tokio::spawn(async { Ok(()) }),
            dashboard: test_dashboard_handle(test_model_catalog()),
        };

        let error = server.stop().await.unwrap_err();

        assert_eq!(error, ApiServerStopError::ShutdownSignalClosed);
    }

    #[tokio::test]
    async fn wait_reports_task_join_failure() {
        let server = RunningApiServer {
            normalized_ip_whitelist: "127.0.0.0/8".to_string(),
            shutdown_tx: None,
            join_handle: tokio::spawn(async {
                panic!("test task failure");
                #[allow(unreachable_code)]
                Ok(())
            }),
            dashboard: test_dashboard_handle(test_model_catalog()),
        };

        let error = server.wait().await.unwrap_err();

        assert!(matches!(error, ApiServerRuntimeError::TaskJoin { .. }));
    }

    #[tokio::test]
    async fn startup_channel_closure_has_a_typed_runtime_error() {
        let join_handle = tokio::spawn(async { Ok(()) });

        let error = startup_channel_closed_error(join_handle).await;

        assert_eq!(error, ApiServerRuntimeError::DashboardChannelClosed);
    }

    #[tokio::test]
    async fn health_endpoint_reports_stats() {
        let temp_dir = tempfile::tempdir().unwrap().path().to_path_buf();
        let models_dir = tempfile::tempdir().unwrap().path().to_path_buf();
        let (tx, _rx) = mpsc::channel(1);
        let state = ServerState {
            job_manager: JobManager::new(tx),
            temp_dir,
            models_dir,
            start_time: std::time::Instant::now(),
            api_key: String::new(),
            streaming_semaphore: Arc::new(tokio::sync::Semaphore::new(1)),
            ip_whitelist: Arc::new(vec![]),
            online_asr_config: Arc::new(RwLock::new(HashMap::new())),
            transcription_defaults: ApiServerTranscriptionDefaults::default(),
            media_validator: Arc::new(AcceptingMediaValidator),
            gpu_availability: Arc::new(FixedGpuAvailability(false)),
            model_catalog: test_model_catalog(),
            batch_plan_resolver: test_batch_plan_resolver(),
            platform: Arc::new(DefaultApiServerPlatform),
        };

        let app = Router::new()
            .route("/health", get(handle_health))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(body["status"], "ok");
        assert!(body["uptime"].is_number());
        assert!(body["activeJobs"].is_number());
        assert!(body["pendingJobs"].is_number());
        assert!(body["cacheSpaceBytes"].is_number());
    }

    #[tokio::test]
    async fn list_jobs_endpoint_reports_known_jobs() {
        let temp_dir = tempfile::tempdir().unwrap().path().to_path_buf();
        let models_dir = tempfile::tempdir().unwrap().path().to_path_buf();
        let (tx, _rx) = mpsc::channel(1);
        let job_manager = JobManager::new(tx);
        job_manager.jobs.write().await.insert(
            "test-job-id".to_string(),
            JobEntry {
                status: JobStatus::Pending,
                created_at: std::time::Instant::now(),
                completed_at: None,
                file_path: None,
                abort_handle: None,
            },
        );
        let state = ServerState {
            job_manager,
            temp_dir,
            models_dir,
            start_time: std::time::Instant::now(),
            api_key: String::new(),
            streaming_semaphore: Arc::new(tokio::sync::Semaphore::new(1)),
            ip_whitelist: Arc::new(vec![]),
            online_asr_config: Arc::new(RwLock::new(HashMap::new())),
            transcription_defaults: ApiServerTranscriptionDefaults::default(),
            media_validator: Arc::new(AcceptingMediaValidator),
            gpu_availability: Arc::new(FixedGpuAvailability(false)),
            model_catalog: test_model_catalog(),
            batch_plan_resolver: test_batch_plan_resolver(),
            platform: Arc::new(DefaultApiServerPlatform),
        };

        let app = Router::new()
            .route("/v1/transcriptions/jobs", get(handle_list_jobs))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/transcriptions/jobs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(body.is_object());
        assert_eq!(body["test-job-id"], "Pending");
    }

    #[tokio::test]
    async fn list_jobs_filters_by_status_and_pagination() {
        let (tx, _rx) = mpsc::channel(1);
        let job_manager = JobManager::new(tx);
        job_manager.jobs.write().await.insert(
            "job-1".to_string(),
            JobEntry {
                status: JobStatus::Pending,
                created_at: std::time::Instant::now(),
                completed_at: None,
                file_path: None,
                abort_handle: None,
            },
        );
        job_manager.jobs.write().await.insert(
            "job-2".to_string(),
            JobEntry {
                status: JobStatus::Processing,
                created_at: std::time::Instant::now(),
                completed_at: None,
                file_path: None,
                abort_handle: None,
            },
        );
        job_manager.jobs.write().await.insert(
            "job-3".to_string(),
            JobEntry {
                status: JobStatus::Completed(vec![]),
                created_at: std::time::Instant::now(),
                completed_at: Some(std::time::Instant::now()),
                file_path: None,
                abort_handle: None,
            },
        );

        let mut state = streaming_authorization_state("", 1, "127.0.0.1/32");
        state.job_manager = job_manager;

        let app = Router::new()
            .route("/v1/transcriptions/jobs", get(handle_list_jobs))
            .with_state(state.clone());

        // Filter by status=completed
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/transcriptions/jobs?status=completed")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body: HashMap<String, serde_json::Value> = serde_json::from_slice(
            &axum::body::to_bytes(res.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body.len(), 1);
        assert!(body.contains_key("job-3"));

        // Pagination with limit=1
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/v1/transcriptions/jobs?limit=1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body: HashMap<String, serde_json::Value> = serde_json::from_slice(
            &axum::body::to_bytes(res.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body.len(), 1);
    }

    #[tokio::test]
    async fn transcribe_rejects_invalid_webhook_url_and_cleans_up() {
        let temp = tempfile::tempdir().unwrap();
        let mut state = streaming_authorization_state("", 1, "127.0.0.1/32");
        state.temp_dir = temp.path().to_path_buf();
        state.media_validator = Arc::new(AcceptingMediaValidator);

        let boundary = "sona-test-boundary";
        let body = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"sample.wav\"\r\nContent-Type: audio/wav\r\n\r\naudio data\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"model_id\"\r\n\r\nsensevoice\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"webhook_url\"\r\n\r\nfile:///etc/passwd\r\n--{boundary}--\r\n"
        );

        let app = Router::new()
            .route("/v1/transcriptions", post(handle_transcribe))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/transcriptions")
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    }

    #[test]
    fn local_transcribe_request_uses_server_defaults() {
        let temp = tempfile::tempdir().unwrap();
        let models_dir = temp.path().to_path_buf();
        let input_path = temp.path().join("sample.wav");
        fs::write(&input_path, b"audio").unwrap();
        let job = TranscriptionJob {
            job_id: "job-1".to_string(),
            file_path: input_path.clone(),
            model_id: "sherpa-onnx-whisper-turbo".to_string(),
            language: "auto".to_string(),
            hotwords: Some("Sona".to_string()),
            webhook_url: None,
            webhook_secret: None,
            engine: "Local".to_string(),
            online_provider_id: None,
            online_provider_config: None,
        };
        let defaults = ApiServerTranscriptionDefaults {
            gpu_acceleration: Some("cuda".to_string()),
            vad_model_id: Some(
                sona_core::models::preset_models::DEFAULT_SILERO_VAD_MODEL_ID.to_string(),
            ),
            punctuation_model_id: None,
            ffmpeg_path: Some("/custom/ffmpeg".to_string()),
        };

        let options = build_local_transcribe_options(&job, &models_dir, &defaults);

        assert_eq!(options.gpu_acceleration.as_deref(), Some("cuda"));
        assert_eq!(
            options.vad_model_id.as_deref(),
            Some(sona_core::models::preset_models::DEFAULT_SILERO_VAD_MODEL_ID)
        );
        assert!(options.punctuation_model_id.is_none());
        assert_eq!(options.input, input_path);
        assert_eq!(options.hotwords.as_deref(), Some("Sona"));
        assert_eq!(options.ffmpeg_path.as_deref(), Some("/custom/ffmpeg"));
    }

    #[test]
    fn format_bind_error_describes_common_failures() {
        let addr = "127.0.0.1:14200";
        let in_use_err = std::io::Error::new(std::io::ErrorKind::AddrInUse, "address in use");
        let in_use = format_bind_error(in_use_err, addr);
        assert_eq!(in_use.address, addr);
        assert_eq!(in_use.kind, ApiServerBindErrorKind::AddressInUse);
        assert!(in_use.to_string().contains("Address already in use"));

        let not_avail_err =
            std::io::Error::new(std::io::ErrorKind::AddrNotAvailable, "not available");
        let not_available = format_bind_error(not_avail_err, addr);
        assert_eq!(
            not_available.kind,
            ApiServerBindErrorKind::AddressNotAvailable
        );
        assert!(not_available.to_string().contains("Address not available"));

        let permission_err =
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied");
        let permission = format_bind_error(permission_err, addr);
        assert_eq!(permission.kind, ApiServerBindErrorKind::PermissionDenied);
        assert!(permission.to_string().contains("Permission denied"));
    }

    #[test]
    fn authorize_streaming_request_rejects_non_whitelisted_clients() {
        let state = streaming_authorization_state("secret", 1, "127.0.0.0/8");

        let error =
            match authorize_streaming_request(&state, "10.0.0.1:14200".parse().unwrap(), None) {
                Ok(_) => panic!("non-whitelisted streaming client without key should be rejected"),
                Err(error) => error,
            };

        assert_eq!(error, StatusCode::FORBIDDEN);

        // Non-whitelisted with wrong key -> FORBIDDEN
        let err_wrong =
            authorize_streaming_request(&state, "10.0.0.1:14200".parse().unwrap(), Some("wrong"))
                .unwrap_err();
        assert_eq!(err_wrong, StatusCode::FORBIDDEN);

        // Non-whitelisted with valid key -> Forbidden (Defense in Depth: network perimeter enforced)
        assert_eq!(
            authorize_streaming_request(&state, "10.0.0.1:14200".parse().unwrap(), Some("secret"),)
                .unwrap_err(),
            StatusCode::FORBIDDEN
        );
    }

    #[test]
    fn authorize_streaming_request_rejects_invalid_tokens() {
        let state = streaming_authorization_state("secret", 1, "127.0.0.0/8");

        let error = match authorize_streaming_request(
            &state,
            "127.0.0.1:14200".parse().unwrap(),
            Some("wrong"),
        ) {
            Ok(_) => panic!("invalid streaming token should be rejected"),
            Err(error) => error,
        };

        assert_eq!(error, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn authorize_streaming_request_holds_streaming_capacity_permit() {
        let state = streaming_authorization_state("", 1, "127.0.0.0/8");

        let permit =
            authorize_streaming_request(&state, "127.0.0.1:14200".parse().unwrap(), None).unwrap();
        let error =
            match authorize_streaming_request(&state, "127.0.0.1:14200".parse().unwrap(), None) {
                Ok(_) => panic!("streaming capacity should be exhausted"),
                Err(error) => error,
            };
        drop(permit);
        let next = authorize_streaming_request(&state, "127.0.0.1:14200".parse().unwrap(), None);

        assert_eq!(error, StatusCode::SERVICE_UNAVAILABLE);
        assert!(next.is_ok());
    }

    #[test]
    fn prepare_runtime_config_maps_resolved_options_and_normalizes_whitelist() {
        let temp_dir = tempfile::tempdir().unwrap().path().join("api-temp");
        let models_dir = tempfile::tempdir().unwrap().path().join("models");
        let (_shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let (bind_tx, _bind_rx) = tokio::sync::oneshot::channel();

        let prepared = prepare_runtime_config(ApiServerRuntimeParts {
            resolved: sona_core::runtime::serve::ResolvedServeRuntimeOptions {
                host: "0.0.0.0".to_string(),
                port: 15555,
                api_key: "secret".to_string(),
                models_dir: models_dir.clone(),
                ip_whitelist: "localhost,10.0.0.0/8".to_string(),
                max_streaming: 5,
                max_concurrent: 3,
                max_queue_size: 44,
                max_upload_size_mb: 256,
                job_ttl_minutes: 9,
                transcription_defaults: sona_core::runtime::serve::ServeTranscriptionDefaults {
                    gpu_acceleration: Some("cuda".to_string()),
                    vad_model_id: Some("vad-model".to_string()),
                    punctuation_model_id: Some("punct-model".to_string()),
                    ffmpeg_path: None,
                },
            },
            temp_dir: temp_dir.clone(),
            online_asr_config: Arc::new(RwLock::new(HashMap::new())),
            batch_transcriber: test_batch_transcriber(),
            media_validator: Arc::new(AcceptingMediaValidator),
            gpu_availability: Arc::new(FixedGpuAvailability(false)),
            model_catalog: test_model_catalog(),
            batch_plan_resolver: test_batch_plan_resolver(),
            platform: Arc::new(DefaultApiServerPlatform),
            streaming_router: None,
            web_dist_dir: None,
            shutdown_rx,
            bind_tx: Some(bind_tx),
        })
        .unwrap();

        assert_eq!(
            prepared.normalized_ip_whitelist,
            "127.0.0.0/8,::1/128,10.0.0.0/8"
        );
        assert_eq!(prepared.config.host, "0.0.0.0");
        assert_eq!(prepared.config.port, 15555);
        assert_eq!(prepared.config.api_key, "secret");
        assert_eq!(prepared.config.temp_dir, temp_dir);
        assert_eq!(prepared.config.models_dir, models_dir);
        assert_eq!(prepared.config.max_concurrent, 3);
        assert_eq!(prepared.config.max_queue_size, 44);
        assert_eq!(prepared.config.max_upload_size_mb, 256);
        assert_eq!(prepared.config.job_ttl_minutes, 9);
        assert_eq!(prepared.config.max_streaming, 5);
        assert_eq!(
            prepared
                .config
                .transcription_defaults
                .gpu_acceleration
                .as_deref(),
            Some("cuda")
        );
        assert_eq!(
            prepared
                .config
                .transcription_defaults
                .vad_model_id
                .as_deref(),
            Some("vad-model")
        );
        assert_eq!(
            prepared
                .config
                .transcription_defaults
                .punctuation_model_id
                .as_deref(),
            Some("punct-model")
        );
    }

    #[test]
    fn prepare_runtime_config_rejects_invalid_whitelist() {
        let (_shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let error = match prepare_runtime_config(ApiServerRuntimeParts {
            resolved: sona_core::runtime::serve::ResolvedServeRuntimeOptions {
                host: "127.0.0.1".to_string(),
                port: 14200,
                api_key: String::new(),
                models_dir: PathBuf::from("models"),
                ip_whitelist: "not-a-rule".to_string(),
                max_streaming: 1,
                max_concurrent: 1,
                max_queue_size: 1,
                max_upload_size_mb: 1,
                job_ttl_minutes: 1,
                transcription_defaults: Default::default(),
            },
            temp_dir: PathBuf::from("temp"),
            online_asr_config: Arc::new(RwLock::new(HashMap::new())),
            batch_transcriber: test_batch_transcriber(),
            media_validator: Arc::new(AcceptingMediaValidator),
            gpu_availability: Arc::new(FixedGpuAvailability(false)),
            model_catalog: test_model_catalog(),
            batch_plan_resolver: test_batch_plan_resolver(),
            platform: Arc::new(DefaultApiServerPlatform),
            streaming_router: None,
            web_dist_dir: None,
            shutdown_rx,
            bind_tx: None,
        }) {
            Ok(_) => panic!("invalid whitelist should be rejected"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            ApiServerConfigurationError::InvalidIpRule {
                rule: "not-a-rule".to_string(),
            }
        );
    }

    #[tokio::test]
    async fn run_server_reports_bind_failure() {
        let occupier = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = occupier.local_addr().unwrap().port();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let (bind_tx, bind_rx) = tokio::sync::oneshot::channel();
        let temp_dir = tempfile::tempdir().unwrap().path().to_path_buf();
        let models_dir = tempfile::tempdir().unwrap().path().to_path_buf();

        let config = ApiServerRuntimeConfig {
            host: "127.0.0.1".to_string(),
            port,
            api_key: String::new(),
            temp_dir,
            models_dir,
            max_concurrent: 1,
            max_queue_size: 0,
            max_upload_size_mb: 1,
            job_ttl_minutes: 1,
            max_streaming: 1,
            ip_whitelist: Arc::new(vec![]),
            online_asr_config: Arc::new(RwLock::new(HashMap::new())),
            transcription_defaults: ApiServerTranscriptionDefaults::default(),
            batch_transcriber: test_batch_transcriber(),
            media_validator: Arc::new(AcceptingMediaValidator),
            gpu_availability: Arc::new(FixedGpuAvailability(false)),
            model_catalog: test_model_catalog(),
            batch_plan_resolver: test_batch_plan_resolver(),
            platform: Arc::new(DefaultApiServerPlatform),
            streaming_router: None,
            web_dist_dir: None,
            shutdown_rx,
            bind_tx: Some(bind_tx),
        };

        let handle = tokio::spawn(async move { run_server(config).await });
        let bind_error = match bind_rx.await.unwrap() {
            Ok(_) => panic!("occupied port should fail to bind"),
            Err(error) => error,
        };
        assert!(matches!(
            bind_error,
            ApiServerRuntimeError::Bind(ApiServerBindError {
                ref address,
                kind: ApiServerBindErrorKind::AddressInUse,
                ..
            }) if address == &format!("127.0.0.1:{port}")
        ));

        let _ = shutdown_tx.send(());
        let _ = handle.await;
    }

    #[tokio::test]
    async fn start_api_server_runtime_reports_bind_failure() {
        let occupier = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = occupier.local_addr().unwrap().port();
        let temp_dir = tempfile::tempdir().unwrap().path().join("api-temp");
        let models_dir = tempfile::tempdir().unwrap().path().join("models");

        let result = start_api_server_runtime(ApiServerServiceParts {
            resolved: sona_core::runtime::serve::ResolvedServeRuntimeOptions {
                host: "127.0.0.1".to_string(),
                port,
                api_key: String::new(),
                models_dir,
                ip_whitelist: "localhost".to_string(),
                max_streaming: 1,
                max_concurrent: 1,
                max_queue_size: 1,
                max_upload_size_mb: 1,
                job_ttl_minutes: 1,
                transcription_defaults: Default::default(),
            },
            temp_dir,
            online_asr_config: Arc::new(RwLock::new(HashMap::new())),
            batch_transcriber: test_batch_transcriber(),
            media_validator: Arc::new(AcceptingMediaValidator),
            gpu_availability: Arc::new(FixedGpuAvailability(false)),
            model_catalog: test_model_catalog(),
            batch_plan_resolver: test_batch_plan_resolver(),
            platform: Arc::new(DefaultApiServerPlatform),
            streaming_router: None,
            web_dist_dir: None,
        })
        .await;

        let error = result.expect_err("occupied port should fail to bind");
        assert!(matches!(
            error,
            ApiServerStartError::Runtime(ApiServerRuntimeError::Bind(ApiServerBindError {
                kind: ApiServerBindErrorKind::AddressInUse,
                ..
            }))
        ));
        assert!(error.to_string().contains("Address already in use"));
    }

    #[tokio::test]
    async fn start_api_server_runtime_reports_configuration_failure() {
        let temp_dir = tempfile::tempdir().unwrap().path().join("api-temp");
        let models_dir = tempfile::tempdir().unwrap().path().join("models");

        let result = start_api_server_runtime(ApiServerServiceParts {
            resolved: sona_core::runtime::serve::ResolvedServeRuntimeOptions {
                host: "127.0.0.1".to_string(),
                port: 14200,
                api_key: String::new(),
                models_dir,
                ip_whitelist: "not-a-rule".to_string(),
                max_streaming: 1,
                max_concurrent: 1,
                max_queue_size: 1,
                max_upload_size_mb: 1,
                job_ttl_minutes: 1,
                transcription_defaults: Default::default(),
            },
            temp_dir,
            online_asr_config: Arc::new(RwLock::new(HashMap::new())),
            batch_transcriber: test_batch_transcriber(),
            media_validator: Arc::new(AcceptingMediaValidator),
            gpu_availability: Arc::new(FixedGpuAvailability(false)),
            model_catalog: test_model_catalog(),
            batch_plan_resolver: test_batch_plan_resolver(),
            platform: Arc::new(DefaultApiServerPlatform),
            streaming_router: None,
            web_dist_dir: None,
        })
        .await;

        let error = result.expect_err("invalid whitelist should fail before starting");
        assert_eq!(
            error,
            ApiServerStartError::Configuration(ApiServerConfigurationError::InvalidIpRule {
                rule: "not-a-rule".to_string(),
            })
        );
    }

    #[tokio::test]
    async fn start_api_server_runtime_returns_stoppable_server_with_dashboard_snapshot() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let temp_dir = tempfile::tempdir().unwrap().path().join("api-temp");
        let models_dir = tempfile::tempdir().unwrap().path().join("models");

        let server = start_api_server_runtime(ApiServerServiceParts {
            resolved: sona_core::runtime::serve::ResolvedServeRuntimeOptions {
                host: "127.0.0.1".to_string(),
                port,
                api_key: String::new(),
                models_dir,
                ip_whitelist: "localhost".to_string(),
                max_streaming: 1,
                max_concurrent: 1,
                max_queue_size: 1,
                max_upload_size_mb: 1,
                job_ttl_minutes: 1,
                transcription_defaults: Default::default(),
            },
            temp_dir,
            online_asr_config: Arc::new(RwLock::new(HashMap::new())),
            batch_transcriber: test_batch_transcriber(),
            media_validator: Arc::new(AcceptingMediaValidator),
            gpu_availability: Arc::new(FixedGpuAvailability(false)),
            model_catalog: test_model_catalog(),
            batch_plan_resolver: test_batch_plan_resolver(),
            platform: Arc::new(DefaultApiServerPlatform),
            streaming_router: None,
            web_dist_dir: None,
        })
        .await
        .unwrap();

        assert_eq!(server.normalized_ip_whitelist, "127.0.0.0/8,::1/128");
        let snapshot = server.dashboard_snapshot().await.unwrap();
        assert_eq!(snapshot.health.status, "ok");
        assert_eq!(snapshot.health.active_jobs, 0);
        assert_eq!(snapshot.health.pending_jobs, 0);
        assert!(snapshot.jobs.is_empty());
        server.stop().await.unwrap();
    }

    #[tokio::test]
    async fn api_key_auth_middleware_accepts_bearer_and_query_token_and_rejects_invalid() {
        let (tx, _rx) = mpsc::channel(1);
        let state = ServerState {
            job_manager: JobManager::new(tx),
            temp_dir: PathBuf::from("temp"),
            models_dir: PathBuf::from("models"),
            start_time: std::time::Instant::now(),
            api_key: "secret-token".to_string(),
            streaming_semaphore: Arc::new(tokio::sync::Semaphore::new(1)),
            ip_whitelist: Arc::new(vec![]),
            online_asr_config: Arc::new(RwLock::new(HashMap::new())),
            media_validator: Arc::new(AcceptingMediaValidator),
            gpu_availability: Arc::new(FixedGpuAvailability(false)),
            model_catalog: test_model_catalog(),
            batch_plan_resolver: test_batch_plan_resolver(),
            platform: Arc::new(DefaultApiServerPlatform),
            transcription_defaults: Default::default(),
        };

        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                api_key_auth_middleware,
            ))
            .with_state(state);

        // 1. Missing auth -> 401
        let res = app
            .clone()
            .oneshot(Request::builder().uri("/test").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        // 2. Invalid Bearer -> 401
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/test")
                    .header("Authorization", "Bearer wrong")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        // 3. Valid Bearer -> 200
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/test")
                    .header("Authorization", "Bearer secret-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // 4. Valid query token -> 200
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/test?token=secret-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // 5. Valid query api_key -> 200
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/test?api_key=secret-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn delete_job_removes_entry_and_file() {
        let (tx, _rx) = mpsc::channel(10);
        let job_manager = JobManager::new(tx);
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();
        assert!(path.exists());

        job_manager.jobs.write().await.insert(
            "job-del".to_string(),
            JobEntry {
                status: JobStatus::Completed(vec![]),
                created_at: std::time::Instant::now(),
                completed_at: Some(std::time::Instant::now()),
                file_path: Some(path.clone()),
                abort_handle: None,
            },
        );

        let mut state = streaming_authorization_state("", 1, "127.0.0.1/32");
        state.job_manager = job_manager;
        state.temp_dir = path.parent().unwrap().to_path_buf();

        let app = Router::new()
            .route(
                "/v1/transcriptions/{job_id}",
                axum::routing::delete(handle_delete_job),
            )
            .with_state(state.clone());

        let res = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/v1/transcriptions/job-del")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::NO_CONTENT);
        assert!(!path.exists());
        assert!(state.job_manager.get_job("job-del").await.is_none());
    }

    #[tokio::test]
    async fn export_job_returns_formatted_subtitles() {
        let (tx, _rx) = mpsc::channel(10);
        let job_manager = JobManager::new(tx);
        let segments = vec![TranscriptSegment {
            id: "seg-1".to_string(),
            start: 0.0,
            end: 2.5,
            text: "Hello world".to_string(),
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            speaker: None,
            speaker_attribution: None,
        }];

        job_manager.jobs.write().await.insert(
            "job-exp".to_string(),
            JobEntry {
                status: JobStatus::Completed(segments),
                created_at: std::time::Instant::now(),
                completed_at: Some(std::time::Instant::now()),
                file_path: None,
                abort_handle: None,
            },
        );

        let mut state = streaming_authorization_state("", 1, "127.0.0.1/32");
        state.job_manager = job_manager;

        let app = Router::new()
            .route(
                "/v1/transcriptions/{job_id}/export",
                axum::routing::get(handle_export_job),
            )
            .with_state(state);

        let res = app
            .oneshot(
                Request::builder()
                    .uri("/v1/transcriptions/job-exp/export?format=srt")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get("content-disposition").unwrap(),
            "attachment; filename=\"job-exp.srt\""
        );
        let body = axum::body::to_bytes(res.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.contains("00:00:00,000 --> 00:00:02,500"));
        assert!(text.contains("Hello world"));
    }

    #[tokio::test]
    async fn streaming_auth_enforces_ip_whitelist_and_key() {
        let state = streaming_authorization_state("my-secret-key", 1, "127.0.0.1/32");

        // 1. Foreign IP without key -> Forbidden
        let foreign_addr = "192.168.1.50:12345".parse().unwrap();
        let err = authorize_streaming_request(&state, foreign_addr, None).unwrap_err();
        assert_eq!(err, StatusCode::FORBIDDEN);

        // 2. Foreign IP with invalid key -> Forbidden (non-whitelist IP)
        let err = authorize_streaming_request(&state, foreign_addr, Some("wrong-key")).unwrap_err();
        assert_eq!(err, StatusCode::FORBIDDEN);

        // 3. Foreign IP with valid key -> Forbidden (Defense in Depth: IP not in whitelist)
        let err =
            authorize_streaming_request(&state, foreign_addr, Some("my-secret-key")).unwrap_err();
        assert_eq!(err, StatusCode::FORBIDDEN);

        // 4. Whitelisted IP with invalid key -> Unauthorized
        let local_addr = "127.0.0.1:12345".parse().unwrap();
        let err_local =
            authorize_streaming_request(&state, local_addr, Some("wrong-key")).unwrap_err();
        assert_eq!(err_local, StatusCode::UNAUTHORIZED);

        // 5. Whitelisted IP with valid key -> Success (permit acquired)
        let permit = authorize_streaming_request(&state, local_addr, Some("my-secret-key"));
        assert!(permit.is_ok());
    }

    #[tokio::test]
    async fn http_ip_whitelist_middleware_enforces_auth_and_network_perimeter() {
        let state = streaming_authorization_state("secret-key", 1, "127.0.0.1/32");
        let app = Router::new()
            .route("/api/test", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                crate::handlers::ip_whitelist_middleware,
            ))
            .with_state(state);

        let foreign_ip: SocketAddr = "192.168.1.50:12345".parse().unwrap();
        let local_ip: SocketAddr = "127.0.0.1:12345".parse().unwrap();

        // 1. Foreign IP without key -> 403 Forbidden
        let req = Request::builder()
            .uri("/api/test")
            .extension(ConnectInfo(foreign_ip))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);

        // 2. Foreign IP with invalid key -> 403 Forbidden
        let req = Request::builder()
            .uri("/api/test")
            .header("Authorization", "Bearer wrong-key")
            .extension(ConnectInfo(foreign_ip))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);

        // 3. Foreign IP with valid key -> 403 Forbidden (Defense in Depth: network perimeter enforced)
        let req = Request::builder()
            .uri("/api/test")
            .header("Authorization", "Bearer secret-key")
            .extension(ConnectInfo(foreign_ip))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);

        // 4. Whitelisted IP with invalid key -> 401 Unauthorized
        let req = Request::builder()
            .uri("/api/test")
            .header("Authorization", "Bearer wrong-key")
            .extension(ConnectInfo(local_ip))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        // 5. Whitelisted IP without key (when key required) -> 401 Unauthorized
        let req = Request::builder()
            .uri("/api/test")
            .extension(ConnectInfo(local_ip))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        // 6. Whitelisted IP with valid key -> 200 OK
        let req = Request::builder()
            .uri("/api/test")
            .header("Authorization", "Bearer secret-key")
            .extension(ConnectInfo(local_ip))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
    #[tokio::test]
    async fn query_token_handles_percent_encoded_special_chars() {
        let state = streaming_authorization_state("my+secret token=123", 1, "10.0.0.1/32");
        let app = Router::new()
            .route("/api/test", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                crate::handlers::ip_whitelist_middleware,
            ))
            .with_state(state);

        let foreign_ip: SocketAddr = "10.0.0.1:12345".parse().unwrap();
        // URL-encoded "my+secret token=123" -> "my%2Bsecret%20token%3D123"
        let req = Request::builder()
            .uri("/api/test?token=my%2Bsecret%20token%3D123")
            .extension(ConnectInfo(foreign_ip))
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[test]
    fn constant_time_eq_str_verifies_equality_correctly() {
        use crate::handlers::constant_time_eq_str;
        assert!(constant_time_eq_str("password123", "password123"));
        assert!(!constant_time_eq_str("password123", "password124"));
        assert!(!constant_time_eq_str("short", "longer_password"));
        assert!(!constant_time_eq_str("", "not_empty"));
        assert!(constant_time_eq_str("", ""));
    }

    #[test]
    fn validate_webhook_url_rejects_link_local_and_invalid_schemes() {
        use crate::handlers::validate_webhook_url;
        assert!(validate_webhook_url("http://localhost:3000/webhook").is_ok());
        assert!(validate_webhook_url("https://api.example.com/callback").is_ok());
        assert!(validate_webhook_url("http://192.168.1.100:8080/hook").is_ok());
        // Block cloud metadata / link-local addresses, unspecified, broadcast
        assert!(validate_webhook_url("http://169.254.169.254/latest/meta-data").is_err());
        assert!(validate_webhook_url("http://169.254.1.1/hook").is_err());
        assert!(validate_webhook_url("http://0.0.0.0/hook").is_err());
        assert!(validate_webhook_url("http://255.255.255.255/hook").is_err());
        assert!(validate_webhook_url("file:///etc/passwd").is_err());
        assert!(validate_webhook_url("ftp://example.com/file").is_err());
        assert!(validate_webhook_url("not-a-url").is_err());
    }
    #[test]
    fn extract_api_key_handles_case_insensitive_bearer_prefix() {
        use crate::handlers::extract_api_key_from_request;
        let req_lower = Request::builder()
            .header("Authorization", "bearer test-key-123")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            extract_api_key_from_request(&req_lower).as_deref(),
            Some("test-key-123")
        );

        let req_upper = Request::builder()
            .header("Authorization", "BEARER test-key-123")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            extract_api_key_from_request(&req_upper).as_deref(),
            Some("test-key-123")
        );

        let req_mixed = Request::builder()
            .header("Authorization", "Bearer test-key-123")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            extract_api_key_from_request(&req_mixed).as_deref(),
            Some("test-key-123")
        );
    }

    #[tokio::test]
    async fn submit_job_returns_queue_full_when_capacity_reached() {
        let (tx, _rx) = mpsc::channel(1);
        let job_manager = JobManager::new(tx);
        let job1 = TranscriptionJob {
            job_id: "job-1".to_string(),
            file_path: PathBuf::from("job1.wav"),
            model_id: "model".to_string(),
            language: "auto".to_string(),
            hotwords: None,
            webhook_url: None,
            webhook_secret: None,
            engine: "Local".to_string(),
            online_provider_id: None,
            online_provider_config: None,
        };
        let job2 = TranscriptionJob {
            job_id: "job-2".to_string(),
            file_path: PathBuf::from("job2.wav"),
            model_id: "model".to_string(),
            language: "auto".to_string(),
            hotwords: None,
            webhook_url: None,
            webhook_secret: None,
            engine: "Local".to_string(),
            online_provider_id: None,
            online_provider_config: None,
        };
        assert!(job_manager.submit_job(job1).await.is_ok());
        let err = job_manager.submit_job(job2).await.unwrap_err();
        assert!(matches!(err, ApiServerJobError::QueueFull { .. }));
    }

    #[tokio::test]
    async fn transcribe_rejects_duplicate_file_fields() {
        let temp = tempfile::tempdir().unwrap();
        let mut state = streaming_authorization_state("", 1, "127.0.0.1/32");
        state.temp_dir = temp.path().to_path_buf();
        state.media_validator = Arc::new(AcceptingMediaValidator);

        let boundary = "sona-duplicate-test";
        let body = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"sample1.wav\"\r\nContent-Type: audio/wav\r\n\r\naudio data 1\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"sample2.wav\"\r\nContent-Type: audio/wav\r\n\r\naudio data 2\r\n--{boundary}--\r\n"
        );

        let app = Router::new()
            .route("/v1/transcriptions", post(handle_transcribe))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/transcriptions")
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        // Confirm all files were cleaned up
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn llm_polish_and_translate_reject_empty_payloads() {
        let state = streaming_authorization_state("", 1, "127.0.0.1/32");
        let app = Router::new()
            .route("/v1/llm/polish", post(crate::handlers::handle_llm_polish))
            .route(
                "/v1/llm/translate",
                post(crate::handlers::handle_llm_translate),
            )
            .with_state(state);

        // Polish with empty segments -> 400 Bad Request
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/llm/polish")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"segments":[]}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);

        // Translate with empty segments -> 400 Bad Request
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/llm/translate")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"segments":[],"target_language":"en"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);

        // Translate with empty target_language -> 400 Bad Request
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/llm/translate")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"segments":[{"id":"1","start":0.0,"end":1.0,"text":"hi","isFinal":true}],"target_language":"   "}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }
}
