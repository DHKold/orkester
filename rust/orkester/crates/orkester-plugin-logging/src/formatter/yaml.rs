use orkester_plugin::logging::LogRecord;
use super::LogFormatter;

pub struct YamlLogFormatter;

impl LogFormatter for YamlLogFormatter {
    fn format(&self, r: &LogRecord) -> String {
        serde_yaml::to_string(r).unwrap_or_else(|_| format!("error: serialize failed\n"))
    }
}
