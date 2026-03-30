use orkester_plugin::logging::LogRecord;
use super::LogFormatter;

pub struct ConsoleLogFormatter;

impl LogFormatter for ConsoleLogFormatter {
    fn format(&self, r: &LogRecord) -> String {
        format!("[{}] {:>5} {} {} - {}", r.timestamp_ms, r.level, r.plugin_id, r.target, r.message)
    }
}
