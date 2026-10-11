use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tauri::{Emitter, Manager};
use tokio::sync::watch;

use crate::platform::agent_control::facade::AgentControlFacade;

#[derive(Clone)]
pub struct ActiveConnectionTracker {
    pub(crate) count: Arc<AtomicUsize>,
    pub(crate) disconnect_tx: Arc<watch::Sender<u64>>,
    pub(crate) disconnect_rx: watch::Receiver<u64>,
}

impl Default for ActiveConnectionTracker {
    fn default() -> Self {
        let (disconnect_tx, disconnect_rx) = watch::channel(0);
        Self {
            count: Arc::new(AtomicUsize::new(0)),
            disconnect_tx: Arc::new(disconnect_tx),
            disconnect_rx,
        }
    }
}

impl ActiveConnectionTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_connected(&self, app_handle: Option<&tauri::AppHandle>) {
        self.count.fetch_add(1, Ordering::SeqCst);
        if let Some(app) = app_handle {
            crate::app::tray::set_agent_connected(app, true);
        }
    }

    pub fn on_disconnected(&self, app_handle: Option<&tauri::AppHandle>) {
        let mut current = self.count.load(Ordering::SeqCst);
        let prev = loop {
            if current == 0 {
                break 0;
            }
            match self.count.compare_exchange_weak(
                current,
                current - 1,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(prev) => break prev,
                Err(actual) => current = actual,
            }
        };
        if prev == 1
            && let Some(app) = app_handle
        {
            crate::app::tray::set_agent_connected(app, false);
        }
    }

    pub fn active_count(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }

    pub fn disconnect_all(&self, app_handle: Option<&tauri::AppHandle>) {
        self.disconnect_tx.send_modify(|v| *v += 1);
        if let Some(app) = app_handle {
            crate::app::tray::set_agent_connected(app, false);
        }
    }
}

pub fn disconnect_all_agents(app_handle: &tauri::AppHandle) {
    if let Some(tracker) = app_handle.try_state::<ActiveConnectionTracker>() {
        tracker.disconnect_all(Some(app_handle));
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
    crate::app::tray::set_agent_connected(app_handle, false);
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
            let mut stopped = false;
            if let Some(facade) = facade {
                stopped = facade
                    .stop_recording(
                        crate::platform::agent_control::facade::StopRecordingRequest {
                            discard: false,
                        },
                    )
                    .await
                    .is_ok();
            }
            if !stopped && let Some(services) = services.as_ref() {
                crate::integrations::audio::stop_all_captures(&services.audio).await;
            }
        }
        let _ = app_handle_clone.emit("tray-stop-recording", ());
        crate::app::tray::set_recording_active(&app_handle_clone, false);
    });
}
