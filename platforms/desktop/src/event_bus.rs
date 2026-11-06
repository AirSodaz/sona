//! In-memory broadcast event bus implementing `sona_core::ports::event::EventEmitterPort`.
//!
//! Provides zero-copy in-process event streaming between decoupled backend services
//! and GPUI UI entities / views.

use serde::{Deserialize, Serialize};
use sona_core::ports::event::{EventEmitterPort, EventError};
use tokio::sync::broadcast::{self, Receiver, Sender};

/// Standard event envelope transported across the in-memory event bus.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AppEvent {
    pub name: String,
    pub payload: serde_json::Value,
}

impl AppEvent {
    /// Creates a new AppEvent with the given name and json payload.
    pub fn new(name: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            name: name.into(),
            payload,
        }
    }

    /// Convenience constructor that serializes any Serializable data structure.
    pub fn from_serializable<T: Serialize>(
        name: impl Into<String>,
        payload: &T,
    ) -> Result<Self, serde_json::Error> {
        Ok(Self {
            name: name.into(),
            payload: serde_json::to_value(payload)?,
        })
    }

    /// Checks if this event matches the specified event name.
    pub fn is(&self, event_name: &str) -> bool {
        self.name == event_name
    }

    /// Attempts to deserialize the inner payload into a concrete domain type.
    pub fn decode<T: for<'de> Deserialize<'de>>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.payload.clone())
    }
}

/// In-memory broadcast event bus.
///
/// Wraps a `tokio::sync::broadcast::Sender` and implements `EventEmitterPort`.
/// Cloning `AppEventBus` clones the underlying channel sender.
#[derive(Clone)]
pub struct AppEventBus {
    sender: Sender<AppEvent>,
}

impl AppEventBus {
    /// Default broadcast queue capacity.
    pub const DEFAULT_CAPACITY: usize = 2048;

    /// Creates a new `AppEventBus` with a custom buffer capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Subscribes to the event bus, returning a `Receiver<AppEvent>`.
    ///
    /// Multiple subscribers can listen concurrently. Each subscriber receives
    /// all events emitted after the subscription is created.
    pub fn subscribe(&self) -> Receiver<AppEvent> {
        self.sender.subscribe()
    }

    /// Returns the number of active subscribers.
    pub fn receiver_count(&self) -> usize {
        self.sender.receiver_count()
    }

    /// Returns a direct reference to the inner broadcast sender.
    pub fn sender(&self) -> &Sender<AppEvent> {
        &self.sender
    }

    /// Direct event emission method accepting an `AppEvent`.
    ///
    /// Note: If there are no active receivers, `send` returns a `SendError`
    /// which is safely ignored and treated as success.
    pub fn emit_event(&self, event: AppEvent) -> Result<(), EventError> {
        let _ = self.sender.send(event);
        Ok(())
    }

    /// Emits a typed event by serializing the payload into JSON.
    pub fn emit_typed<T: Serialize>(&self, name: &str, payload: &T) -> Result<(), EventError> {
        let value = serde_json::to_value(payload)
            .map_err(|e| EventError::new(name, format!("Failed to serialize payload: {e}")))?;
        self.emit(name, value)
    }
}

impl Default for AppEventBus {
    fn default() -> Self {
        Self::new(Self::DEFAULT_CAPACITY)
    }
}

impl EventEmitterPort for AppEventBus {
    fn emit(&self, event: &str, payload: serde_json::Value) -> Result<(), EventError> {
        let _ = self.sender.send(AppEvent {
            name: event.to_string(),
            payload,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn emits_and_receives_events() {
        let bus = AppEventBus::default();
        let mut rx = bus.subscribe();

        bus.emit("test:ping", json!({ "status": "ok" })).unwrap();

        let event = rx.recv().await.expect("failed to recv event");
        assert_eq!(event.name, "test:ping");
        assert_eq!(event.payload["status"], "ok");
        assert!(event.is("test:ping"));
    }

    #[tokio::test]
    async fn broadcasts_to_multiple_subscribers() {
        let bus = AppEventBus::default();
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        assert_eq!(bus.receiver_count(), 2);

        bus.emit("metric:cpu", json!({ "percent": 42.5 })).unwrap();

        let e1 = rx1.recv().await.unwrap();
        let e2 = rx2.recv().await.unwrap();

        assert_eq!(e1, e2);
        assert_eq!(e1.payload["percent"], 42.5);
    }

    #[tokio::test]
    async fn emit_with_zero_subscribers_succeeds() {
        let bus = AppEventBus::default();
        assert_eq!(bus.receiver_count(), 0);

        // Must not return Err or panic
        let result = bus.emit("orphan:event", json!({ "key": "val" }));
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn emits_and_decodes_typed_payload() {
        #[derive(Debug, Serialize, Deserialize, PartialEq)]
        struct DownloadProgress {
            model_id: String,
            progress: f32,
        }

        let bus = AppEventBus::default();
        let mut rx = bus.subscribe();

        let payload = DownloadProgress {
            model_id: "whisper-base".to_string(),
            progress: 78.5,
        };

        bus.emit_typed("model:download-progress", &payload).unwrap();

        let event = rx.recv().await.unwrap();
        let decoded: DownloadProgress = event.decode().unwrap();
        assert_eq!(decoded, payload);
    }
}
