use serde::{Deserialize, Serialize};
use super::level::LogLevel;

/// A structured log record emitted by plugin code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRecord {
    /// Severity level.
    pub level:        LogLevel,
    /// Rust module path of the call site (e.g. `my_plugin::router`).
    pub target:       String,
    /// Human-readable message.
    pub message:      String,
    /// Source file name.
    pub file:         String,
    /// Source line number.
    pub line:         u32,
    /// Milliseconds since Unix epoch.
    pub timestamp_ms: u64,
    /// Plugin identity set by `init_logging` (package name).
    pub plugin_id:    String,
}
