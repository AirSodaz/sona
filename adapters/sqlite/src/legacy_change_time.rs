//! Compatibility clock for legacy repository methods that do not yet accept a clock port.
//!
//! New application paths receive `UnixMillisClock` through their composition root. Tag and
//! Automation compatibility stores still need a timestamp for Sync outbox records even though
//! their legacy store traits do not carry an operation timestamp.

pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}
