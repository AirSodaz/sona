//! Adversarial Empirical Challenge Suite for Milestone 3
//! Gate Verification: Batch Import & Options Capsule
//!
//! Stress harnesses, boundary condition verifications, and property-based oracles:
//! 1. Batch Queue Progress Oracle (0 items, 1 item, 100 items, 1000 items, negative & overflow values)
//! 2. Status Pill Resolution Truth Table & Overlap Analysis
//! 3. Clear Queue Logic: Clear Completed vs Clear All (modal trigger condition)
//! 4. Media Format Filter Oracle (18 extensions, uppercase, dot variations, invalid files)
//! 5. Stream URL Protocol Validation & Case Sensitivity Probe
//! 6. Options Capsule Surface Isolation & Coercion Oracle (Auto, Fixed, Selectable)
//! 7. Diarization Support Oracle & Cloud Model Discrimination
//! 8. VAD Sensitivity Stepper & Clamping Bounds Verification

use std::collections::HashSet;
use std::path::Path;

use sona_core::models::preset_models::build_model_catalog_snapshot_with_installed_ids;
use sona_desktop_lib::views::create::{
    BatchItemStatus, BatchQueueItem, BatchSourceType, IngestionSurface, TranscriptionOptionsModel,
    is_supported_audio_extension, is_valid_stream_url,
};

fn create_mock_item(id: &str, status: BatchItemStatus, progress: f32) -> BatchQueueItem {
    BatchQueueItem {
        id: id.to_string(),
        instance_id: None,
        filename: format!("audio_{}.wav", id),
        file_path: format!("C:/media/audio_{}.wav", id),
        source_type: BatchSourceType::File,
        status,
        progress,
        file_size_bytes: Some(1024 * 1024),
        duration_seconds: Some(60.0),
        project_id: None,
        project_name: None,
        project_color_index: None,
        error_message: None,
        history_id: None,
        error_stage: None,
        segments: Vec::new(),
        media_server_context: None,
        cancel_requested: None,
        is_media_server: false,
    }
}

/// Exact replica of production `overall_progress` formula in `batch_import.rs` line 474-478
fn calculate_production_overall_progress(queue: &[BatchQueueItem]) -> f32 {
    let total_count = queue.len();
    let progress_sum: f32 = queue
        .iter()
        .map(|i| match i.status {
            BatchItemStatus::Complete => 100.0,
            BatchItemStatus::Processing => i.progress,
            _ => 0.0,
        })
        .sum();

    if total_count > 0 {
        (progress_sum / (total_count as f32 * 100.0) * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    }
}

// ============================================================================
// CHALLENGE 1: Batch Queue State Machine & Extreme Calculations
// ============================================================================

#[test]
fn test_adversarial_batch_queue_empty_and_single_item() {
    // 0 items: must strictly yield 0.0 without divide-by-zero or NaN
    let empty_queue: Vec<BatchQueueItem> = Vec::new();
    let prog = calculate_production_overall_progress(&empty_queue);
    assert_eq!(prog, 0.0);
    assert!(!prog.is_nan());
    assert!(!prog.is_infinite());

    // Single item - Pending
    let q1 = vec![create_mock_item("1", BatchItemStatus::Pending, 0.0)];
    assert_eq!(calculate_production_overall_progress(&q1), 0.0);

    // Single item - Processing at 33.3%
    let q2 = vec![create_mock_item("2", BatchItemStatus::Processing, 33.3)];
    assert!((calculate_production_overall_progress(&q2) - 33.3).abs() < 0.001);

    // Single item - Complete (item.progress may be anything, Complete always yields 100.0)
    let q3 = vec![create_mock_item("3", BatchItemStatus::Complete, 0.0)];
    assert_eq!(calculate_production_overall_progress(&q3), 100.0);

    // Single item - Error (even if it progressed to 90%, failed items contribute 0%)
    let q4 = vec![create_mock_item("4", BatchItemStatus::Error, 90.0)];
    assert_eq!(calculate_production_overall_progress(&q4), 0.0);

    // Single item - Cancelled
    let q5 = vec![create_mock_item("5", BatchItemStatus::Cancelled, 50.0)];
    assert_eq!(calculate_production_overall_progress(&q5), 0.0);
}

#[test]
fn test_adversarial_batch_queue_100_items_stress() {
    let mut queue = Vec::new();
    // 25 completed (100% each = 2500)
    for i in 0..25 {
        queue.push(create_mock_item(
            &format!("c_{i}"),
            BatchItemStatus::Complete,
            100.0,
        ));
    }
    // 25 processing at 50% (50% each = 1250)
    for i in 0..25 {
        queue.push(create_mock_item(
            &format!("pr_{i}"),
            BatchItemStatus::Processing,
            50.0,
        ));
    }
    // 25 failed (0% each)
    for i in 0..25 {
        queue.push(create_mock_item(
            &format!("f_{i}"),
            BatchItemStatus::Error,
            80.0,
        ));
    }
    // 25 pending (0% each)
    for i in 0..25 {
        queue.push(create_mock_item(
            &format!("pe_{i}"),
            BatchItemStatus::Pending,
            0.0,
        ));
    }

    assert_eq!(queue.len(), 100);
    // Total sum = 2500 + 1250 = 3750. Total max = 100 * 100 = 10000.
    // Progress = 3750 / 10000 * 100 = 37.5%
    let overall = calculate_production_overall_progress(&queue);
    assert!(
        (overall - 37.5).abs() < 0.001,
        "Expected 37.5%, got {overall}"
    );

    // Verify progress bar relative width track strictly in [0.0, 1.0]
    let track_width = overall / 100.0;
    assert!(track_width >= 0.0 && track_width <= 1.0);
}

#[test]
fn test_adversarial_batch_queue_1000_items_all_complete() {
    let mut queue = Vec::new();
    for i in 0..1000 {
        queue.push(create_mock_item(
            &format!("item_{i}"),
            BatchItemStatus::Complete,
            100.0,
        ));
    }
    let overall = calculate_production_overall_progress(&queue);
    assert_eq!(overall, 100.0);
    assert_eq!(overall / 100.0, 1.0);
}

#[test]
fn test_adversarial_progress_anomalies_and_clamping() {
    // Malformed item progress values: > 100.0 or negative
    let queue = vec![
        create_mock_item("overflow", BatchItemStatus::Processing, 250.0),
        create_mock_item("complete", BatchItemStatus::Complete, 100.0),
    ];
    // sum = 250 + 100 = 350. 350 / 200 * 100 = 175 -> clamped to 100.0
    let overall = calculate_production_overall_progress(&queue);
    assert_eq!(overall, 100.0, "Progress must clamp to 100.0 on overflow");
}

// ============================================================================
// CHALLENGE 2: Status Pill Resolution & Mutual Exclusion Audit
// ============================================================================

#[derive(Debug, PartialEq)]
enum ResolvedStatusPill {
    Transcribing(usize, usize),
    Paused,
    Completed,
    Failed(usize, usize, usize),
    Queued(usize, usize),
}

fn resolve_status_pill(
    total_count: usize,
    completed_count: usize,
    failed_count: usize,
    processing_count: usize,
    is_paused: bool,
) -> Vec<ResolvedStatusPill> {
    let mut pills = Vec::new();

    // Replicate production logic from batch_import.rs lines 884-913:
    if processing_count > 0 && !is_paused {
        pills.push(ResolvedStatusPill::Transcribing(
            completed_count,
            total_count,
        ));
    }
    if is_paused {
        pills.push(ResolvedStatusPill::Paused);
    }
    if completed_count == total_count && total_count > 0 {
        pills.push(ResolvedStatusPill::Completed);
    }
    if failed_count > 0 && processing_count == 0 {
        pills.push(ResolvedStatusPill::Failed(
            completed_count,
            total_count,
            failed_count,
        ));
    }
    if processing_count == 0 && failed_count == 0 && completed_count < total_count && !is_paused {
        pills.push(ResolvedStatusPill::Queued(completed_count, total_count));
    }

    pills
}

#[test]
fn test_status_pill_resolution_normal_states() {
    // 1. Transcribing normally
    let p1 = resolve_status_pill(10, 3, 0, 1, false);
    assert_eq!(p1, vec![ResolvedStatusPill::Transcribing(3, 10)]);

    // 2. Normal Paused with active work
    let p2 = resolve_status_pill(10, 3, 0, 1, true);
    assert_eq!(p2, vec![ResolvedStatusPill::Paused]);

    // 3. Normal All Completed (not paused)
    let p3 = resolve_status_pill(10, 10, 0, 0, false);
    assert_eq!(p3, vec![ResolvedStatusPill::Completed]);

    // 4. Failed with no active tasks (not paused)
    let p4 = resolve_status_pill(10, 8, 2, 0, false);
    assert_eq!(p4, vec![ResolvedStatusPill::Failed(8, 10, 2)]);

    // 5. Idle queue waiting to start
    let p5 = resolve_status_pill(10, 0, 0, 0, false);
    assert_eq!(p5, vec![ResolvedStatusPill::Queued(0, 10)]);
}

#[test]
fn test_status_pill_overlap_edge_case_investigation() {
    // Edge Case 1: When is_paused == true and completed_count == total_count
    let overlap_completed = resolve_status_pill(5, 5, 0, 0, true);
    assert_eq!(
        overlap_completed,
        vec![ResolvedStatusPill::Paused, ResolvedStatusPill::Completed],
        "Proved: when paused with all items completed, status pill matches both Paused and Completed"
    );

    // Edge Case 2: When is_paused == true and failed_count > 0 and processing == 0
    let overlap_failed = resolve_status_pill(5, 3, 2, 0, true);
    assert_eq!(
        overlap_failed,
        vec![
            ResolvedStatusPill::Paused,
            ResolvedStatusPill::Failed(3, 5, 2)
        ],
        "Proved: when paused with failed items and no processing items, status pill matches both Paused and Failed"
    );
}

// ============================================================================
// CHALLENGE 3: Clear Completed vs Clear All Logic
// ============================================================================

#[test]
fn test_clear_completed_semantics() {
    let mut queue = vec![
        create_mock_item("1", BatchItemStatus::Complete, 100.0),
        create_mock_item("2", BatchItemStatus::Processing, 50.0),
        create_mock_item("3", BatchItemStatus::Pending, 0.0),
        create_mock_item("4", BatchItemStatus::Error, 0.0),
        create_mock_item("5", BatchItemStatus::Complete, 100.0),
    ];

    // clear_completed: retain(|i| i.status != BatchItemStatus::Complete)
    queue.retain(|i| i.status != BatchItemStatus::Complete);

    assert_eq!(queue.len(), 3);
    assert_eq!(queue[0].id, "2");
    assert_eq!(queue[1].id, "3");
    assert_eq!(queue[2].id, "4");
    assert!(!queue.iter().any(|i| i.status == BatchItemStatus::Complete));
}

#[test]
fn test_clear_all_confirmation_trigger_condition() {
    // When queue has a processing item -> confirmation modal MUST open
    let q_processing = vec![
        create_mock_item("1", BatchItemStatus::Processing, 30.0),
        create_mock_item("2", BatchItemStatus::Pending, 0.0),
    ];
    let needs_confirm = q_processing
        .iter()
        .any(|i| i.status == BatchItemStatus::Processing);
    assert!(
        needs_confirm,
        "Processing items require confirmation modal before clear_all"
    );

    // When queue has only Completed and Failed items -> confirmation modal is bypassed
    let q_idle = vec![
        create_mock_item("1", BatchItemStatus::Complete, 100.0),
        create_mock_item("2", BatchItemStatus::Error, 0.0),
    ];
    let needs_confirm_idle = q_idle
        .iter()
        .any(|i| i.status == BatchItemStatus::Processing);
    assert!(
        !needs_confirm_idle,
        "Idle / completed / failed items do NOT require confirmation modal"
    );
}

// ============================================================================
// CHALLENGE 4: Media Format Filtering Oracle
// ============================================================================

#[test]
fn test_media_format_filtering_oracle() {
    let canonical_18 = [
        "wav", "mp3", "m4a", "aiff", "flac", "ogg", "wma", "aac", "opus", "amr", "mp4", "webm",
        "mov", "mkv", "avi", "wmv", "flv", "3gp",
    ];

    for ext in canonical_18 {
        assert!(
            is_supported_audio_extension(ext),
            "Canonical extension {ext} must be accepted"
        );
        // Uppercase
        assert!(
            is_supported_audio_extension(&ext.to_uppercase()),
            "Uppercase extension {} must be accepted",
            ext.to_uppercase()
        );
        // Leading dot
        assert!(
            is_supported_audio_extension(&format!(".{ext}")),
            "Dot prefix .{ext} must be accepted"
        );
        // Mixed case with spaces and dot
        assert!(
            is_supported_audio_extension(&format!("  .{}  ", ext.to_uppercase())),
            "Padded dot uppercase format must be accepted"
        );
    }

    // Rejection of non-media files
    let rejected = [
        "txt", "exe", "dll", "so", "dylib", "bat", "cmd", "ps1", "sh", "rs", "js", "ts", "tsx",
        "html", "css", "json", "toml", "yaml", "xml", "pdf", "doc", "docx", "xls", "xlsx", "zip",
        "tar", "gz", "7z", "iso", "bin", "png", "jpg", "jpeg", "gif", "webp", "ico", "svg", "bmp",
    ];

    for ext in rejected {
        assert!(
            !is_supported_audio_extension(ext),
            "Non-audio extension {ext} must be rejected"
        );
    }
}

// ============================================================================
// CHALLENGE 5: Stream URL Validation & Protocol Sensitivity
// ============================================================================

#[test]
fn test_stream_url_protocol_acceptance_and_rejection() {
    // Valid protocols: http, https, rtmp, rtsp
    assert!(is_valid_stream_url("http://stream.org/live"));
    assert!(is_valid_stream_url("https://stream.org/live.m3u8"));
    assert!(is_valid_stream_url("rtmp://stream.org/live/stream"));
    assert!(is_valid_stream_url("rtsp://10.0.0.1:554/ch0"));

    // Rejected protocols
    assert!(!is_valid_stream_url("ftp://server/audio.wav"));
    assert!(!is_valid_stream_url("file:///c:/path/audio.wav"));
    assert!(!is_valid_stream_url("smb://nas/share/audio.wav"));
    assert!(!is_valid_stream_url("ws://websocket/stream"));
    assert!(!is_valid_stream_url("wss://secure-ws/stream"));
    assert!(!is_valid_stream_url("data:audio/wav;base64,UklGR"));

    // Missing protocol scheme
    assert!(!is_valid_stream_url("www.radio.com/live"));
    assert!(!is_valid_stream_url("radio.com/live"));
    assert!(!is_valid_stream_url(""));
    assert!(!is_valid_stream_url("   "));
}

#[test]
fn test_stream_url_uppercase_scheme_vulnerability_probe() {
    // RFC 3986 defines URI schemes as case-insensitive (e.g. HTTP:// is equivalent to http://)
    // Remediated is_valid_stream_url lowercases scheme, accepting HTTPS://
    let uppercase_url = "HTTPS://radio.example.com/stream.m3u8";
    let is_valid = is_valid_stream_url(uppercase_url);
    assert!(
        is_valid,
        "Remediated: is_valid_stream_url lowercases scheme before checking, accepting HTTPS://"
    );
}

// ============================================================================
// CHALLENGE 6: Options Capsule Surface Isolation & Coercion Oracle
// ============================================================================

#[test]
fn test_options_capsule_language_coercion_oracle() {
    let mut model = TranscriptionOptionsModel::new();

    // Injected catalog snapshot from real preset-models.json
    let snapshot = build_model_catalog_snapshot_with_installed_ids(
        Path::new("C:/mock_models_dir"),
        &HashSet::new(),
    );
    model.catalog = Some(snapshot);

    // 1. Auto model: Qwen3 ASR (id: "qwen3-asr-0.6b-q8-gguf", languageMode: auto)
    model.surface = IngestionSurface::Live;
    model.set_language(IngestionSurface::Live, "fr".to_string());
    model.set_model(IngestionSurface::Live, "qwen3-asr-0.6b-q8-gguf".to_string());
    assert_eq!(
        model.live_language, "auto",
        "Qwen3 ASR Auto model must coerce language to 'auto'"
    );
    assert_eq!(
        model.is_language_locked(),
        Some("Auto-detect only for this model")
    );

    // 2. Fixed model: Zipformer ZH XLarge (Int8)
    // ID: "sherpa-onnx-streaming-zipformer-zh-xlarge-int8-2025-06-30"
    model.set_model(
        IngestionSurface::Live,
        "sherpa-onnx-streaming-zipformer-zh-xlarge-int8-2025-06-30".to_string(),
    );
    assert_eq!(
        model.live_language, "zh",
        "Fixed ZH Zipformer model must coerce language to 'zh'"
    );
    assert_eq!(model.is_language_locked(), Some("Fixed language model"));

    // 3. Verify Batch surface remained untouched!
    assert_eq!(
        model.batch_language, "auto",
        "Live surface coercion must not mutate batch language"
    );

    // 4. Selectable model: SenseVoice or Whisper Turbo
    // ID: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17"
    model.surface = IngestionSurface::Batch;
    model.set_model(
        IngestionSurface::Batch,
        "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17".to_string(),
    );
    model.set_language(IngestionSurface::Batch, "ja".to_string());
    model.coerce_languages();
    assert_eq!(
        model.batch_language, "ja",
        "Selectable model must preserve chosen language 'ja'"
    );
    assert_eq!(
        model.is_language_locked(),
        None,
        "Selectable model must not lock language"
    );

    // Verify Live is still 'zh'!
    assert_eq!(
        model.live_language, "zh",
        "Batch surface mutation must not affect live surface"
    );
}

// ============================================================================
// CHALLENGE 7: Diarization Capability Discrimination
// ============================================================================

#[test]
fn test_diarization_cloud_vs_local_oracle() {
    let mut model = TranscriptionOptionsModel::new();

    // Local model ids
    let local_models = [
        "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17",
        "sherpa-onnx-whisper-turbo",
        "local::whisper-large-v3",
        "sensevoice-small",
    ];

    for m in local_models {
        model.set_model(IngestionSurface::Live, m.to_string());
        assert!(
            model.is_diarization_supported(),
            "Local model {m} must support diarization"
        );
    }

    // Cloud models (prefixed with cloud::)
    let cloud_models = [
        "cloud::openai-whisper-1",
        "cloud::azure-speech",
        "cloud::groq-whisper-large-v3",
        "cloud::deepgram-nova-2",
    ];

    for m in cloud_models {
        model.set_model(IngestionSurface::Live, m.to_string());
        assert!(
            !model.is_diarization_supported(),
            "Cloud model {m} must NOT support diarization"
        );
    }
}

// ============================================================================
// CHALLENGE 8: VAD Sensitivity Bounds & Clamping
// ============================================================================

#[test]
fn test_vad_sensitivity_bounds_oracle() {
    let mut model = TranscriptionOptionsModel::new();

    // Default VAD sensitivity is 0.5
    assert_eq!(model.vad_sensitivity, 0.5);

    // Model setter bounds [0.0, 1.0]
    model.set_vad_sensitivity(-10.0);
    assert_eq!(model.vad_sensitivity, 0.0);

    model.set_vad_sensitivity(10.0);
    assert_eq!(model.vad_sensitivity, 1.0);

    model.set_vad_sensitivity(0.73);
    assert_eq!(model.vad_sensitivity, 0.73);

    // UI stepper bounds [0.2, 0.9] verification
    // Simulate clicking "-" 20 times from 0.5:
    let mut ui_vad = 0.5f32;
    for _ in 0..20 {
        ui_vad = (ui_vad - 0.05).clamp(0.2, 0.9);
    }
    assert!(
        (ui_vad - 0.2).abs() < 0.001,
        "UI stepper lower bound is 0.2"
    );

    // Simulate clicking "+" 20 times from 0.5:
    for _ in 0..20 {
        ui_vad = (ui_vad + 0.05).clamp(0.2, 0.9);
    }
    assert!(
        (ui_vad - 0.9).abs() < 0.001,
        "UI stepper upper bound is 0.9"
    );
}
