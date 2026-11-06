//! Native auto-start registration helper using `auto-launch`.
//!
//! Configures the application to run automatically on system boot.

pub use auto_launch::{AutoLaunch, AutoLaunchBuilder};
use std::env;

pub const AUTO_START_ARG: &str = "--autostart";
pub const APP_NAME: &str = "Sona";

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
    let manager = build_auto_launch()?;
    manager
        .enable()
        .map_err(|e| format!("Failed to register auto-start: {e}"))
}

/// Disables automatic startup at system boot.
pub fn disable_auto_start() -> Result<(), String> {
    let manager = build_auto_launch()?;
    manager
        .disable()
        .map_err(|e| format!("Failed to unregister auto-start: {e}"))
}

/// Checks whether automatic startup is currently registered.
pub fn is_auto_start_enabled() -> Result<bool, String> {
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
}
