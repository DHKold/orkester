//! Shared Axum application state, route registry types, and path matching.
//!
//! [`AppState`] is intentionally not a passive bag — it exposes domain methods
//! (`resolve_route`, `send_to_hub`, …) so handlers and the hub task never touch
//! the internal lock fields directly.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, RwLock};

use orkester_common::messaging::Message;

// ── Route registry types ──────────────────────────────────────────────────────

#[derive(Clone)]
pub(super) struct RouteRegistration {
    /// Instance name of the server that owns this route.
    pub(super) target: String,
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
}

impl AppState {
    pub(super) fn new(to_hub: std::sync::mpsc::Sender<Message>) -> Self {
        Self {
            routes: RwLock::new(HashMap::new()),
            pending: Mutex::new(HashMap::new()),
            to_hub: Mutex::new(to_hub),
            next_id: AtomicU64::new(1),
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

    /// Insert or overwrite a route registration.
    pub(super) fn register_route(&self, method: String, path: String, source: String) {
        self.routes.write().unwrap().insert(
            RouteKey { method, path },
            RouteRegistration { target: source },
        );
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
