//! API handler — processes `http_request` messages for the Workspace routes
//! and sends responses back through the hub.
//!
//! # Routes
//!
//! | Method | Path                                              | Description                     |
//! |--------|---------------------------------------------------|---------------------------------|
//! | GET    | /v1/namespaces                                    | List all namespaces             |
//! | GET    | /v1/namespaces/{name}                             | Get one namespace               |
//! | GET    | /v1/namespaces/{ns}/tasks                         | List tasks in a namespace       |
//! | GET    | /v1/namespaces/{ns}/tasks/{name}/{version}        | Get a specific task             |
//! | GET    | /v1/namespaces/{ns}/works                         | List works in a namespace       |
//! | GET    | /v1/namespaces/{ns}/works/{name}/{version}        | Get a specific work             |

use std::sync::mpsc;

use orkester_common::messaging::Message;
use orkester_common::{log_debug, log_warn};
use serde_json::{json, Value};

use super::store::WorkspaceStore;

/// Routes that must be registered with REST servers at startup.
pub const ROUTES: &[(&str, &str)] = &[
    ("GET", "/v1/namespaces"),
    ("GET", "/v1/namespaces/{name}"),
    ("GET", "/v1/namespaces/{ns}/tasks"),
    ("GET", "/v1/namespaces/{ns}/tasks/{name}/{version}"),
    ("GET", "/v1/namespaces/{ns}/works"),
    ("GET", "/v1/namespaces/{ns}/works/{name}/{version}"),
];

// ── ApiHandler ────────────────────────────────────────────────────────────────

/// Handles HTTP requests forwarded by REST servers through the hub.
///
/// `handle` is designed to be called from a Tokio async context (the
/// `WorkspaceServer` event loop).
pub struct ApiHandler {
    pub store: WorkspaceStore,
    pub to_hub: mpsc::Sender<Message>,
}

impl ApiHandler {
    pub async fn handle(&self, msg: Message) -> u16 {
        let path = msg
            .content
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let corr_id = msg
            .content
            .get("correlation_id")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let source = msg.source.clone();

        log_debug!("GET {} (correlation_id={})", path, corr_id);

        let (status, body) = self.dispatch(&path).await;
        self.reply(&source, corr_id, status, body);
        status
    }

    async fn dispatch(&self, path: &str) -> (u16, Value) {
        // Segment-based routing — no regex or framework needed.
        let segs: Vec<&str> = path.trim_start_matches('/').split('/').collect();

        match segs.as_slice() {
            // GET /v1/namespaces
            ["v1", "namespaces"] => match self.store.list_namespaces().await {
                Ok(list) => (200, json!({ "namespaces": list })),
                Err(e) => server_error(e),
            },

            // GET /v1/namespaces/{name}
            ["v1", "namespaces", name] => match self.store.get_namespace(name).await {
                Ok(ns) => (200, json!(ns)),
                Err(e) => not_found_or_error(e),
            },

            // GET /v1/namespaces/{ns}/tasks
            ["v1", "namespaces", ns, "tasks"] => match self.store.list_tasks(ns).await {
                Ok(list) => (200, json!({ "tasks": list })),
                Err(e) => server_error(e),
            },

            // GET /v1/namespaces/{ns}/tasks/{name}/{version}
            ["v1", "namespaces", ns, "tasks", name, version] => {
                match self.store.get_task(ns, name, version).await {
                    Ok(t) => (200, json!(t)),
                    Err(e) => not_found_or_error(e),
                }
            }

            // GET /v1/namespaces/{ns}/works
            ["v1", "namespaces", ns, "works"] => match self.store.list_works(ns).await {
                Ok(list) => (200, json!({ "works": list })),
                Err(e) => server_error(e),
            },

            // GET /v1/namespaces/{ns}/works/{name}/{version}
            ["v1", "namespaces", ns, "works", name, version] => {
                match self.store.get_work(ns, name, version).await {
                    Ok(w) => (200, json!(w)),
                    Err(e) => not_found_or_error(e),
                }
            }

            _ => (404, json!({ "error": "not found" })),
        }
    }

    fn reply(&self, target: &str, corr_id: u64, status: u16, body: Value) {
        let msg = Message::new(
            0,
            "",
            target,
            "http_response",
            json!({ "correlation_id": corr_id, "status": status, "body": body }),
        );
        if self.to_hub.send(msg).is_err() {
            log_warn!("Could not send response — hub disconnected");
        }
    }
}

// ── Error helpers ─────────────────────────────────────────────────────────────

fn not_found_or_error(e: super::store::StoreError) -> (u16, Value) {
    if matches!(e, super::store::StoreError::NotFound(_)) {
        (404, json!({ "error": "not found" }))
    } else {
        server_error(e)
    }
}

fn server_error(e: impl std::fmt::Display) -> (u16, Value) {
    (500, json!({ "error": e.to_string() }))
}
