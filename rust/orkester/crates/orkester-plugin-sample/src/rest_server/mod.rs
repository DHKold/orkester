use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    thread,
};

use orkester_plugin::{abi::AbiHost, prelude::*, hub::Envelope};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

#[derive(Clone, Copy)]
pub struct HostPtr (*mut AbiHost);
unsafe impl Send for HostPtr {}
unsafe impl Sync for HostPtr {}

impl RestServer {
    pub fn new(cfg: RestServerConfig, host_ptr: *mut AbiHost) -> Self {
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

        let host_box = HostPtr(host_ptr); 

        let http_thread = thread::Builder::new()
            .name("rest-http".to_owned())
            .spawn(move || {
                let server = match tiny_http::Server::http(&bind_addr) {
                    Ok(s)  => { log::info!("[rest] listening on http://{bind_addr}"); s }
                    Err(e) => { log::error!("[rest] failed to bind {bind_addr}: {e}"); return; }
                };

                for mut request in server.incoming_requests() {
                    let method = request.method().to_string().to_uppercase();
                    let path   = request.url().split('?').next().unwrap_or("/").to_owned();

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

                    // Create a HUB Envelope and send it to the host via the ABI pointer.
                    let id = thread_next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let envelope = Envelope {
                        id: id,
                        kind: action.clone(),
                        owner: None,
                        format: "std/json".to_string(),
                        payload: body_buf, // Json String as Vec<u8>
                    };
                    let host_box = host_box; // Move the HostPtr into the closure
                    let mut host = unsafe { Host::from_abi(host_box.0) };
                    let result: Value = host.handle(&envelope).unwrap_or_else(|e| {
                        log::error!("[rest] failed to send request to host: {e}");
                        Value::Null
                    });

                    let body_str = match serde_json::to_string(&result) {
                        Ok(s) => s,
                        Err(e) => {
                            log::error!("[rest] failed to serialize response: {e}");
                            r#"{"error":"response serialization failed"}"#.to_string()
                        }
                    };
                    let _ = request.respond(
                        tiny_http::Response::from_string(body_str)
                            .with_status_code(200)
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

