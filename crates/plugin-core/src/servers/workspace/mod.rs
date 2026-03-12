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

use orkester_common::{log_error, log_info, log_warn};
use orkester_common::messaging::{Message, ServerSide};
use orkester_common::plugin::providers::persistence::{
    EntityKey, PersistenceProvider,
};
use orkester_common::plugin::servers::{Server, ServerBuilder, ServerError};
use serde_json::{json, Value};

use api::ApiHandler;
use loader::{loader_from_config, LoaderEvent, ObjectLoader};
use store::WorkspaceStore;

use crate::persistence::MemoryPersistenceProvider;

// ── WorkspaceServer ────────────────────────────────────────────────────────────

pub struct WorkspaceServer {
    config: Value,
}

impl Server for WorkspaceServer {
    fn start(&self, channel: ServerSide) -> Result<(), ServerError> {
        let config = self.config.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to build Tokio runtime");

            rt.block_on(run(config, channel));
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
                        log_error!(
                            "Failed to store {} '{}': {}",
                            obj.kind(),
                            obj.name(),
                            e
                        );
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
    log_info!(
        "Route registrations sent to '{}'.",
        rest_target
    );

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
                        handler.handle(msg).await;
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
                    }
                    LoaderEvent::Removed(obj) => {
                        log_info!("Removing {} '{}'", obj.kind(), obj.name());
                        if let Err(e) = handler.store.remove(&obj).await {
                            log_error!("Store error on remove: {}", e);
                        }
                    }
                }
            }

            else => break,
        }
    }

    log_info!("Server stopped.");
}

// ── Builder ────────────────────────────────────────────────────────────────────

pub struct WorkspaceServerBuilder;

impl ServerBuilder for WorkspaceServerBuilder {
    fn build(&self, config: Value) -> Result<Box<dyn Server>, ServerError> {
        Ok(Box::new(WorkspaceServer { config }))
    }
}
