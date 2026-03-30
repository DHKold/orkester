use orkester_plugin::logging::LogRecord;
use super::LogFormatter;

pub struct JsonLogFormatter;

impl LogFormatter for JsonLogFormatter {
    fn format(&self, r: &LogRecord) -> String {
        serde_json::to_string(r).unwrap_or_else(|_| format!(r#"{{"error":"serialize failed"}}"#))
    }
}
