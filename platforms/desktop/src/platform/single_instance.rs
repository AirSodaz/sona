//! Single-instance application lock enforcement.
//!
//! Prevents multiple simultaneous Sona instances from accessing the SQLite
//! WAL database concurrently, and un-minimizes/focuses the active instance on Windows.

use single_instance::SingleInstance;

/// Unique system-wide lock identifier for Sona Desktop.
pub const SONA_SINGLE_INSTANCE_LOCK_NAME: &str = "sona-desktop-lock";

/// Guard that holds the system single-instance lock.
///
/// MUST be kept in scope for the entire duration of the application.
/// Dropping this struct releases the OS-level lock.
pub struct SingleInstanceGuard {
    _instance: SingleInstance,
}

impl SingleInstanceGuard {
    /// Attempts to acquire the default application lock ("sona-desktop-lock").
    ///
    /// - Returns `Ok(Some(guard))` if this is the primary and only running instance.
    /// - Returns `Ok(None)` if another instance is already running.
    /// - Returns `Err(error)` if the underlying OS primitive failed to initialize.
    pub fn acquire() -> Result<Option<Self>, single_instance::error::SingleInstanceError> {
        Self::acquire_named(SONA_SINGLE_INSTANCE_LOCK_NAME)
    }

    /// Attempts to acquire a named single-instance lock.
    pub fn acquire_named(
        name: &str,
    ) -> Result<Option<Self>, single_instance::error::SingleInstanceError> {
        let instance = SingleInstance::new(name)?;
        if instance.is_single() {
            Ok(Some(Self {
                _instance: instance,
            }))
        } else {
            Ok(None)
        }
    }

    /// Checks if this instance currently holds the unique single instance lock.
    pub fn is_single(&self) -> bool {
        self._instance.is_single()
    }
}

/// On Windows, searches for an existing top-level window with the specified title
/// and restores and activates it.
#[cfg(target_os = "windows")]
pub fn focus_existing_window(window_title: &str) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, SW_RESTORE, SetForegroundWindow, ShowWindow,
    };
    use windows::core::HSTRING;

    unsafe {
        let title = HSTRING::from(window_title);
        if let Ok(hwnd) = FindWindowW(None, &title) {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = SetForegroundWindow(hwnd);
            true
        } else {
            false
        }
    }
}

/// Fallback for non-Windows platforms.
#[cfg(not(target_os = "windows"))]
pub fn focus_existing_window(_window_title: &str) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquires_unique_named_lock() {
        let lock_name = format!("sona-test-lock-{}", std::process::id());
        let guard = SingleInstanceGuard::acquire_named(&lock_name)
            .expect("failed to attempt lock acquisition");
        assert!(
            guard.is_some(),
            "first instance should successfully acquire lock"
        );

        // Second acquisition of the same lock name must return None
        let second_guard = SingleInstanceGuard::acquire_named(&lock_name)
            .expect("failed to attempt second lock acquisition");
        assert!(
            second_guard.is_none(),
            "second instance must detect conflict"
        );

        // Release first guard
        drop(guard);

        // After dropping, acquisition should succeed again
        let third_guard = SingleInstanceGuard::acquire_named(&lock_name)
            .expect("failed to attempt third lock acquisition");
        assert!(
            third_guard.is_some(),
            "re-acquisition after release should succeed"
        );
    }
}
