//! HTTP request handling for the metrics server.
//!
//! This module processes the two REST-protocol messages the metrics server
//! receives from the hub: the registration ack and the actual HTTP requests.

use std::sync::mpsc::Sender;

use orkester_common::messaging::Message;
use orkester_common::{log_debug, log_error, log_info};

use super::collector::MetricsCollector;
use super::rest_api;

/// Log the `route_registered` ack received from the REST server.
pub(super) fn on_route_registered(msg: &Message) {
    log_info!("Route confirmed by '{}': {}", msg.source, msg.content);
}

/// Respond to an `http_request` forwarded by the REST server.
///
/// Builds a metrics snapshot and sends it back as an `http_response` message.
pub(super) fn on_http_request(
    msg: &Message,
    to_hub: &Sender<Message>,
    collector: &MetricsCollector,
) {
    let corr_id = msg
        .content
        .get("correlation_id")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    log_debug!("Handling HTTP request (correlation_id={}).", corr_id);

    let reply = rest_api::response_message(msg.source.as_str(), corr_id, collector.snapshot());
    if to_hub.send(reply).is_err() {
        log_error!("Hub channel closed while sending metrics response.");
    }
}
