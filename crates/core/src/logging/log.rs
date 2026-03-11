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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_populates_all_fields() {
        let before = Utc::now();
        let log = Log::new(Level::INFO, "test-source", vec!["tag1".into()], "hello");
        let after = Utc::now();

        assert_eq!(log.level, Level::INFO);
        assert_eq!(log.source, "test-source");
        assert_eq!(log.tags, vec!["tag1"]);
        assert_eq!(log.message, "hello");
        assert!(log.datetime >= before && log.datetime <= after);
    }

    #[test]
    fn new_with_empty_tags() {
        let log = Log::new(Level::WARN, "src", vec![], "msg");
        assert!(log.tags.is_empty());
    }

    #[test]
    fn serializes_to_json_with_all_expected_fields() {
        let log = Log::new(Level::ERROR, "auth", vec!["req:123".into()], "failed");
        let json = serde_json::to_string(&log).unwrap();

        assert!(json.contains("\"level\""));
        assert!(json.contains("\"source\":\"auth\""));
        assert!(json.contains("\"message\":\"failed\""));
        assert!(json.contains("\"tags\""));
        assert!(json.contains("\"datetime\""));
        assert!(json.contains("req:123"));
    }

    #[test]
    fn deserializes_from_json_round_trip() {
        let original = Log::new(Level::DEBUG, "svc", vec!["a".into()], "round-trip");
        let json = serde_json::to_string(&original).unwrap();
        let restored: Log = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.level, original.level);
        assert_eq!(restored.source, original.source);
        assert_eq!(restored.tags, original.tags);
        assert_eq!(restored.message, original.message);
    }
}
