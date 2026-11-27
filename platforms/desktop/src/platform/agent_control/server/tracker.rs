use parking_lot::Mutex;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tokio::sync::watch;

use crate::platform::agent_control::facade::AgentControlFacade;

#[derive(Default)]
struct ConnectionTrackerState {
    generation: u64,
    active_in_gen: usize,
}

#[derive(Clone)]
pub struct ActiveConnectionTracker {
    state: Arc<Mutex<ConnectionTrackerState>>,
    pub(crate) disconnect_tx: Arc<watch::Sender<u64>>,
    pub(crate) disconnect_rx: watch::Receiver<u64>,
}

impl Default for ActiveConnectionTracker {
    fn default() -> Self {
        let (disconnect_tx, disconnect_rx) = watch::channel(0);
        Self {
            state: Arc::new(Mutex::new(ConnectionTrackerState::default())),
            disconnect_tx: Arc::new(disconnect_tx),
            disconnect_rx,
        }
    }
}

impl ActiveConnectionTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_connected(&self, app_handle: Option<&tauri::AppHandle>) -> u64 {
        let (generation, is_connected) = {
            let mut guard = self.state.lock();
            guard.active_in_gen += 1;
            (guard.generation, guard.active_in_gen > 0)
        };
        if let Some(app) = app_handle {
            crate::app::tray::set_agent_connected(app, is_connected);
        }
        generation
    }

    pub fn on_disconnected(&self, app_handle: Option<&tauri::AppHandle>) {
        let is_connected = {
            let mut guard = self.state.lock();
            guard.active_in_gen = guard.active_in_gen.saturating_sub(1);
            guard.active_in_gen > 0
        };
        if let Some(app) = app_handle {
            crate::app::tray::set_agent_connected(app, is_connected);
        }
    }

    pub fn on_disconnected_generation(
        &self,
        generation: u64,
        app_handle: Option<&tauri::AppHandle>,
    ) {
        let is_connected = {
            let mut guard = self.state.lock();
            // Ignore connections invalidated by an earlier `disconnect_all` so their teardown
            // cannot decrement newer connections or briefly flip the tray back to connected.
            if guard.generation == generation {
                guard.active_in_gen = guard.active_in_gen.saturating_sub(1);
            }
            guard.active_in_gen > 0
        };
        if let Some(app) = app_handle {
            crate::app::tray::set_agent_connected(app, is_connected);
        }
    }

    pub fn active_count(&self) -> usize {
        self.state.lock().active_in_gen
    }

    pub fn disconnect_all(&self, app_handle: Option<&tauri::AppHandle>) {
        // Publish the watch update under the same lock as `state.generation` so a concurrent
        // `on_connected` cannot observe the new generation before `disconnect_tx` advances.
        let mut guard = self.state.lock();
        guard.generation = guard.generation.wrapping_add(1);
        guard.active_in_gen = 0;
        let _ = self.disconnect_tx.send_replace(guard.generation);
        drop(guard);

        if let Some(app) = app_handle {
            crate::app::tray::set_agent_connected(app, false);
        }
    }
}

pub fn disconnect_all_agents(app_handle: &tauri::AppHandle) {
    if let Some(tracker) = app_handle.try_state::<ActiveConnectionTracker>() {
        tracker.disconnect_all(Some(app_handle));
    } else {
        crate::app::tray::set_agent_connected(app_handle, false);
    }
    if let Some(facade) = app_handle.try_state::<AgentControlFacade>() {
        let facade_clone = facade.inner().clone();
        tauri::async_runtime::spawn(async move {
            let _ = facade_clone
                .stop_recording(
                    crate::platform::agent_control::facade::StopRecordingRequest { discard: false },
                )
                .await;
        });
    }
}

pub fn stop_recording_from_tray(app_handle: &tauri::AppHandle) {
    let facade = app_handle
        .try_state::<AgentControlFacade>()
        .map(|f| f.inner().clone());
    let services = app_handle
        .try_state::<crate::services::DesktopServices>()
        .map(|s| s.inner().clone());
    let app_handle_clone = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let is_ui = if let Some(services) = services.as_ref() {
            services
                .recording_session
                .lock()
                .await
                .as_ref()
                .map(|s| s.is_ui_session)
                .unwrap_or(false)
        } else {
            false
        };

        if !is_ui {
            if let Some(facade) = facade {
                let _ = facade
                    .stop_recording(
                        crate::platform::agent_control::facade::StopRecordingRequest {
                            discard: false,
                        },
                    )
                    .await;
            }
            crate::app::tray::set_recording_active(&app_handle_clone, false);
        }
        let _ = app_handle_clone.emit("tray-stop-recording", ());
    });
}
