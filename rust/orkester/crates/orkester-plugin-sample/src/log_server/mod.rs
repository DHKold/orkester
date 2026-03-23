mod consumer;
mod formatter;

pub use consumer::{ConsoleConsumer, FileConsumer, LogConsumer};
pub use formatter::{JsonFormatter, LogEntry, LogFormatter, TextFormatter};

use orkester_plugin::prelude::*;
use serde::Deserialize;

// ── Config ────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FormatterConfig {
    Text,
    Json,
}

impl Default for FormatterConfig {
    fn default() -> Self { FormatterConfig::Text }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConsumerConfig {
    Console,
    File { path: String },
}

#[derive(Debug, Deserialize)]
pub struct LoggingServerConfig {
    #[serde(default)]
    pub formatter: FormatterConfig,
    #[serde(default)]
    pub consumers: Vec<ConsumerConfig>,
}

impl Default for LoggingServerConfig {
    fn default() -> Self {
        Self {
            formatter: FormatterConfig::Text,
            consumers: vec![ConsumerConfig::Console],
        }
    }
}

// ── Messages ──────────────────────────────────────────────────────────────────

#[derive(serde::Serialize, Default)]
pub struct LogAck {
    pub ok: bool,
}

// ── Component ─────────────────────────────────────────────────────────────────

/// Structured logging server.
///
/// Handles `log/Entry` — formats with the configured [`LogFormatter`] then
/// writes through each [`LogConsumer`].
pub struct LoggingServer {
    formatter: Box<dyn LogFormatter>,
    consumers: Vec<Box<dyn LogConsumer>>,
}

impl LoggingServer {
    pub fn new(cfg: LoggingServerConfig) -> Result<Self> {
        let formatter: Box<dyn LogFormatter> = match cfg.formatter {
            FormatterConfig::Text => Box::new(TextFormatter),
            FormatterConfig::Json => Box::new(JsonFormatter),
        };

        let mut consumers: Vec<Box<dyn LogConsumer>> = Vec::new();
        for c in cfg.consumers {
            match c {
                ConsumerConfig::Console        => consumers.push(Box::new(ConsoleConsumer)),
                ConsumerConfig::File { path }  => consumers.push(
                    Box::new(FileConsumer::new(&path)
                        .map_err(|e| format!("cannot open log file '{path}': {e}"))?)
                ),
            }
        }
        if consumers.is_empty() {
            consumers.push(Box::new(ConsoleConsumer));
        }

        Ok(Self { formatter, consumers })
    }
}

#[component(
    kind        = "sample/LoggingServer:1.0",
    name        = "LoggingServer",
    description = "Structured logger with pluggable formatters and consumers."
)]
impl LoggingServer {
    #[handle("log/Entry")]
    fn log_entry(&mut self, entry: LogEntry) -> Result<LogAck> {
        let line = self.formatter.format(&entry);
        for consumer in &mut self.consumers {
            consumer.consume(&line);
        }
        Ok(LogAck { ok: true })
    }
}
