use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use axum::body::Body;
use axum::extract::RawPathParams;
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use serde_json::Value;
use tokio::net::TcpListener;
use tracing::info;
use orkester_common::servers::rest::{
    ApiContributor, ApiRequest, ApiResponse, HttpMethod, RestError, RestHandle, RestServer,
    RestServerDeps, RestServerFactory, RouteHandler,
};

// ── Request / response conversion ─────────────────────────────────────────────

async fn to_api_request(
    req: axum::extract::Request,
    path_params: HashMap<String, String>,
) -> ApiRequest {
    let method = match req.method() {
        &axum::http::Method::GET => HttpMethod::Get,
        &axum::http::Method::POST => HttpMethod::Post,
        &axum::http::Method::PUT => HttpMethod::Put,
        &axum::http::Method::PATCH => HttpMethod::Patch,
        &axum::http::Method::DELETE => HttpMethod::Delete,
        _ => HttpMethod::Get,
    };

    let path = req.uri().path().to_string();

    let query_params: HashMap<String, String> = req
        .uri()
        .query()
        .unwrap_or("")
        .split('&')
        .filter(|s| !s.is_empty())
        .filter_map(|pair| {
            let mut it = pair.splitn(2, '=');
            let k = it.next()?.to_string();
            let v = it.next().unwrap_or("").to_string();
            Some((k, v))
        })
        .collect();

    let headers: HashMap<String, String> = req
        .headers()
        .iter()
        .filter_map(|(k, v)| {
            Some((
                k.as_str().to_string(),
                v.to_str().ok()?.to_string(),
            ))
        })
        .collect();

    let body = axum::body::to_bytes(req.into_body(), usize::MAX)
        .await
        .unwrap_or_default()
        .to_vec();

    ApiRequest {
        method,
        path,
        headers,
        path_params,
        query_params,
        body,
    }
}

fn to_axum_response(api_resp: ApiResponse) -> axum::response::Response {
    let status = StatusCode::from_u16(api_resp.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut builder = axum::http::Response::builder().status(status);

    for (k, v) in api_resp.headers {
        if let (Ok(name), Ok(val)) = (
            HeaderName::from_bytes(k.as_bytes()),
            HeaderValue::from_str(&v),
        ) {
            builder = builder.header(name, val);
        }
    }

    builder
        .body(Body::from(api_resp.body))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

// ── Router assembly ───────────────────────────────────────────────────────────

fn build_router(contributors: &[Arc<dyn ApiContributor>], api_base: &str) -> axum::Router {
    let mut router = axum::Router::new();

    for contributor in contributors {
        for handler_box in contributor.routes() {
            let handler: Arc<dyn RouteHandler> = Arc::from(handler_box);
            let method = handler.method();
            let full_path = format!(
                "{}{}{}", api_base,
                contributor.prefix(),
                handler.path()
            );
            // Normalise duplicate slashes (e.g. "/api/v1/metrics/")
            let full_path = normalise_path(&full_path);

            info!("  {:?} {}", method, full_path);

            let h = handler.clone();
            let axum_handler = move |req: axum::extract::Request| {
                let h = h.clone();
                async move {
                    // Extract path params stored by axum matchit
                    let path_params: HashMap<String, String> = req
                        .extensions()
                        .get::<RawPathParams>()
                        .map(|p| {
                            p.iter()
                                .map(|(k, v)| (k.to_string(), v.to_string()))
                                .collect()
                        })
                        .unwrap_or_default();

                    let api_req = to_api_request(req, path_params).await;
                    let api_resp = h.handle(api_req).await;
                    to_axum_response(api_resp)
                }
            };

            let method_router = match method {
                HttpMethod::Get => axum::routing::get(axum_handler),
                HttpMethod::Post => axum::routing::post(axum_handler),
                HttpMethod::Put => axum::routing::put(axum_handler),
                HttpMethod::Patch => axum::routing::patch(axum_handler),
                HttpMethod::Delete => axum::routing::delete(axum_handler),
            };

            router = router.route(&full_path, method_router);
        }
    }

    router
}

fn normalise_path(p: &str) -> String {
    // Collapse duplicate slashes; ensure leading slash; strip trailing slash
    let mut s = String::with_capacity(p.len());
    let mut prev_slash = false;
    for c in p.chars() {
        if c == '/' {
            if !prev_slash {
                s.push(c);
            }
            prev_slash = true;
        } else {
            s.push(c);
            prev_slash = false;
        }
    }
    if s.len() > 1 && s.ends_with('/') {
        s.pop();
    }
    if s.is_empty() {
        s.push('/');
    }
    s
}

// ── Handle ────────────────────────────────────────────────────────────────────

pub struct AxumRestHandle {
    bound_addr: String,
}

impl RestHandle for AxumRestHandle {
    fn bound_addr(&self) -> &str {
        &self.bound_addr
    }
}

// ── Server ────────────────────────────────────────────────────────────────────

pub struct AxumRestServer {
    addr: String,
    router: axum::Router,
    handle: Arc<AxumRestHandle>,
}

#[async_trait]
impl RestServer for AxumRestServer {
    fn name(&self) -> &str {
        "axum-rest-server"
    }

    fn handle(&self) -> Arc<dyn RestHandle> {
        self.handle.clone()
    }

    async fn run(self: Box<Self>) {
        // Spin up a dedicated OS thread with its own single-threaded tokio runtime.
        // This keeps the plugin's reactor completely isolated from the host's — no
        // handle-sharing, no cross-cdylib thread-local confusion.
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build REST runtime");
            rt.block_on(async move {
                info!("AxumRestServer listening on {}", self.addr);
                let listener = TcpListener::bind(&self.addr)
                    .await
                    .expect("failed to bind REST listener");
                axum::serve(listener, self.router)
                    .await
                    .expect("axum server error");
            });
            let _ = tx.send(());
        });
        // The host task stays alive until the server thread exits.
        rx.await.ok();
    }
}

// ── Factory ───────────────────────────────────────────────────────────────────

/// Configuration keys (all optional):
/// - `bind`     : `"<address>:<port>"` — default `"0.0.0.0:8080"`
/// - `api_base` : global path prefix   — default `"/api/v1"`
pub struct AxumRestServerFactory;

impl RestServerFactory for AxumRestServerFactory {
    fn name(&self) -> &str {
        "axum-rest-server"
    }

    fn build(&self, config: Value, deps: RestServerDeps) -> Result<Box<dyn RestServer>, RestError> {
        let bind_addr = config
            .get("bind")
            .and_then(|v| v.as_str())
            .unwrap_or("0.0.0.0:8080");

        let api_base = config
            .get("api_base")
            .and_then(|v| v.as_str())
            .unwrap_or("/api/v1")
            .to_string();

        info!("Building AxumRestServer with API base path '{}'", api_base);
        for c in &deps.contributors {
            info!("  contributor: {} (prefix={})", c.name(), c.prefix());
        }

        let router = build_router(&deps.contributors, &api_base);

        Ok(Box::new(AxumRestServer {
            addr: bind_addr.to_string(),
            router,
            handle: Arc::new(AxumRestHandle { bound_addr: bind_addr.to_string() }),
        }))
    }
}
