//! Streaming translation coordinator for enriching live transcript updates with translation.

use parking_lot::Mutex;
use sona_core::ports::translation::TranslationEnginePort;
use sona_core::transcription::streaming_stabilizer::{StreamingStabilizer, is_cjk_char};
use sona_core::transcription::transcript::TranscriptUpdate;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
struct SegmentTranslationState {
    last_text: String,
    last_translate_time: Instant,
    stabilizer: StreamingStabilizer,
    last_draft_translation: Option<String>,
}

pub struct StreamingTranslationCoordinator {
    engine: Arc<dyn TranslationEnginePort>,
    source_lang: String,
    target_lang: String,
    debounce_ms: u64,
    min_char_delta: usize,
    states: Arc<Mutex<HashMap<String, SegmentTranslationState>>>,
}

impl StreamingTranslationCoordinator {
    pub fn new(
        engine: Arc<dyn TranslationEnginePort>,
        source_lang: impl Into<String>,
        target_lang: impl Into<String>,
    ) -> Self {
        Self {
            engine,
            source_lang: source_lang.into(),
            target_lang: target_lang.into(),
            debounce_ms: 300,
            min_char_delta: 3,
            states: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_debounce(mut self, debounce_ms: u64, min_char_delta: usize) -> Self {
        self.debounce_ms = debounce_ms;
        self.min_char_delta = min_char_delta;
        self
    }

    pub fn source_lang(&self) -> &str {
        &self.source_lang
    }

    pub fn target_lang(&self) -> &str {
        &self.target_lang
    }
    pub fn debounce_ms(&self) -> u64 {
        self.debounce_ms
    }

    pub fn debounce_delay(&self, update: &TranscriptUpdate) -> Duration {
        let states = self.states.lock();
        let debounce = Duration::from_millis(self.debounce_ms);
        let mut max_remaining = Duration::ZERO;
        for segment in &update.upsert_segments {
            if let Some(state) = states.get(&segment.id) {
                let elapsed = state.last_translate_time.elapsed();
                let remaining = debounce.saturating_sub(elapsed);
                if remaining > max_remaining {
                    max_remaining = remaining;
                }
            }
        }
        max_remaining
    }

    pub fn clear(&self) {
        self.states.lock().clear();
    }
    pub fn fill_cached(&self, update: &mut TranscriptUpdate) {
        let states = self.states.lock();
        for segment in &mut update.upsert_segments {
            if segment.is_final {
                continue;
            }
            let text = segment.text.trim();
            let effective_source = if self.source_lang.trim().is_empty()
                || self.source_lang.trim().eq_ignore_ascii_case("auto")
            {
                detect_script_language(text)
            } else {
                normalize_language_code(&self.source_lang)
            };
            let effective_target = normalize_language_code(&self.target_lang);
            if effective_source.eq_ignore_ascii_case(effective_target) {
                continue;
            }

            if let Some(state) = states.get(&segment.id) {
                if segment.translation.is_none() {
                    let frozen = state.stabilizer.frozen_prefix();
                    if !frozen.is_empty() {
                        segment.translation = Some(frozen.to_string());
                    }
                }
                if segment.draft_translation.is_none() {
                    segment.draft_translation = state.last_draft_translation.clone();
                }
            }
        }
    }

    pub async fn enrich_update(&self, update: &mut TranscriptUpdate) {
        if !update.remove_ids.is_empty() {
            let mut states = self.states.lock();
            for remove_id in &update.remove_ids {
                states.remove(remove_id);
            }
        }

        for segment in &mut update.upsert_segments {
            let text = segment.text.trim();
            if text.is_empty() {
                if segment.is_final {
                    let mut states = self.states.lock();
                    states.remove(&segment.id);
                }
                continue;
            }

            let effective_source = if self.source_lang.trim().is_empty()
                || self.source_lang.trim().eq_ignore_ascii_case("auto")
            {
                detect_script_language(text)
            } else {
                normalize_language_code(&self.source_lang)
            };
            let effective_target = normalize_language_code(&self.target_lang);

            if effective_source.eq_ignore_ascii_case(effective_target) {
                segment.translation = None;
                segment.draft_translation = None;
                if segment.is_final {
                    let mut states = self.states.lock();
                    states.remove(&segment.id);
                }
                continue;
            }

            if segment.is_final {
                let translate_fut = self
                    .engine
                    .translate(text, effective_source, effective_target);
                match tokio::time::timeout(Duration::from_millis(5000), translate_fut).await {
                    Ok(Ok(translated_text)) => {
                        let mut states = self.states.lock();
                        let mut stabilizer = states
                            .remove(&segment.id)
                            .map(|s| s.stabilizer)
                            .unwrap_or_default();
                        let out = stabilizer.update(&translated_text, true);
                        segment.translation = out.frozen_prefix;
                        segment.draft_translation = None;
                    }
                    Ok(Err(err)) => {
                        log::warn!("Final translation failed for segment {}: {err}", segment.id);
                        let mut states = self.states.lock();
                        if let Some(state) = states.remove(&segment.id) {
                            let frozen = state.stabilizer.frozen_prefix().to_string();
                            if !frozen.is_empty() {
                                segment.translation = Some(frozen);
                            } else {
                                segment.translation = state.last_draft_translation;
                            }
                        }
                        segment.draft_translation = None;
                    }
                    Err(_timeout) => {
                        log::warn!(
                            "Final translation timed out (5000ms) for segment {}",
                            segment.id
                        );
                        let mut states = self.states.lock();
                        if let Some(state) = states.remove(&segment.id) {
                            let frozen = state.stabilizer.frozen_prefix().to_string();
                            if !frozen.is_empty() {
                                segment.translation = Some(frozen);
                            } else {
                                segment.translation = state.last_draft_translation;
                            }
                        }
                        segment.draft_translation = None;
                    }
                }
            } else {
                let should_translate = {
                    let mut states = self.states.lock();
                    if let Some(state) = states.get_mut(&segment.id) {
                        if text == state.last_text {
                            false
                        } else {
                            let is_cjk = text.chars().any(is_cjk_char);
                            let min_delta = if is_cjk { 1 } else { self.min_char_delta };
                            let char_delta = text
                                .chars()
                                .count()
                                .abs_diff(state.last_text.chars().count());
                            let elapsed = state.last_translate_time.elapsed();
                            let eligible = elapsed >= Duration::from_millis(self.debounce_ms)
                                && (char_delta >= min_delta || char_delta == 0);
                            if eligible {
                                state.last_text = text.to_string();
                                state.last_translate_time = Instant::now();
                                true
                            } else {
                                false
                            }
                        }
                    } else {
                        // Record immediately inside lock to avoid stampede from concurrent partials
                        states.insert(
                            segment.id.clone(),
                            SegmentTranslationState {
                                last_text: text.to_string(),
                                last_translate_time: Instant::now(),
                                stabilizer: StreamingStabilizer::new(),
                                last_draft_translation: None,
                            },
                        );
                        true
                    }
                };

                if should_translate {
                    let translate_fut =
                        self.engine
                            .translate(text, effective_source, effective_target);
                    match tokio::time::timeout(Duration::from_millis(3000), translate_fut).await {
                        Ok(Ok(translated_text)) => {
                            let mut states = self.states.lock();
                            if let Some(entry) = states.get_mut(&segment.id) {
                                if entry.last_text == text {
                                    let out = entry.stabilizer.update(&translated_text, false);
                                    entry.last_draft_translation = out.draft_tail.clone();
                                    entry.last_translate_time = Instant::now();
                                    segment.translation = out.frozen_prefix;
                                    segment.draft_translation = out.draft_tail;
                                } else {
                                    segment.translation =
                                        Some(entry.stabilizer.frozen_prefix().to_string())
                                            .filter(|s| !s.is_empty());
                                    segment.draft_translation =
                                        entry.last_draft_translation.clone();
                                }
                            }
                        }
                        Ok(Err(err)) => {
                            log::warn!(
                                "Streaming translation failed for segment {}: {err}",
                                segment.id
                            );
                            let states = self.states.lock();
                            if let Some(state) = states.get(&segment.id) {
                                segment.translation =
                                    Some(state.stabilizer.frozen_prefix().to_string())
                                        .filter(|s| !s.is_empty());
                                segment.draft_translation = state.last_draft_translation.clone();
                            }
                        }
                        Err(_timeout) => {
                            log::warn!(
                                "Streaming translation timed out (3000ms) for segment {}",
                                segment.id
                            );
                            let states = self.states.lock();
                            if let Some(state) = states.get(&segment.id) {
                                segment.translation =
                                    Some(state.stabilizer.frozen_prefix().to_string())
                                        .filter(|s| !s.is_empty());
                                segment.draft_translation = state.last_draft_translation.clone();
                            }
                        }
                    }
                } else {
                    let states = self.states.lock();
                    if let Some(state) = states.get(&segment.id) {
                        segment.translation = Some(state.stabilizer.frozen_prefix().to_string())
                            .filter(|s| !s.is_empty());
                        segment.draft_translation = state.last_draft_translation.clone();
                    }
                }
            }
        }
    }
}

/// Detect script language when source language is "auto" or empty.
/// Japanese hiragana/katakana -> "ja"
/// CJK ideographs -> "zh"
/// Others -> "en"
pub fn detect_script_language(text: &str) -> &'static str {
    let has_kana = text
        .chars()
        .any(|c| matches!(c, '\u{3040}'..='\u{309F}' | '\u{30A0}'..='\u{30FF}'));
    if has_kana {
        return "ja";
    }
    let has_cjk_ideograph = text.chars().any(|c| {
        matches!(
            c,
            '\u{4E00}'..='\u{9FFF}'
                | '\u{3400}'..='\u{4DBF}'
                | '\u{F900}'..='\u{FAFF}'
                | '\u{20000}'..='\u{2FA1F}'
                | '\u{30000}'..='\u{323AF}'
        )
    });
    if has_cjk_ideograph {
        return "zh";
    }
    "en"
}

/// Normalize language codes (e.g. "zh-CN" -> "zh", "en-US" -> "en", "ja-JP" -> "ja", "中文" -> "zh").
pub fn normalize_language_code(code: &str) -> &str {
    let trimmed = code.trim();
    if trimmed.starts_with("中文") {
        return "zh";
    }
    if let Some(prefix) = trimmed.get(..2) {
        if prefix.eq_ignore_ascii_case("zh")
            || trimmed
                .get(..3)
                .is_some_and(|p| p.eq_ignore_ascii_case("cmn"))
        {
            return "zh";
        }
        if prefix.eq_ignore_ascii_case("en") {
            return "en";
        }
        if prefix.eq_ignore_ascii_case("ja")
            || trimmed
                .get(..3)
                .is_some_and(|p| p.eq_ignore_ascii_case("jpn"))
        {
            return "ja";
        }
    }
    trimmed
}
