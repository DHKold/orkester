use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use orkester_plugin::abi::AbiHost;
use serde_json::json;

/// Shared atomic counters emitted periodically via `metrics/Record`.
#[derive(Clone, Default)]
pub struct LogMetrics(Arc<Inner>);

#[derive(Default)]
struct Inner {
    received:   AtomicU64,
    processed:  AtomicU64,
    dropped:    AtomicU64,
    suppressed: AtomicU64,
}

impl LogMetrics {
    pub fn new(_host_ptr: *mut AbiHost) -> Self { Self::default() }

    pub fn inc_received(&self)   { self.0.received.fetch_add(1, Ordering::Relaxed); }
    pub fn inc_processed(&self)  { self.0.processed.fetch_add(1, Ordering::Relaxed); }
    pub fn inc_dropped(&self)    { self.0.dropped.fetch_add(1, Ordering::Relaxed); }
    pub fn inc_suppressed(&self) { self.0.suppressed.fetch_add(1, Ordering::Relaxed); }

    pub fn snapshot(&self) -> serde_json::Value {
        json!({
            "logging.records.received":   self.0.received.load(Ordering::Relaxed),
            "logging.records.processed":  self.0.processed.load(Ordering::Relaxed),
            "logging.records.dropped":    self.0.dropped.load(Ordering::Relaxed),
            "logging.records.suppressed": self.0.suppressed.load(Ordering::Relaxed),
        })
    }
}
