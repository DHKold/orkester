use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::level::Level;

/// A single structured log entry produced when [`Logger::log`] is called.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Log {
    /// UTC timestamp at the moment the entry was created.
    pub datetime: DateTime<Utc>,
    /// Severity level.
    pub level: Level,
    /// Identifies the component or logger that produced this entry.
    pub source: String,
    /// Optional free-form tags for filtering or grouping.
    pub tags: Vec<String>,
    /// Human-readable log message.
    pub message: String,
}

impl Log {
    pub(crate) fn new(
        level: Level,
        source: impl Into<String>,
        tags: Vec<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            datetime: Utc::now(),
            level,
            source: source.into(),
            tags,
            message: message.into(),
        }
    }
}
