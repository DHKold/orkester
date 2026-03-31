mod antispam;
pub mod config;
mod metrics;
mod queue;
mod worker;

pub use config::LoggingServerConfig;

use orkester_plugin::{abi::AbiHost, logging::LogRecord, prelude::*};

use antispam::AntiSpam;
use metrics::LogMetrics;
use queue::LogQueue;

pub struct LoggingServer {
    queue:   LogQueue,
    metrics: LogMetrics,
    _worker: std::thread::JoinHandle<()>,
}

#[component(
    kind = "logging/LoggingServer:1.0",
    name = "Logging Server",
    description = "Receives structured log records and routes to configurable sinks."
)]
impl LoggingServer {
    pub fn new(cfg: LoggingServerConfig, host_ptr: *mut AbiHost) -> Self {
        let queue_cap = cfg.queue_capacity.unwrap_or(1024);
        log_info!("[log-server] initialising with queue_capacity={queue_cap}");
        let (queue, receiver) = LogQueue::new(queue_cap);
        let metrics  = LogMetrics::new(host_ptr);
        let antispam = AntiSpam::new(cfg.antispam.clone());
        let sinks    = config::build_sinks(cfg.sinks);
        log_info!("[log-server] built {} sink(s)", sinks.len());
        let worker_metrics = metrics.clone();
        let handle = worker::spawn(receiver, sinks, antispam, worker_metrics);
        Self { queue, metrics, _worker: handle }
    }

    /// Receive a log record from the host logging bridge.
    #[handle("logging/Ingest")]
    fn handle_ingest(&mut self, record: LogRecord) -> Result<()> {
        self.metrics.inc_received();
        if self.queue.try_send(record).is_err() {
            self.metrics.inc_dropped();
            log_warn!("[log-server] queue full — record dropped");
        }
        Ok(())
    }
}
