use std::{collections::HashMap, time::Instant};

use super::config::AntiSpamConfig;
use orkester_plugin::logging::LogRecord;

/// Tracks message frequency per (plugin_id, target) pair and suppresses bursts.
pub struct AntiSpam {
    max_per_minute: u64,
    counters:       HashMap<String, (u64, Instant)>,
    suppressed:     HashMap<String, u64>,
}

impl AntiSpam {
    pub fn new(cfg: Option<AntiSpamConfig>) -> Self {
        let max_per_minute = cfg.map(|c| c.max_per_minute).unwrap_or(1000);
        Self { max_per_minute, counters: HashMap::new(), suppressed: HashMap::new() }
    }

    /// Returns `true` when the record should be delivered.  `false` means it
    /// has been suppressed; `drain_suppressed` returns pending summaries.
    pub fn allow(&mut self, record: &LogRecord) -> bool {
        let key = format!("{}::{}", record.plugin_id, record.target);
        let now = Instant::now();
        let entry = self.counters.entry(key.clone()).or_insert((0, now));
        // Reset window every ~60 s.
        if entry.1.elapsed().as_secs() >= 60 {
            *entry = (0, now);
        }
        entry.0 += 1;
        if entry.0 <= self.max_per_minute {
            return true;
        }
        *self.suppressed.entry(key).or_insert(0) += 1;
        false
    }

    /// Drain pending "N messages suppressed from X" notifications.
    pub fn drain_suppressed(&mut self) -> Vec<(String, u64)> {
        self.suppressed.drain().collect()
    }
}
