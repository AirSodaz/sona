use sona_sync::SyncBackoffPolicy;

#[test]
fn backoff_uses_the_documented_schedule_and_caps_at_one_hour() {
    let policy = SyncBackoffPolicy::default();
    let midpoint = 500_000;

    assert_eq!(policy.delay_ms(1, midpoint), 30_000);
    assert_eq!(policy.delay_ms(2, midpoint), 120_000);
    assert_eq!(policy.delay_ms(3, midpoint), 600_000);
    assert_eq!(policy.delay_ms(4, midpoint), 1_800_000);
    assert_eq!(policy.delay_ms(5, midpoint), 3_600_000);
    assert_eq!(policy.delay_ms(20, midpoint), 3_600_000);
}

#[test]
fn backoff_adds_bounded_jitter_and_saturates_retry_timestamps() {
    let policy = SyncBackoffPolicy::default();

    assert_eq!(policy.delay_ms(1, 0), 24_000);
    assert_eq!(policy.delay_ms(1, 1_000_000), 36_000);
    assert_eq!(policy.next_retry_at_ms(u64::MAX - 10, 1, 500_000), u64::MAX);
}
