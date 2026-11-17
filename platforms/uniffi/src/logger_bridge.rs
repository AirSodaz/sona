use log::{LevelFilter, Log, Metadata, Record};
use std::sync::{Arc, Mutex, OnceLock};

#[uniffi::export(callback_interface)]
pub trait FfiLogCallback: Send + Sync {
    fn log(&self, level: String, target: String, message: String);
}

static LOGGER_STATE: OnceLock<Mutex<Option<Arc<dyn FfiLogCallback>>>> = OnceLock::new();

struct DelegatingMobileLogger;

impl Log for DelegatingMobileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let callback = LOGGER_STATE
            .get()
            .and_then(|m| m.lock().ok().and_then(|guard| guard.clone()));

        let message = sona_core::runtime::logging::sanitize_log_text(&record.args().to_string());
        if let Some(cb) = callback {
            cb.log(
                record.level().to_string(),
                record.target().to_string(),
                message,
            );
        } else {
            eprintln!(
                "[sona-mobile][{} {}] {}",
                record.level(),
                record.target(),
                message
            );
        }
    }

    fn flush(&self) {
        use std::io::Write;
        let _ = std::io::stderr().flush();
    }
}

static GLOBAL_MOBILE_LOGGER: DelegatingMobileLogger = DelegatingMobileLogger;

pub fn parse_log_level_filter(level: &str) -> LevelFilter {
    match level.trim().to_ascii_lowercase().as_str() {
        "trace" => LevelFilter::Trace,
        "debug" => LevelFilter::Debug,
        "info" => LevelFilter::Info,
        "warn" => LevelFilter::Warn,
        "error" => LevelFilter::Error,
        _ => LevelFilter::Info,
    }
}

pub fn init_mobile_logger_impl(
    callback: Option<Box<dyn FfiLogCallback>>,
    max_level: Option<String>,
) -> bool {
    let cb_arc: Option<Arc<dyn FfiLogCallback>> = callback.map(Arc::from);
    let state = LOGGER_STATE.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = state.lock() {
        *guard = cb_arc;
    }

    let filter = max_level
        .as_deref()
        .map(parse_log_level_filter)
        .unwrap_or(LevelFilter::Info);

    let _ = log::set_logger(&GLOBAL_MOBILE_LOGGER);
    log::set_max_level(filter);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestLogCallback {
        count: AtomicUsize,
    }

    impl FfiLogCallback for TestLogCallback {
        fn log(&self, _level: String, _target: String, _message: String) {
            self.count.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn parses_level_filters_safely() {
        assert_eq!(parse_log_level_filter("debug"), LevelFilter::Debug);
        assert_eq!(parse_log_level_filter("WARN"), LevelFilter::Warn);
        assert_eq!(parse_log_level_filter("invalid"), LevelFilter::Info);
    }

    #[test]
    fn logger_initialization_sets_callback_state() {
        let callback = Box::new(TestLogCallback {
            count: AtomicUsize::new(0),
        });
        assert!(init_mobile_logger_impl(
            Some(callback),
            Some("debug".to_string())
        ));
    }
}
