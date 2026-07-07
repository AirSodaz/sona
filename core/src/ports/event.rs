pub trait EventEmitter: Send + Sync + 'static {
    fn emit(&self, event: &str, payload: serde_json::Value) -> Result<(), String>;
}

impl<T: EventEmitter + ?Sized> EventEmitter for std::sync::Arc<T> {
    fn emit(&self, event: &str, payload: serde_json::Value) -> Result<(), String> {
        (**self).emit(event, payload)
    }
}
