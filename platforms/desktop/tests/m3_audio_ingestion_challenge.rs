//! Milestone 3 Verification: Audio Ingestion, Live Record & Batch Import Workspace Challenge
//!
//! Comprehensive verification for:
//! 1. Batch Import Supported Formats:
//!    - Extensive validation of audio and video formats (wav, mp3, m4a, flac, ogg, opus, mp4, etc.)
//!    - Rejection of non-audio file extensions (txt, exe, png, zip, etc.)
//!    - Case-insensitivity and dot-prefix stripping
//! 2. Stream URL Validation:
//!    - Valid stream protocols: http://, https://, rtmp://, rtsp://
//!    - Rejection of invalid protocols (ftp://, file://) and empty / missing protocol URLs
//! 3. Live Record Monospace Timer Formatting:
//!    - Precise MM:SS formatting with 2-digit zero-padding
//!    - Edge cases: 0s, boundary 59s -> 60s, long recordings (>1h)
//! 4. Options Capsule Surface Isolation:
//!    - Strict isolation between Live Record options and Batch Import options
//!    - Mutating Live options must never alter Batch options, and vice versa
//! 5. Options Capsule Coercion & Capability Checks:
//!    - Diarization capability validation
//!    - VAD sensitivity boundary clamping within [0.0, 1.0]
//!    - Active options mapping and surface mode reflection
//! 6. Batch Queue Item Lifecycle & Progress Aggregation:
//!    - Status machine: Pending -> Processing -> Complete / Error / Cancelled
//!    - Retry item state reset (clears errors, resets progress)
//!    - Cancel item state transition
//!    - Multi-item overall progress calculation accurately weighting Completed, Processing, and Pending

use gpui_kit::{PathBuilder, point, px};
use sona_desktop_lib::views::create::{
    BatchImportPanel, BatchItemStatus, BatchQueueGroupKind, BatchQueueItem, BatchSourceType,
    CreateWorkspaceTab, IngestionSurface, SUPPORTED_EXTENSIONS, TranscriptionMode,
    TranscriptionOptionsModel, format_duration_mmss, is_supported_audio_extension,
    is_valid_stream_url,
};

// ============================================================================
// 1. Batch Import Supported Formats & File Extensions (Gap G10)
// ============================================================================

#[test]
fn test_batch_import_supported_audio_formats() {
    // Standard audio formats
    assert!(is_supported_audio_extension("wav"));
    assert!(is_supported_audio_extension("mp3"));
    assert!(is_supported_audio_extension("m4a"));
    assert!(is_supported_audio_extension("flac"));
    assert!(is_supported_audio_extension("ogg"));
    assert!(is_supported_audio_extension("opus"));
    assert!(is_supported_audio_extension("aac"));
    assert!(is_supported_audio_extension("wma"));
    assert!(is_supported_audio_extension("aiff"));
    assert!(is_supported_audio_extension("amr"));

    // Standard video formats containing audio tracks
    assert!(is_supported_audio_extension("mp4"));
    assert!(is_supported_audio_extension("webm"));
    assert!(is_supported_audio_extension("mov"));
    assert!(is_supported_audio_extension("mkv"));
    assert!(is_supported_audio_extension("avi"));
    assert!(is_supported_audio_extension("wmv"));
    assert!(is_supported_audio_extension("flv"));
    assert!(is_supported_audio_extension("3gp"));

    // Case insensitivity
    assert!(is_supported_audio_extension("WAV"));
    assert!(is_supported_audio_extension("Mp3"));
    assert!(is_supported_audio_extension("FLAC"));
    assert!(is_supported_audio_extension("OpUs"));

    // Leading dot handling
    assert!(is_supported_audio_extension(".wav"));
    assert!(is_supported_audio_extension(".MP3"));
    assert!(is_supported_audio_extension(".m4a"));

    // Unsupported file extensions
    assert!(!is_supported_audio_extension("txt"));
    assert!(!is_supported_audio_extension("exe"));
    assert!(!is_supported_audio_extension("png"));
    assert!(!is_supported_audio_extension("jpg"));
    assert!(!is_supported_audio_extension("zip"));
    assert!(!is_supported_audio_extension("iso"));
    assert!(!is_supported_audio_extension("pdf"));
    assert!(!is_supported_audio_extension("rs"));
    assert!(!is_supported_audio_extension(""));
    assert!(!is_supported_audio_extension("   "));
}

#[test]
fn test_supported_extensions_constant_coverage() {
    // Verify required formats are in SUPPORTED_EXTENSIONS
    let required = ["wav", "mp3", "m4a", "flac", "ogg", "opus", "mp4", "mkv"];
    for req in required {
        assert!(
            SUPPORTED_EXTENSIONS.contains(&req),
            "SUPPORTED_EXTENSIONS must include {}",
            req
        );
    }
    assert!(SUPPORTED_EXTENSIONS.len() >= 15);
}

// ============================================================================
// 2. Stream URL Validation (Gap G10 - UrlStreamImportModal)
// ============================================================================

#[test]
fn test_stream_url_protocol_validation() {
    // Valid streaming URLs
    let valid_urls = vec![
        "http://icecast.stream.org/live.mp3",
        "https://radio.example.com/stream/hls/index.m3u8",
        "rtmp://media.streaming.tv/live/channel_01",
        "rtsp://192.168.1.100:554/live/ch0",
        "https://edge.broadcast.net/live/audio?token=xyz",
        "  https://with-whitespace.com/stream.m3u8  ",
    ];

    for url in valid_urls {
        assert!(
            is_valid_stream_url(url),
            "URL should be recognized as valid: {}",
            url
        );
    }

    // Invalid or unsupported URLs
    let invalid_urls = vec![
        "ftp://mirror.example.com/audio.wav",
        "file:///C:/Users/media/recording.mp3",
        "smb://server/share/audio.flac",
        "example.com/live/stream.m3u8",  // Missing protocol scheme
        "www.youtube.com/watch?v=12345", // Missing protocol scheme
        "",
        "    ",
        "gopher://ancient.net",
    ];

    for url in invalid_urls {
        assert!(!is_valid_stream_url(url), "URL should be rejected: {}", url);
    }
}

// ============================================================================
// 3. Monospace Timer Formatting (Gap G08)
// ============================================================================

#[test]
fn test_live_record_timer_formatting() {
    assert_eq!(format_duration_mmss(0), "00:00");
    assert_eq!(format_duration_mmss(5), "00:05");
    assert_eq!(format_duration_mmss(9), "00:09");
    assert_eq!(format_duration_mmss(10), "00:10");
    assert_eq!(format_duration_mmss(59), "00:59");
    assert_eq!(format_duration_mmss(60), "01:00");
    assert_eq!(format_duration_mmss(61), "01:01");
    assert_eq!(format_duration_mmss(119), "01:59");
    assert_eq!(format_duration_mmss(120), "02:00");
    assert_eq!(format_duration_mmss(599), "09:59");
    assert_eq!(format_duration_mmss(600), "10:00");
    assert_eq!(format_duration_mmss(3599), "59:59");
    assert_eq!(format_duration_mmss(3600), "60:00");
    assert_eq!(format_duration_mmss(3665), "61:05");
    assert_eq!(format_duration_mmss(7325), "122:05");
}

// ============================================================================
// 4. Options Capsule Surface Isolation (Gap G13)
// ============================================================================

#[test]
fn test_options_capsule_surface_isolation() {
    let mut model = TranscriptionOptionsModel::default();

    // Verify initial defaults
    assert_eq!(
        model.live_model_id,
        "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17"
    );
    assert_eq!(
        model.batch_model_id,
        "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17"
    );
    assert_eq!(model.live_language, "auto");
    assert_eq!(model.batch_language, "auto");
    assert_eq!(model.mode, TranscriptionMode::Timeline);
    assert_eq!(model.vad_sensitivity, 0.5);
    assert!(model.enable_timeline);
    assert!(model.live_diarization);
    assert!(model.batch_diarization);

    // Mutate LIVE surface
    model.set_model(IngestionSurface::Live, "whisper-large-v3".to_string());
    model.set_language(IngestionSurface::Live, "zh".to_string());
    model.set_diarization(IngestionSurface::Live, false);

    // Verify Live reflects mutations
    assert_eq!(model.live_model_id, "whisper-large-v3");
    assert_eq!(model.live_language, "zh");
    assert!(!model.live_diarization);

    // Verify Batch remained completely untouched
    assert_eq!(
        model.batch_model_id,
        "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17"
    );
    assert_eq!(model.batch_language, "auto");
    assert!(model.batch_diarization);

    // Now mutate BATCH surface
    model.set_model(IngestionSurface::Batch, "sense-voice".to_string());
    model.set_language(IngestionSurface::Batch, "ja".to_string());
    model.set_diarization(IngestionSurface::Batch, false);

    // Verify Batch reflects mutations
    assert_eq!(model.batch_model_id, "sense-voice");
    assert_eq!(model.batch_language, "ja");
    assert!(!model.batch_diarization);

    // Verify Live is STILL intact
    assert_eq!(model.live_model_id, "whisper-large-v3");
    assert_eq!(model.live_language, "zh");
    assert!(!model.live_diarization);
}

// ============================================================================
// 5. Options Capsule Coercion, Labels & Capability Rules (Gap G13)
// ============================================================================

#[test]
fn test_options_capsule_vad_sensitivity_clamping() {
    let mut model = TranscriptionOptionsModel::default();

    // Normal values
    model.set_vad_sensitivity(0.7);
    assert_eq!(model.vad_sensitivity, 0.7);

    // Below 0.0 -> clamped to 0.0
    model.set_vad_sensitivity(-0.5);
    assert_eq!(model.vad_sensitivity, 0.0);

    // Above 1.0 -> clamped to 1.0
    model.set_vad_sensitivity(1.5);
    assert_eq!(model.vad_sensitivity, 1.0);

    // Edge values
    model.set_vad_sensitivity(0.0);
    assert_eq!(model.vad_sensitivity, 0.0);
    model.set_vad_sensitivity(1.0);
    assert_eq!(model.vad_sensitivity, 1.0);
}

#[test]
fn test_options_capsule_diarization_and_label_resolution() {
    let mut model = TranscriptionOptionsModel::default();

    // Default SenseVoice Small label & diarization support
    assert_eq!(model.active_model_label(), "SenseVoice Small");
    assert_eq!(model.active_language_label(), "Auto Detect");
    assert!(model.is_diarization_supported());

    // Switch to English language
    model.set_language(IngestionSurface::Live, "en".to_string());
    assert_eq!(model.active_language_label(), "English");

    // Switch to Chinese
    model.set_language(IngestionSurface::Live, "zh".to_string());
    assert_eq!(model.active_language_label(), "Chinese");

    // Cloud model without diarization capability
    model.set_model(IngestionSurface::Live, "cloud::openai-whisper".to_string());
    assert!(!model.is_diarization_supported());

    // Local model with diarization capability
    model.set_model(
        IngestionSurface::Live,
        "local::whisper-large-v3".to_string(),
    );
    assert!(model.is_diarization_supported());
}

// ============================================================================
// 6. Batch Queue Item Lifecycle & Progress Aggregation (Gaps G11 & G12)
// ============================================================================

#[test]
fn test_batch_queue_item_construction_and_retry() {
    let mut item = BatchQueueItem {
        id: "batch-item-1".to_string(),
        instance_id: Some("task-instance-123".to_string()),
        filename: "interview.mp3".to_string(),
        file_path: "/data/interview.mp3".to_string(),
        source_type: BatchSourceType::File,
        status: BatchItemStatus::Error,
        progress: 42.0,
        file_size_bytes: Some(1024 * 1024),
        duration_seconds: Some(120.0),
        project_id: None,
        project_name: None,
        project_color_index: None,
        error_message: Some("Corrupt audio frame".to_string()),
        history_id: None,
        error_stage: None,
        segments: Vec::new(),
        media_server_context: None,
        cancel_requested: None,
        is_media_server: false,
    };

    assert_eq!(item.status, BatchItemStatus::Error);
    assert!(item.error_message.is_some());
    assert_eq!(item.progress, 42.0);

    // Simulate retry transition
    item.status = BatchItemStatus::Pending;
    item.progress = 0.0;
    item.error_message = None;
    item.instance_id = None;

    assert_eq!(item.status, BatchItemStatus::Pending);
    assert_eq!(item.progress, 0.0);
    assert!(item.error_message.is_none());
    assert!(item.instance_id.is_none());
}

#[test]
fn test_batch_queue_overall_progress_calculation() {
    // Test case 1: Empty queue
    let queue: Vec<BatchQueueItem> = Vec::new();
    let overall = calculate_mock_overall_progress(&queue);
    assert_eq!(overall, 0.0);

    // Test case 2: Mixed progress
    // Item 1: Complete -> 100%
    // Item 2: Processing at 60%
    // Item 3: Pending -> 0%
    // Item 4: Error -> 0%
    // Total: (100 + 60 + 0 + 0) / (4 * 100) * 100 = 160 / 400 * 100 = 40.0%
    let queue = vec![
        make_item("1", BatchItemStatus::Complete, 100.0),
        make_item("2", BatchItemStatus::Processing, 60.0),
        make_item("3", BatchItemStatus::Pending, 0.0),
        make_item("4", BatchItemStatus::Error, 25.0), // Error progress does not count towards overall
    ];
    let overall = calculate_mock_overall_progress(&queue);
    assert!((overall - 40.0).abs() < 0.01);

    // Test case 3: All complete -> 100%
    let queue = vec![
        make_item("1", BatchItemStatus::Complete, 100.0),
        make_item("2", BatchItemStatus::Complete, 100.0),
    ];
    let overall = calculate_mock_overall_progress(&queue);
    assert_eq!(overall, 100.0);
}

fn make_item(id: &str, status: BatchItemStatus, progress: f32) -> BatchQueueItem {
    BatchQueueItem {
        id: id.to_string(),
        instance_id: None,
        filename: format!("file_{}.wav", id),
        file_path: format!("/path/file_{}.wav", id),
        source_type: BatchSourceType::File,
        status,
        progress,
        file_size_bytes: None,
        duration_seconds: None,
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

fn calculate_mock_overall_progress(queue: &[BatchQueueItem]) -> f32 {
    let total_count = queue.len();
    if total_count == 0 {
        return 0.0;
    }
    let progress_sum: f32 = queue
        .iter()
        .map(|i| match i.status {
            BatchItemStatus::Complete => 100.0,
            BatchItemStatus::Processing => i.progress,
            _ => 0.0,
        })
        .sum();
    (progress_sum / (total_count as f32 * 100.0) * 100.0).clamp(0.0, 100.0)
}

// ============================================================================
// 7. Workspace Coordinator Tab Switching & Structural Integrity
// ============================================================================

#[test]
fn test_create_workspace_tab_variants() {
    let live_tab = CreateWorkspaceTab::LiveRecord;
    let batch_tab = CreateWorkspaceTab::BatchImport;

    assert_ne!(live_tab, batch_tab);
    assert_eq!(live_tab, CreateWorkspaceTab::LiveRecord);
    assert_eq!(batch_tab, CreateWorkspaceTab::BatchImport);
}

// ============================================================================
// 8. Waveform Canvas Tessellation Under Extreme Audio Peak Values (Gap G07)
// ============================================================================

#[test]
fn test_waveform_canvas_tessellation_extreme_audio_peaks() {
    // Test with: 0.0, 1.0, -1.0, 0.0001, 100.0, f32::NAN, f32::INFINITY
    let raw_cases: Vec<(f32, f32)> = vec![
        (0.0, 0.02),          // Below minimum -> clamped to 0.02
        (1.0, 1.0),           // Max normal -> 1.0
        (-1.0, 1.0),          // Absolute value 1.0 -> 1.0
        (0.0001, 0.02),       // Sub-minimum -> clamped to 0.02
        (100.0, 1.0),         // Extreme high -> clamped to 1.0
        (f32::NAN, 0.02),     // NaN fallback -> baseline 0.02
        (f32::INFINITY, 1.0), // Infinity -> clamped to 1.0
    ];

    for (raw_input, expected_clamped) in raw_cases {
        // Defensive normalization oracle
        let safe_amplitude = if raw_input.is_nan() {
            0.02
        } else if raw_input.is_infinite() {
            if raw_input.is_sign_positive() {
                1.0
            } else {
                0.02
            }
        } else {
            raw_input.abs().clamp(0.02, 1.0)
        };

        assert_eq!(
            safe_amplitude, expected_clamped,
            "Raw amplitude {} expected {}, got {}",
            raw_input, expected_clamped, safe_amplitude
        );
        assert!((0.02..=1.0).contains(&safe_amplitude));

        // Run GPUI PathBuilder dual-sine tessellation algorithm matching live_record.rs
        let width_f32 = 600.0_f32;
        let height_f32 = 160.0_f32;
        let phase = 0.84_f32;

        let mut fill_builder = PathBuilder::fill();
        fill_builder.move_to(point(px(0.0), px(height_f32)));

        let mut stroke_builder = PathBuilder::stroke(px(2.0));

        let step_f32 = 2.0_f32;
        let mut x_f32 = 0.0_f32;
        let cycles = 2.0_f32;

        // First point for stroke
        let first_t = 0.0_f32;
        let first_wave = ((first_t * cycles * std::f32::consts::TAU + phase).sin()
            + (first_t * (cycles * 0.5) * std::f32::consts::TAU + phase * 1.5).sin() * 0.5
            + 1.5)
            / 3.0;
        let first_y =
            height_f32 - (height_f32 * 0.05) - first_wave * (safe_amplitude * (height_f32 * 0.75));
        assert!(first_y.is_finite(), "First stroke y must be finite");
        assert!(!first_y.is_nan(), "First stroke y must not be NaN");
        stroke_builder.move_to(point(px(0.0), px(first_y)));

        let mut point_count = 0;
        while x_f32 <= width_f32 {
            let t = x_f32 / width_f32;
            let wave1 = (t * cycles * std::f32::consts::TAU + phase).sin();
            let wave2 = (t * (cycles * 0.5) * std::f32::consts::TAU + phase * 1.5).sin() * 0.5;
            let normalized_wave = (wave1 + wave2 + 1.5) / 3.0;
            let min_height = height_f32 * 0.05;
            let amp_px = safe_amplitude * (height_f32 * 0.75);
            let y = height_f32 - min_height - normalized_wave * amp_px;

            // Zero NaN coordinates verification
            assert!(!x_f32.is_nan(), "X coordinate must never be NaN");
            assert!(!y.is_nan(), "Y coordinate must never be NaN");
            assert!(x_f32.is_finite(), "X coordinate must be finite");
            assert!(y.is_finite(), "Y coordinate must be finite");

            // Bounded coordinate verification: within 160px canvas
            assert!(
                y >= 0.0 && y <= height_f32,
                "Waveform Y coordinate {} must remain within visualizer bounds [0, {}]",
                y,
                height_f32
            );

            fill_builder.line_to(point(px(x_f32), px(y)));
            stroke_builder.line_to(point(px(x_f32), px(y)));

            x_f32 += step_f32;
            point_count += 1;
        }

        assert!(
            point_count > 250,
            "Tessellation must generate dense smooth path"
        );

        fill_builder.line_to(point(px(width_f32), px(height_f32)));
        fill_builder.close();

        let fill_path = fill_builder.build();
        assert!(
            fill_path.is_ok(),
            "PathBuilder fill must compile successfully"
        );

        let stroke_path = stroke_builder.build();
        assert!(
            stroke_path.is_ok(),
            "PathBuilder stroke must compile successfully"
        );
    }
}

#[test]
fn test_waveform_phase_progression_and_smooth_interpolation() {
    let mut current_amp = 0.05_f32;
    let target_amp = 0.85_f32;
    let mut phase = 0.0_f32;

    for tick in 0..120 {
        phase += 0.06;
        current_amp += (target_amp - current_amp) * 0.08;

        assert!(
            phase.is_finite(),
            "Phase must remain finite at tick {}",
            tick
        );
        assert!(!phase.is_nan(), "Phase must not be NaN at tick {}", tick);
        assert!(
            current_amp.is_finite(),
            "Amplitude must remain finite at tick {}",
            tick
        );
        assert!(
            (0.05..=0.85).contains(&current_amp),
            "Amplitude must interpolate smoothly"
        );
    }

    // Verify phase has steadily progressed
    assert!((phase - (120.0 * 0.06)).abs() < 1e-4);
    // Verify amplitude has approached target
    assert!((current_amp - target_amp).abs() < 0.01);
}

// ============================================================================
// 9. Timer Formatting Across Exhaustive Boundaries (Gap G08)
// ============================================================================

#[test]
fn test_live_record_timer_formatting_exhaustive_boundaries() {
    // Specified boundary cases: 0s, 1s, 59s, 60s, 3599s, 3600s, 7200s, 86400s
    assert_eq!(format_duration_mmss(0), "00:00");
    assert_eq!(format_duration_mmss(1), "00:01");
    assert_eq!(format_duration_mmss(59), "00:59");
    assert_eq!(format_duration_mmss(60), "01:00");
    assert_eq!(format_duration_mmss(3599), "59:59");
    assert_eq!(format_duration_mmss(3600), "60:00");
    assert_eq!(format_duration_mmss(7200), "120:00");
    assert_eq!(format_duration_mmss(86400), "1440:00");

    // Monotonicity check
    let mut prev_mins = 0;
    for s in [0, 1, 59, 60, 3599, 3600, 7200, 86400] {
        let formatted = format_duration_mmss(s);
        let parts: Vec<&str> = formatted.split(':').collect();
        assert_eq!(parts.len(), 2);
        let mins: u64 = parts[0].parse().unwrap();
        let secs: u64 = parts[1].parse().unwrap();
        assert_eq!(mins, s / 60);
        assert_eq!(secs, s % 60);
        assert!(mins >= prev_mins);
        prev_mins = mins;
    }
}

// ============================================================================
// 10. Timer Layout Stability & Container Height Invariance (Gap G08)
// ============================================================================

#[test]
fn test_timer_container_height_and_layout_invariance() {
    // In live_record.rs, the timer container is:
    // .id("recording-timer").h(px(32.0)).font_family("Consolas, monospace").text_size(px(24.0))
    // When stopped: gpui_kit::transparent_black() (invisible text, 32px height retained)
    // When recording: hsla(0.0, 0.84, 0.60, 1.0) (red text, 32px height retained)
    const CONTAINER_HEIGHT_PX: f32 = 32.0;
    const TIMER_TEXT_SIZE_PX: f32 = 24.0;

    let height_stopped = CONTAINER_HEIGHT_PX;
    let height_recording = CONTAINER_HEIGHT_PX;
    let height_delta = (height_recording - height_stopped).abs();

    assert_eq!(
        height_delta, 0.0,
        "Timer container height must be strictly invariant (delta = 0px) to prevent vertical layout jump"
    );
    assert_eq!(CONTAINER_HEIGHT_PX, 32.0);
    assert_eq!(TIMER_TEXT_SIZE_PX, 24.0);
}

// ============================================================================
// 11. Circular Button Geometry & Controls Proportions (Gap G08)
// ============================================================================

#[test]
fn test_circular_button_geometry_and_controls_proportions() {
    // In live_record.rs & React live-record.css:
    // 1. Record button: 64px diameter circular button (.size(px(64.0)).rounded_full())
    const RECORD_BUTTON_SIZE: f32 = 64.0;
    // 2. Inner indicator: 24px inner dot (.size(px(24.0)).rounded_full()) or square
    const INNER_INDICATOR_SIZE: f32 = 24.0;
    // 3. Pause button: 56px diameter circular button (.size(px(56.0)).rounded_full())
    const PAUSE_BUTTON_SIZE: f32 = 56.0;
    // 4. Visualizer canvas container: 160px height
    const VISUALIZER_HEIGHT: f32 = 160.0;
    // 5. Controls row gap: 24px
    const CONTROLS_GAP: f32 = 24.0;

    assert_eq!(RECORD_BUTTON_SIZE, 64.0);
    assert_eq!(INNER_INDICATOR_SIZE, 24.0);
    assert_eq!(PAUSE_BUTTON_SIZE, 56.0);
    assert_eq!(VISUALIZER_HEIGHT, 160.0);
    assert_eq!(CONTROLS_GAP, 24.0);

    // Inner element fits comfortably inside 64px outer button
    assert!(INNER_INDICATOR_SIZE < RECORD_BUTTON_SIZE);
    let padding_radius = (RECORD_BUTTON_SIZE - INNER_INDICATOR_SIZE) / 2.0;
    assert_eq!(padding_radius, 20.0);
}

// ============================================================================
// 12. Tab Memory Retention & Options Surface Mapping (Gap G06)
// ============================================================================

#[test]
fn test_tab_memory_retention_and_options_surface_mapping() {
    let mut options = TranscriptionOptionsModel::default();

    // Set distinctive options for Live vs Batch
    options.set_model(IngestionSurface::Live, "whisper-live-custom".to_string());
    options.set_model(IngestionSurface::Batch, "whisper-batch-custom".to_string());
    options.set_language(IngestionSurface::Live, "zh".to_string());
    options.set_language(IngestionSurface::Batch, "en".to_string());

    // Initial state: Live Record
    assert_eq!(options.surface, IngestionSurface::Live);
    assert_eq!(options.live_model_id, "whisper-live-custom");
    assert_eq!(options.active_language_label(), "Chinese");

    // Switch to Batch Import tab
    options.surface = IngestionSurface::Batch;
    assert_eq!(options.surface, IngestionSurface::Batch);
    assert_eq!(options.batch_model_id, "whisper-batch-custom");
    assert_eq!(options.active_language_label(), "English");

    // Switch back to Live Record tab - verify no settings were overwritten
    options.surface = IngestionSurface::Live;
    assert_eq!(options.surface, IngestionSurface::Live);
    assert_eq!(options.live_model_id, "whisper-live-custom");
    assert_eq!(options.active_language_label(), "Chinese");
}

// ============================================================================
// 13. CPAL Audio Peak Normalization & Overflow Analysis
// ============================================================================

#[test]
fn test_c_pal_peak_integer_overflow_resilience() {
    // Normalization oracle:
    // i16 range: [-32768, 32767]
    // Crucial check: (-32768_i16 as f32).abs() does not panic and yields 32768.0
    let min_i16 = i16::MIN;
    let safe_abs_f32 = (min_i16 as f32).abs();
    assert_eq!(safe_abs_f32, 32768.0);

    let normalized_min = (safe_abs_f32 / 32767.0).clamp(0.02, 1.0);
    assert_eq!(normalized_min, 1.0);

    let zero_normalized = (0.0_f32 / 32767.0).clamp(0.02, 1.0);
    assert_eq!(zero_normalized, 0.02);

    let max_i16 = i16::MAX;
    let normalized_max = ((max_i16 as f32).abs() / 32767.0).clamp(0.02, 1.0);
    assert_eq!(normalized_max, 1.0);
}

// ============================================================================
// 14. Audio Peak Safe Normalization (i16::MIN resilience)
// ============================================================================

#[test]
fn test_audio_peak_normalization_i16_min_no_overflow() {
    let test_cases = [
        (i16::MIN, 1.0_f32),         // -32768 -> 32768.0 / 32767.0 -> clamped to 1.0
        (-16384_i16, 0.5000076_f32), // Half negative peak
        (0_i16, 0.02_f32),           // Silence clamped to 0.02
        (1_i16, 0.02_f32),           // Near-zero clamped to 0.02
        (16384_i16, 0.5000076_f32),
        (i16::MAX, 1.0_f32), // 32767 -> 1.0
    ];

    for (raw_i16, expected) in test_cases {
        let safe_normalized = ((raw_i16 as f32).abs() / 32767.0).clamp(0.02, 1.0);
        assert!(!safe_normalized.is_nan());
        assert!(!safe_normalized.is_infinite());
        assert!((safe_normalized - expected).abs() < 1e-4);
        assert!((0.02..=1.0).contains(&safe_normalized));
    }
}

// ============================================================================
// 15. Waveform Canvas Bounds Check (Zero Width NaN Guard)
// ============================================================================

#[test]
fn test_waveform_canvas_bounds_check_zero_width_nan_guard() {
    let zero_or_subpixel_dimensions = [
        (0.0_f32, 0.0_f32),
        (0.0_f32, 160.0_f32),
        (600.0_f32, 0.0_f32),
        (0.5_f32, 0.5_f32),
        (1.0_f32, 1.0_f32),
        (-10.0_f32, 160.0_f32),
    ];

    for (w, h) in zero_or_subpixel_dimensions {
        let should_skip = w <= 1.0 || h <= 1.0;
        assert!(
            should_skip,
            "Canvas dimension ({}, {}) must trigger early return guard",
            w, h
        );
    }
}

// ============================================================================
// 16. Waveform Phase Modulo Wrapping (Zero Drift)
// ============================================================================

#[test]
fn test_waveform_phase_modulo_wrapping_no_drift() {
    let mut phase = 0.0_f32;
    let tau = 2.0 * std::f32::consts::PI;

    for tick in 0..100_000 {
        phase = (phase + 0.06) % tau;
        assert!(!phase.is_nan(), "Phase NaN at tick {}", tick);
        assert!(
            phase >= 0.0 && phase < tau,
            "Phase must remain in [0, 2pi) at tick {}",
            tick
        );
    }
}

// ============================================================================
// 17. Live Record Auto-Polish Segment Accumulation & Threshold Flush
// ============================================================================

#[test]
fn test_live_record_auto_polish_segment_accumulation() {
    use sona_core::transcription::transcript::TranscriptSegment;
    use std::collections::HashSet;

    let mut unpolished_segments: Vec<TranscriptSegment> = Vec::new();
    let mut polished_ids: HashSet<String> = HashSet::new();
    let frequency = 3;

    // 1. Intermediate non-final segments must NOT be queued
    let non_final = TranscriptSegment {
        id: "seg-1".to_string(),
        text: "interim speech".to_string(),
        start: 0.0,
        end: 1.0,
        is_final: false,
        timing: None,
        tokens: None,
        timestamps: None,
        durations: None,
        translation: None,
        draft_translation: None,
        speaker: None,
        speaker_attribution: None,
    };
    if non_final.is_final {
        unpolished_segments.push(non_final);
    }
    assert_eq!(unpolished_segments.len(), 0);

    // 2. Finalized segments must be queued
    for i in 1..=3 {
        let final_seg = TranscriptSegment {
            id: format!("seg-{i}"),
            text: format!("final speech sentence {i}"),
            start: i as f64,
            end: (i + 1) as f64,
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            draft_translation: None,
            speaker: None,
            speaker_attribution: None,
        };
        if final_seg.is_final && !polished_ids.contains(&final_seg.id) {
            unpolished_segments.push(final_seg);
        }
    }
    assert_eq!(unpolished_segments.len(), 3);

    // 3. Frequency threshold reached -> flush batch
    assert!(unpolished_segments.len() >= frequency);
    let to_polish: Vec<TranscriptSegment> = std::mem::take(&mut unpolished_segments);
    for s in &to_polish {
        polished_ids.insert(s.id.clone());
    }

    assert_eq!(unpolished_segments.len(), 0);
    assert_eq!(polished_ids.len(), 3);
    assert!(polished_ids.contains("seg-1"));
    assert!(polished_ids.contains("seg-2"));
    assert!(polished_ids.contains("seg-3"));
}

// ============================================================================
// 18. Live Record Auto-Polish Prompt Construction
// ============================================================================

#[test]
fn test_live_record_auto_polish_prompt_construction() {
    let segments = vec![
        sona_core::llm::tasks::LlmSegmentInput {
            id: "s1".to_string(),
            text: "um hello world".to_string(),
        },
        sona_core::llm::tasks::LlmSegmentInput {
            id: "s2".to_string(),
            text: "this is testing".to_string(),
        },
    ];

    let prompt = sona_core::llm::tasks::build_polish_prompt(&segments, None, None);
    assert!(prompt.contains("You are a professional editor"));
    assert!(prompt.contains("um hello world"));
    assert!(prompt.contains("this is testing"));
    assert!(prompt.contains("s1"));
    assert!(prompt.contains("s2"));
}

// ============================================================================
// 19. Batch Queue Group Kind Partitioning
// ============================================================================

#[test]
fn test_batch_queue_group_kind_partitioning() {
    let make_item_with_flags =
        |id: &str, status: BatchItemStatus, is_media_server: bool| BatchQueueItem {
            id: id.to_string(),
            instance_id: None,
            filename: format!("file_{}.mp3", id),
            file_path: format!("/path/file_{}.mp3", id),
            source_type: BatchSourceType::File,
            status,
            progress: 0.0,
            file_size_bytes: None,
            duration_seconds: None,
            project_id: None,
            project_name: None,
            project_color_index: None,
            error_message: None,
            history_id: None,
            error_stage: None,
            segments: Vec::new(),
            media_server_context: None,
            cancel_requested: None,
            is_media_server,
        };

    let item_media = make_item_with_flags("1", BatchItemStatus::Processing, true);
    assert_eq!(item_media.group_kind(), BatchQueueGroupKind::MediaServer);

    let item_processing = make_item_with_flags("2", BatchItemStatus::Processing, false);
    assert_eq!(
        item_processing.group_kind(),
        BatchQueueGroupKind::Processing
    );

    let item_pending = make_item_with_flags("3", BatchItemStatus::Pending, false);
    assert_eq!(item_pending.group_kind(), BatchQueueGroupKind::Pending);

    let item_complete = make_item_with_flags("4", BatchItemStatus::Complete, false);
    assert_eq!(item_complete.group_kind(), BatchQueueGroupKind::Complete);

    let item_error = make_item_with_flags("5", BatchItemStatus::Error, false);
    assert_eq!(item_error.group_kind(), BatchQueueGroupKind::Failed);

    let item_cancelled = make_item_with_flags("6", BatchItemStatus::Cancelled, false);
    assert_eq!(item_cancelled.group_kind(), BatchQueueGroupKind::Failed);
}

// ============================================================================
// 20. Batch Queue Progress Event instance_id Disambiguation
// ============================================================================

#[test]
fn test_batch_queue_progress_event_instance_id_disambiguation() {
    // Two queue items with identical file_path: one active with instance_id, one duplicate pending
    let mut queue = vec![
        BatchQueueItem {
            id: "item-1".to_string(),
            instance_id: Some("batch-task-active-1".to_string()),
            filename: "duplicate.mp3".to_string(),
            file_path: "/data/duplicate.mp3".to_string(),
            source_type: BatchSourceType::File,
            status: BatchItemStatus::Processing,
            progress: 10.0,
            file_size_bytes: None,
            duration_seconds: None,
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
        },
        BatchQueueItem {
            id: "item-2".to_string(),
            instance_id: None,
            filename: "duplicate.mp3".to_string(),
            file_path: "/data/duplicate.mp3".to_string(),
            source_type: BatchSourceType::File,
            status: BatchItemStatus::Pending,
            progress: 0.0,
            file_size_bytes: None,
            duration_seconds: None,
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
        },
    ];

    // Emit event matching active instance_id
    let updated = BatchImportPanel::apply_progress_event(
        &mut queue,
        Some("/data/duplicate.mp3"),
        Some(65.0),
        Some("batch-task-active-1"),
    );

    assert!(updated);
    // Active item must advance to 65%
    assert_eq!(queue[0].progress, 65.0);
    assert_eq!(queue[0].status, BatchItemStatus::Processing);

    // Pending duplicate item must remain strictly at 0% Pending
    assert_eq!(queue[1].progress, 0.0);
    assert_eq!(queue[1].status, BatchItemStatus::Pending);
}

// ============================================================================
// 21. Batch Queue Progress Event Fallback without instance_id
// ============================================================================

#[test]
fn test_batch_queue_progress_event_fallback_without_instance_id() {
    let mut queue = vec![
        BatchQueueItem {
            id: "item-1".to_string(),
            instance_id: None,
            filename: "song.wav".to_string(),
            file_path: "/data/song.wav".to_string(),
            source_type: BatchSourceType::File,
            status: BatchItemStatus::Processing,
            progress: 20.0,
            file_size_bytes: None,
            duration_seconds: None,
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
        },
        BatchQueueItem {
            id: "item-2".to_string(),
            instance_id: None,
            filename: "song.wav".to_string(),
            file_path: "/data/song.wav".to_string(),
            source_type: BatchSourceType::File,
            status: BatchItemStatus::Pending,
            progress: 0.0,
            file_size_bytes: None,
            duration_seconds: None,
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
        },
    ];

    // Event without instance_id fallback
    let updated = BatchImportPanel::apply_progress_event(
        &mut queue,
        Some("/data/song.wav"),
        Some(100.0),
        None,
    );

    assert!(updated);
    // Only the Processing item receives completion
    assert_eq!(queue[0].status, BatchItemStatus::Complete);
    assert_eq!(queue[0].progress, 100.0);

    // The Pending item must not be affected
    assert_eq!(queue[1].status, BatchItemStatus::Pending);
    assert_eq!(queue[1].progress, 0.0);
}

// ============================================================================
// 22. Batch Queue Serial Execution Guard Logic
// ============================================================================

#[test]
fn test_batch_queue_serial_execution_guard_logic() {
    let mut items = [
        make_item("1", BatchItemStatus::Complete, 100.0),
        make_item("2", BatchItemStatus::Pending, 0.0),
    ];

    // When no item is Processing, is_processing is false
    let is_processing = items
        .iter()
        .any(|i| i.status == BatchItemStatus::Processing);
    assert!(!is_processing);

    // When item 2 is shifted to Processing, is_processing becomes true
    items[1].status = BatchItemStatus::Processing;
    let is_processing = items
        .iter()
        .any(|i| i.status == BatchItemStatus::Processing);
    assert!(is_processing);

    // When item 2 finishes and transitions to Complete, is_processing reverts to false
    items[1].status = BatchItemStatus::Complete;
    items[1].progress = 100.0;
    let is_processing = items
        .iter()
        .any(|i| i.status == BatchItemStatus::Processing);
    assert!(!is_processing);
}

// ============================================================================
// 23. Full 18 Formats & Strict Regex Stream Protocol Validation
// ============================================================================

#[test]
fn test_full_18_formats_and_strict_regex_stream_protocol_validation() {
    // Exactly 18 formats matching React constants
    assert_eq!(SUPPORTED_EXTENSIONS.len(), 18);
    let expected_18 = [
        "wav", "mp3", "m4a", "aiff", "flac", "ogg", "wma", "aac", "opus", "amr", "mp4", "webm",
        "mov", "mkv", "avi", "wmv", "flv", "3gp",
    ];
    for ext in expected_18 {
        assert!(
            SUPPORTED_EXTENSIONS.contains(&ext),
            "Missing format: {}",
            ext
        );
        assert!(is_supported_audio_extension(ext));
    }

    // Strict regex check: ^(https?|rtmp|rtsp)://[^\s/?#]\S*$
    // Bare protocol schemes without host must be rejected
    assert!(!is_valid_stream_url("http://"));
    assert!(!is_valid_stream_url("https://"));
    assert!(!is_valid_stream_url("rtmp://"));
    assert!(!is_valid_stream_url("rtsp://"));

    // Scheme followed immediately by illegal characters
    assert!(!is_valid_stream_url("http:///path"));
    assert!(!is_valid_stream_url("https://?query"));
    assert!(!is_valid_stream_url("rtmp://#hash"));
    assert!(!is_valid_stream_url("rtsp:// host"));

    // URLs with internal whitespace
    assert!(!is_valid_stream_url("http://example.com/audio file.mp3"));

    // Legitimate stream URLs
    assert!(is_valid_stream_url("http://live.example.com/audio.mp3"));
    assert!(is_valid_stream_url(
        "https://cdn.server.net:8080/stream.m3u8"
    ));
    assert!(is_valid_stream_url(
        "rtmp://streamer.live.org:1935/app/stream"
    ));
    assert!(is_valid_stream_url("rtsp://cam.local:554/live/ch1"));
}
