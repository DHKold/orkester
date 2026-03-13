//! REST API concerns for the metrics server.
//!
//! This module owns all REST-protocol details so that `server.rs` never has to
//! reason about paths, correlation ids, or OpenAPI schemas.
//!
//! # Responsibilities
//! - Build the `register_route` message (with full OpenAPI metadata).
//! - Build the `http_response` message from a metrics snapshot.

use orkester_common::messaging::Message;
use serde_json::{json, Value};

/// Build the `register_route` message to send to the REST server on startup.
pub(super) fn registration_message(rest_target: &str) -> Message {
    Message::new(
        1,
        "", // hub stamps source
        rest_target,
        "register_route",
        json!({
            "method": "GET",
            "path":   "/v1/metrics",
            "openapi": {
                "summary":     "Runtime metrics",
                "description": "Returns the current values of all registered metrics, \
                                including the built-in `uptime_seconds` counter. \
                                Any server can publish metrics by sending \
                                `update_metric` messages to the metrics server.",
                "tags": ["observability"],
                "responses": {
                    "200": {
                        "description": "Current metrics snapshot.",
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "uptime_seconds": {
                                            "type":        "number",
                                            "description": "Seconds since the metrics server started.",
                                            "example":     3600
                                        }
                                    },
                                    "additionalProperties": {
                                        "type":        "number",
                                        "description": "Any metric published via `update_metric`."
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }),
    )
}

/// Build the `http_response` message carrying `snapshot` as the body.
pub(super) fn response_message(source: &str, corr_id: u64, snapshot: Value) -> Message {
    Message::new(
        0,
        "", // hub stamps source
        source,
        "http_response",
        json!({
            "correlation_id": corr_id,
            "status":         200,
            "body":           snapshot,
        }),
    )
}
