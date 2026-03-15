//! Workspace server — loads and exposes Namespace, Task, and Work objects.
//!
//! # Configuration
//!
//! ```yaml
//! servers:
//!   workspace:
//!     plugin_id: orkester-plugin-core
//!     server_id: workspace-server
//!     config:
//!       # One or more loaders.  The first matching 'type' is used.
//!       loaders:
//!         - type: local
//!           dir: /etc/orkester/objects
//!         - type: s3
//!           bucket: my-bucket
//!           prefix: objects/
//!           poll_interval_seconds: 60
//!
//!       # REST server to register APIs on.
//!       rest_target: rest_api          # default: "rest_api"
//!
//!       # Persistence provider — defaults to the in-process MemoryPersistence.
//!       # (Future: reference an external provider by id)
//! ```
//!
//! # Architecture
//!
//! ```
//!  ┌─────────────────────────────────────┐
//!  │  WorkspaceServer  (hub participant) │
//!  │                                     │
//!  │  ┌──────────┐   ┌───────────────┐  │
//!  │  │  Loaders ├──►│ WorkspaceStore│  │
//!  │  └──────────┘   └──────┬────────┘  │
//!  │                         │           │
//!  │              ┌──────────▼────────┐  │
//!  │              │   ApiHandler      │  │
//!  │              │  (hub messages)   │  │
//!  └──────────────┴───────────────────┘  
//! ```

pub mod api;
pub mod loader;
pub mod store;

use std::sync::Arc;

use orkester_common::messaging::{Message, ServerSide};
use orkester_common::plugin::providers::persistence::PersistenceProvider;
use orkester_common::plugin::servers::{Server, ServerBuilder, ServerContext, ServerError};
use orkester_common::{log_error, log_info, log_warn};
use serde_json::{json, Value};

use api::ApiHandler;
use loader::{loader_from_config, LoaderEvent, ObjectLoader};
use store::WorkspaceStore;

use crate::persistence::memory::MemoryPersistenceProvider;

// ── WorkspaceServer ────────────────────────────────────────────────────────────

pub struct WorkspaceServer {
    config: Value,
}

impl Server for WorkspaceServer {
    fn start(&self, ctx: ServerContext) -> Result<(), ServerError> {
        let config = self.config.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to build Tokio runtime");

            rt.block_on(run(config, ctx.channel));
        });

        Ok(())
    }

    fn stop(&self) -> Result<(), ServerError> {
        // The thread will stop when the hub drops the channel.
        Ok(())
    }
}

// ── Main async entry point ────────────────────────────────────────────────────

async fn run(config: Value, channel: ServerSide) {
    // ── Build persistence store ───────────────────────────────────────────
    let provider: Arc<dyn PersistenceProvider> = Arc::new(MemoryPersistenceProvider::default());
    let store = WorkspaceStore::new(provider);

    // ── Build loaders from config ─────────────────────────────────────────
    let loaders: Vec<Arc<dyn ObjectLoader>> =
        if let Some(arr) = config.get("loaders").and_then(|v| v.as_array()) {
            let mut out = Vec::new();
            for loader_cfg in arr {
                match loader_from_config(loader_cfg) {
                    Ok(l) => out.push(l),
                    Err(e) => log_error!("Invalid loader config: {}", e),
                }
            }
            out
        } else {
            log_warn!("No loaders configured — workspace will be empty.");
            Vec::new()
        };

    // ── Initial load ──────────────────────────────────────────────────────
    for loader in &loaders {
        match loader.load_all().await {
            Ok(objs) => {
                let count = objs.len();
                for obj in objs {
                    if let Err(e) = store.upsert(&obj).await {
                        log_error!("Failed to store {} '{}': {}", obj.kind(), obj.name(), e);
                    }
                }
                log_info!("Initial load complete: {} object(s).", count);
            }
            Err(e) => log_error!("Loader failed during initial load: {}", e),
        }
    }

    // ── Watch for changes ─────────────────────────────────────────────────
    let (watch_tx, mut watch_rx) = tokio::sync::mpsc::unbounded_channel::<LoaderEvent>();
    for loader in &loaders {
        loader.watch(watch_tx.clone()).await;
    }

    // ── Register routes with the REST server ──────────────────────────────
    let rest_target = config
        .get("rest_target")
        .and_then(|v| v.as_str())
        .unwrap_or("rest_api")
        .to_string();

    let metrics_target = config
        .get("metrics_target")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

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

    // ── Emit initial object counts ────────────────────────────────────────
    emit_workspace_counts(&store, &channel.to_hub, &metrics_target).await;

    // ── Build API handler ─────────────────────────────────────────────────
    let handler = ApiHandler {
        store: store.clone(),
        to_hub: channel.to_hub.clone(),
    };

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
    log_info!("Server ready.");

    loop {
        tokio::select! {
            // Hub messages (HTTP requests + acks).
            Some(msg) = hub_rx.recv() => {
                match msg.message_type.as_str() {
                    "http_request" => {
                        let status = handler.handle(msg).await;
                        send_metric(&channel.to_hub, &metrics_target, "workspace.reads_total", "increment", 1.0);
                        if status == 404 {
                            send_metric(&channel.to_hub, &metrics_target, "workspace.not_found_total", "increment", 1.0);
                        }
                    }
                    "workspace_request" => {
                        handle_workspace_request(&store, msg, &channel.to_hub).await;
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

            // Loader change events.
            Some(event) = watch_rx.recv() => {
                match event {
                    LoaderEvent::Upserted(obj) => {
                        log_info!(
                            "Reloaded {} '{}'",
                            obj.kind(), obj.name()
                        );
                        if let Err(e) = handler.store.upsert(&obj).await {
                            log_error!("Store error on reload: {}", e);
                        }
                        emit_workspace_counts(&handler.store, &channel.to_hub, &metrics_target).await;
                    }
                    LoaderEvent::Removed(obj) => {
                        log_info!("Removing {} '{}'", obj.kind(), obj.name());
                        if let Err(e) = handler.store.remove(&obj).await {
                            log_error!("Store error on remove: {}", e);
                        }
                        emit_workspace_counts(&handler.store, &channel.to_hub, &metrics_target).await;
                    }
                }
            }

            else => break,
        }
    }

    log_info!("Server stopped.");
}

// ── Direct messaging handler ──────────────────────────────────────────────────

/// Handles `workspace_request` messages sent directly by other servers (e.g.
/// the Workflows server) — no REST layer involved.
///
/// Request content:
/// ```json
/// { "correlation_id": 1, "op": "get_task", "namespace": "acme", "name": "my-task", "version": "1.0.0" }
/// ```
/// Response (`workspace_response`):
/// ```json
/// { "correlation_id": 1, "ok": true,  "object": { ... } }
/// { "correlation_id": 1, "ok": false, "error": "not found: ..." }
/// ```
async fn handle_workspace_request(
    store: &WorkspaceStore,
    msg: Message,
    to_hub: &std::sync::mpsc::Sender<Message>,
) {
    let corr_id = msg.content.get("correlation_id").and_then(|v| v.as_u64()).unwrap_or(0);
    let op      = msg.content.get("op").and_then(|v| v.as_str()).unwrap_or("");
    let ns      = msg.content.get("namespace").and_then(|v| v.as_str()).unwrap_or("");
    let name    = msg.content.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let version = msg.content.get("version").and_then(|v| v.as_str()).unwrap_or("");

    let reply_content = match op {
        "get_namespace" => match store.get_namespace(name).await {
            Ok(obj)  => json!({ "correlation_id": corr_id, "ok": true,  "object": obj }),
            Err(e)   => json!({ "correlation_id": corr_id, "ok": false, "error": e.to_string() }),
        },
        "list_namespaces" => match store.list_namespaces().await {
            Ok(objs) => json!({ "correlation_id": corr_id, "ok": true,  "objects": objs }),
            Err(e)   => json!({ "correlation_id": corr_id, "ok": false, "error": e.to_string() }),
        },
        "get_task" => match store.get_task(ns, name, version).await {
            Ok(obj)  => json!({ "correlation_id": corr_id, "ok": true,  "object": obj }),
            Err(e)   => json!({ "correlation_id": corr_id, "ok": false, "error": e.to_string() }),
        },
        "list_tasks" => match store.list_tasks(ns).await {
            Ok(objs) => json!({ "correlation_id": corr_id, "ok": true,  "objects": objs }),
            Err(e)   => json!({ "correlation_id": corr_id, "ok": false, "error": e.to_string() }),
        },
        "get_work" => match store.get_work(ns, name, version).await {
            Ok(obj)  => json!({ "correlation_id": corr_id, "ok": true,  "object": obj }),
            Err(e)   => json!({ "correlation_id": corr_id, "ok": false, "error": e.to_string() }),
        },
        "list_works" => match store.list_works(ns).await {
            Ok(objs) => json!({ "correlation_id": corr_id, "ok": true,  "objects": objs }),
            Err(e)   => json!({ "correlation_id": corr_id, "ok": false, "error": e.to_string() }),
        },
        other => json!({
            "correlation_id": corr_id,
            "ok": false,
            "error": format!("unknown workspace op: '{other}'"),
        }),
    };

    let reply = Message::new(
        corr_id,
        "",
        msg.source.as_str(),
        "workspace_response",
        reply_content,
    );
    if let Err(e) = to_hub.send(reply) {
        log_warn!("Failed to send workspace_response: {}", e);
    }
}

// ── Metrics helpers ───────────────────────────────────────────────────────────

fn send_metric(
    to_hub: &std::sync::mpsc::Sender<Message>,
    metrics_target: &str,
    name: &str,
    operation: &str,
    value: f64,
) {
    if metrics_target.is_empty() {
        return;
    }
    let msg = Message::new(
        0,
        "",
        metrics_target,
        "update_metric",
        json!({ "name": name, "operation": operation, "value": value }),
    );
    let _ = to_hub.send(msg);
}

async fn emit_workspace_counts(
    store: &WorkspaceStore,
    to_hub: &std::sync::mpsc::Sender<Message>,
    metrics_target: &str,
) {
    if metrics_target.is_empty() {
        return;
    }
    if let Ok(ns) = store.list_namespaces().await {
        send_metric(to_hub, metrics_target, "workspace.namespaces_count", "set", ns.len() as f64);
    }
    // Tasks and works span all namespaces; we sum across namespaces we know.
    let namespaces: Vec<String> = store
        .list_namespaces()
        .await
        .map(|ns| ns.into_iter().map(|n| n.meta.name.clone()).collect())
        .unwrap_or_default();
    let mut tasks_total = 0usize;
    let mut works_total = 0usize;
    for ns in &namespaces {
        if let Ok(t) = store.list_tasks(ns).await {
            tasks_total += t.len();
        }
        if let Ok(w) = store.list_works(ns).await {
            works_total += w.len();
        }
    }
    send_metric(to_hub, metrics_target, "workspace.tasks_count", "set", tasks_total as f64);
    send_metric(to_hub, metrics_target, "workspace.works_count", "set", works_total as f64);
}

// ── Builder ────────────────────────────────────────────────────────────────────

pub struct WorkspaceServerBuilder;

impl ServerBuilder for WorkspaceServerBuilder {
    fn build(&self, config: Value) -> Result<Box<dyn Server>, ServerError> {
        Ok(Box::new(WorkspaceServer { config }))
    }
}
