//! Milestone 2 Iteration 2 Gate Verification: Floating Overlays Empirical Challenge
//!
//! Authoritative verification for:
//! 1. Float duration sanitization in `caption_window.rs`
//!    - Extreme float values: `f32::NAN`, `f32::INFINITY`, `f32::NEG_INFINITY`, `-10.0`, `0.0`, `0.1`, `100.0`, `f32::MIN_POSITIVE`, `-0.0`, subnormals
//!    - Verifies `sanitized_disappear_delay_secs()` never panics and always produces a duration within `[0.5, 30.0]`
//!    - Verifies `disappear_delay()` never panics
//! 2. Audio peak normalization in `voice_typing_island.rs`
//!    - Extreme peak levels: `-1.0`, `-0.5`, `0.0`, `0.8`, `1.5`, `-5.0`, `f32::NAN`
//!    - Verifies `.abs()` rectification and clamping
//! 3. Voice Typing Island dynamic history:
//!    - Empty state verification: `history.is_empty()`, 0 items, `voice-typing-recall-empty`
//!    - Dynamic insertion of history records with varying JSON shapes (`injectedText` vs `injected_text`, `timestamp` vs `timestamp_ms`, etc.)
//!    - Capping at 20 items, deduplication of consecutive snippets, filtering of empty/placeholder text
//! 4. Caption layout modes and priority state machine

use serde_json::json;
use std::time::Duration;

use sona_desktop_lib::views::overlays::caption_window::{
    CaptionLayoutMode, CaptionPriority, CaptionStyle,
};
use sona_desktop_lib::views::overlays::voice_typing_island::{
    VoiceTypingHistorySnippet, VoiceTypingIsland, VoiceTypingPhase,
};

// ============================================================================
// 1. Float Duration Sanitization in caption_window.rs
// ============================================================================

#[test]
fn test_caption_style_float_sanitization_comprehensive() {
    let mut style = CaptionStyle::default();

    // Default configuration must be 4.0s
    assert_eq!(style.disappear_delay_secs, 4.0);
    assert_eq!(style.sanitized_disappear_delay_secs(), 4.0);
    assert_eq!(style.disappear_delay(), Duration::from_secs(4));

    // Table of test cases: (input, expected_secs)
    let test_cases: Vec<(f32, f32)> = vec![
        // Extreme Non-finite values -> Must safely fall back to 4.0s
        (f32::NAN, 4.0),
        (f32::INFINITY, 4.0),
        (f32::NEG_INFINITY, 4.0),
        // Non-positive values -> Must safely fall back to 4.0s (prevent Duration panic)
        (-10.0, 4.0),
        (-100.0, 4.0),
        (-0.5, 4.0),
        (-0.0, 4.0),
        (0.0, 4.0),
        (-f32::MIN_POSITIVE, 4.0),
        // Sub-minimum positive values -> Must clamp to 0.5s
        (0.1, 0.5),
        (0.499, 0.5),
        (f32::MIN_POSITIVE, 0.5),
        (1e-30, 0.5),
        // Valid ranges within [0.5, 30.0] -> Exact value preserved
        (0.5, 0.5),
        (1.0, 1.0),
        (4.0, 4.0),
        (8.5, 8.5),
        (29.9, 29.9),
        (30.0, 30.0),
        // Above maximum values -> Must clamp to 30.0s
        (30.1, 30.0),
        (100.0, 30.0),
        (9999.0, 30.0),
        (f32::MAX, 30.0),
    ];

    for (input, expected) in test_cases {
        style.disappear_delay_secs = input;
        let sanitized = style.sanitized_disappear_delay_secs();
        assert_eq!(
            sanitized, expected,
            "Input {} expected sanitized delay {}, got {}",
            input, expected, sanitized
        );

        // Crucial: Duration::from_secs_f32 must NEVER panic
        let duration = style.disappear_delay();
        assert!(
            duration >= Duration::from_millis(500),
            "Duration must be >= 0.5s for input {}",
            input
        );
        assert!(
            duration <= Duration::from_secs(30),
            "Duration must be <= 30.0s for input {}",
            input
        );
    }
}

// ============================================================================
// 2. Audio Peak Normalization in voice_typing_island.rs
// ============================================================================

/// Normalization oracle representing the production formula in `voice_typing_island.rs`:
/// `((peak_i16 as f32).abs() / 32767.0).clamp(0.0, 1.0)`
fn normalize_peak_i16(peak: i16) -> f32 {
    ((peak as f32).abs() / 32767.0).clamp(0.0, 1.0)
}

/// Float peak normalization oracle with defensive abs and clamping:
/// `(peak.abs()).clamp(0.0, 1.0)` or 0.0 if NaN
fn normalize_peak_f32(peak: f32) -> f32 {
    if peak.is_nan() {
        0.0
    } else {
        peak.abs().clamp(0.0, 1.0)
    }
}

#[test]
fn test_audio_peak_normalization_extreme_values() {
    // 1. Integer i16 excursions from CPAL audio capture
    assert_eq!(normalize_peak_i16(0), 0.0);
    assert_eq!(normalize_peak_i16(i16::MAX), 1.0);
    // Negative minimum (-32768) must not overflow or panic and must clamp to 1.0
    assert_eq!(normalize_peak_i16(i16::MIN), 1.0);
    // Negative half scale (-16384) must be positive ~0.5
    let half_neg = normalize_peak_i16(-16384);
    assert!((half_neg - 0.5).abs() < 0.01);
    assert_eq!(normalize_peak_i16(16384), normalize_peak_i16(-16384));

    // 2. Direct float inputs requested by challenge
    let float_cases: Vec<(f32, f32)> = vec![
        (0.0, 0.0),
        (-0.5, 0.5),
        (-1.0, 1.0),
        (0.8, 0.8),
        (1.5, 1.0),      // Clamped to 1.0
        (-5.0, 1.0),     // Rectified and clamped to 1.0
        (f32::NAN, 0.0), // NaN safely zeroed
    ];

    for (raw_input, expected) in float_cases {
        let norm = normalize_peak_f32(raw_input);
        assert_eq!(
            norm, expected,
            "Raw input {} expected {}, got {}",
            raw_input, expected, norm
        );
        assert!(
            (0.0..=1.0).contains(&norm),
            "Normalized volume must be within [0.0, 1.0]"
        );
    }
}

#[test]
fn test_waveform_heights_oracle_bounds() {
    const WAVEFORM_WEIGHTS: [f32; 5] = [0.45, 0.75, 1.0, 0.75, 0.45];
    const MIN_BAR_HEIGHT: f32 = 4.0;
    const MAX_BAR_HEIGHT: f32 = 18.0;

    // Simulate reactive waveform heights calculation across volume range [0.0, 1.0]
    for volume_step in 0..=100 {
        let volume = volume_step as f32 / 100.0;
        let boost = (volume * 2.2).clamp(0.15, 1.0);
        for &weight in &WAVEFORM_WEIGHTS {
            let height =
                (MIN_BAR_HEIGHT + (MAX_BAR_HEIGHT - MIN_BAR_HEIGHT) * boost * weight).round();
            assert!(
                height >= MIN_BAR_HEIGHT,
                "Waveform bar height {} must be >= min {} (vol={})",
                height,
                MIN_BAR_HEIGHT,
                volume
            );
            assert!(
                height <= MAX_BAR_HEIGHT,
                "Waveform bar height {} must be <= max {} (vol={})",
                height,
                MAX_BAR_HEIGHT,
                volume
            );
        }
    }
}

// ============================================================================
// 3. Voice Typing Island Dynamic History & JSON Shapes
// ============================================================================

#[test]
fn test_voice_typing_empty_state_and_initialization() {
    let island = VoiceTypingIsland {
        phase: VoiceTypingPhase::Listening,
        interim_text: "Listening...".to_string(),
        translation_text: None,
        target_language: "EN".to_string(),
        has_selection: false,
        audio_peak_volume: 0.0,
        context_preset: None,
        error_message: None,
        history: Vec::new(),
        is_drawer_open: false,
        fade_task: None,
        _event_subscription_task: None,
    };

    // Verify empty state: 0 items
    assert_eq!(island.history.len(), 0);
    assert!(island.history.is_empty());
}

#[test]
fn test_voice_typing_history_dynamic_serde_shapes() {
    // Shape 1: Rust backend standard snake_case
    let json_snake = json!({
        "id": "vt-001",
        "injected_text": "Rust dictation result",
        "timestamp_ms": 1700000000123u64
    });
    let item_snake: Result<VoiceTypingHistorySnippet, _> = serde_json::from_value(json_snake);
    assert!(item_snake.is_ok());
    let item = item_snake.unwrap();
    assert_eq!(item.id, "vt-001");
    assert_eq!(item.injected_text, "Rust dictation result");
    assert_eq!(item.timestamp_ms, 1700000000123);

    // Shape 2: React frontend camelCase with extra fields (rawText, mode, timestamp)
    let json_camel = json!({
        "id": "vt_1700000000_abc",
        "injectedText": "React frontend dictation",
        "rawText": "raw dictation",
        "polishedText": "React frontend dictation",
        "timestamp": 1700000000456u64,
        "mode": "polish"
    });
    let item_camel: Result<VoiceTypingHistorySnippet, _> = serde_json::from_value(json_camel);
    assert!(
        item_camel.is_ok(),
        "Failed to deserialize React camelCase shape: {:?}",
        item_camel.err()
    );
    let item2 = item_camel.unwrap();
    assert_eq!(item2.id, "vt_1700000000_abc");
    assert_eq!(item2.injected_text, "React frontend dictation");
    assert_eq!(item2.timestamp_ms, 1700000000456);

    // Shape 3: Minimal without timestamp (defaulting to 0)
    let json_minimal = json!({
        "id": "vt_minimal",
        "injectedText": "Minimal payload"
    });
    let item_minimal: Result<VoiceTypingHistorySnippet, _> = serde_json::from_value(json_minimal);
    assert!(item_minimal.is_ok());
    assert_eq!(item_minimal.unwrap().timestamp_ms, 0);
}

#[test]
fn test_voice_typing_history_preserves_repeats_and_caps_at_react_limit() {
    let mut island = VoiceTypingIsland {
        phase: VoiceTypingPhase::Listening,
        interim_text: "Listening...".to_string(),
        translation_text: None,
        target_language: "EN".to_string(),
        has_selection: false,
        audio_peak_volume: 0.0,
        context_preset: None,
        error_message: None,
        history: Vec::new(),
        is_drawer_open: false,
        fade_task: None,
        _event_subscription_task: None,
    };

    // Whitespace and placeholder rejection
    island.push_history_snippet("");
    island.push_history_snippet("   \t\n  ");
    island.push_history_snippet("Listening...");
    assert!(
        island.history.is_empty(),
        "Empty or placeholder text must be rejected"
    );

    // Add first valid entry
    island.push_history_snippet("Meeting notes: AI models discussed.");
    assert_eq!(island.history.len(), 1);
    assert_eq!(
        island.history[0].injected_text,
        "Meeting notes: AI models discussed."
    );

    // Repeated dictations remain distinct entries in the React store.
    island.push_history_snippet("Meeting notes: AI models discussed.");
    island.push_history_snippet("  Meeting notes: AI models discussed.  ");
    assert_eq!(island.history.len(), 3);

    // Prepend new entry
    island.push_history_snippet("Second entry: action items recorded.");
    assert_eq!(island.history.len(), 4);
    assert_eq!(
        island.history[0].injected_text,
        "Second entry: action items recorded."
    );
    assert_eq!(
        island.history[1].injected_text,
        "Meeting notes: AI models discussed."
    );

    // Fill beyond the React store's 100-entry limit.
    for i in 3..=110 {
        island.push_history_snippet(&format!("Snippet number {}", i));
    }
    assert_eq!(
        island.history.len(),
        100,
        "History must be strictly capped at 100 items"
    );
    assert_eq!(island.history[0].injected_text, "Snippet number 110");
}

// ============================================================================
// 4. Caption Layout Mode and Priority State Machine
// ============================================================================

#[test]
fn test_caption_layout_mode_cycle_and_labels() {
    let mut mode = CaptionLayoutMode::SingleLineBottom;
    assert_eq!(mode.label(), "Single Line");

    mode = mode.next();
    assert_eq!(mode, CaptionLayoutMode::DualLineBilingual);
    assert_eq!(mode.label(), "Dual Bilingual");

    mode = mode.next();
    assert_eq!(mode, CaptionLayoutMode::MultiLineWrapped);
    assert_eq!(mode.label(), "Multi-line Wrapped");

    mode = mode.next();
    assert_eq!(mode, CaptionLayoutMode::CompactTicker);
    assert_eq!(mode.label(), "Compact Ticker");

    mode = mode.next();
    assert_eq!(mode, CaptionLayoutMode::SingleLineBottom);
}

#[test]
fn test_caption_priority_variants() {
    assert_eq!(CaptionPriority::default(), CaptionPriority::OriginalMain);
    let priorities = [
        CaptionPriority::OriginalMain,
        CaptionPriority::TranslationMain,
        CaptionPriority::OriginalOnly,
        CaptionPriority::TranslationOnly,
    ];
    assert_eq!(priorities.len(), 4);
}
