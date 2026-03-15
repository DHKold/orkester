use std::sync::RwLock;

use crate::logging::consumer::LogConsumer;
use crate::logging::filter::LogFilter;
use crate::logging::log::Log;

// ── ConsoleConsumer ───────────────────────────────────────────────────────────

/// Prints each log entry to stdout as a human-readable text line:
///
/// ```text
/// [2026-03-11T14:22:01.042Z] INFO  [auth] (request-id:abc) user authenticated
/// ```
///
/// Call [`ConsoleConsumer::set_filter`] at any time to install or remove a
/// filter. Use [`AllFilter`][crate::logging::filter::AllFilter] /
/// [`AnyFilter`][crate::logging::filter::AnyFilter] to compose multiple conditions.
///
/// # Example
/// ```no_run
/// use orkester_common::logging::consumers::{ConsoleConsumer, MinLevel};
/// use orkester_common::logging::Level;
///
/// let consumer = ConsoleConsumer::new();
/// consumer.set_filter(Some(MinLevel::new(Level::INFO)));
/// ```
pub struct ConsoleConsumer {
    filter: RwLock<Option<Box<dyn LogFilter>>>,
}

impl ConsoleConsumer {
    /// Creates a new consumer with no filter (accepts every log entry).
    pub fn new() -> Self {
        Self {
            filter: RwLock::new(None),
        }
    }

    fn passes(&self, log: &Log) -> bool {
        match &*self.filter.read().unwrap() {
            Some(f) => f.matches(log),
            None => true,
        }
    }
}

impl Default for ConsoleConsumer {
    fn default() -> Self {
        Self::new()
    }
}

impl LogConsumer for ConsoleConsumer {
    fn consume(&self, log: &Log) {
        if !self.passes(log) {
            return;
        }
        let tags = if log.tags.is_empty() {
            String::new()
        } else {
            format!("({}) ", log.tags.join(", "))
        };
        println!(
            "[{}] {:<5} [{}] {}{}",
            log.datetime.format("%Y-%m-%dT%H:%M:%S%.3fZ"),
            log.level.to_string(),
            log.source,
            tags,
            log.message,
        );
    }

    fn set_filter(&self, filter: Option<Box<dyn LogFilter + 'static>>) {
        *self.filter.write().unwrap() = filter;
    }
}

// ── ConsoleJsonConsumer ───────────────────────────────────────────────────────

/// Prints each log entry to stdout as a single JSON line.
///
/// ```json
/// {"datetime":"2026-03-11T14:22:01.042Z","level":20,"source":"auth","tags":[],"message":"user authenticated"}
/// ```
///
/// Call [`ConsoleJsonConsumer::set_filter`] at any time to install or remove a
/// filter.
pub struct ConsoleJsonConsumer {
    filter: RwLock<Option<Box<dyn LogFilter>>>,
}

impl ConsoleJsonConsumer {
    /// Creates a new consumer with no filter.
    pub fn new() -> Self {
        Self {
            filter: RwLock::new(None),
        }
    }

    fn passes(&self, log: &Log) -> bool {
        match &*self.filter.read().unwrap() {
            Some(f) => f.matches(log),
            None => true,
        }
    }
}

impl Default for ConsoleJsonConsumer {
    fn default() -> Self {
        Self::new()
    }
}

impl LogConsumer for ConsoleJsonConsumer {
    fn consume(&self, log: &Log) {
        if !self.passes(log) {
            return;
        }
        match serde_json::to_string(log) {
            Ok(json) => println!("{}", json),
            Err(e) => eprintln!("[logging] failed to serialize log entry: {}", e),
        }
    }

    fn set_filter(&self, filter: Option<Box<dyn LogFilter + 'static>>) {
        *self.filter.write().unwrap() = filter;
    }
}

#[cfg(test)]
mod tests {
    use crate::logging::{level::Level, log::Log};

    fn make_log(level: Level, source: &str, tags: Vec<String>, msg: &str) -> Log {
        Log::new(level, source, tags, msg)
    }

    fn format_plain(log: &Log) -> String {
        let tags = if log.tags.is_empty() {
            String::new()
        } else {
            format!("({}) ", log.tags.join(", "))
        };
        format!(
            "[{}] {:<5} [{}] {}{}",
            log.datetime.format("%Y-%m-%dT%H:%M:%S%.3fZ"),
            log.level.to_string(),
            log.source,
            tags,
            log.message,
        )
    }

    #[test]
    fn plain_format_contains_required_parts() {
        let log = make_log(Level::INFO, "svc", vec![], "hello world");
        let line = format_plain(&log);
        assert!(line.contains("INFO"), "missing level: {line}");
        assert!(line.contains("[svc]"), "missing source: {line}");
        assert!(line.contains("hello world"), "missing message: {line}");
    }

    #[test]
    fn plain_format_includes_tags() {
        let log = make_log(
            Level::WARN,
            "auth",
            vec!["req:1".into(), "user:bob".into()],
            "denied",
        );
        let line = format_plain(&log);
        assert!(line.contains("req:1"), "missing tag: {line}");
        assert!(line.contains("user:bob"), "missing tag: {line}");
        assert!(line.contains("denied"), "missing message: {line}");
    }

    #[test]
    fn plain_format_no_tags_has_no_parentheses() {
        let log = make_log(Level::DEBUG, "svc", vec![], "msg");
        let line = format_plain(&log);
        assert!(!line.contains('('), "unexpected parens: {line}");
    }
}
