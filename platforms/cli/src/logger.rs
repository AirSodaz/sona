use log::{LevelFilter, Log, Metadata, Record};
use std::io::Write;

struct CliLogger;

impl Log for CliLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            eprintln!("[{} {}] {}", record.level(), record.target(), record.args());
        }
    }

    fn flush(&self) {
        let _ = std::io::stderr().flush();
    }
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
