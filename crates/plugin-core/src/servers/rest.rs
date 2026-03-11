use async_trait::async_trait;
use axum::body::Body;
use axum::extract::RawPathParams;
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use orkester_common::plugin::servers::{
    rest::{ApiContributor, ApiRequest, ApiResponse, HttpMethod, RestServer, RouteHandler},
    AnyServer, ServerBuildError, ServerContext, ServerFactory,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

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
        .filter_map(|(k, v)| Some((k.as_str().to_string(), v.to_str().ok()?.to_string())))
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
    // Built-in health endpoint: GET {api_base}/health
    let health_path = normalise_path(&format!("{}/health", api_base));
    info!("  GET {} (built-in)", health_path);
    let mut router = axum::Router::new().route(
        &health_path,
        axum::routing::get(|| async { axum::Json(serde_json::json!({ "status": "ok" })) }),
    );

    for contributor in contributors {
        for handler_box in contributor.routes() {
            let handler: Arc<dyn RouteHandler> = Arc::from(handler_box);
            let method = handler.method();
            let full_path = format!("{}{}{}", api_base, contributor.prefix(), handler.path());
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

// ── Server ────────────────────────────────────────────────────────────────────

pub struct AxumRestServer {
    config: Value,
    router: axum::Router,
}

#[async_trait]
impl RestServer for AxumRestServer {
    fn name(&self) -> &str {
        "axum-rest-server"
    }

    fn run(self: Box<Self>) -> ServerContext<(), ()> {
        let config = self.config.clone();
        let hd = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async move {
                // Get configuration with defaults.
                let bind_host = config
                    .get("hostname")
                    .and_then(|v| v.as_str())
                    .unwrap_or("localhost");
                let bind_port = config.get("port").and_then(|v| v.as_u64()).unwrap_or(8080);
                let bind_addr = format!("{}:{}", bind_host, bind_port);

                // Start the server (this will block until shutdown)
                info!("Starting AxumRestServer on {}", bind_addr);
                let listener = tokio::net::TcpListener::bind(&bind_addr).await.unwrap();
                axum::serve(listener, self.router).await.unwrap();
                info!("AxumRestServer on {} has shut down", bind_addr);
            });
        });
        ServerContext {
            receiver: Option::None, // No messages received from the server in this implementation
            sender: Option::None,   // No messages sent to the server in this implementation
            handle: hd,
        }
    }
}

// ── AnyServer ─────────────────────────────────────────────────────────────────

impl AnyServer for AxumRestServer {
    fn name(&self) -> &str {
        "axum-rest-server"
    }

    fn server_type(&self) -> &str {
        "rest"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// ── Factory ───────────────────────────────────────────────────────────────────

pub struct AxumRestServerFactory;

impl ServerFactory for AxumRestServerFactory {
    fn server_type(&self) -> &str {
        "rest"
    }

    fn name(&self) -> &str {
        "axum-rest-server"
    }

    fn build(&self, config: Value) -> Result<Box<dyn AnyServer>, ServerBuildError> {
        info!("Building AxumRestServer");
        let api_base = config
            .get("base_path")
            .and_then(|v| v.as_str())
            .unwrap_or("/api")
            .to_string();

        // Contributors are injected by Orkester core at runtime via AnyServer downcasting.
        let router = build_router(&[], &api_base);

        Ok(Box::new(AxumRestServer { config, router }))
    }
}
