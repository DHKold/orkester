//! RestServer â€” an embedded HTTP server component.
//!
//! ## Design
//!
//! `RestServer` runs a `tiny_http` listener in a background thread.  Incoming
//! HTTP requests are placed into a shared `pending` queue.  The host's main
//! loop polls the component via `rest/Poll` to drain pending requests, routes
//! each one to the appropriate ABI component, and delivers the result back via
//! `rest/Respond {id, status, body}`.  The HTTP thread is blocked waiting on a
//! per-request one-shot channel and only unblocks when `rest/Respond` is called.
//!
//! This design keeps all ABI component calls on the host thread, avoiding any
//! cross-thread sharing of `*mut AbiComponent`.
//!
//! ## Route config
//!
//! ```yaml
//! config:
//!   bind: "127.0.0.1:8080"
//!   routes:
//!     - path:   /ping
//!       method: GET
//!       action: ping/Ping
//! ```

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    thread,
};

use orkester_plugin::{abi::AbiHost, prelude::*};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// â”€â”€ Config â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[derive(Debug, Deserialize)]
pub struct RouteEntry {
    pub path:   String,
    pub method: String,
    pub action: String,
}

#[derive(Debug, Deserialize)]
pub struct RestServerConfig {
    #[serde(default = "default_bind")]
    pub bind:   String,
    #[serde(default)]
    pub routes: Vec<RouteEntry>,
}

fn default_bind() -> String { "127.0.0.1:8080".into() }

/// A pending HTTP request that has been matched to a route action but not yet
/// answered.  Placed in the `pending` queue by the HTTP thread.
struct PendingHttpRequest {
    id:     u64,
    method: String,
    path:   String,
    action: String,
    body:   Value,
}

/// One-shot response data sent back from `rest/Respond` to the HTTP thread.
struct HttpResponseData {
    status: u16,
    body:   Value,
}

/// One entry in the `rest/Poll` response.
#[derive(Serialize)]
pub struct PendingRequestDto {
    pub id:     u64,
    pub method: String,
    pub path:   String,
    pub action: String,
    pub body:   Value,
}

/// Response to `rest/Poll`.
#[derive(Serialize)]
pub struct PollResponse {
    pub requests: Vec<PendingRequestDto>,
}

/// Payload for `rest/Respond`.
#[derive(Deserialize)]
pub struct RespondRequest {
    pub id:     u64,
    pub status: u16,
    pub body:   Value,
}

/// Acknowledgement for `rest/Respond`.
#[derive(Serialize)]
pub struct RespondAck {
    pub ok: bool,
}

/// Payload for `rest/AddRoute`.
#[derive(Deserialize)]
pub struct AddRouteRequest {
    pub path:   String,
    pub method: String,
    pub action: String,
}

/// Acknowledgement for `rest/AddRoute`.
#[derive(Serialize)]
pub struct AddRouteAck {
    pub ok: bool,
}

type RouteTable   = Vec<(String, String, String)>;  // (method, path, action)
type PendingQueue = Arc<Mutex<VecDeque<PendingHttpRequest>>>;
type WaiterMap    = Arc<Mutex<HashMap<u64, crossbeam_channel::Sender<HttpResponseData>>>>;

/// Embedded HTTP server with host-polled request dispatch.
pub struct RestServer {
    routes:       Arc<Mutex<RouteTable>>,
    pending:      PendingQueue,
    waiters:      WaiterMap,
    _next_id:     Arc<std::sync::atomic::AtomicU64>,
    _http_thread: Option<thread::JoinHandle<()>>,
}

impl RestServer {
    pub fn new(cfg: RestServerConfig, _host_ptr: *mut AbiHost) -> Self {
        let initial: RouteTable = cfg.routes
            .into_iter()
            .map(|r| (r.method.to_uppercase(), r.path, r.action))
            .collect();

        let routes  = Arc::new(Mutex::new(initial));
        let pending = Arc::new(Mutex::new(VecDeque::new()));
        let waiters = Arc::new(Mutex::new(HashMap::new()));
        let next_id = Arc::new(std::sync::atomic::AtomicU64::new(1));

        let thread_routes  = routes.clone();
        let thread_pending = pending.clone();
        let thread_waiters = waiters.clone();
        let thread_next_id = next_id.clone();
        let bind_addr      = cfg.bind;

        let http_thread = thread::Builder::new()
            .name("rest-http".to_owned())
            .spawn(move || {
                let server = match tiny_http::Server::http(&bind_addr) {
                    Ok(s)  => { log::info!("[rest] listening on http://{bind_addr}"); s }
                    Err(e) => { log::error!("[rest] failed to bind {bind_addr}: {e}"); return; }
                };

                for mut request in server.incoming_requests() {
                    let method = request.method().to_string().to_uppercase();
                    let path   = request.url()
                        .split('?').next().unwrap_or("/").to_owned();

                    // Match route
                    let action_opt = {
                        let rt = thread_routes.lock().unwrap();
                        rt.iter()
                            .find(|(m, p, _)| m == &method && p == &path)
                            .map(|(_, _, a)| a.clone())
                    };

                    let action = match action_opt {
                        Some(a) => a,
                        None    => {
                            log::warn!("[rest] 404 {method} {path}");
                            let _ = request.respond(
                                tiny_http::Response::from_string(r#"{"error":"not found"}"#)
                                    .with_status_code(404)
                                    .with_header(json_content_type()),
                            );
                            continue;
                        }
                    };

                    // Read body
                    let mut body_buf = Vec::new();
                    let _ = std::io::Read::read_to_end(request.as_reader(), &mut body_buf);
                    let body: Value = serde_json::from_slice(&body_buf).unwrap_or(Value::Null);

                    // Register one-shot response channel
                    let (tx, rx) = crossbeam_channel::bounded::<HttpResponseData>(1);
                    let id = thread_next_id
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                    thread_waiters.lock().unwrap().insert(id, tx);
                    thread_pending.lock().unwrap().push_back(PendingHttpRequest {
                        id, method, path, action, body,
                    });

                    // Block until host delivers the response (or timeout)
                    let resp = rx
                        .recv_timeout(std::time::Duration::from_secs(30))
                        .unwrap_or(HttpResponseData {
                            status: 504,
                            body:   Value::String("gateway timeout".into()),
                        });
                    thread_waiters.lock().unwrap().remove(&id);

                    let body_str = serde_json::to_string(&resp.body).unwrap_or_default();
                    let _ = request.respond(
                        tiny_http::Response::from_string(body_str)
                            .with_status_code(resp.status)
                            .with_header(json_content_type()),
                    );
                }
            })
            .ok();

        Self { routes, pending, waiters, _next_id: next_id, _http_thread: http_thread }
    }
}

fn json_content_type() -> tiny_http::Header {
    tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap()
}

// â”€â”€ PluginComponent impl â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[component(
    kind        = "sample/RestServer:1.0",
    name        = "RestServer",
    description = "Embedded HTTP server with host-polled request dispatch."
)]
impl RestServer {
    /// Drain pending HTTP requests.  Returns at most 16 per call.
    #[handle("rest/Poll")]
    fn poll(&mut self, _: serde_json::Value) -> Result<PollResponse> {
        let mut queue = self.pending.lock().unwrap();
        let n = queue.len().min(16);
        let requests = queue
            .drain(..n)
            .map(|p| PendingRequestDto {
                id:     p.id,
                method: p.method,
                path:   p.path,
                action: p.action,
                body:   p.body,
            })
            .collect();
        Ok(PollResponse { requests })
    }

    /// Send the component response back to the waiting HTTP client.
    #[handle("rest/Respond")]
    fn respond(&mut self, req: RespondRequest) -> Result<RespondAck> {
        let tx_opt = self.waiters.lock().unwrap().remove(&req.id);
        if let Some(tx) = tx_opt {
            let _ = tx.send(HttpResponseData { status: req.status, body: req.body });
        }
        Ok(RespondAck { ok: true })
    }

    /// Register or replace a route at runtime.
    #[handle("rest/AddRoute")]
    fn add_route(&mut self, req: AddRouteRequest) -> Result<AddRouteAck> {
        let method = req.method.to_uppercase();
        let mut routes = self.routes.lock().unwrap();
        routes.retain(|(m, p, _)| !(m == &method && p == &req.path));
        routes.push((method, req.path, req.action));
        Ok(AddRouteAck { ok: true })
    }
}

