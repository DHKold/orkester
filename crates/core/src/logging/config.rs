//! Logging configuration types

#[derive(Debug, Clone)]
pub struct LoggingConfig {
    pub log_format: LogFormat,
    pub log_level: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            log_format: LogFormat::Json,
            log_level: "info".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Json,
    Plain,
}
