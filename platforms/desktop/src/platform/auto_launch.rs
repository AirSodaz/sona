//! Native auto-start registration helper using `auto-launch`.
//!
//! Configures the application to run automatically on system boot.

pub use auto_launch::{AutoLaunch, AutoLaunchBuilder};
use parking_lot::RwLock;
use std::env;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub const AUTO_START_ARG: &str = "--autostart";
pub const APP_NAME: &str = "Sona";

/// Trait representing an auto-launch manager capable of querying and toggling autostart.
pub trait AutoLaunchManager: Send + Sync {
    fn is_enabled(&self) -> Result<bool, String>;
    fn enable(&self) -> Result<(), String>;
    fn disable(&self) -> Result<(), String>;
    fn get_app_name(&self) -> &str;
    fn get_app_path(&self) -> &str;
}

/// Real implementation delegating to the `auto-launch` crate.
pub struct RealAutoLaunch(pub AutoLaunch);

impl AutoLaunchManager for RealAutoLaunch {
    fn is_enabled(&self) -> Result<bool, String> {
        self.0.is_enabled().map_err(|e| e.to_string())
    }

    fn enable(&self) -> Result<(), String> {
        self.0.enable().map_err(|e| e.to_string())
    }

    fn disable(&self) -> Result<(), String> {
        self.0.disable().map_err(|e| e.to_string())
    }

    fn get_app_name(&self) -> &str {
        self.0.get_app_name()
    }

    fn get_app_path(&self) -> &str {
        self.0.get_app_path()
    }
}

/// In-memory mock implementation of `AutoLaunchManager` for testing without touching OS registry.
#[derive(Debug, Clone)]
pub struct MockAutoLaunch {
    app_name: String,
    app_path: String,
    args: Vec<String>,
    enabled: Arc<AtomicBool>,
}

impl MockAutoLaunch {
    pub fn new(
        app_name: impl Into<String>,
        app_path: impl Into<String>,
        args: &[impl AsRef<str>],
    ) -> Self {
        Self {
            app_name: app_name.into(),
            app_path: app_path.into(),
            args: args.iter().map(|s| s.as_ref().to_string()).collect(),
            enabled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn with_enabled(self, enabled: bool) -> Self {
        self.enabled.store(enabled, Ordering::SeqCst);
        self
    }

    pub fn get_args(&self) -> &[String] {
        &self.args
    }
}

impl AutoLaunchManager for MockAutoLaunch {
    fn is_enabled(&self) -> Result<bool, String> {
        Ok(self.enabled.load(Ordering::SeqCst))
    }

    fn enable(&self) -> Result<(), String> {
        self.enabled.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn disable(&self) -> Result<(), String> {
        self.enabled.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn get_app_name(&self) -> &str {
        &self.app_name
    }

    fn get_app_path(&self) -> &str {
        &self.app_path
    }
}

static MOCK_PROVIDER: RwLock<Option<Arc<dyn AutoLaunchManager>>> = RwLock::new(None);

/// Overrides the auto-launch manager with a mock implementation for testing.
pub fn set_mock_manager(mock: Option<Arc<dyn AutoLaunchManager>>) {
    *MOCK_PROVIDER.write() = mock;
}

/// Clears any registered mock manager.
pub fn clear_mock_manager() {
    *MOCK_PROVIDER.write() = None;
}

/// Constructs an `AutoLaunch` manager bound to the current executable.
pub fn build_auto_launch() -> Result<AutoLaunch, String> {
    let current_exe = env::current_exe()
        .map_err(|e| format!("Failed to resolve current executable path: {e}"))?;

    let app_path = current_exe
        .to_str()
        .ok_or_else(|| "Current executable path is not valid UTF-8".to_string())?;

    AutoLaunchBuilder::new()
        .set_app_name(APP_NAME)
        .set_app_path(app_path)
        .set_args(&[AUTO_START_ARG])
        .build()
        .map_err(|e| format!("Failed to build auto-launch manager: {e}"))
}

/// Enables automatic startup at system boot.
pub fn enable_auto_start() -> Result<(), String> {
    if let Some(mock) = MOCK_PROVIDER.read().as_ref() {
        return mock.enable();
    }
    let manager = build_auto_launch()?;
    manager
        .enable()
        .map_err(|e| format!("Failed to register auto-start: {e}"))
}

/// Disables automatic startup at system boot.
pub fn disable_auto_start() -> Result<(), String> {
    if let Some(mock) = MOCK_PROVIDER.read().as_ref() {
        return mock.disable();
    }
    let manager = build_auto_launch()?;
    manager
        .disable()
        .map_err(|e| format!("Failed to unregister auto-start: {e}"))
}

/// Checks whether automatic startup is currently registered.
pub fn is_auto_start_enabled() -> Result<bool, String> {
    if let Some(mock) = MOCK_PROVIDER.read().as_ref() {
        return mock.is_enabled();
    }
    let manager = build_auto_launch()?;
    manager
        .is_enabled()
        .map_err(|e| format!("Failed to query auto-start status: {e}"))
}

/// Toggles auto-start registration based on boolean setting.
pub fn set_auto_start(enabled: bool) -> Result<(), String> {
    if enabled {
        enable_auto_start()
    } else {
        disable_auto_start()
    }
}

/// Status payload for auto-launch configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AutoLaunchStatus {
    pub enabled: bool,
}

pub fn get_auto_launch_status() -> Result<AutoLaunchStatus, String> {
    let enabled = is_auto_start_enabled()?;
    Ok(AutoLaunchStatus { enabled })
}

pub fn enable_auto_launch() -> Result<(), String> {
    enable_auto_start()
}

pub fn disable_auto_launch() -> Result<(), String> {
    disable_auto_start()
}

pub fn is_auto_launch_enabled() -> Result<bool, String> {
    is_auto_start_enabled()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_auto_launch_manager() {
        let manager_res = build_auto_launch();
        assert!(
            manager_res.is_ok(),
            "AutoLaunchBuilder should resolve current_exe cleanly: {:?}",
            manager_res.err()
        );
        let manager = manager_res.unwrap();
        assert_eq!(manager.get_app_name(), APP_NAME);
    }

    #[test]
    fn mock_auto_launch_lifecycle() {
        let mock = MockAutoLaunch::new("TestApp", "test.exe", &["--autostart"]);
        assert_eq!(mock.get_app_name(), "TestApp");
        assert_eq!(mock.get_app_path(), "test.exe");
        assert_eq!(mock.get_args(), &["--autostart"]);
        assert!(!mock.is_enabled().unwrap());

        mock.enable().unwrap();
        assert!(mock.is_enabled().unwrap());

        mock.disable().unwrap();
        assert!(!mock.is_enabled().unwrap());
    }

    #[test]
    fn mock_provider_delegation() {
        let mock = Arc::new(MockAutoLaunch::new("TestApp", "test.exe", &["--autostart"]));
        set_mock_manager(Some(mock));

        assert!(!is_auto_start_enabled().unwrap());
        assert!(!get_auto_launch_status().unwrap().enabled);

        enable_auto_launch().unwrap();
        assert!(is_auto_start_enabled().unwrap());
        assert!(get_auto_launch_status().unwrap().enabled);

        disable_auto_launch().unwrap();
        assert!(!is_auto_start_enabled().unwrap());
        assert!(!get_auto_launch_status().unwrap().enabled);

        clear_mock_manager();
    }
}
