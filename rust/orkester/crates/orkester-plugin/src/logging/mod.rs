pub mod level;
pub mod record;
mod buffer;
mod state;

pub use level::LogLevel;
pub use record::LogRecord;
pub use state::{init_logging, plugin_id, send_log};

/// Current milliseconds since the Unix epoch.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
