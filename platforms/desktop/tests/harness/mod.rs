//! E2E Test Harness for Sona Desktop Pure Rust GPUI Architecture.
//!
//! Provides isolated in-process testing of `DesktopServices` backed by
//! production `sona_desktop_lib::event_bus::AppEventBus` and temporary SQLite storage.

use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;

use sona_core::ports::event::EventEmitterPort;
use sona_desktop_lib::event_bus::{AppEvent, AppEventBus};
use sona_desktop_lib::platform::database::DesktopSqliteState;
use sona_desktop_lib::services::DesktopServices;

/// Primary test harness for isolated desktop testing.
#[allow(dead_code)]
pub struct TestHarness {
    pub temp_dir: TempDir,
    pub event_bus: Arc<AppEventBus>,
    pub services: DesktopServices,
}

#[allow(dead_code)]
impl TestHarness {
    /// Creates a new isolated test harness with a fresh temporary SQLite context and production event bus.
    pub fn new() -> Self {
        let temp_dir =
            tempfile::tempdir().expect("Failed to create temporary directory for test harness");
        let ctx = Arc::new(
            sona_sqlite::SqliteApplicationContext::open(temp_dir.path())
                .expect("Failed to open SQLite application context"),
        );
        let sqlite = DesktopSqliteState::new(ctx);
        let event_bus = Arc::new(AppEventBus::new(256));
        let emitter: Arc<dyn EventEmitterPort> = event_bus.clone();

        let sync_config_path = temp_dir.path().join("sync_config.json");
        let services = DesktopServices::builder()
            .sqlite(sqlite)
            .event_emitter(emitter)
            .sync_config_path(sync_config_path)
            .build()
            .expect("Failed to build DesktopServices");

        Self {
            temp_dir,
            event_bus,
            services,
        }
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<AppEvent> {
        self.event_bus.subscribe()
    }

    pub fn temp_path(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Helper to wait for a specific named event within a timeout.
    pub async fn wait_for_event(
        rx: &mut tokio::sync::broadcast::Receiver<AppEvent>,
        event_name: &str,
        timeout: std::time::Duration,
    ) -> Result<AppEvent, String> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(format!("Timed out waiting for event '{}'", event_name));
            }
            match tokio::time::timeout(remaining, rx.recv()).await {
                Ok(Ok(event)) => {
                    if event.name == event_name {
                        return Ok(event);
                    }
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped))) => {
                    eprintln!("Event receiver lagged, skipped {skipped} events");
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                    return Err("Event bus closed".to_string());
                }
                Err(_) => {
                    return Err(format!("Timed out waiting for event '{}'", event_name));
                }
            }
        }
    }
}
