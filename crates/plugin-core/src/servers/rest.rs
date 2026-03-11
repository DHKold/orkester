use orkester_common::messaging::ServerSide;
use orkester_common::plugin::servers::{Server, ServerError, ServerBuilder};
use serde_json::Value;

pub struct AxumRestServer {
    config: Value,
}

impl Server for AxumRestServer {
    fn start(&self, _channel: ServerSide) -> Result<(), ServerError> {
        Ok(())
    }

    fn stop(&self) -> Result<(), ServerError> {
        Ok(())
    }
}

pub struct AxumRestServerBuilder;

impl ServerBuilder for AxumRestServerBuilder {
    fn build(&self, config: Value) -> Result<Box<dyn Server>, ServerError> {
        Ok(Box::new(AxumRestServer { config }))
    }
}
