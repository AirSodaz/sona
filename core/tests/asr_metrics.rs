use sona_core::transcription::asr_metrics::{
    AsrInferenceMetric, AsrModelLoadMetric, AsrRuntimeMetricsSnapshot,
};

#[test]
fn asr_runtime_metrics_transport_shape_lives_in_core() {
    let snapshot = AsrRuntimeMetricsSnapshot {
        model_load: Some(AsrModelLoadMetric {
            occurred_at_ms: 1000,
            instance_id: "instance-1".to_string(),
            model_path: "model.onnx".to_string(),
            model_type: "sensevoice".to_string(),
            recognizer_kind: "batch".to_string(),
            num_threads: 4,
            reused_from_pool: true,
            load_ms: 12.5,
            rss_before_mb: Some(128.0),
            rss_after_mb: Some(160.0),
            rss_delta_mb: Some(32.0),
            process_rss_mb: Some(160.0),
        }),
        live_inference: Some(AsrInferenceMetric {
            occurred_at_ms: 1200,
            source: "live".to_string(),
            instance_id: Some("instance-1".to_string()),
            stage: "decode".to_string(),
            is_final: true,
            audio_duration_ms: 5000.0,
            buffered_samples: 80_000,
            audio_extract_ms: None,
            decode_ms: 420.0,
            emit_latency_ms: Some(40.0),
            total_ms: Some(470.0),
            rtf: Some(0.084),
            segment_count: Some(3),
            process_rss_mb: Some(162.0),
        }),
        batch_inference: None,
    };

    let value = serde_json::to_value(snapshot).unwrap();

    assert_eq!(value["modelLoad"]["reusedFromPool"], true);
    assert_eq!(value["liveInference"]["isFinal"], true);
    assert_eq!(value["liveInference"]["bufferedSamples"], 80_000);
    assert!(value.get("model_load").is_none());
    assert!(value["liveInference"].get("buffered_samples").is_none());
}
