//! Generic thread-safe metrics collector.
//!
//! Metrics are identified by a string name and stored as `f64` values.
//! Any server can update any metric by sending an `update_metric` hub message
//! — there is no programmatic API visible outside this module.
//!
//! The built-in `uptime_seconds` metric is derived from the start [`Instant`]
//! and is never stored in the map.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use serde_json::{json, Value};

// ── MetricsCollector ──────────────────────────────────────────────────────────

/// Cheap-to-clone handle to the shared metric store.
#[derive(Clone)]
pub(super) struct MetricsCollector(Arc<Inner>);

struct Inner {
    started_at: Instant,
    values: RwLock<HashMap<String, f64>>,
}

impl MetricsCollector {
    pub(super) fn new() -> Self {
        Self(Arc::new(Inner {
            started_at: Instant::now(),
            values: RwLock::new(HashMap::new()),
        }))
    }

    /// Add `by` to the named metric (creating it at 0 if absent).
    pub(super) fn increment(&self, name: &str, by: f64) {
        *self.0.values.write().unwrap().entry(name.to_owned()).or_insert(0.0) += by;
    }

    /// Set the named metric to an exact value.
    pub(super) fn set(&self, name: &str, value: f64) {
        self.0.values.write().unwrap().insert(name.to_owned(), value);
    }

    /// Reset the named metric to zero.
    pub(super) fn reset(&self, name: &str) {
        self.0.values.write().unwrap().insert(name.to_owned(), 0.0);
    }

    /// Snapshot all current metrics as a JSON object.
    ///
    /// `uptime_seconds` is always present as a built-in derived field.
    /// All other metrics appear under their registered names.
    pub(super) fn snapshot(&self) -> Value {
        let mut map = serde_json::Map::new();
        map.insert(
            "uptime_seconds".to_owned(),
            json!(self.0.started_at.elapsed().as_secs()),
        );
        for (name, value) in self.0.values.read().unwrap().iter() {
            map.insert(name.clone(), json!(value));
        }
        Value::Object(map)
    }
}
