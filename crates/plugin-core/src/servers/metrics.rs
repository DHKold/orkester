use orkester_common::messaging::{Message, ServerSide};
use orkester_common::plugin::servers::{Server, ServerBuilder, ServerError};
use serde_json::{json, Value};

pub struct NoMetricsServer {
    /// Instance name of the REST server to register the metrics route on.
    rest_target: String,
}

impl Server for NoMetricsServer {
    fn start(&self, channel: ServerSide) -> Result<(), ServerError> {
        let rest_target = self.rest_target.clone();
        std::thread::spawn(move || {
            // Register GET /v1/metrics with the REST server.
            let msg = Message::new(
                1,
                "", // hub stamps source
                rest_target.as_str(),
                "register_route",
                json!({ "method": "GET", "path": "/v1/metrics" }),
            );
            println!("[metrics] Sending register_route to '{}'.", rest_target);
            if channel.to_hub.send(msg).is_err() {
                println!("[metrics] Hub channel closed — could not send.");
                return;
            }

            // Event loop: handle incoming messages indefinitely.
            loop {
                match channel.from_hub.recv() {
                    Ok(msg) => match msg.message_type.as_str() {
                        "route_registered" => {
                            println!(
                                "[metrics] Route confirmed by '{}': {}",
                                msg.source, msg.content
                            );
                        }
                        "http_request" => {
                            let corr_id = msg
                                .content
                                .get("correlation_id")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);
                            println!(
                                "[metrics] Handling HTTP request (correlation_id={}).",
                                corr_id
                            );
                            let reply = Message::new(
                                0,
                                "", // hub stamps source
                                msg.source.as_str(),
                                "http_response",
                                json!({
                                    "correlation_id": corr_id,
                                    "status": 200,
                                    "body": {
                                        "uptime_seconds": 42,
                                        "requests_total": 1,
                                    }
                                }),
                            );
                            if channel.to_hub.send(reply).is_err() {
                                println!("[metrics] Hub channel closed.");
                                return;
                            }
                        }
                        other => {
                            println!("[metrics] Unhandled message type '{}'.", other);
                        }
                    },
                    Err(_) => {
                        println!("[metrics] Hub channel disconnected — stopping.");
                        break;
                    }
                }
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
