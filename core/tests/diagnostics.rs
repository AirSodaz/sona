use serde_json::json;
use sona_core::models::preset_models::{
    ModelCatalogSnapshot, build_model_catalog_snapshot_with_installed_ids,
};
use sona_core::runtime::diagnostics::{
    DeviceOptionInput, DeviceProbeInput, DiagnosticsConfigInput, DiagnosticsCoreInput,
    DiagnosticsEnrichmentMeasurements, DiagnosticsEnrichmentRepository, DiagnosticsError,
    DiagnosticsService, ModelRuleInput, ModelRulesInput, ModelSummaryInput, PathStatusesInput,
    RuntimeEnvironmentStatus, RuntimePathKind, RuntimePathStatus, SelectedModelsInput,
    VoiceTypingReadinessInput, build_diagnostics_core_snapshot_at,
};
use sona_core::transcription::asr_metrics::{
    AsrInferenceMetric, AsrModelLoadMetric, AsrRuntimeMetricsSnapshot,
};
use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, Mutex};

const SCANNED_AT: &str = "2026-07-08T01:02:03.004Z";
const LIVE_MODEL_ID: &str = "sherpa-onnx-streaming-paraformer-trilingual-zh-cantonese-en";
const BATCH_MODEL_ID: &str = "sherpa-onnx-whisper-turbo";

struct FixedRepository {
    measurements: Mutex<Option<DiagnosticsEnrichmentMeasurements>>,
}

impl DiagnosticsEnrichmentRepository for FixedRepository {
    fn collect_measurements(
        &self,
        _config: &DiagnosticsConfigInput,
    ) -> Result<DiagnosticsEnrichmentMeasurements, DiagnosticsError> {
        Ok(self.measurements.lock().unwrap().take().unwrap())
    }
}

struct FailingRepository;

impl DiagnosticsEnrichmentRepository for FailingRepository {
    fn collect_measurements(
        &self,
        _config: &DiagnosticsConfigInput,
    ) -> Result<DiagnosticsEnrichmentMeasurements, DiagnosticsError> {
        Err(DiagnosticsError::Repository(
            "catalog scan failed".to_string(),
        ))
    }
}

fn model_catalog(models_dir: &Path) -> ModelCatalogSnapshot {
    build_model_catalog_snapshot_with_installed_ids(models_dir, &HashSet::new())
}

fn model_path(catalog: &ModelCatalogSnapshot, model_id: &str) -> String {
    catalog
        .models
        .iter()
        .find(|model| model.id == model_id)
        .unwrap()
        .install_path
        .clone()
}

fn measurements(catalog: ModelCatalogSnapshot) -> DiagnosticsEnrichmentMeasurements {
    DiagnosticsEnrichmentMeasurements {
        model_catalog: catalog,
        path_statuses: PathStatusesInput {
            live_model: Some(path_status("live-measured", RuntimePathKind::Directory)),
            batch_model: Some(path_status("batch-measured", RuntimePathKind::Directory)),
            vad: Some(path_status("vad-measured", RuntimePathKind::File)),
            punctuation: None,
        },
    }
}

fn base_input() -> DiagnosticsCoreInput {
    DiagnosticsCoreInput {
        config: DiagnosticsConfigInput {
            streaming_model_path: "C:\\models\\live".to_string(),
            batch_model_path: "C:\\models\\batch".to_string(),
            vad_model_path: "C:\\models\\vad.onnx".to_string(),
            punctuation_model_path: "".to_string(),
            microphone_id: "default".to_string(),
        },
        selected_models: SelectedModelsInput {
            live: Some(ModelSummaryInput {
                id: "live".to_string(),
                name: "Live Model".to_string(),
            }),
            batch: Some(ModelSummaryInput {
                id: "batch".to_string(),
                name: "Batch Model".to_string(),
            }),
        },
        model_rules: ModelRulesInput {
            live: Some(ModelRuleInput {
                requires_vad: true,
                requires_punctuation: false,
            }),
            batch: Some(ModelRuleInput {
                requires_vad: false,
                requires_punctuation: false,
            }),
        },
        path_statuses: PathStatusesInput {
            live_model: Some(path_status("C:\\models\\live", RuntimePathKind::Directory)),
            batch_model: Some(path_status("C:\\models\\batch", RuntimePathKind::Directory)),
            vad: Some(path_status("C:\\models\\vad.onnx", RuntimePathKind::File)),
            punctuation: None,
        },
        permission_state: "granted".to_string(),
        microphone_probe: DeviceProbeInput {
            options: vec![DeviceOptionInput {
                label: "Auto".to_string(),
                value: "default".to_string(),
            }],
            available: true,
            error_message: None,
        },
        system_audio_probe: DeviceProbeInput {
            options: vec![],
            available: true,
            error_message: None,
        },
        voice_typing_readiness: VoiceTypingReadinessInput {
            state: "ready".to_string(),
            last_error_message: None,
        },
        runtime_environment: RuntimeEnvironmentStatus {
            ffmpeg_path: "C:\\app\\ffmpeg.exe".to_string(),
            ffmpeg_exists: true,
            log_dir_path: "C:\\app\\logs".to_string(),
        },
        asr_runtime_metrics: AsrRuntimeMetricsSnapshot::default(),
        onboarding_ready: true,
        punctuation_required: false,
    }
}

fn path_status(path: &str, kind: RuntimePathKind) -> RuntimePathStatus {
    RuntimePathStatus {
        path: path.to_string(),
        kind,
        error: None,
    }
}

fn build_snapshot(
    input: DiagnosticsCoreInput,
) -> sona_core::runtime::diagnostics::DiagnosticsCoreSnapshot {
    build_diagnostics_core_snapshot_at(input, SCANNED_AT.to_string())
}

#[test]
fn core_snapshot_serializes_fact_fields_without_ui_spec() {
    let value = serde_json::to_value(build_snapshot(base_input())).unwrap();

    assert!(value.get("overview").is_none());
    assert!(value.get("sections").is_none());
    assert!(value.get("config").is_some());
    assert!(value.get("selectedModels").is_some());
    assert!(value.get("pathStatuses").is_some());
    assert!(value.to_string().find("settingsTab").is_none());
    assert!(
        value
            .to_string()
            .find("settings.diagnostics.open_model_settings")
            .is_none()
    );
}

#[test]
fn core_snapshot_preserves_fact_fields_for_frontend_ui_builder() {
    let mut input = base_input();
    input.permission_state = "prompt".to_string();
    input.runtime_environment.ffmpeg_exists = false;
    input.path_statuses.live_model =
        Some(path_status("C:\\models\\live", RuntimePathKind::Unknown));
    input.path_statuses.punctuation = Some(path_status(
        "C:\\models\\punct.onnx",
        RuntimePathKind::Unknown,
    ));
    input.config.punctuation_model_path = "C:\\models\\punct.onnx".to_string();
    input.punctuation_required = true;

    let snapshot = build_snapshot(input);

    assert_eq!(snapshot.permission_state, "prompt");
    assert!(!snapshot.runtime_environment.ffmpeg_exists);
    assert!(snapshot.punctuation_required);
    assert_eq!(
        snapshot.path_statuses.live_model.as_ref().unwrap().kind,
        RuntimePathKind::Unknown
    );
    assert_eq!(
        snapshot.path_statuses.punctuation.as_ref().unwrap().path,
        "C:\\models\\punct.onnx"
    );
    assert_eq!(
        snapshot.selected_models.live.as_ref().unwrap().name,
        "Live Model"
    );
}

#[test]
fn core_snapshot_carries_asr_metrics_without_formatting() {
    let mut input = base_input();
    let metrics = AsrRuntimeMetricsSnapshot {
        model_load: Some(AsrModelLoadMetric {
            occurred_at_ms: 1,
            instance_id: "record".to_string(),
            model_path: "C:\\models\\live".to_string(),
            model_type: "sensevoice".to_string(),
            recognizer_kind: "batch".to_string(),
            num_threads: 4,
            reused_from_pool: false,
            load_ms: 123.4,
            rss_before_mb: Some(416.25),
            rss_after_mb: Some(512.5),
            rss_delta_mb: Some(96.25),
            process_rss_mb: Some(512.5),
        }),
        live_inference: Some(AsrInferenceMetric {
            occurred_at_ms: 2,
            source: "live".to_string(),
            instance_id: Some("record".to_string()),
            stage: "final".to_string(),
            is_final: true,
            audio_duration_ms: 1600.0,
            buffered_samples: 25_600,
            audio_extract_ms: None,
            decode_ms: 42.2,
            emit_latency_ms: Some(60.1),
            total_ms: None,
            rtf: Some(0.026),
            segment_count: None,
            process_rss_mb: Some(520.1),
        }),
        batch_inference: None,
    };
    input.asr_runtime_metrics = metrics.clone();

    let snapshot = build_snapshot(input);

    assert_eq!(snapshot.asr_runtime_metrics, metrics);
    assert_eq!(
        snapshot
            .asr_runtime_metrics
            .model_load
            .as_ref()
            .map(|metric| metric.process_rss_mb)
            .unwrap(),
        Some(512.5)
    );
    assert_eq!(
        snapshot
            .asr_runtime_metrics
            .live_inference
            .as_ref()
            .and_then(|metric| metric.emit_latency_ms),
        Some(60.1)
    );
}

#[test]
fn core_snapshot_uses_supplied_scanned_at_timestamp() {
    let snapshot = build_diagnostics_core_snapshot_at(base_input(), SCANNED_AT.to_string());

    assert_eq!(snapshot.scanned_at, SCANNED_AT);
}

#[test]
fn service_owns_model_selection_rules_and_readiness_policy() {
    let models_dir = Path::new("C:\\sona\\模型");
    let catalog = model_catalog(models_dir);
    let live_path = model_path(&catalog, LIVE_MODEL_ID);
    let batch_path = model_path(&catalog, BATCH_MODEL_ID);
    let mut input = base_input();
    input.config.streaming_model_path = live_path;
    input.config.batch_model_path = batch_path;
    input.selected_models = SelectedModelsInput::default();
    input.model_rules = ModelRulesInput::default();
    input.path_statuses = PathStatusesInput::default();
    input.onboarding_ready = false;
    input.punctuation_required = false;
    let expected_environment = input.runtime_environment.clone();
    let expected_metrics = input.asr_runtime_metrics.clone();
    let service = DiagnosticsService::new(Arc::new(FixedRepository {
        measurements: Mutex::new(Some(measurements(catalog))),
    }));

    let snapshot = service
        .build_snapshot_at(input, SCANNED_AT.to_string())
        .unwrap();

    assert_eq!(
        snapshot.selected_models.live.as_ref().unwrap().id,
        LIVE_MODEL_ID
    );
    assert_eq!(
        snapshot.selected_models.batch.as_ref().unwrap().id,
        BATCH_MODEL_ID
    );
    assert!(!snapshot.model_rules.live.as_ref().unwrap().requires_vad);
    assert!(
        snapshot
            .model_rules
            .live
            .as_ref()
            .unwrap()
            .requires_punctuation
    );
    assert!(snapshot.model_rules.batch.as_ref().unwrap().requires_vad);
    assert!(snapshot.onboarding_ready);
    assert!(snapshot.punctuation_required);
    assert_eq!(
        snapshot.path_statuses.live_model.as_ref().unwrap().path,
        "live-measured"
    );
    assert_eq!(snapshot.runtime_environment, expected_environment);
    assert_eq!(snapshot.asr_runtime_metrics, expected_metrics);
    assert_eq!(snapshot.scanned_at, SCANNED_AT);
}

#[test]
fn service_clears_derived_fields_for_blank_model_paths() {
    let catalog = model_catalog(Path::new("C:\\sona\\models"));
    let mut input = base_input();
    input.config.streaming_model_path = "  ".to_string();
    input.config.batch_model_path.clear();
    input.selected_models = SelectedModelsInput {
        live: Some(ModelSummaryInput {
            id: "stale-live".to_string(),
            name: "Stale".to_string(),
        }),
        batch: None,
    };
    input.punctuation_required = true;
    input.onboarding_ready = true;
    let service = DiagnosticsService::new(Arc::new(FixedRepository {
        measurements: Mutex::new(Some(DiagnosticsEnrichmentMeasurements {
            model_catalog: catalog,
            path_statuses: PathStatusesInput::default(),
        })),
    }));

    let snapshot = service
        .build_snapshot_at(input, SCANNED_AT.to_string())
        .unwrap();

    assert!(snapshot.selected_models.live.is_none());
    assert!(snapshot.selected_models.batch.is_none());
    assert!(snapshot.model_rules.live.is_none());
    assert!(snapshot.model_rules.batch.is_none());
    assert!(!snapshot.onboarding_ready);
    assert!(!snapshot.punctuation_required);
    assert!(snapshot.path_statuses.live_model.is_none());
}

#[test]
fn service_propagates_repository_errors() {
    let error = DiagnosticsService::new(Arc::new(FailingRepository))
        .build_snapshot_at(base_input(), SCANNED_AT.to_string())
        .unwrap_err();

    assert_eq!(
        error,
        DiagnosticsError::Repository("catalog scan failed".to_string())
    );
}

#[test]
fn service_snapshot_preserves_the_existing_camel_case_json_contract() {
    let catalog = model_catalog(Path::new("C:\\sona\\models"));
    let live_path = model_path(&catalog, LIVE_MODEL_ID);
    let batch_path = model_path(&catalog, BATCH_MODEL_ID);
    let mut input = base_input();
    input.config.streaming_model_path = live_path.clone();
    input.config.batch_model_path = batch_path.clone();
    let service = DiagnosticsService::new(Arc::new(FixedRepository {
        measurements: Mutex::new(Some(measurements(catalog))),
    }));

    let value = serde_json::to_value(
        service
            .build_snapshot_at(input, SCANNED_AT.to_string())
            .unwrap(),
    )
    .unwrap();

    assert_eq!(
        value,
        json!({
            "scannedAt": SCANNED_AT,
            "config": {
                "streamingModelPath": live_path,
                "batchModelPath": batch_path,
                "vadModelPath": "C:\\models\\vad.onnx",
                "punctuationModelPath": "",
                "microphoneId": "default"
            },
            "selectedModels": {
                "live": {"id": LIVE_MODEL_ID, "name": "Paraformer"},
                "batch": {"id": BATCH_MODEL_ID, "name": "Whisper"}
            },
            "modelRules": {
                "live": {"requiresVad": false, "requiresPunctuation": true},
                "batch": {"requiresVad": true, "requiresPunctuation": false}
            },
            "pathStatuses": {
                "liveModel": {"path": "live-measured", "kind": "directory", "error": null},
                "batchModel": {"path": "batch-measured", "kind": "directory", "error": null},
                "vad": {"path": "vad-measured", "kind": "file", "error": null},
                "punctuation": null
            },
            "permissionState": "granted",
            "microphoneProbe": {
                "options": [{"label": "Auto", "value": "default"}],
                "available": true,
                "errorMessage": null
            },
            "systemAudioProbe": {"options": [], "available": true, "errorMessage": null},
            "voiceTypingReadiness": {"state": "ready", "lastErrorMessage": null},
            "runtimeEnvironment": {
                "ffmpegPath": "C:\\app\\ffmpeg.exe",
                "ffmpegExists": true,
                "logDirPath": "C:\\app\\logs"
            },
            "asrRuntimeMetrics": {
                "modelLoad": null,
                "liveInference": null,
                "batchInference": null
            },
            "onboardingReady": true,
            "punctuationRequired": true
        })
    );
}
