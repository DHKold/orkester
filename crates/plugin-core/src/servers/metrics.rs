use std::time::Duration;

use orkester_common::messaging::{Message, ServerSide};
use orkester_common::plugin::servers::{Server, ServerError, ServerBuilder};
use serde_json::Value;

pub struct NoMetricsServer {
    /// Instance name of the REST server to register the metrics route on.
    rest_target: String,
}

impl Server for NoMetricsServer {
    fn start(&self, channel: ServerSide) -> Result<(), ServerError> {
        let rest_target = self.rest_target.clone();
        std::thread::spawn(move || {
            let msg = Message::new(
                1,
                "",   // hub stamps the real source
                rest_target.as_str(),
                "register_route",
                serde_json::json!({ "method": "GET", "path": "/v1/metrics" }),
            );
            println!("[metrics] Sending register_route to '{}'.", rest_target);
            if channel.to_hub.send(msg).is_err() {
                println!("[metrics] Hub channel closed — could not send.");
                return;
            }

            match channel.from_hub.recv_timeout(Duration::from_secs(5)) {
                Ok(reply) => println!(
                    "[metrics] Route confirmed by '{}': {}",
                    reply.source, reply.content
                ),
                Err(_) => println!("[metrics] Timed out waiting for route_registered confirmation."),
            }
        });
        Ok(())
    }

    fn stop(&self) -> Result<(), ServerError> {
        Ok(())
    }
}

pub struct NoMetricsServerBuilder;

impl ServerBuilder for NoMetricsServerBuilder {
    fn build(&self, config: Value) -> Result<Box<dyn Server>, ServerError> {
        let rest_target = config
            .get("rest_server")
            .and_then(|v| v.as_str())
            .unwrap_or("rest_api")
            .to_string();
        Ok(Box::new(NoMetricsServer { rest_target }))
    }
}
