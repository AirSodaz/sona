pub(crate) struct AppSettings {
    pub(crate) minimize_to_tray: std::sync::Mutex<bool>,
}

impl AppSettings {
    pub(crate) fn new() -> Self {
        Self {
            minimize_to_tray: std::sync::Mutex::new(true),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MainWindowCloseAction {
    Ignore,
    HideToTray,
    RequestQuit,
}

#[tauri::command]
pub(crate) fn set_minimize_to_tray(state: tauri::State<'_, AppSettings>, enabled: bool) {
    if let Ok(mut minimize) = state.minimize_to_tray.lock() {
        *minimize = enabled;
    }
}

pub(crate) fn resolve_main_window_close_action(
    window_label: &str,
    minimize_to_tray: bool,
) -> MainWindowCloseAction {
    if window_label != "main" {
        return MainWindowCloseAction::Ignore;
    }

    if minimize_to_tray {
        MainWindowCloseAction::HideToTray
    } else {
        MainWindowCloseAction::RequestQuit
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_main_window_close_action, MainWindowCloseAction};

    #[test]
    fn main_window_close_hides_to_tray_when_enabled() {
        let action = resolve_main_window_close_action("main", true);

        assert_eq!(action, MainWindowCloseAction::HideToTray);
    }

    #[test]
    fn main_window_close_requests_quit_when_tray_minimize_is_disabled() {
        let action = resolve_main_window_close_action("main", false);

        assert_eq!(action, MainWindowCloseAction::RequestQuit);
    }

    #[test]
    fn non_main_windows_are_ignored_by_quit_guard() {
        let action = resolve_main_window_close_action("caption", false);

        assert_eq!(action, MainWindowCloseAction::Ignore);
    }
}
