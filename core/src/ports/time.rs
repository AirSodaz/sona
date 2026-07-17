use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ClockError {
    #[error("System clock is before the Unix epoch: {0}")]
    BeforeUnixEpoch(String),
    #[error("System clock timestamp is out of range: {0}")]
    OutOfRange(String),
    #[error("System clock is unavailable: {0}")]
    Unavailable(String),
}

pub trait UnixMillisClock: Send + Sync {
    fn now_ms(&self) -> Result<u64, ClockError>;
}
