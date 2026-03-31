mod config;
mod event;
mod store;

pub use config::MetricsServerConfig;
pub use event::{
    HistoryRequest, HistoryResponse,
    RecordAck, RecordRequest, SnapshotRequest, SnapshotResponse,
};

use std::sync::Mutex;

use orkester_plugin::prelude::*;

use store::MetricsStore;

/// Stateful metrics server — thread-safe via an internal Mutex-guarded store.
pub struct MetricsServer {
    store: Mutex<MetricsStore>,
}

impl MetricsServer {
    pub fn new(cfg: MetricsServerConfig) -> Self {
        Self {
            store: Mutex::new(MetricsStore::new(
                cfg.keep_history,
                cfg.retention_secs,
                cfg.max_granularity_per_minute,
            )),
        }
    }
}

#[component(
    kind        = "metrics/MetricsServer:1.0",
    name        = "Metrics Server",
    description = "Records metric events (SET/INCREASE/DECREASE/RESET) and exposes snapshots and full history."
)]
impl MetricsServer {
    /// Record a single metric event; returns the updated value.
    #[handle("metrics/Record")]
    fn record(&mut self, req: RecordRequest) -> Result<RecordAck> {
        let current = self.store.lock().unwrap().apply(&req.key, &req.operation, req.value);
        log_trace!("[metrics] record key='{}' op={:?} -> {}", req.key, req.operation, current);
        Ok(RecordAck { ok: true, current })
    }

    /// Return a point-in-time snapshot of all (or one) metric values.
    #[handle("metrics/GetSnapshot")]
    fn get_snapshot(&mut self, req: SnapshotRequest) -> Result<SnapshotResponse> {
        let metrics = self.store.lock().unwrap().snapshot(req.key.as_deref());
        log_trace!("[metrics] snapshot: {} key(s)", metrics.len());
        Ok(SnapshotResponse { metrics })
    }

    /// Return ordered historical data points per metric key.
    #[handle("metrics/GetHistory")]
    fn get_history(&mut self, req: HistoryRequest) -> Result<HistoryResponse> {
        let history = self.store.lock().unwrap().history(req.key.as_deref(), req.limit);
        log_trace!("[metrics] history: {} series", history.len());
        Ok(HistoryResponse { history })
    }
}
