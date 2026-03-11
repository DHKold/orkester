use crate::plugin::servers::ServerContext;
use async_trait::async_trait;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MetricsError {
    #[error("Internal error: {0}")]
    Internal(String),
}

/// A recorded metric value.
#[derive(Debug, Clone)]
pub enum MetricValue {
    Counter(u64),
    Gauge(f64),
    Histogram(Vec<f64>),
}

/// Interface for recording and reading metrics.
/// Other servers push metrics through this handle.
#[async_trait]
pub trait MetricsHandle: Send + Sync {
    /// Increment a counter by `delta`.
    async fn increment(&self, name: &str, delta: u64, labels: &[(&str, &str)]);

    /// Set a gauge to an absolute value.
    async fn gauge(&self, name: &str, value: f64, labels: &[(&str, &str)]);

    /// Record an observation in a histogram.
    async fn histogram(&self, name: &str, value: f64, labels: &[(&str, &str)]);

    /// Render all metrics (e.g. in Prometheus text format).
    async fn render(&self) -> Result<String, MetricsError>;
}

/// A running Metrics server (e.g. exposes /metrics HTTP endpoint).
#[async_trait]
pub trait MetricsServer: Send + Sync {
    fn name(&self) -> &str;
    fn handle(&self) -> Arc<dyn MetricsHandle>;
    fn run(self: Box<Self>) -> ServerContext<(), ()>;
}
