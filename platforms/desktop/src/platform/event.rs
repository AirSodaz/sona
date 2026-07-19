/// Re-export the core trait so platform consumers can use either path.
pub use sona_core::ports::event::{EventEmitter, EventError};

use tauri::{AppHandle, Emitter, Runtime};

/// Tauri adapter: wraps AppHandle into the core EventEmitter port.
///
/// Using a newtype avoids Rust's orphan rule — the trait lives in `sona-core`
/// and the foreign type `AppHandle` comes from `tauri`, so a direct impl is
/// not allowed in this crate. The newtype keeps the dependency direction
/// explicit: platform code adapts Tauri into the core port.
pub struct TauriEventEmitter<R: Runtime>(pub AppHandle<R>);

impl<R: Runtime> EventEmitter for TauriEventEmitter<R> {
    fn emit(&self, event: &str, payload: serde_json::Value) -> Result<(), EventError> {
        Emitter::emit(&self.0, event, &payload)
            .map_err(|error| EventError::new(event, error.to_string()))
    }
}

#[cfg(test)]
pub struct MockEventEmitter {
    pub emitted: std::sync::Mutex<Vec<(String, serde_json::Value)>>,
}

#[cfg(test)]
impl Default for MockEventEmitter {
    fn default() -> Self {
        Self::new()
    }
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
    fn emit(&self, event: &str, payload: serde_json::Value) -> Result<(), EventError> {
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
