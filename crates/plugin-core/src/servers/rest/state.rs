//! Shared Axum application state, route registry types, and path matching.
//!
//! [`AppState`] is intentionally not a passive bag — it exposes domain methods
//! (`resolve_route`, `send_to_hub`, …) so handlers and the hub task never touch
//! the internal lock fields directly.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Mutex, RwLock};

use orkester_common::messaging::Message;
use serde_json::{json, Value};

// ── Route registry types ──────────────────────────────────────────────────────

#[derive(Clone)]
pub(super) struct RouteRegistration {
    /// Instance name of the server that owns this route.
    pub(super) target: String,
    /// Optional OpenAPI 3.0 [Operation Object] supplied by the registrant.
    ///
    /// Supported fields: `summary`, `description`, `tags`, `parameters`,
    /// `requestBody`, `responses`, `deprecated`, and any extension (`x-*`).
    ///
    /// [Operation Object]: https://spec.openapis.org/oas/v3.0.3#operation-object
    pub(super) openapi: Option<Value>,
}

#[derive(Hash, Eq, PartialEq, Clone)]
pub(super) struct RouteKey {
    pub(super) method: String,
    pub(super) path: String,
}

// ── Shared state ──────────────────────────────────────────────────────────────

pub(super) struct AppState {
    pub(super) routes: RwLock<HashMap<RouteKey, RouteRegistration>>,
    pending: Mutex<HashMap<u64, tokio::sync::oneshot::Sender<Message>>>,
    to_hub: Mutex<std::sync::mpsc::Sender<Message>>,
    next_id: AtomicU64,
    metrics_target: String,
    active_requests: AtomicI64,
}

impl AppState {
    pub(super) fn new(to_hub: std::sync::mpsc::Sender<Message>, metrics_target: String) -> Self {
        Self {
            routes: RwLock::new(HashMap::new()),
            pending: Mutex::new(HashMap::new()),
            to_hub: Mutex::new(to_hub),
            next_id: AtomicU64::new(1),
            metrics_target,
            active_requests: AtomicI64::new(0),
        }
    }

    /// Look up the registered route for the given method + path.
    ///
    /// Tries an exact key match first (fast path), then falls back to
    /// template matching for parameterised paths like `/v1/namespaces/{name}`.
    pub(super) fn resolve_route(&self, method: &str, path: &str) -> Option<RouteRegistration> {
        let key = RouteKey {
            method: method.to_owned(),
            path: path.to_owned(),
        };
        let routes = self.routes.read().unwrap();
        routes.get(&key).cloned().or_else(|| {
            routes
                .iter()
                .find(|(k, _)| k.method == method && match_path_template(&k.path, path))
                .map(|(_, v)| v.clone())
        })
    }

    /// Atomically allocate the next correlation id.
    pub(super) fn next_correlation_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Register a one-shot sender that will receive the hub reply for `id`.
    pub(super) fn register_pending(&self, id: u64, tx: tokio::sync::oneshot::Sender<Message>) {
        self.pending.lock().unwrap().insert(id, tx);
    }

    /// Remove and return the pending sender for `id`, if it still exists.
    pub(super) fn remove_pending(
        &self,
        id: u64,
    ) -> Option<tokio::sync::oneshot::Sender<Message>> {
        self.pending.lock().unwrap().remove(&id)
    }

    /// Send a message to the hub. Returns `false` if the hub channel is closed.
    pub(super) fn send_to_hub(&self, msg: Message) -> bool {
        self.to_hub.lock().unwrap().send(msg).is_ok()
    }

    /// Emit an `update_metric` message to the configured metrics server.
    ///
    /// Does nothing when `metrics_target` is empty (metrics disabled).
    pub(super) fn send_metric(&self, name: &str, operation: &str, value: f64) {
        if self.metrics_target.is_empty() {
            return;
        }
        let msg = Message::new(
            0,
            "",
            &self.metrics_target,
            "update_metric",
            json!({ "name": name, "operation": operation, "value": value }),
        );
        // Fire-and-forget — metric loss on hub disconnect is acceptable.
        let _ = self.to_hub.lock().unwrap().send(msg);
    }

    /// Increment the active-request counter and emit a `set` metric.
    pub(super) fn active_request_start(&self) {
        let active = self.active_requests.fetch_add(1, Ordering::Relaxed) + 1;
        self.send_metric("rest.requests_active", "set", active as f64);
    }

    /// Decrement the active-request counter and emit a `set` metric.
    pub(super) fn active_request_end(&self) {
        let active = self.active_requests.fetch_sub(1, Ordering::Relaxed) - 1;
        self.send_metric("rest.requests_active", "set", active as f64);
    }

    /// Returns `true` when `target` is the metrics server itself, meaning
    /// requests to it should not be counted in REST metrics.
    pub(super) fn is_metrics_target(&self, target: &str) -> bool {
        !self.metrics_target.is_empty() && target == self.metrics_target
    }

    /// Insert or overwrite a route registration.
    ///
    /// `openapi` is an optional OpenAPI 3.0 Operation Object the registrant can
    /// supply to enrich the spec served at `GET /v1/openapi.json`.
    pub(super) fn register_route(
        &self,
        method: String,
        path: String,
        source: String,
        openapi: Option<Value>,
    ) {
        self.routes.write().unwrap().insert(
            RouteKey { method, path },
            RouteRegistration { target: source, openapi },
        );
    }

    /// Assemble a live OpenAPI 3.0.3 document from all currently registered routes.
    ///
    /// Path parameters are inferred from `{name}` segments in the path template.
    /// Per-operation metadata (summary, description, schemas, …) is taken from the
    /// `openapi` field that registrants may supply in their `register_route` message.
    /// Inferred path parameters are merged with any explicitly declared ones,
    /// deduplicating by name so registrants can override the defaults.
    pub(super) fn build_openapi_spec(&self) -> Value {
        let routes = self.routes.read().unwrap();
        let mut paths: serde_json::Map<String, Value> = serde_json::Map::new();

        for (key, reg) in routes.iter() {
            // Infer path parameters from {name} segments.
            let inferred_params: Vec<Value> = key
                .path
                .split('/')
                .filter(|s| s.starts_with('{') && s.ends_with('}'))
                .map(|s| {
                    json!({
                        "name": &s[1..s.len() - 1],
                        "in": "path",
                        "required": true,
                        "schema": { "type": "string" }
                    })
                })
                .collect();

            // Start from the plugin-supplied operation object or an empty one.
            let mut op: serde_json::Map<String, Value> = reg
                .openapi
                .as_ref()
                .and_then(|v| v.as_object().cloned())
                .unwrap_or_default();

            // Apply defaults for required OpenAPI fields when not provided.
            op.entry("tags".to_owned())
                .or_insert_with(|| json!([reg.target]));
            op.entry("responses".to_owned()).or_insert_with(|| {
                json!({
                    "200":     { "description": "Success" },
                    "default": { "description": "Unexpected error" }
                })
            });

            // Merge inferred path params, skipping any already declared by name.
            if !inferred_params.is_empty() {
                let existing = op
                    .entry("parameters".to_owned())
                    .or_insert_with(|| json!([]));
                if let Some(arr) = existing.as_array_mut() {
                    let declared: std::collections::HashSet<String> = arr
                        .iter()
                        .filter_map(|p| {
                            p.get("name").and_then(|n| n.as_str()).map(str::to_owned)
                        })
                        .collect();
                    for p in &inferred_params {
                        let name = p.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        if !declared.contains(name) {
                            arr.push(p.clone());
                        }
                    }
                }
            }

            let method = key.method.to_lowercase();
            let path_entry = paths.entry(key.path.clone()).or_insert_with(|| json!({}));
            if let Some(obj) = path_entry.as_object_mut() {
                obj.insert(method, Value::Object(op));
            }
        }

        json!({
            "openapi": "3.0.3",
            "info": { "title": "Orkester API", "version": "1.0.0" },
            "paths": paths
        })
    }
}

// ── Path template matching ────────────────────────────────────────────────────

/// Returns `true` when `template` (e.g. `/v1/namespaces/{name}`) matches the
/// concrete request `path` (e.g. `/v1/namespaces/default`).
///
/// Segments wrapped in `{…}` are treated as single-segment wildcards.
pub(super) fn match_path_template(template: &str, path: &str) -> bool {
    let t: Vec<&str> = template.trim_start_matches('/').split('/').collect();
    let p: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    if t.len() != p.len() {
        return false;
    }
    t.iter()
        .zip(p.iter())
        .all(|(ts, ps)| (ts.starts_with('{') && ts.ends_with('}')) || ts == ps)
}
