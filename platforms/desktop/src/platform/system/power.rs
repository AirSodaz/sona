#[derive(Debug)]
pub struct SleepPreventionGuard {
    _reason: String,
    #[cfg(windows)]
    _active: bool,
}

#[cfg(windows)]
mod windows_coordinator {
    use std::sync::LazyLock;
    use std::sync::mpsc::{Sender, channel};
    use windows::Win32::System::Power::{
        ES_AWAYMODE_REQUIRED, ES_CONTINUOUS, ES_SYSTEM_REQUIRED, SetThreadExecutionState,
    };

    enum PowerAction {
        Acquire,
        Release,
    }

    static SENDER: LazyLock<Sender<PowerAction>> = LazyLock::new(|| {
        let (tx, rx) = channel::<PowerAction>();
        let _ = std::thread::Builder::new()
                .name("sona-power-coordinator".to_string())
                .spawn(move || {
                    let mut active_count = 0usize;
                    while let Ok(action) = rx.recv() {
                        match action {
                            PowerAction::Acquire => {
                                if active_count == 0 {
                                    log::debug!("[Power] Dedicated thread acquiring sleep prevention assertion");
                                    let state = unsafe {
                                        SetThreadExecutionState(
                                            ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_AWAYMODE_REQUIRED,
                                        )
                                    };
                                    if state.0 == 0 {
                                        unsafe {
                                            SetThreadExecutionState(
                                                ES_CONTINUOUS | ES_SYSTEM_REQUIRED,
                                            );
                                        }
                                    }
                                }
                                active_count += 1;
                            }
                            PowerAction::Release => {
                                if active_count > 0 {
                                    active_count -= 1;
                                    if active_count == 0 {
                                        log::debug!("[Power] Dedicated thread releasing sleep prevention assertion");
                                        unsafe {
                                            SetThreadExecutionState(ES_CONTINUOUS);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if active_count > 0 {
                        unsafe {
                            SetThreadExecutionState(ES_CONTINUOUS);
                        }
                    }
                });
        tx
    });

    pub fn acquire() {
        let _ = SENDER.send(PowerAction::Acquire);
    }

    pub fn release() {
        let _ = SENDER.send(PowerAction::Release);
    }
}

impl SleepPreventionGuard {
    pub fn acquire(reason: &str) -> Self {
        #[cfg(windows)]
        {
            log::debug!("[Power] Acquiring sleep prevention assertion for: {reason}");
            windows_coordinator::acquire();
            Self {
                _reason: reason.to_string(),
                _active: true,
            }
        }

        #[cfg(target_os = "macos")]
        {
            log::debug!("[Power] Acquiring sleep prevention assertion for macOS (no-op): {reason}");
            Self {
                _reason: reason.to_string(),
            }
        }

        #[cfg(not(any(windows, target_os = "macos")))]
        {
            log::debug!("[Power] Sleep prevention guard created (no-op on this OS) for: {reason}");
            Self {
                _reason: reason.to_string(),
            }
        }
    }

    pub fn reason(&self) -> &str {
        &self._reason
    }
}

impl Drop for SleepPreventionGuard {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            if self._active {
                windows_coordinator::release();
                log::debug!(
                    "[Power] Released sleep prevention assertion for: {}",
                    self._reason
                );
            }
        }

        #[cfg(target_os = "macos")]
        {
            log::debug!(
                "[Power] Released macOS sleep prevention assertion for: {}",
                self._reason
            );
        }

        #[cfg(not(any(windows, target_os = "macos")))]
        {
            log::debug!(
                "[Power] Released sleep prevention guard for: {}",
                self._reason
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acquire_and_drop_sleep_prevention_guard() {
        let guard = SleepPreventionGuard::acquire("Unit Test Assertion");
        assert_eq!(guard.reason(), "Unit Test Assertion");
        drop(guard);
    }
}
