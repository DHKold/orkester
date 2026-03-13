//! Hub-message processing task.
//!
//! Messages arriving from the hub fall into two categories, each handled by a
//! dedicated private function:
//!
//! - `register_route` → insert the route into [`AppState`] and send an ack.
//! - everything else → look up the correlation id and relay to the waiting HTTP handler.

use std::sync::Arc;

use orkester_common::messaging::Message;
use orkester_common::{log_error, log_info};
use serde_json::json;

use super::state::AppState;

// ── Public task ───────────────────────────────────────────────────────────────

pub(super) async fn hub_message_task(
    mut hub_msg_rx: tokio::sync::mpsc::UnboundedReceiver<Message>,
    state: Arc<AppState>,
) {
    while let Some(msg) = hub_msg_rx.recv().await {
        match msg.message_type.as_str() {
            "register_route" => handle_register_route(msg, &state),
            _ => handle_hub_response(msg, &state),
        }
    }
}

// ── Private handlers ──────────────────────────────────────────────────────────

fn handle_register_route(msg: Message, state: &AppState) {
    let method = msg
        .content
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let path = msg
        .content
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or("/")
        .to_string();

    log_info!(
        "Registering route {} {} (requested by '{}').",
        method, path, msg.source,
    );

    state.register_route(method.clone(), path.clone(), msg.source.clone());

    let ack = Message::new(
        0,
        "", // hub stamps source
        msg.source.as_str(),
        "route_registered",
        json!({ "status": "ok", "method": method, "path": path }),
    );
    if !state.send_to_hub(ack) {
        log_error!(
            "Hub: failed to send route_registered ack to '{}'.",
            msg.source,
        );
    }
}

fn handle_hub_response(msg: Message, state: &AppState) {
    let corr_id = msg.content.get("correlation_id").and_then(|v| v.as_u64());
    if let Some(id) = corr_id {
        if let Some(tx) = state.remove_pending(id) {
            let _ = tx.send(msg);
        }
    }
}
