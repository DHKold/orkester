use crossbeam_channel::Receiver;
use orkester_plugin::logging::LogRecord;

use super::{antispam::AntiSpam, config::SinkEntry, metrics::LogMetrics};

/// Spawn the background worker thread.
pub fn spawn(
    receiver: Receiver<LogRecord>,
    sinks:    Vec<SinkEntry>,
    antispam: AntiSpam,
    metrics:  LogMetrics,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("log-server-worker".into())
        .spawn(move || run(receiver, sinks, antispam, metrics))
        .expect("failed to spawn log-server worker thread")
}

fn run(
    receiver: Receiver<LogRecord>,
    sinks:    Vec<SinkEntry>,
    mut antispam: AntiSpam,
    metrics:  LogMetrics,
) {
    for record in &receiver {
        if !antispam.allow(&record) {
            metrics.inc_suppressed();
            continue;
        }
        deliver(&record, &sinks);
        metrics.inc_processed();

        // Emit suppression summaries as synthetic records.
        for (source, count) in antispam.drain_suppressed() {
            let summary = synthesize_suppression(&record, &source, count);
            deliver(&summary, &sinks);
        }
    }
}

fn deliver(record: &LogRecord, sinks: &[SinkEntry]) {
    for entry in sinks {
        let text = entry.formatter.format(record);
        if let Err(e) = entry.sink.write(record, &text) {
            log::warn!("[log-server] sink write error: {e}");
        }
    }
}

fn synthesize_suppression(base: &LogRecord, source: &str, count: u64) -> LogRecord {
    LogRecord {
        level:        orkester_plugin::logging::LogLevel::Warn,
        target:       "log-server::antispam".into(),
        message:      format!("{count} messages suppressed from '{source}'"),
        file:         String::new(),
        line:         0,
        timestamp_ms: base.timestamp_ms,
        plugin_id:    base.plugin_id.clone(),
    }
}
