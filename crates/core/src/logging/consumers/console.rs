use crate::logging::consumer::LogConsumer;
use crate::logging::log::Log;

// ── ConsoleConsumer ──────────────────────────────────────────────────────────

/// Prints each log entry to stdout as a human-readable text line:
///
/// ```text
/// [2026-03-11T14:22:01.042Z] INFO  [auth] (request-id:abc) user authenticated
/// ```
pub struct ConsoleConsumer;

impl LogConsumer for ConsoleConsumer {
    fn consume(&self, log: &Log) {
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
}

// ── ConsoleJsonConsumer ──────────────────────────────────────────────────────

/// Prints each log entry to stdout as a single JSON line.
///
/// ```json
/// {"datetime":"2026-03-11T14:22:01.042Z","level":20,"source":"auth","tags":[],"message":"user authenticated"}
/// ```
pub struct ConsoleJsonConsumer;

impl LogConsumer for ConsoleJsonConsumer {
    fn consume(&self, log: &Log) {
        match serde_json::to_string(log) {
            Ok(json) => println!("{}", json),
            Err(e) => eprintln!("[logging] failed to serialize log entry: {}", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::logging::{level::Level, log::Log};

    fn make_log(level: Level, source: &str, tags: Vec<String>, msg: &str) -> Log {
        Log::new(level, source, tags, msg)
    }

    // ── ConsoleConsumer format ───────────────────────────────────────────────
    // stdout capture is brittle in parallel tests; we validate the format
    // logic by checking the string built the same way the consumer does.

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
        let log = make_log(Level::WARN, "auth", vec!["req:1".into(), "user:bob".into()], "denied");
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

    // ── ConsoleJsonConsumer serialization ────────────────────────────────────

    #[test]
    fn json_output_contains_all_fields() {
        let log = make_log(Level::ERROR, "db", vec!["tx:42".into()], "query failed");
        let json = serde_json::to_string(&log).unwrap();
        assert!(json.contains("\"level\""));
        assert!(json.contains("\"source\":\"db\""));
        assert!(json.contains("\"message\":\"query failed\""));
        assert!(json.contains("\"tags\""));
        assert!(json.contains("\"datetime\""));
        assert!(json.contains("tx:42"));
    }

    #[test]
    fn json_level_is_integer() {
        let log = make_log(Level::INFO, "svc", vec![], "msg");
        let json = serde_json::to_string(&log).unwrap();
        // Level::INFO == 20
        assert!(json.contains("\"level\":20"), "expected numeric level in: {json}");
    }
}
