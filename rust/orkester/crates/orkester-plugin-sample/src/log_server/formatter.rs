use std::fmt::Write as _;

use serde::Deserialize;

// ── LogFormatter trait ────────────────────────────────────────────────────────

pub trait LogFormatter: Send {
    /// Convert a log entry into its output representation.
    fn format(&self, entry: &LogEntry) -> String;
}

// ── LogEntry ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Default)]
pub struct LogEntry {
    #[serde(default = "default_info")]
    pub level:   String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub source:  Option<String>,
    #[serde(flatten)]
    pub extra:   serde_json::Map<String, serde_json::Value>,
}

fn default_info() -> String { "info".into() }

// ── Text formatter ────────────────────────────────────────────────────────────

pub struct TextFormatter;

impl LogFormatter for TextFormatter {
    fn format(&self, entry: &LogEntry) -> String {
        let source = entry.source.as_deref().unwrap_or("-");
        let mut s = format!("[{}] [{}] {}", entry.level.to_uppercase(), source, entry.message);
        for (k, v) in &entry.extra {
            write!(s, "  {k}={v}").ok();
        }
        s
    }
}

// ── JSON formatter ────────────────────────────────────────────────────────────

pub struct JsonFormatter;

impl LogFormatter for JsonFormatter {
    fn format(&self, entry: &LogEntry) -> String {
        // Serialize back to a compact JSON line
        let mut map = entry.extra.clone();
        map.insert("level".into(),   entry.level.clone().into());
        map.insert("message".into(), entry.message.clone().into());
        if let Some(src) = &entry.source {
            map.insert("source".into(), src.clone().into());
        }
        serde_json::to_string(&map).unwrap_or_default()
    }
}
