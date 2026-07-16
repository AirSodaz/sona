const RETRY_DELAYS_MS: [u64; 5] = [30_000, 120_000, 600_000, 1_800_000, 3_600_000];
const JITTER_SCALE: u32 = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SyncBackoffPolicy {
    jitter_percent: u8,
}

impl Default for SyncBackoffPolicy {
    fn default() -> Self {
        Self { jitter_percent: 20 }
    }
}

impl SyncBackoffPolicy {
    pub fn delay_ms(self, consecutive_failures: u32, jitter_sample: u32) -> u64 {
        let index = consecutive_failures
            .saturating_sub(1)
            .min((RETRY_DELAYS_MS.len() - 1) as u32) as usize;
        let base = RETRY_DELAYS_MS[index];
        let maximum_jitter = base.saturating_mul(u64::from(self.jitter_percent)) / 100;
        let sample = jitter_sample.min(JITTER_SCALE);
        if sample <= JITTER_SCALE / 2 {
            let distance = u64::from(JITTER_SCALE / 2 - sample);
            base.saturating_sub(
                maximum_jitter.saturating_mul(distance) / u64::from(JITTER_SCALE / 2),
            )
        } else {
            let distance = u64::from(sample - JITTER_SCALE / 2);
            base.saturating_add(
                maximum_jitter.saturating_mul(distance) / u64::from(JITTER_SCALE / 2),
            )
        }
    }

    pub fn next_retry_at_ms(
        self,
        now_ms: u64,
        consecutive_failures: u32,
        jitter_sample: u32,
    ) -> u64 {
        now_ms.saturating_add(self.delay_ms(consecutive_failures, jitter_sample))
    }
}
