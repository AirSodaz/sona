use std::time::Duration;

/// Adaptation level for pseudo-streaming partial hypothesis refresh rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackoffLevel {
    /// Level 0: baseline refresh rate (e.g. 200ms or 400ms)
    Normal = 0,
    /// Level 1: 1.5x baseline
    Relaxed = 1,
    /// Level 2: 2.5x baseline
    Conservative = 2,
    /// Level 3: 4.0x baseline
    Sparse = 3,
    /// Level 4: SentenceOnly (no partial refreshes during speech, only final segment)
    SentenceOnly = 4,
}

impl BackoffLevel {
    pub fn step_down(self) -> Self {
        match self {
            Self::Normal => Self::Relaxed,
            Self::Relaxed => Self::Conservative,
            Self::Conservative => Self::Sparse,
            Self::Sparse | Self::SentenceOnly => Self::SentenceOnly,
        }
    }

    pub fn step_up(self) -> Self {
        match self {
            Self::SentenceOnly => Self::Sparse,
            Self::Sparse => Self::Conservative,
            Self::Conservative => Self::Relaxed,
            Self::Relaxed | Self::Normal => Self::Normal,
        }
    }
}

/// Dynamic backoff controller that adjusts partial inference intervals based on
/// decoding latency and concurrency overruns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicBackoffState {
    initial_interval_ms: u64,
    level: BackoffLevel,
    consecutive_clean_utterances: u32,
    overrun_in_current_utterance: bool,
}

impl Default for DynamicBackoffState {
    fn default() -> Self {
        Self::new(200)
    }
}

impl DynamicBackoffState {
    pub fn new(initial_interval_ms: u64) -> Self {
        Self {
            initial_interval_ms: initial_interval_ms.max(50),
            level: BackoffLevel::Normal,
            consecutive_clean_utterances: 0,
            overrun_in_current_utterance: false,
        }
    }

    pub fn initial_interval_ms(&self) -> u64 {
        self.initial_interval_ms
    }

    pub fn set_initial_interval_ms(&mut self, interval_ms: u64) {
        self.initial_interval_ms = interval_ms.max(50);
    }

    pub fn level(&self) -> BackoffLevel {
        self.level
    }

    pub fn is_sentence_only(&self) -> bool {
        self.level == BackoffLevel::SentenceOnly
    }

    pub fn current_interval_ms(&self) -> Option<u64> {
        match self.level {
            BackoffLevel::Normal => Some(self.initial_interval_ms),
            BackoffLevel::Relaxed => Some((self.initial_interval_ms * 3) / 2),
            BackoffLevel::Conservative => Some((self.initial_interval_ms * 5) / 2),
            BackoffLevel::Sparse => Some(self.initial_interval_ms * 4),
            BackoffLevel::SentenceOnly => None,
        }
    }

    pub fn should_run_partial(&self, elapsed: Duration) -> bool {
        match self.current_interval_ms() {
            Some(interval_ms) => elapsed.as_millis() as u64 >= interval_ms,
            None => false,
        }
    }

    pub fn record_overrun(&mut self) {
        self.overrun_in_current_utterance = true;
        self.consecutive_clean_utterances = 0;
        self.level = self.level.step_down();
    }

    pub fn record_decode_duration(&mut self, decode_ms: u64) {
        if let Some(target_interval) = self.current_interval_ms() {
            // If decode time exceeds 85% of the target interval, step down to reduce pressure
            if decode_ms * 100 > target_interval * 85 {
                self.overrun_in_current_utterance = true;
                self.consecutive_clean_utterances = 0;
                self.level = self.level.step_down();
            }
        }
    }

    pub fn on_utterance_end(&mut self, clean: bool) {
        if clean && !self.overrun_in_current_utterance {
            self.consecutive_clean_utterances = self.consecutive_clean_utterances.saturating_add(1);
            if self.consecutive_clean_utterances >= 2 {
                self.level = self.level.step_up();
                self.consecutive_clean_utterances = 0;
            }
        } else {
            self.consecutive_clean_utterances = 0;
        }
        self.overrun_in_current_utterance = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_level_step_transitions() {
        let mut level = BackoffLevel::Normal;
        level = level.step_down();
        assert_eq!(level, BackoffLevel::Relaxed);
        level = level.step_down();
        assert_eq!(level, BackoffLevel::Conservative);
        level = level.step_down();
        assert_eq!(level, BackoffLevel::Sparse);
        level = level.step_down();
        assert_eq!(level, BackoffLevel::SentenceOnly);
        level = level.step_down();
        assert_eq!(level, BackoffLevel::SentenceOnly);

        level = level.step_up();
        assert_eq!(level, BackoffLevel::Sparse);
        level = level.step_up();
        assert_eq!(level, BackoffLevel::Conservative);
        level = level.step_up();
        assert_eq!(level, BackoffLevel::Relaxed);
        level = level.step_up();
        assert_eq!(level, BackoffLevel::Normal);
        level = level.step_up();
        assert_eq!(level, BackoffLevel::Normal);
    }

    #[test]
    fn dynamic_backoff_steps_down_on_overrun_and_recovers_after_clean_utterances() {
        let mut state = DynamicBackoffState::new(400);
        assert_eq!(state.current_interval_ms(), Some(400));

        state.record_overrun();
        assert_eq!(state.level(), BackoffLevel::Relaxed);
        assert_eq!(state.current_interval_ms(), Some(600));

        // The utterance where overrun happened ends; this utterance was not clean.
        state.on_utterance_end(true);
        assert_eq!(state.level(), BackoffLevel::Relaxed);

        // 1st clean utterance
        state.on_utterance_end(true);
        assert_eq!(state.level(), BackoffLevel::Relaxed);

        // 2nd clean utterance triggers step up to Normal
        state.on_utterance_end(true);
        assert_eq!(state.level(), BackoffLevel::Normal);
        assert_eq!(state.current_interval_ms(), Some(400));
    }

    #[test]
    fn dynamic_backoff_steps_down_when_decode_exceeds_threshold() {
        let mut state = DynamicBackoffState::new(200);
        // 85% of 200ms is 170ms. 180ms should trigger step down.
        state.record_decode_duration(180);
        assert_eq!(state.level(), BackoffLevel::Relaxed);
        assert_eq!(state.current_interval_ms(), Some(300));
    }
}
