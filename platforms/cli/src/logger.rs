use log::{LevelFilter, Log, Metadata, Record};
use std::io::Write;

struct CliLogger;

impl Log for CliLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            eprintln!(
                "[{now}] [{} {}] {}",
                record.level(),
                record.target(),
                sona_core::runtime::logging::sanitize_log_text(&record.args().to_string())
            );
        }
    }

    fn flush(&self) {
        let _ = std::io::stderr().flush();
    }
}

pub fn setup_cli_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
            *s
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.as_str()
        } else {
            "An unexpected panic occurred."
        };
        let location = info
            .location()
            .map(|loc| format!(" at {}:{}", loc.file(), loc.line()))
            .unwrap_or_default();
        let sanitized = sona_core::runtime::logging::sanitize_log_text(message);
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        eprintln!("[{now}] [sona-cli][ERROR] Panic: {sanitized}{location}");
    }));
}

pub fn init_logger(default_level: LevelFilter) {
    let level = std::env::var("RUST_LOG")
        .ok()
        .and_then(|val| val.parse::<LevelFilter>().ok())
        .unwrap_or(default_level);

    static LOGGER: CliLogger = CliLogger;
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(level);
}
