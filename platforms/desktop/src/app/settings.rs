use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU8, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum AppLogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

impl AppLogLevel {
    fn rank(self) -> u8 {
        self as u8
    }

    fn from_rank(rank: u8) -> Self {
        match rank {
            0 => Self::Trace,
            1 => Self::Debug,
            2 => Self::Info,
            3 => Self::Warn,
            4 => Self::Error,
            _ => Self::Info,
        }
    }

    pub(crate) fn to_level_filter(self) -> log::LevelFilter {
        match self {
            Self::Trace => log::LevelFilter::Trace,
            Self::Debug => log::LevelFilter::Debug,
            Self::Info => log::LevelFilter::Info,
            Self::Warn => log::LevelFilter::Warn,
            Self::Error => log::LevelFilter::Error,
        }
    }
}

#[derive(Clone)]
pub(crate) struct RuntimeLogLevel {
    level: Arc<AtomicU8>,
}

impl RuntimeLogLevel {
    fn new() -> Self {
        Self {
            level: Arc::new(AtomicU8::new(AppLogLevel::Info.rank())),
        }
    }

    pub(crate) fn current_log_level(&self) -> AppLogLevel {
        AppLogLevel::from_rank(self.level.load(Ordering::Relaxed))
    }

    fn set_log_level(&self, level: AppLogLevel) {
        self.level.store(level.rank(), Ordering::Relaxed);
    }

    pub(crate) fn should_log(&self, metadata: &log::Metadata<'_>) -> bool {
        should_log_target_level(
            self.current_log_level(),
            metadata.level(),
            metadata.target(),
        )
    }
}

pub struct AppSettings {
    minimize_to_tray: Mutex<bool>,
    log_level: RuntimeLogLevel,
}

impl AppSettings {
    pub(crate) fn new() -> Self {
        Self {
            minimize_to_tray: Mutex::new(true),
            log_level: RuntimeLogLevel::new(),
        }
    }

    pub(crate) fn log_level_filter(&self) -> RuntimeLogLevel {
        self.log_level.clone()
    }

    pub(crate) fn minimize_to_tray(&self) -> bool {
        self.minimize_to_tray
            .lock()
            .map(|value| *value)
            .unwrap_or(true)
    }

    pub(crate) fn set_minimize_to_tray_enabled(&self, enabled: bool) {
        if let Ok(mut minimize) = self.minimize_to_tray.lock() {
            *minimize = enabled;
        }
    }

    pub(crate) fn current_log_level(&self) -> AppLogLevel {
        self.log_level.current_log_level()
    }

    fn set_log_level(&self, level: AppLogLevel) {
        self.log_level.set_log_level(level);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MainWindowCloseAction {
    Ignore,
    HideToTray,
    RequestQuit,
}

pub(crate) fn set_minimize_to_tray(state: tauri::State<'_, AppSettings>, enabled: bool) {
    state.set_minimize_to_tray_enabled(enabled);
}

pub(crate) fn set_log_level(
    state: tauri::State<'_, AppSettings>,
    level: String,
) -> Result<(), String> {
    let parsed =
        parse_log_level(&level).ok_or_else(|| format!("Unsupported log level: {level}"))?;
    state.set_log_level(parsed);
    log::set_max_level(parsed.to_level_filter());
    Ok(())
}

pub(crate) fn parse_log_level(level: &str) -> Option<AppLogLevel> {
    match level.trim().to_ascii_lowercase().as_str() {
        "trace" => Some(AppLogLevel::Trace),
        "debug" => Some(AppLogLevel::Debug),
        "info" => Some(AppLogLevel::Info),
        "warn" => Some(AppLogLevel::Warn),
        "error" => Some(AppLogLevel::Error),
        _ => None,
    }
}

pub(crate) fn is_noisy_external_target(target: &str) -> bool {
    const NOISY_PREFIXES: &[&str] = &[
        "hyper",
        "tokio",
        "mio",
        "h2",
        "symphonia",
        "reqwest",
        "rusqlite",
        "tungstenite",
        "tracing",
        "tao",
        "wry",
    ];
    NOISY_PREFIXES.iter().any(|prefix| {
        target == *prefix
            || target
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with("::") || rest.starts_with('_'))
    })
}

pub(crate) fn should_log_target_level(
    configured_level: AppLogLevel,
    record_level: log::Level,
    target: &str,
) -> bool {
    if !should_log_level(configured_level, record_level) {
        return false;
    }
    if is_noisy_external_target(target) {
        return record_level_rank(record_level) >= AppLogLevel::Warn.rank();
    }
    true
}

pub(crate) fn should_log_level(configured_level: AppLogLevel, record_level: log::Level) -> bool {
    record_level_rank(record_level) >= configured_level.rank()
}

fn record_level_rank(level: log::Level) -> u8 {
    match level {
        log::Level::Trace => AppLogLevel::Trace.rank(),
        log::Level::Debug => AppLogLevel::Debug.rank(),
        log::Level::Info => AppLogLevel::Info.rank(),
        log::Level::Warn => AppLogLevel::Warn.rank(),
        log::Level::Error => AppLogLevel::Error.rank(),
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
    use super::{
        AppLogLevel, AppSettings, MainWindowCloseAction, is_noisy_external_target, parse_log_level,
        resolve_main_window_close_action, should_log_level, should_log_target_level,
    };

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

    #[test]
    fn default_log_level_is_info() {
        let settings = AppSettings::new();

        assert_eq!(settings.current_log_level(), AppLogLevel::Info);
    }

    #[test]
    fn app_settings_tracks_minimize_to_tray_via_methods() {
        let settings = AppSettings::new();

        assert!(settings.minimize_to_tray());

        settings.set_minimize_to_tray_enabled(false);

        assert!(!settings.minimize_to_tray());
    }

    #[test]
    fn parses_supported_log_levels_case_insensitively() {
        assert_eq!(parse_log_level("trace"), Some(AppLogLevel::Trace));
        assert_eq!(parse_log_level("DEBUG"), Some(AppLogLevel::Debug));
        assert_eq!(parse_log_level("info"), Some(AppLogLevel::Info));
        assert_eq!(parse_log_level("warn"), Some(AppLogLevel::Warn));
        assert_eq!(parse_log_level("error"), Some(AppLogLevel::Error));
        assert_eq!(parse_log_level("verbose"), None);
    }

    #[test]
    fn filters_records_at_or_above_the_configured_level() {
        assert!(should_log_level(AppLogLevel::Info, log::Level::Info));
        assert!(should_log_level(AppLogLevel::Info, log::Level::Warn));
        assert!(should_log_level(AppLogLevel::Info, log::Level::Error));
        assert!(!should_log_level(AppLogLevel::Info, log::Level::Debug));
        assert!(!should_log_level(AppLogLevel::Info, log::Level::Trace));
    }

    #[test]
    fn suppresses_noisy_external_targets_when_debug_enabled() {
        assert!(is_noisy_external_target("hyper"));
        assert!(is_noisy_external_target("hyper::proto::h1::io"));
        assert!(is_noisy_external_target("tokio::runtime"));
        assert!(is_noisy_external_target("symphonia_bundle_mp3"));
        assert!(is_noisy_external_target("reqwest::connect"));
        assert!(!is_noisy_external_target("sona::app"));
        assert!(!is_noisy_external_target("webview"));
        assert!(!is_noisy_external_target("my_tokio"));
        assert!(!is_noisy_external_target("tokiox"));

        // Application targets allow debug when configured level is Debug
        assert!(should_log_target_level(
            AppLogLevel::Debug,
            log::Level::Debug,
            "sona::app"
        ));
        assert!(should_log_target_level(
            AppLogLevel::Debug,
            log::Level::Debug,
            "webview"
        ));

        // Noisy external targets suppress debug and info, only allowing warn and error
        assert!(!should_log_target_level(
            AppLogLevel::Debug,
            log::Level::Debug,
            "hyper::proto"
        ));
        assert!(!should_log_target_level(
            AppLogLevel::Debug,
            log::Level::Info,
            "hyper::proto"
        ));
        assert!(should_log_target_level(
            AppLogLevel::Debug,
            log::Level::Warn,
            "hyper::proto"
        ));
        assert!(should_log_target_level(
            AppLogLevel::Debug,
            log::Level::Error,
            "hyper::proto"
        ));
    }
}
