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
