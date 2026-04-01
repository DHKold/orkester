//! Axum REST server: static file serving, CORS, hub dispatch, and OpenAPI docs.

pub mod config;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, atomic::{AtomicU64, Ordering}};

use axum::{Router, extract::State, http::StatusCode, response::{IntoResponse, Json}, routing::{any, get}};
use orkester_plugin::{abi::AbiHost, hub::Envelope, prelude::*};
use serde_json::Value;
use tower_http::{cors::CorsLayer, services::{ServeDir, ServeFile}};

pub use config::RestServerConfig;
use config::{RouteEntry, StaticFolderEntry};

// ─── HostPtr ──────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct HostPtr(*mut AbiHost);
unsafe impl Send for HostPtr {}
unsafe impl Sync for HostPtr {}
impl HostPtr { fn as_raw(self) -> *mut AbiHost { self.0 } }

// ─── Shared state ─────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    routes:  Arc<Mutex<Vec<(String, String, String)>>>,
    host:    HostPtr,
    next_id: Arc<AtomicU64>,
    openapi: Arc<String>,
}

// ─── RestServer ───────────────────────────────────────────────────────────────

pub struct RestServer {
    routes:       Arc<Mutex<Vec<(String, String, String)>>>,
    _http_thread: Option<std::thread::JoinHandle<()>>,
}

impl RestServer {
    pub fn new(cfg: RestServerConfig, host_ptr: *mut AbiHost) -> Self {
        let initial = cfg.routes.iter()
            .map(|r| (r.method.to_uppercase(), r.path.clone(), r.action.clone()))
            .collect::<Vec<_>>();
        let routes   = Arc::new(Mutex::new(initial));
        let next_id  = Arc::new(AtomicU64::new(1));
        let openapi  = Arc::new(build_openapi_json("Orkester API", &cfg.routes));
        let state    = AppState { routes: Arc::clone(&routes), host: HostPtr(host_ptr), next_id, openapi };
        let bind     = cfg.bind.clone();
        let statics  = cfg.static_folders.clone();
        let cors_ori = cfg.cors_origins.clone();

        if cfg.tls_cert.is_some() || cfg.tls_key.is_some() {
            log_warn!("[rest] TLS config detected but TLS is not yet enabled; serving HTTP only");
        }

        let http_thread = std::thread::Builder::new()
            .name("rest-axum".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all().build().expect("[rest] tokio runtime");
                rt.block_on(async move {
                    let router = build_router(state, &statics, &cors_ori);
                    match tokio::net::TcpListener::bind(&bind).await {
                        Ok(l) => {
                            log_info!("[rest] listening on http://{bind}");
                            let _ = axum::serve(l, router).await;
                        }
                        Err(e) => log_error!("[rest] bind '{bind}' failed: {e}"),
                    }
                });
            }).ok();

        Self { routes, _http_thread: http_thread }
    }
}

#[component(
    kind        = "core/RestServer:1.0",
    name        = "Axum REST Server",
    description = "HTTP server with static files, CORS, hub dispatch, and OpenAPI docs.",
)]
impl RestServer {
    /// Register or replace a hub route at runtime.
    #[handle("rest/AddRoute")]
    fn add_route(&mut self, req: RouteEntry) -> Result<Value> {
        let method = req.method.to_uppercase();
        let mut routes = self.routes.lock().unwrap();
        routes.retain(|(m, p, _)| !(m == &method && p == &req.path));
        routes.push((method, req.path, req.action));
        Ok(serde_json::json!({ "ok": true }))
    }
}

// ─── Router / middleware setup ────────────────────────────────────────────────

fn build_router(state: AppState, statics: &[StaticFolderEntry], cors_origins: &[String]) -> Router {
    let mut router = Router::new()
        .route("/openapi.json", get(openapi_json_handler))
        .route("/openapi/ui",   get(swagger_ui_handler));

    for folder in statics {
        let svc = ServeDir::new(&folder.dir)
            .fallback(ServeFile::new(format!("{}/index.html", folder.dir)));
        router = router.nest_service(&folder.url_path, svc);
    }

    let cors = if cors_origins.is_empty() {
        CorsLayer::permissive()
    } else {
        log_info!("[rest] CORS: {} allowed origin(s)", cors_origins.len());
        CorsLayer::permissive() // per-origin config requires header parsing; use permissive for now
    };

    router.fallback(any(hub_handler)).layer(cors).with_state(state)
}

// ─── Request handlers ─────────────────────────────────────────────────────────

async fn openapi_json_handler(State(s): State<AppState>) -> impl IntoResponse {
    ([("content-type", "application/json")], s.openapi.as_str().to_string())
}

async fn swagger_ui_handler() -> impl IntoResponse {
    const HTML: &str = concat!(
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\">",
        "<title>Orkester API</title>",
        "<link rel=\"stylesheet\" href=\"https://cdn.jsdelivr.net/npm/swagger-ui-dist/swagger-ui.css\">",
        "</head><body><div id=\"swagger-ui\"></div>",
        "<script src=\"https://cdn.jsdelivr.net/npm/swagger-ui-dist/swagger-ui-bundle.js\"></script>",
        "<script>SwaggerUIBundle({url:\"/openapi.json\",dom_id:\"#swagger-ui\",",
        "presets:[SwaggerUIBundle.presets.apis]});</script></body></html>",
    );
    ([("content-type", "text/html; charset=utf-8")], HTML)
}

async fn hub_handler(
    State(state): State<AppState>,
    request:      axum::http::Request<axum::body::Body>,
) -> impl IntoResponse {
    let method = request.method().to_string().to_uppercase();
    let path   = request.uri().path().to_string();
    let query  = request.uri().query().unwrap_or("").to_string();

    let (action, path_params) = {
        let routes = state.routes.lock().unwrap();
        find_route(&routes, &method, &path)
    };
    let Some(action) = action else {
        log_warn!("[rest] no route: {method} {path}");
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"not found"}))).into_response();
    };

    let body = axum::body::to_bytes(request.into_body(), 16 * 1024 * 1024)
        .await.unwrap_or_default();
    let payload = merge_payload(&body, &path_params, &query);
    let id      = state.next_id.fetch_add(1, Ordering::SeqCst);
    let host    = state.host;

    log_debug!("[rest] → id={id} {method} {path} action={action}");

    let result = tokio::task::spawn_blocking(move || {
        let mut h = unsafe { Host::from_abi(host.as_raw()) };
        let env   = Envelope::from_json(id, None, &action, payload);
        h.handle::<Envelope, Value>(&env)
    }).await;

    match result {
        Ok(Ok(v))  => { log_debug!("[rest] ← id={id} ok"); Json(unwrap_single(v)).into_response() }
        Ok(Err(e)) => { log_error!("[rest] ← id={id} error: {e}"); (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":e.to_string()}))).into_response() }
        Err(e)     => { log_error!("[rest] ← id={id} spawn: {e}"); StatusCode::INTERNAL_SERVER_ERROR.into_response() }
    }
}

// ─── Route matching ───────────────────────────────────────────────────────────

fn find_route(
    routes: &[(String, String, String)],
    method: &str,
    path:   &str,
) -> (Option<String>, HashMap<String, String>) {
    for (m, pattern, action) in routes {
        if m == method || m == "ANY" {
            if let Some(params) = match_path(pattern, path) {
                return (Some(action.clone()), params);
            }
        }
    }
    (None, HashMap::new())
}

fn match_path(pattern: &str, path: &str) -> Option<HashMap<String, String>> {
    let pp: Vec<&str> = pattern.trim_end_matches('/').split('/').collect();
    let pt: Vec<&str> = path.trim_end_matches('/').split('/').collect();
    if pp.len() != pt.len() { return None; }
    let mut params = HashMap::new();
    for (part, seg) in pp.iter().zip(pt.iter()) {
        if part.starts_with('{') && part.ends_with('}') {
            params.insert(part[1..part.len() - 1].to_string(), seg.to_string());
        } else if !part.eq_ignore_ascii_case(seg) {
            return None;
        }
    }
    Some(params)
}

// ─── Payload / response helpers ───────────────────────────────────────────────

fn merge_payload(body: &[u8], path_params: &HashMap<String, String>, query: &str) -> Value {
    let mut map = serde_json::Map::new();
    for pair in query.split('&').filter(|s| !s.is_empty()) {
        if let Some((k, v)) = pair.split_once('=') {
            map.insert(k.to_string(), Value::String(v.to_string()));
        }
    }
    for (k, v) in path_params {
        map.insert(k.clone(), Value::String(v.clone()));
    }
    if let Ok(Value::Object(b)) = serde_json::from_slice::<Value>(body) { map.extend(b); }
    Value::Object(map)
}

fn unwrap_single(v: Value) -> Value {
    if let Value::Object(ref m) = v {
        if let Some(Value::Array(r)) = m.get("responses") {
            if r.len() == 1 { return r[0].clone(); }
        }
    }
    v
}

// ─── OpenAPI JSON builder ─────────────────────────────────────────────────────

fn build_openapi_json(title: &str, routes: &[RouteEntry]) -> String {
    let mut paths = serde_json::Map::new();
    for r in routes {
        let entry = paths.entry(r.path.clone()).or_insert_with(|| serde_json::json!({}));
        if let Some(obj) = entry.as_object_mut() {
            obj.insert(r.method.to_lowercase(), serde_json::json!({
                "summary":     format!("{} {}", r.method, r.path),
                "operationId": r.action.replace(['/', ':'], "_"),
                "tags":        ["hub"],
                "requestBody": { "content": { "application/json": { "schema": { "type": "object" } } } },
                "responses":   { "200": { "description": "Success", "content": { "application/json": { "schema": { "type": "object" } } } } }
            }));
        }
    }
    serde_json::json!({ "openapi": "3.0.0", "info": { "title": title, "version": "1.0.0" }, "paths": paths }).to_string()
}
