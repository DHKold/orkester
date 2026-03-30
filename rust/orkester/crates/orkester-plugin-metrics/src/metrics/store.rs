use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

use super::event::{DataPoint, MetricOperation};

/// In-memory store for metric values and optional time-bounded history.
pub struct MetricsStore {
    values:                     HashMap<String, f64>,
    history:                    HashMap<String, VecDeque<DataPoint>>,
    keep_history:               bool,
    /// How long to retain history in milliseconds.
    retention_ms:               u64,
    /// Max data points kept per minute per key (granularity bucket width).
    max_granularity_per_minute: u64,
}

impl MetricsStore {
    pub fn new(keep_history: bool, retention_secs: u64, max_granularity_per_minute: u64) -> Self {
        Self {
            values:  HashMap::new(),
            history: HashMap::new(),
            keep_history,
            retention_ms: retention_secs.saturating_mul(1_000),
            max_granularity_per_minute,
        }
    }

    /// Apply an operation to a key and return the resulting value.
    pub fn apply(&mut self, key: &str, op: &MetricOperation, value: f64) -> f64 {
        let cur = self.values.entry(key.to_string()).or_insert(0.0);
        *cur = match op {
            MetricOperation::Set      => value,
            MetricOperation::Increase => *cur + value,
            MetricOperation::Decrease => *cur - value,
            MetricOperation::Reset    => 0.0,
        };
        let result = *cur;
        if self.keep_history {
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64);
            self.push_history(key, result, ts);
        }
        result
    }

    /// Return a snapshot of current values, optionally filtered to a single key.
    pub fn snapshot(&self, key: Option<&str>) -> HashMap<String, f64> {
        match key {
            Some(k) => {
                let mut m = HashMap::new();
                if let Some(v) = self.values.get(k) { m.insert(k.to_string(), *v); }
                m
            }
            None => self.values.clone(),
        }
    }

    /// Return historical data points, optionally filtered and capped per key.
    pub fn history(&self, key: Option<&str>, limit: Option<usize>) -> HashMap<String, Vec<DataPoint>> {
        match key {
            Some(k) => single_history_entry(&self.history, k, limit),
            None    => self.history.iter()
                .map(|(k, dq)| (k.clone(), collect_tail(dq, limit)))
                .collect(),
        }
    }

    /// Push a new value into history, applying granularity bucketing and time-based pruning.
    fn push_history(&mut self, key: &str, value: f64, timestamp_ms: u64) {
        let interval_ms = bucket_interval_ms(self.max_granularity_per_minute);
        let cutoff_ms   = timestamp_ms.saturating_sub(self.retention_ms);
        let bucket      = timestamp_ms / interval_ms;

        let dq = self.history.entry(key.to_string()).or_default();

        // Prune expired points from the front.
        while dq.front().map_or(false, |p| p.timestamp_ms < cutoff_ms) {
            dq.pop_front();
        }

        // Merge into the current bucket (replace the last point if same interval).
        if let Some(last) = dq.back_mut() {
            if last.timestamp_ms / interval_ms == bucket {
                last.value        = value;
                last.timestamp_ms = timestamp_ms;
                return;
            }
        }
        dq.push_back(DataPoint { value, timestamp_ms });
    }
}

/// Width of one granularity bucket in milliseconds.
fn bucket_interval_ms(max_granularity_per_minute: u64) -> u64 {
    let gpm = max_granularity_per_minute.max(1);
    (60_000u64).saturating_div(gpm).max(1)
}

fn single_history_entry(
    map: &HashMap<String, VecDeque<DataPoint>>,
    key: &str,
    limit: Option<usize>,
) -> HashMap<String, Vec<DataPoint>> {
    let mut result = HashMap::new();
    if let Some(dq) = map.get(key) {
        result.insert(key.to_string(), collect_tail(dq, limit));
    }
    result
}

fn collect_tail(dq: &VecDeque<DataPoint>, limit: Option<usize>) -> Vec<DataPoint> {
    match limit {
        None    => dq.iter().cloned().collect(),
        Some(n) => dq.iter().rev().take(n).cloned().collect::<Vec<_>>()
                      .into_iter().rev().collect(),
    }
}
