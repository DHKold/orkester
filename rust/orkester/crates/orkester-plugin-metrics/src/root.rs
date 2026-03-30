use orkester_plugin::prelude::*;

use crate::metrics::{MetricsServer, MetricsServerConfig};

/// Root component of the metrics plugin — exposes MetricsServer as a child component.
#[derive(Default)]
pub struct MetricsPlugin;

#[component(
    kind = "metrics/Plugin:1.0",
    name = "Metrics Plugin",
    description = "Metrics plugin for Orkester",
)]
impl MetricsPlugin {
    #[factory("metrics/MetricsServer:1.0")]
    fn create_metrics_server(&mut self, config: MetricsServerConfig) -> Result<MetricsServer> {
        Ok(MetricsServer::new(config))
    }
}
