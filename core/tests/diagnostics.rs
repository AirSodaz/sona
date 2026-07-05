use sona_core::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric, AsrRuntimeMetricsSnapshot};
use sona_core::diagnostics::{
    DeviceOptionInput, DeviceProbeInput, DiagnosticsConfigInput, DiagnosticsCoreInput,
    ModelRuleInput, ModelRulesInput, ModelSummaryInput, PathStatusesInput,
    RuntimeEnvironmentStatus, RuntimePathKind, RuntimePathStatus, SelectedModelsInput,
    VoiceTypingReadinessInput, build_diagnostics_core_snapshot,
};

fn base_input() -> DiagnosticsCoreInput {
    DiagnosticsCoreInput {
        config: DiagnosticsConfigInput {
            streaming_model_path: "C:\\models\\live".to_string(),
            offline_model_path: "C:\\models\\offline".to_string(),
            vad_model_path: "C:\\models\\vad.onnx".to_string(),
            punctuation_model_path: "".to_string(),
            microphone_id: "default".to_string(),
        },
        selected_models: SelectedModelsInput {
            live: Some(ModelSummaryInput {
                id: "live".to_string(),
                name: "Live Model".to_string(),
            }),
            offline: Some(ModelSummaryInput {
                id: "offline".to_string(),
                name: "Offline Model".to_string(),
            }),
        },
        model_rules: ModelRulesInput {
            live: Some(ModelRuleInput {
                requires_vad: true,
                requires_punctuation: false,
            }),
            offline: Some(ModelRuleInput {
                requires_vad: false,
                requires_punctuation: false,
            }),
        },
        path_statuses: PathStatusesInput {
            live_model: Some(path_status("C:\\models\\live", RuntimePathKind::Directory)),
            offline_model: Some(path_status(
                "C:\\models\\offline",
                RuntimePathKind::Directory,
            )),
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

#[test]
fn core_snapshot_serializes_fact_fields_without_ui_spec() {
    let value = serde_json::to_value(build_diagnostics_core_snapshot(base_input())).unwrap();

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

    let snapshot = build_diagnostics_core_snapshot(input);

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
            recognizer_kind: "offline".to_string(),
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

    let snapshot = build_diagnostics_core_snapshot(input);

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
