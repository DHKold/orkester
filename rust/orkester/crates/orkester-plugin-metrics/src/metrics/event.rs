use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// The operation to apply to a metric data point.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricOperation {
    /// Set the metric to the given absolute value.
    Set,
    /// Add `value` to the current metric value (defaults to 0.0 if not set).
    Increase,
    /// Subtract `value` from the current metric value.
    Decrease,
    /// Reset the metric to 0.0, ignoring `value`.
    Reset,
}

/// Request to record a single metric event.
#[derive(Debug, Deserialize)]
pub struct RecordRequest {
    pub key:       String,
    pub operation: MetricOperation,
    /// The delta or absolute value; ignored for `reset`. Default: 1.0.
    #[serde(default = "default_value")]
    pub value: f64,
}

fn default_value() -> f64 { 1.0 }

/// Acknowledgement returned after recording a metric.
#[derive(Debug, Serialize)]
pub struct RecordAck {
    pub ok:      bool,
    pub current: f64,
}

/// Request to fetch a metrics snapshot (current values).
#[derive(Debug, Deserialize, Default)]
pub struct SnapshotRequest {
    /// If set, return only this key; otherwise return all metrics.
    pub key: Option<String>,
}

/// Response containing current metric values.
#[derive(Debug, Serialize)]
pub struct SnapshotResponse {
    pub metrics: HashMap<String, f64>,
}

/// A single recorded data point with its timestamp.
#[derive(Debug, Clone, Serialize)]
pub struct DataPoint {
    pub value:        f64,
    /// Unix time in milliseconds when this data point was recorded.
    pub timestamp_ms: u64,
}

/// Request to fetch metric history.
#[derive(Debug, Deserialize, Default)]
pub struct HistoryRequest {
    /// If set, return history only for this key; otherwise all keys.
    pub key:   Option<String>,
    /// Cap the number of entries per key (most recent first).
    pub limit: Option<usize>,
}

/// Response containing per-key ordered history slices.
#[derive(Debug, Serialize)]
pub struct HistoryResponse {
    pub history: HashMap<String, Vec<DataPoint>>,
}
