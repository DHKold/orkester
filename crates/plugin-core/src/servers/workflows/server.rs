//! Server lifecycle — builds and runs the Workflows server event loop.

use std::sync::Arc;
use std::time::Duration;

use orkester_common::{log_error, log_info, log_warn};
use orkester_common::messaging::{Message, ServerSide};
use serde_json::{json, Value};

use super::api::{self, ApiHandler};
use super::scheduler;
use super::store::WorkflowsStore;
use super::workspace_client::WorkspaceClient;
use crate::persistence::MemoryPersistenceProvider;

pub async fn run(config: Value, channel: ServerSide) {
    // ── Build persistence store ───────────────────────────────────────────
    let provider: Arc<dyn orkester_common::plugin::providers::persistence::PersistenceProvider> =
        Arc::new(MemoryPersistenceProvider::default());
    let store = WorkflowsStore::new(provider);

    // ── Config ────────────────────────────────────────────────────────────
    let rest_target = config
        .get("rest_target")
        .and_then(|v| v.as_str())
        .unwrap_or("rest_api")
        .to_string();

    let workspace_target = config
        .get("workspace_target")
        .and_then(|v| v.as_str())
        .unwrap_or("workspace")
        .to_string();

    let scheduler_interval = Duration::from_secs(
        config
            .get("scheduler_interval_seconds")
            .and_then(|v| v.as_u64())
            .unwrap_or(30),
    );

    // ── Register routes with the REST server ──────────────────────────────
    for (method, path) in api::ROUTES {
        let msg = Message::new(
            0,
            "",
            rest_target.as_str(),
            "register_route",
            json!({ "method": method, "path": path }),
        );
        if channel.to_hub.send(msg).is_err() {
            log_error!("Hub disconnected before routes could be registered.");
            return;
        }
    }
    log_info!("Route registrations sent to '{}'.", rest_target);

    // ── Build components ──────────────────────────────────────────────────
    let handler = ApiHandler {
        store: store.clone(),
        to_hub: channel.to_hub.clone(),
    };
    let workspace_client = WorkspaceClient::new(workspace_target, channel.to_hub.clone());

    // ── Bridge std channel into Tokio ─────────────────────────────────────
    let from_hub = channel.from_hub;
    let (hub_tx, mut hub_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
    std::thread::spawn(move || {
        while let Ok(msg) = from_hub.recv() {
            if hub_tx.send(msg).is_err() {
                break;
            }
        }
    });

    // ── Event loop ────────────────────────────────────────────────────────
    let mut scheduler_tick = tokio::time::interval(scheduler_interval);

    log_info!("Server ready.");

    loop {
        tokio::select! {
            Some(msg) = hub_rx.recv() => {
                match msg.message_type.as_str() {
                    "http_request" => {
                        handler.handle(msg).await;
                    }
                    "http_response" => {
                        workspace_client.handle_response(msg);
                    }
                    "route_registered" => {
                        let method = msg.content.get("method").and_then(|v| v.as_str()).unwrap_or("?");
                        let path   = msg.content.get("path").and_then(|v| v.as_str()).unwrap_or("?");
                        log_info!("Route confirmed: {} {}", method, path);
                    }
                    "error" => {
                        log_warn!(
                            "Hub error: {}",
                            msg.content.get("error").and_then(|v| v.as_str()).unwrap_or("unknown")
                        );
                    }
                    other => {
                        log_warn!("Unexpected message type '{}'", other);
                    }
                }
            }

            _ = scheduler_tick.tick() => {
                scheduler::run_tick(&store, &workspace_client).await;
            }

            else => break,
        }
    }

    log_info!("Server stopped.");
}
