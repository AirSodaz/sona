/// A trait for objects that can emit events, abstracting away Tauri's AppHandle/Window.
pub trait EventEmitter: Send + Sync + 'static {
    fn emit(&self, event: &str, payload: serde_json::Value) -> Result<(), String>;
}

impl<T: EventEmitter + ?Sized> EventEmitter for std::sync::Arc<T> {
    fn emit(&self, event: &str, payload: serde_json::Value) -> Result<(), String> {
        (**self).emit(event, payload)
    }
}

use tauri::{AppHandle, Emitter, Runtime, Window};

impl<R: Runtime> EventEmitter for AppHandle<R> {
    fn emit(&self, event: &str, payload: serde_json::Value) -> Result<(), String> {
        Emitter::emit(self, event, &payload).map_err(|error| error.to_string())
    }
}

impl<R: Runtime> EventEmitter for Window<R> {
    fn emit(&self, event: &str, payload: serde_json::Value) -> Result<(), String> {
        Emitter::emit(self, event, &payload).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
pub struct MockEventEmitter {
    pub emitted: std::sync::Mutex<Vec<(String, serde_json::Value)>>,
}

#[cfg(test)]
impl MockEventEmitter {
    pub fn new() -> Self {
        Self {
            emitted: std::sync::Mutex::new(Vec::new()),
        }
    }
}

#[cfg(test)]
impl EventEmitter for MockEventEmitter {
    fn emit(&self, event: &str, payload: serde_json::Value) -> Result<(), String> {
        let mut guard = self.emitted.lock().expect("emitted lock poisoned");
        guard.push((event.to_string(), payload));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn mock_event_emitter_captures_events() {
        let emitter = MockEventEmitter::new();
        emitter.emit("test-event", json!({"status": "ok"})).unwrap();

        let emitted = emitter.emitted.lock().unwrap();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].0, "test-event");
        assert_eq!(emitted[0].1["status"], "ok");
    }
}
