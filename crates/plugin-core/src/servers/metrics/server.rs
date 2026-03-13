//! `MetricsServer` lifecycle and hub event loop.
//!
//! The server spawns a single background thread on `start()`. That thread:
//!
//! 1. Sends a `register_route` message (built by [`rest_api`]) to the REST server.
//! 2. Enters an event loop dispatching three message types:
//!
//! | Message type    | Handler               | Description                              |
//! |-----------------|-----------------------|------------------------------------------|
//! | `route_registered` | `on_route_registered` | Ack from the REST server — log only.  |
//! | `http_request`  | `on_http_request`     | Serve the metrics snapshot via REST.     |
//! | `update_metric` | `on_update_metric`    | Update a named counter from any server.  |
//!
//! # `update_metric` message schema
//!
//! Any server can update a metric by sending a hub message to the metrics
//! server instance with the following content:
//!
//! ```json
//! {
//!   "name":      "my_counter",
//!   "operation": "increment",
//!   "value":     1.0
//! }
//! ```
//!
//! Supported operations:
//!
//! | Operation   | Effect                              | `value` field       |
//! |-------------|-------------------------------------|---------------------|
//! | `increment` | Adds `value` to the metric (default `1.0`) | optional   |
//! | `set`       | Sets the metric to `value`          | required            |
//! | `reset`     | Sets the metric to `0`              | ignored             |

use orkester_common::messaging::Message;
use orkester_common::plugin::servers::{Server, ServerBuilder, ServerContext, ServerError};
use orkester_common::{log_debug, log_error, log_info, log_warn};
use serde_json::Value;

use super::collector::MetricsCollector;
use super::rest_api;
use super::rest_handler;

// ── MetricsServer ─────────────────────────────────────────────────────────────

pub struct MetricsServer {
    rest_target: String,
    collector: MetricsCollector,
}

impl Server for MetricsServer {
    fn start(&self, ctx: ServerContext) -> Result<(), ServerError> {
        let channel = ctx.channel;
        let rest_target = self.rest_target.clone();
        let collector = self.collector.clone();

        std::thread::spawn(move || {
            if Self::send_registration(&channel.to_hub, &rest_target).is_err() {
                return;
            }
            Self::run_event_loop(channel.from_hub, channel.to_hub, collector);
        });

        Ok(())
    }

    fn stop(&self) -> Result<(), ServerError> {
        Ok(())
    }
}

// ── Private lifecycle helpers ─────────────────────────────────────────────────

impl MetricsServer {
    fn send_registration(
        to_hub: &std::sync::mpsc::Sender<Message>,
        rest_target: &str,
    ) -> Result<(), ()> {
        log_info!("Sending register_route to '{}'.", rest_target);
        to_hub
            .send(rest_api::registration_message(rest_target))
            .map_err(|_| log_error!("Hub channel closed — could not register route."))
    }

    fn run_event_loop(
        from_hub: std::sync::mpsc::Receiver<Message>,
        to_hub: std::sync::mpsc::Sender<Message>,
        collector: MetricsCollector,
    ) {
        loop {
            match from_hub.recv() {
                Ok(msg) => match msg.message_type.as_str() {
                    "route_registered" => rest_handler::on_route_registered(&msg),
                    "http_request"     => rest_handler::on_http_request(&msg, &to_hub, &collector),
                    "update_metric"    => Self::on_update_metric(&msg, &collector),
                    other              => log_warn!("Unhandled message type '{}'.", other),
                },
                Err(_) => {
                    log_info!("Hub channel disconnected — stopping.");
                    break;
                }
            }
        }
    }
}

// ── Private message handlers ──────────────────────────────────────────────────

impl MetricsServer {
    fn on_update_metric(msg: &Message, collector: &MetricsCollector) {
        let name = match msg.content.get("name").and_then(|v| v.as_str()) {
            Some(n) => n.to_owned(),
            None => {
                log_warn!("update_metric from '{}' missing 'name' field.", msg.source);
                return;
            }
        };
        let operation = msg
            .content
            .get("operation")
            .and_then(|v| v.as_str())
            .unwrap_or("increment");
        let value = msg
            .content
            .get("value")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);

        match operation {
            "increment" => collector.increment(&name, value),
            "set"       => collector.set(&name, value),
            "reset"     => collector.reset(&name),
            other => {
                log_warn!(
                    "update_metric from '{}': unknown operation '{}' for metric '{}'.",
                    msg.source, other, name,
                );
                return;
            }
        }
        log_debug!(
            "Metric '{}' updated by '{}' (op={}, value={}).",
            name, msg.source, operation, value,
        );
    }
}

// ── Builder ───────────────────────────────────────────────────────────────────

pub struct MetricsServerBuilder;

impl ServerBuilder for MetricsServerBuilder {
    fn build(&self, config: Value) -> Result<Box<dyn Server>, ServerError> {
        let rest_target = config
            .get("rest_server")
            .and_then(|v| v.as_str())
            .unwrap_or("rest_api")
            .to_string();
        Ok(Box::new(MetricsServer {
            rest_target,
            collector: MetricsCollector::new(),
        }))
    }
}
