use orkester_common::plugin::servers::{Server, ServerError, ServerBuilder};
use serde_json::Value;

pub struct NoMetricsServer;

impl Server for NoMetricsServer {
    fn start(&self) -> Result<(), ServerError> {
        Ok(())
    }

    fn stop(&self) -> Result<(), ServerError> {
        Ok(())
    }
}

pub struct NoMetricsServerBuilder;

impl ServerBuilder for NoMetricsServerBuilder {
    fn build(&self, _config: Value) -> Result<Box<dyn Server>, ServerError> {
        Ok(Box::new(NoMetricsServer))
    }
}
