use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use axum::{
    extract::State,
    http::{Method, StatusCode, Uri},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use orkester_common::messaging::{Message, ServerSide};
use orkester_common::plugin::servers::{Server, ServerBuilder, ServerError};
use serde_json::{json, Value};

// ── Route registry ────────────────────────────────────────────────────────────

#[derive(Clone)]
struct RouteRegistration {
    /// Instance name of the server that owns this route.
    target: String,
}

#[derive(Hash, Eq, PartialEq, Clone)]
struct RouteKey {
    method: String,
    path: String,
}

// ── Shared app state ──────────────────────────────────────────────────────────

struct AppState {
    /// Dynamically registered routes.
    routes: RwLock<HashMap<RouteKey, RouteRegistration>>,
    /// Pending HTTP handlers waiting for an upstream response, keyed by correlation id.
    pending: Mutex<HashMap<u64, tokio::sync::oneshot::Sender<Message>>>,
    /// Hub send channel — wrapped in Mutex because std::mpsc::Sender is Send but not Sync.
    to_hub: Mutex<std::sync::mpsc::Sender<Message>>,
    /// Monotone counter for correlation ids.
    next_id: AtomicU64,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn list_routes_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let routes = state.routes.read().unwrap();
    let list: Vec<Value> = routes
        .iter()
        .map(|(k, v)| json!({ "method": k.method, "path": k.path, "registrant": v.target }))
        .collect();
    Json(json!({ "routes": list }))
}

async fn dynamic_route_handler(
    State(state): State<Arc<AppState>>,
    method: Method,
    uri: Uri,
) -> impl IntoResponse {
    let key = RouteKey {
        method: method.to_string(),
        path: uri.path().to_string(),
    };

    let reg = match state.routes.read().unwrap().get(&key).cloned() {
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "route not found" })),
            )
                .into_response()
        }
        Some(r) => r,
    };

    let corr_id = state.next_id.fetch_add(1, Ordering::SeqCst);
    let (tx, rx) = tokio::sync::oneshot::channel::<Message>();
    state.pending.lock().unwrap().insert(corr_id, tx);

    let msg = Message::new(
        corr_id,
        "", // hub stamps source
        reg.target.as_str(),
        "http_request",
        json!({
            "correlation_id": corr_id,
            "method": method.to_string(),
            "path": uri.path().to_string(),
        }),
    );

    if state.to_hub.lock().unwrap().send(msg).is_err() {
        state.pending.lock().unwrap().remove(&corr_id);
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "hub disconnected" })),
        )
            .into_response();
    }

    match tokio::time::timeout(Duration::from_secs(30), rx).await {
        Ok(Ok(reply)) => {
            let status = reply
                .content
                .get("status")
                .and_then(|v| v.as_u64())
                .and_then(|s| StatusCode::from_u16(s as u16).ok())
                .unwrap_or(StatusCode::OK);
            let body = reply.content.get("body").cloned().unwrap_or(Value::Null);
            (status, Json(body)).into_response()
        }
        Ok(Err(_)) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "upstream disconnected" })),
        )
            .into_response(),
        Err(_) => {
            // Timed out — clean up the pending slot.
            state.pending.lock().unwrap().remove(&corr_id);
            (
                StatusCode::GATEWAY_TIMEOUT,
                Json(json!({ "error": "upstream timeout" })),
            )
                .into_response()
        }
    }
}

// ── Hub message task ──────────────────────────────────────────────────────────

/// Processes messages arriving from the hub:
/// - `register_route` — inserts the route and sends back an ack.
/// - everything else — correlates with a waiting HTTP handler via `correlation_id` in content.
async fn hub_message_task(
    mut hub_msg_rx: tokio::sync::mpsc::UnboundedReceiver<Message>,
    state: Arc<AppState>,
) {
    while let Some(msg) = hub_msg_rx.recv().await {
        match msg.message_type.as_str() {
            "register_route" => {
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

                println!(
                    "[rest] Registering route {} {} (requested by '{}').",
                    method, path, msg.source
                );

                state.routes.write().unwrap().insert(
                    RouteKey {
                        method: method.clone(),
                        path: path.clone(),
                    },
                    RouteRegistration {
                        target: msg.source.clone(),
                    },
                );

                let ack = Message::new(
                    0,
                    "", // hub stamps source
                    msg.source.as_str(),
                    "route_registered",
                    json!({ "status": "ok", "method": method, "path": path }),
                );
                let _ = state.to_hub.lock().unwrap().send(ack);
            }
            _ => {
                // Route response back to the waiting HTTP handler.
                let corr_id = msg.content.get("correlation_id").and_then(|v| v.as_u64());
                if let Some(id) = corr_id {
                    if let Some(tx) = state.pending.lock().unwrap().remove(&id) {
                        let _ = tx.send(msg);
                    }
                }
            }
        }
    }
}

// ── AxumRestServer ────────────────────────────────────────────────────────────

pub struct AxumRestServer {
    config: Value,
    /// Oneshot sender used by `stop()` to trigger graceful shutdown.
    shutdown_tx: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    /// Handle to the server thread so `stop()` can join it.
    thread_handle: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl Server for AxumRestServer {
    fn start(&self, channel: ServerSide) -> Result<(), ServerError> {
        let bind_addr = self
            .config
            .get("bind")
            .and_then(|v| v.as_str())
            .unwrap_or("0.0.0.0:8080")
            .to_string();

        // Creating an oneshot channel does not require a running runtime.
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        *self.shutdown_tx.lock().unwrap() = Some(shutdown_tx);

        let handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("[rest] Failed to build Tokio runtime");

            rt.block_on(async move {
                let state = Arc::new(AppState {
                    routes: RwLock::new(HashMap::new()),
                    pending: Mutex::new(HashMap::new()),
                    to_hub: Mutex::new(channel.to_hub),
                    next_id: AtomicU64::new(1),
                });

                // Bridge the synchronous from_hub receiver into a tokio channel
                // so the async hub_message_task can await it.
                let (hub_msg_tx, hub_msg_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
                let from_hub = channel.from_hub;
                std::thread::spawn(move || {
                    while let Ok(msg) = from_hub.recv() {
                        if hub_msg_tx.send(msg).is_err() {
                            break;
                        }
                    }
                });

                tokio::spawn(hub_message_task(hub_msg_rx, state.clone()));

                let router = Router::new()
                    .route("/v1/routes", get(list_routes_handler))
                    .fallback(dynamic_route_handler)
                    .with_state(state);

                let listener = tokio::net::TcpListener::bind(&bind_addr)
                    .await
                    .expect("[rest] Bind failed");

                println!("[rest] Listening on {}.", bind_addr);

                axum::serve(listener, router)
                    .with_graceful_shutdown(async move {
                        let _ = shutdown_rx.await;
                        println!("[rest] Shutdown signal received.");
                    })
                    .await
                    .expect("[rest] Server error");

                println!("[rest] Server stopped.");
            });
        });

        *self.thread_handle.lock().unwrap() = Some(handle);
        Ok(())
    }

    fn stop(&self) -> Result<(), ServerError> {
        if let Some(tx) = self.shutdown_tx.lock().unwrap().take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.thread_handle.lock().unwrap().take() {
            handle.join().ok();
        }
        Ok(())
    }
}

pub struct AxumRestServerBuilder;

impl ServerBuilder for AxumRestServerBuilder {
    fn build(&self, config: Value) -> Result<Box<dyn Server>, ServerError> {
        Ok(Box::new(AxumRestServer {
            config,
            shutdown_tx: Mutex::new(None),
            thread_handle: Mutex::new(None),
        }))
    }
}
