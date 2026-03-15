//! Server lifecycle — builds and runs the Workflows server event loop.

use std::sync::Arc;
use std::time::Duration;

use orkester_common::messaging::Message;
use orkester_common::plugin::servers::ServerContext;
use orkester_common::{log_error, log_info, log_warn};
use serde_json::{json, Value};

use super::api::{self, ApiHandler};
use super::model::Workflow;
use super::scheduler;
use super::store::WorkflowsStore;
use super::worker::{LocalWorker, Worker};
use super::workspace_client::WorkspaceClient;

pub async fn run(config: Value, ctx: ServerContext) {
    let channel = ctx.channel;
    let registry = ctx.registry;
    let executor_registry = ctx.executor_registry;

    // ── Build persistence store ────────────────────────────────────────────────────
    let persistence_config = config.get("persistence").cloned().unwrap_or(Value::Null);
    let provider_id = persistence_config
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("memory-persistence")
        .to_string();
    let provider = match registry.persistence_provider(&provider_id) {
        Ok(orkester_common::plugin::PluginComponent::PersistenceProvider(builder)) => {
            match builder.build(persistence_config) {
                Ok(p) => Arc::from(p),
                Err(e) => {
                    log_error!("Failed to build persistence provider '{}': {}", provider_id, e);
                    return;
                }
            }
        }
        Ok(_) => {
            log_error!("Component '{}' is not a PersistenceProvider.", provider_id);
            return;
        }
        Err(e) => {
            log_error!("No persistence provider found for type '{}': {}", provider_id, e);
            return;
        }
    };
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

    let metrics_target = config
        .get("metrics_target")
        .and_then(|v| v.as_str())
        .unwrap_or("")
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
    let (spawn_tx, mut spawn_rx) = tokio::sync::mpsc::unbounded_channel::<Workflow>();
    let handler = ApiHandler {
        store: store.clone(),
        to_hub: channel.to_hub.clone(),
        spawn_tx,
        metrics_target: metrics_target.clone(),
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
                    "workspace_response" => {
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

            Some(wf) = spawn_rx.recv() => {
                // Only start immediately when there is no future start_datetime.
                // Workflows with a future start_datetime will be picked up by
                // the scheduler once their time arrives (future work).
                let defer = wf.schedule.start_datetime
                    .map(|t| t > chrono::Utc::now())
                    .unwrap_or(false);
                if !defer {
                    let store_c = store.clone();
                    let workspace_c = workspace_client.clone();
                    let executors_c = Arc::clone(&executor_registry);
                    let to_hub_c = channel.to_hub.clone();
                    let metrics_c = metrics_target.clone();
                    tokio::spawn(async move {
                        LocalWorker {
                            executor_registry: executors_c,
                            to_hub: to_hub_c,
                            metrics_target: metrics_c,
                        }
                            .run(wf, store_c, workspace_c)
                            .await;
                    });
                }
            }

            _ = scheduler_tick.tick() => {
                scheduler::run_tick(&store, &workspace_client, &executor_registry, &channel.to_hub, &metrics_target).await;
            }

            else => break,
        }
    }

    log_info!("Server stopped.");
}
