use serde::{Deserialize, Serialize};

use crate::formatter::{build_formatter, ConsoleLogFormatter};
use crate::sink::{BoxedSink, console::ConsoleLogSink, local_fs::LocalFsLogSink, s3::S3LogSink};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AntiSpamConfig {
    /// Max records per source per minute before suppression.
    #[serde(default = "default_max_per_minute")]
    pub max_per_minute: u64,
}

fn default_max_per_minute() -> u64 { 1000 }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FormatterConfig {
    pub kind:   Option<String>,
    pub config: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SinkConfig {
    pub kind:      String,
    pub formatter: Option<FormatterConfig>,
    pub config:    Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LoggingServerConfig {
    pub queue_capacity: Option<usize>,
    pub antispam:       Option<AntiSpamConfig>,
    #[serde(default)]
    pub sinks:          Vec<SinkConfig>,
}

/// A sink paired with its formatter — the unit that the worker uses.
pub struct SinkEntry {
    pub sink:      BoxedSink,
    pub formatter: Box<dyn crate::formatter::LogFormatter>,
}

pub fn build_sinks(cfgs: Vec<SinkConfig>) -> Vec<SinkEntry> {
    let mut entries = Vec::new();
    for cfg in cfgs {
        let formatter = build_formatter(cfg.formatter.as_ref());
        let sink: Option<BoxedSink> = match cfg.kind.as_str() {
            "logging/ConsoleLogSink:1.0" => Some(Box::new(ConsoleLogSink)),
            "logging/LocalFsLogSink:1.0" => {
                let c = cfg.config.unwrap_or_default();
                LocalFsLogSink::from_config(&c).map(|s| Box::new(s) as BoxedSink)
                    .map_err(|e| log::warn!("[logging] failed to build LocalFsLogSink: {e}"))
                    .ok()
            }
            "logging/S3LogSink:1.0" => {
                let c = cfg.config.unwrap_or_default();
                S3LogSink::from_config(&c).map(|s| Box::new(s) as BoxedSink)
                    .map_err(|e| log::warn!("[logging] failed to build S3LogSink: {e}"))
                    .ok()
            }
            other => { log::warn!("[logging] unknown sink kind '{other}'"); None }
        };
        if let Some(s) = sink {
            entries.push(SinkEntry { sink: s, formatter });
        }
    }
    // Always add a console sink if no sinks were configured.
    if entries.is_empty() {
        entries.push(SinkEntry {
            sink:      Box::new(ConsoleLogSink),
            formatter: Box::new(ConsoleLogFormatter),
        });
    }
    entries
}
