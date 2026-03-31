use chrono::{DateTime, Utc};

use orkester_plugin::logging::{LogRecord, LogLevel};
use super::LogFormatter;

pub struct ConsoleLogFormatter;

impl ConsoleLogFormatter {
    fn format_timestamp(&self, timestamp_ms: u64) -> String {
        let dt: DateTime<Utc> = DateTime::from_timestamp((timestamp_ms / 1000) as i64, 0).expect("Invalid timestamp");
        dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string()
    }
}

impl LogFormatter for ConsoleLogFormatter {
    fn format(&self, r: &LogRecord) -> String {
        let color_code = match r.level {
            LogLevel::Trace => "\x1b[90m", // Bright Black
            LogLevel::Debug => "\x1b[34m", // Blue
            LogLevel::Info  => "\x1b[32m", // Green
            LogLevel::Warn  => "\x1b[33m", // Yellow
            LogLevel::Error => "\x1b[31m", // Red
        };

        let level = r.level.to_string().to_uppercase();
        let date = self.format_timestamp(r.timestamp_ms);
        format!("{}[{}] {:>5} {} (in: {})\x1b[0m", color_code, date, level, r.message, r.target)
    }
}
