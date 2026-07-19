#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("Failed to emit event {event}: {reason}")]
pub struct EventError {
    pub event: String,
    pub reason: String,
}

impl EventError {
    pub fn new(event: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            event: event.into(),
            reason: reason.into(),
        }
    }
}

pub trait EventEmitter: Send + Sync + 'static {
    fn emit(&self, event: &str, payload: serde_json::Value) -> Result<(), EventError>;
}

impl<T: EventEmitter + ?Sized> EventEmitter for std::sync::Arc<T> {
    fn emit(&self, event: &str, payload: serde_json::Value) -> Result<(), EventError> {
        (**self).emit(event, payload)
    }
}
