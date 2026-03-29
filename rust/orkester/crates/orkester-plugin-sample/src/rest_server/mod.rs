use std::{
    collections::HashMap,
    io::Cursor,
    sync::{Arc, Mutex},
    thread,
};

use orkester_plugin::{abi::AbiHost, hub::Envelope, prelude::*};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── Config types ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RouteEntry {
    pub path:   String,
    pub method: String,
    pub action: String,
}

/// Serves all files beneath `dir` under the `url_path` URL prefix.
///
/// Any request whose path starts with `url_path` is handled as a static file.
/// If the exact file is not found the server falls back to `index.html` in
/// `dir`, enabling client-side routing for single-page applications.
#[derive(Debug, Deserialize, Clone)]
pub struct StaticFolderEntry {
    /// URL prefix to serve under, e.g. `"/ui"`.
    pub url_path: String,
    /// Local filesystem root directory, e.g. `"/orkester/ui"`.
    pub dir: String,
}

#[derive(Debug, Deserialize)]
pub struct RestServerConfig {
    #[serde(default = "default_bind")]
    pub bind: String,
    /// API routes dispatched to the host hub.
    #[serde(default)]
    pub routes: Vec<RouteEntry>,
    /// Directories to serve as static files.
    #[serde(default)]
    pub static_folders: Vec<StaticFolderEntry>,
}

fn default_bind() -> String { "127.0.0.1:8080".into() }

// ── Dynamic route management ──────────────────────────────────────────────────

/// Payload for `rest/AddRoute`.
#[derive(Deserialize)]
pub struct AddRouteRequest {
    pub path:   String,
    pub method: String,
    pub action: String,
}

/// Acknowledgement for `rest/AddRoute`.
#[derive(Serialize)]
pub struct AddRouteAck { pub ok: bool }

// (method, path-pattern, action)
type RouteTable = Vec<(String, String, String)>;

// ── Server ────────────────────────────────────────────────────────────────────

/// Embedded HTTP server with static file serving and hub-dispatched API routes.
pub struct RestServer {
    routes:       Arc<Mutex<RouteTable>>,
    _next_id:     Arc<std::sync::atomic::AtomicU64>,
    _http_thread: Option<thread::JoinHandle<()>>,
}

#[derive(Clone, Copy)]
pub struct HostPtr(*mut AbiHost);
unsafe impl Send for HostPtr {}
unsafe impl Sync for HostPtr {}
impl HostPtr {
    /// Returns the raw `*mut AbiHost` pointer.
    ///
    /// Using a method instead of direct field access (`.0`) prevents Rust 2024
    /// precise closure capture from capturing `*mut AbiHost` directly (which is
    /// `!Send`); the closure captures `HostPtr: Send` instead.
    fn as_raw(self) -> *mut AbiHost { self.0 }
}

impl RestServer {
    pub fn new(cfg: RestServerConfig, host_ptr: *mut AbiHost) -> Self {
        let initial: RouteTable = cfg.routes
            .into_iter()
            .map(|r| (r.method.to_uppercase(), r.path, r.action))
            .collect();

        let routes         = Arc::new(Mutex::new(initial));
        let static_folders = Arc::new(cfg.static_folders);
        let next_id        = Arc::new(std::sync::atomic::AtomicU64::new(1));

        let thread_routes         = routes.clone();
        let thread_static_folders = static_folders.clone();
        let thread_next_id        = next_id.clone();
        let bind_addr             = cfg.bind;
        let host_box              = HostPtr(host_ptr);

        let http_thread = thread::Builder::new()
            .name("rest-http".to_owned())
            .spawn(move || {
                let server = match tiny_http::Server::http(&bind_addr) {
                    Ok(s)  => { log::info!("[rest] listening on http://{bind_addr}"); s }
                    Err(e) => { log::error!("[rest] failed to bind {bind_addr}: {e}"); return; }
                };

                for mut request in server.incoming_requests() {
                    let method  = request.method().to_string().to_uppercase();
                    let raw_url = request.url().to_string();
                    let (path, query_str) = split_url(&raw_url);

                    log::debug!("[rest] {method} {path}");

                    // ── 1. Static folder serving ─────────────────────────────
                    // Compute the response outside the borrow of `request` so
                    // the move into `.respond()` happens after the loop.
                    let static_response: Option<(u16, &'static str, Vec<u8>)> = {
                        let mut found = None;
                        for sf in thread_static_folders.iter() {
                            let prefix = sf.url_path.trim_end_matches('/');
                            let under  = path == prefix
                                || path == format!("{prefix}/")
                                || path.starts_with(&format!("{prefix}/"));
                            if !under { continue; }

                            let rel      = path[prefix.len()..].trim_start_matches('/');
                            let file_rel = if rel.is_empty() { "index.html" } else { rel };
                            let candidate = std::path::Path::new(&sf.dir).join(file_rel);

                            // SPA fallback: missing files → index.html
                            let serve = if candidate.is_file() {
                                candidate
                            } else {
                                std::path::Path::new(&sf.dir).join("index.html")
                            };
                            let ext = serve.extension().and_then(|e| e.to_str()).unwrap_or("");
                            let ct  = mime_type(ext);
                            let (status, body) = match std::fs::read(&serve) {
                                Ok(b) => (200u16, b),
                                Err(e) => {
                                    log::warn!("[rest/static] {}: {e}", serve.display());
                                    (404u16, b"not found".to_vec())
                                }
                            };
                            found = Some((status, ct, body));
                            break;
                        }
                        found
                    };
                    if let Some((status, ct, body)) = static_response {
                        let len = body.len();
                        let _ = request.respond(tiny_http::Response::new(
                            tiny_http::StatusCode(status),
                            vec![tiny_http::Header::from_bytes("Content-Type", ct).unwrap()],
                            Cursor::new(body),
                            Some(len),
                            None,
                        ));
                        continue;
                    }

                    // ── 2. API route matching (supports {param} segments) ─────
                    let match_result = {
                        let rt = thread_routes.lock().unwrap();
                        rt.iter().find_map(|(m, pat, action)| {
                            if m == &method {
                                match_path(pat, &path).map(|params| (action.clone(), params))
                            } else {
                                None
                            }
                        })
                    };

                    let (action, path_params) = match match_result {
                        Some(r) => r,
                        None => {
                            log::warn!("[rest] 404 {method} {path}");
                            let body = br#"{"error":"not found"}"#.to_vec();
                            let len  = body.len();
                            let _ = request.respond(tiny_http::Response::new(
                                tiny_http::StatusCode(404),
                                vec![json_content_type()],
                                Cursor::new(body),
                                Some(len),
                                None,
                            ));
                            continue;
                        }
                    };

                    // ── 3. Build request payload ──────────────────────────────
                    // Merge body JSON + path params + query params.
                    // Priority: body > path params > query params.
                    let mut body_buf = Vec::new();
                    let _ = std::io::Read::read_to_end(request.as_reader(), &mut body_buf);
                    let query_params = parse_query(&query_str);
                    let payload      = merge_payload(&body_buf, &path_params, &query_params);

                    // ── 4. Dispatch to host ───────────────────────────────────
                    let id = thread_next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let envelope = Envelope {
                        id,
                        kind:    action,
                        owner:   None,
                        format:  "std/json".to_string(),
                        payload,
                    };
                    let mut host = unsafe { Host::from_abi(host_box.as_raw()) };
                    let result: Value = host.handle(&envelope).unwrap_or_else(|e| {
                        log::error!("[rest] dispatch failed: {e}");
                        serde_json::json!({ "error": e.to_string() })
                    });

                    // ── 5. Respond ────────────────────────────────────────────
                    // Unwrap single-dispatcher responses for cleaner REST API ergonomics.
                    let resp_bytes = serde_json::to_vec(&unwrap_single(result))
                        .unwrap_or_else(|_| br#"{"error":"serialization failed"}"#.to_vec());
                    let len = resp_bytes.len();
                    let _ = request.respond(tiny_http::Response::new(
                        tiny_http::StatusCode(200),
                        vec![json_content_type()],
                        Cursor::new(resp_bytes),
                        Some(len),
                        None,
                    ));
                }
            })
            .ok();

        Self { routes, _next_id: next_id, _http_thread: http_thread }
    }
}

// ── Component ─────────────────────────────────────────────────────────────────

#[component(
    kind        = "sample/RestServer:1.0",
    name        = "RestServer",
    description = "Embedded HTTP server with static file serving and hub-dispatched API routes."
)]
impl RestServer {
    /// Register or replace an API route at runtime.
    #[handle("rest/AddRoute")]
    fn add_route(&mut self, req: AddRouteRequest) -> Result<AddRouteAck> {
        let method = req.method.to_uppercase();
        let mut routes = self.routes.lock().unwrap();
        routes.retain(|(m, p, _)| !(m == &method && p == &req.path));
        routes.push((method, req.path, req.action));
        Ok(AddRouteAck { ok: true })
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn json_content_type() -> tiny_http::Header {
    tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap()
}

/// Split a raw URL into `(path, query_string)`.
fn split_url(url: &str) -> (String, String) {
    match url.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None         => (url.to_string(), String::new()),
    }
}

/// Match `path` against a route `pattern` that may contain `{param}` segments.
/// Returns `Some(params_map)` on a full match, `None` otherwise.
fn match_path(pattern: &str, path: &str) -> Option<HashMap<String, String>> {
    let pats: Vec<&str> = pattern.split('/').collect();
    let urls: Vec<&str> = path.split('/').collect();
    if pats.len() != urls.len() { return None; }

    let mut params = HashMap::new();
    for (p, u) in pats.iter().zip(urls.iter()) {
        if let Some(name) = p.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
            params.insert(name.to_string(), (*u).to_string());
        } else if p != u {
            return None;
        }
    }
    Some(params)
}

/// Parse a URL query string into a `String → String` map, percent-decoding values.
fn parse_query(qs: &str) -> HashMap<String, String> {
    if qs.is_empty() { return HashMap::new(); }
    qs.split('&').filter_map(|pair| {
        let mut kv = pair.splitn(2, '=');
        let k = kv.next().filter(|s| !s.is_empty())?;
        let v = kv.next().unwrap_or("");
        Some((percent_decode(k), percent_decode(v)))
    }).collect()
}

/// Merge body JSON, path params, and query params into a single JSON payload.
fn merge_payload(
    body_buf:     &[u8],
    path_params:  &HashMap<String, String>,
    query_params: &HashMap<String, String>,
) -> Vec<u8> {
    let mut map: serde_json::Map<String, Value> = if body_buf.is_empty() {
        serde_json::Map::new()
    } else {
        serde_json::from_slice(body_buf).unwrap_or_default()
    };
    for (k, v) in path_params {
        map.entry(k.clone()).or_insert_with(|| Value::String(v.clone()));
    }
    for (k, v) in query_params {
        map.entry(k.clone()).or_insert_with(|| Value::String(v.clone()));
    }
    serde_json::to_vec(&Value::Object(map)).unwrap_or_default()
}

/// Unwrap a single-component pipeline response.
///
/// `{ status, dispatched_to: 1, responses: [X] }` → `X`
fn unwrap_single(value: Value) -> Value {
    if value.get("dispatched_to").and_then(|v| v.as_u64()) == Some(1) {
        if let Some(Value::Array(arr)) = value.get("responses") {
            if arr.len() == 1 {
                return arr[0].clone();
            }
        }
    }
    value
}

/// Return the MIME content-type string for a file extension.
fn mime_type(ext: &str) -> &'static str {
    match ext {
        "html" | "htm" => "text/html; charset=utf-8",
        "css"           => "text/css; charset=utf-8",
        "js" | "mjs"    => "application/javascript; charset=utf-8",
        "json"          => "application/json",
        "svg"           => "image/svg+xml",
        "png"           => "image/png",
        "jpg" | "jpeg"  => "image/jpeg",
        "ico"           => "image/x-icon",
        "woff2"         => "font/woff2",
        "woff"          => "font/woff",
        _               => "application/octet-stream",
    }
}

/// Minimal percent-decoder for URL query string values.
fn percent_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let h1 = chars.next().and_then(|c| c.to_digit(16));
            let h2 = chars.next().and_then(|c| c.to_digit(16));
            if let (Some(h1), Some(h2)) = (h1, h2) {
                if let Some(ch) = char::from_u32(h1 * 16 + h2) {
                    out.push(ch);
                    continue;
                }
            }
            out.push('%');
        } else if c == '+' {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

