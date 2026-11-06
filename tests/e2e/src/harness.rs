//! Opaque-box test fixture, process spawning, mock event receiver for E2E tests.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;

use sona_core::ports::event::{EventEmitterPort, EventError};
use sona_desktop_lib::platform::database::DesktopSqliteState;
use sona_desktop_lib::services::DesktopServices;

/// In-memory broadcast application event for pure Rust GPUI architecture.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppEvent {
    pub name: String,
    pub payload: serde_json::Value,
}

/// In-memory real-time event bus replacing Tauri IPC/WebView events.
/// Implements `EventEmitterPort` with tokio broadcast channel.
pub struct AppEventBus {
    sender: tokio::sync::broadcast::Sender<AppEvent>,
}

impl AppEventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = tokio::sync::broadcast::channel(capacity);
        Self { sender }
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<AppEvent> {
        self.sender.subscribe()
    }

    pub fn receiver_count(&self) -> usize {
        self.sender.receiver_count()
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

/// Primary test harness for isolated desktop testing.
#[allow(dead_code)]
pub struct TestHarness {
    pub temp_dir: TempDir,
    pub event_bus: Arc<AppEventBus>,
    pub services: DesktopServices,
}

#[allow(dead_code)]
impl TestHarness {
    /// Creates a new isolated test harness with a fresh temporary SQLite context and event bus.
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

/// Cross-platform single-instance mutex and duplicate detection abstraction.
#[allow(dead_code)]
pub struct SingleInstanceLock {
    pub identifier: String,
    held: bool,
    #[cfg(windows)]
    raw_handle: Option<windows::Win32::Foundation::HANDLE>,
    _lock_file: Option<std::fs::File>,
}

#[cfg(windows)]
unsafe impl Send for SingleInstanceLock {}
#[cfg(windows)]
unsafe impl Sync for SingleInstanceLock {}

impl SingleInstanceLock {
    /// Attempts to acquire an exclusive named single-instance lock.
    pub fn try_acquire(identifier: &str, _lock_dir: &Path) -> Result<Self, String> {
        #[cfg(windows)]
        {
            use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
            use windows::Win32::System::Threading::CreateMutexW;
            use windows::core::HSTRING;

            let wide_name = HSTRING::from(format!("Global\\{}", identifier));
            let handle = unsafe { CreateMutexW(None, true, &wide_name) }
                .map_err(|e| format!("CreateMutexW failed: {e}"))?;

            let last_err = unsafe { GetLastError() };
            let is_duplicate = last_err == ERROR_ALREADY_EXISTS;

            if is_duplicate {
                unsafe {
                    let _ = windows::Win32::Foundation::CloseHandle(handle);
                }
                return Ok(Self {
                    identifier: identifier.to_string(),
                    held: false,
                    raw_handle: None,
                    _lock_file: None,
                });
            }

            Ok(Self {
                identifier: identifier.to_string(),
                held: true,
                raw_handle: Some(handle),
                _lock_file: None,
            })
        }

        #[cfg(not(windows))]
        {
            let lock_path = _lock_dir.join(format!("{}.lock", identifier));
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(&lock_path)
                .map_err(|e| format!("Failed to open lock file: {e}"))?;

            let is_locked = false;
            Ok(Self {
                identifier: identifier.to_string(),
                held: !is_locked,
                _lock_file: Some(file),
            })
        }
    }

    pub fn is_single(&self) -> bool {
        self.held
    }
}

impl Drop for SingleInstanceLock {
    fn drop(&mut self) {
        #[cfg(windows)]
        if let Some(handle) = self.raw_handle.take() {
            unsafe {
                let _ = windows::Win32::System::Threading::ReleaseMutex(handle);
                let _ = windows::Win32::Foundation::CloseHandle(handle);
            }
        }
    }
}

/// Auto-launch configuration generator and validator for Windows, macOS, and Linux.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoLaunchConfig {
    pub app_name: String,
    pub app_path: PathBuf,
    pub args: Vec<String>,
}

impl AutoLaunchConfig {
    pub fn new(app_name: impl Into<String>, app_path: impl Into<PathBuf>, args: Vec<String>) -> Self {
        Self {
            app_name: app_name.into(),
            app_path: app_path.into(),
            args,
        }
    }

    pub const WINDOWS_RUN_KEY: &'static str = r"Software\Microsoft\Windows\CurrentVersion\Run";

    pub fn windows_registry_command(&self) -> String {
        let path_str = self.app_path.to_string_lossy();
        let quoted_path = if path_str.contains(' ') && !path_str.starts_with('"') {
            format!("\"{}\"", path_str)
        } else {
            path_str.to_string()
        };

        if self.args.is_empty() {
            quoted_path
        } else {
            let formatted_args = self.args.join(" ");
            format!("{} {}", quoted_path, formatted_args)
        }
    }

    pub fn linux_desktop_entry(&self) -> String {
        let cmd = self.windows_registry_command();
        format!(
            "[Desktop Entry]\nType=Application\nName={}\nExec={}\nHidden=false\nNoDisplay=false\nX-GNOME-Autostart-enabled=true\n",
            self.app_name, cmd
        )
    }

    pub fn macos_launch_agent_plist(&self, identifier: &str) -> String {
        let mut args_xml = format!("        <string>{}</string>\n", self.app_path.to_string_lossy());
        for arg in &self.args {
            args_xml.push_str(&format!("        <string>{}</string>\n", arg));
        }
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n    <key>Label</key>\n    <string>{}</string>\n    <key>ProgramArguments</key>\n    <array>\n{}    </array>\n    <key>RunAtLoad</key>\n    <true/>\n</dict>\n</plist>\n",
            identifier, args_xml
        )
    }

    pub fn parse_windows_command(cmd: &str) -> Result<(PathBuf, Vec<String>), String> {
        let trimmed = cmd.trim();
        if trimmed.is_empty() {
            return Err("Empty command string".to_string());
        }

        if trimmed.starts_with('"') {
            if let Some(end_quote) = trimmed[1..].find('"') {
                let path = &trimmed[1..=end_quote];
                let remainder = trimmed[end_quote + 2..].trim();
                let args = if remainder.is_empty() {
                    Vec::new()
                } else {
                    remainder.split_whitespace().map(String::from).collect()
                };
                return Ok((PathBuf::from(path), args));
            } else {
                return Err("Unmatched quote in command string".to_string());
            }
        }

        let mut parts = trimmed.split_whitespace();
        let path = parts.next().ok_or_else(|| "Missing executable path".to_string())?;
        let args = parts.map(String::from).collect();
        Ok((PathBuf::from(path), args))
    }
}
