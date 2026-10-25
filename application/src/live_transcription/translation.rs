//! Streaming translation coordinator for enriching live transcript updates with translation.

use parking_lot::Mutex;
use sona_core::ports::translation::TranslationEnginePort;
use sona_core::transcription::streaming_stabilizer::StreamingStabilizer;
use sona_core::transcription::transcript::TranscriptUpdate;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
struct SegmentTranslationState {
    last_text: String,
    last_translate_time: Instant,
    stabilizer: StreamingStabilizer,
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
                continue;
            }

            if segment.is_final {
                match self
                    .engine
                    .translate(text, &self.source_lang, &self.target_lang)
                    .await
                {
                    Ok(translated_text) => {
                        let mut states = self.states.lock();
                        let mut stabilizer = states
                            .remove(&segment.id)
                            .map(|s| s.stabilizer)
                            .unwrap_or_default();
                        let out = stabilizer.update(&translated_text, true);
                        segment.translation = out.frozen_prefix;
                        segment.draft_translation = None;
                    }
                    Err(err) => {
                        log::warn!("Final translation failed for segment {}: {err}", segment.id);
                        let mut states = self.states.lock();
                        states.remove(&segment.id);
                    }
                }
            } else {
                let should_translate = {
                    let states = self.states.lock();
                    if let Some(state) = states.get(&segment.id) {
                        let char_delta = text
                            .chars()
                            .count()
                            .saturating_sub(state.last_text.chars().count());
                        let elapsed = state.last_translate_time.elapsed();
                        char_delta >= self.min_char_delta
                            || elapsed >= Duration::from_millis(self.debounce_ms)
                    } else {
                        true
                    }
                };

                if should_translate {
                    match self
                        .engine
                        .translate(text, &self.source_lang, &self.target_lang)
                        .await
                    {
                        Ok(translated_text) => {
                            let mut states = self.states.lock();
                            let entry = states.entry(segment.id.clone()).or_insert_with(|| {
                                SegmentTranslationState {
                                    last_text: String::new(),
                                    last_translate_time: Instant::now(),
                                    stabilizer: StreamingStabilizer::new(),
                                }
                            });
                            entry.last_text = text.to_string();
                            entry.last_translate_time = Instant::now();
                            let out = entry.stabilizer.update(&translated_text, false);
                            segment.translation = out.frozen_prefix;
                            segment.draft_translation = out.draft_tail;
                        }
                        Err(err) => {
                            log::warn!(
                                "Streaming translation failed for segment {}: {err}",
                                segment.id
                            );
                            let states = self.states.lock();
                            if let Some(state) = states.get(&segment.id) {
                                segment.translation =
                                    Some(state.stabilizer.frozen_prefix().to_string())
                                        .filter(|s| !s.is_empty());
                            }
                        }
                    }
                } else {
                    let states = self.states.lock();
                    if let Some(state) = states.get(&segment.id) {
                        segment.translation = Some(state.stabilizer.frozen_prefix().to_string())
                            .filter(|s| !s.is_empty());
                    }
                }
            }
        }
    }
}
