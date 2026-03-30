pub mod console;
pub mod json;
pub mod yaml;

pub use console::ConsoleLogFormatter;
pub use json::JsonLogFormatter;
pub use yaml::YamlLogFormatter;

use orkester_plugin::logging::LogRecord;

use crate::server::config::FormatterConfig;

/// Converts a `LogRecord` to a printable / writable string.
pub trait LogFormatter: Send + 'static {
    fn format(&self, record: &LogRecord) -> String;
}

pub fn build_formatter(cfg: Option<&FormatterConfig>) -> Box<dyn LogFormatter> {
    let kind = cfg.and_then(|c| c.kind.as_deref()).unwrap_or("logging/ConsoleLogFormatter:1.0");
    match kind {
        "logging/JsonLogFormatter:1.0"    => Box::new(JsonLogFormatter),
        "logging/YamlLogFormatter:1.0"    => Box::new(YamlLogFormatter),
        _                                 => Box::new(ConsoleLogFormatter),
    }
}
