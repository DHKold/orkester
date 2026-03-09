use crate::servers::ServerContext;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RestError {
    #[error("Bind error: {0}")]
    Bind(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

// ── Framework-agnostic HTTP types ─────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

/// A framework-agnostic inbound HTTP request passed to a route handler.
#[derive(Debug, Clone)]
pub struct ApiRequest {
    pub method: HttpMethod,
    /// Full path including prefix (e.g. "/api/v1/metrics").
    pub path: String,
    pub headers: HashMap<String, String>,
    pub path_params: HashMap<String, String>,
    pub query_params: HashMap<String, String>,
    pub body: Vec<u8>,
}

/// A framework-agnostic outbound HTTP response produced by a route handler.
#[derive(Debug, Clone)]
pub struct ApiResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl ApiResponse {
    pub fn ok(body: impl Into<Vec<u8>>) -> Self {
        Self {
            status: 200,
            headers: HashMap::new(),
            body: body.into(),
        }
    }
    pub fn json(status: u16, value: &Value) -> Self {
        let body = value.to_string().into_bytes();
        let mut headers = HashMap::new();
        headers.insert("content-type".to_string(), "application/json".to_string());
        Self {
            status,
            headers,
            body,
        }
    }
    pub fn error(status: u16, message: &str) -> Self {
        Self::json(status, &serde_json::json!({ "error": message }))
    }
}

// ── Route contribution ────────────────────────────────────────────────────────

/// A single route handler: one HTTP method + relative path + async callback.
#[async_trait]
pub trait RouteHandler: Send + Sync {
    fn method(&self) -> HttpMethod;
    /// Relative path within this contributor's prefix (e.g. "/" or "/:id").
    fn path(&self) -> &str;
    async fn handle(&self, request: ApiRequest) -> ApiResponse;
}

/// A plugin component that contributes one or more API routes to the REST server.
///
/// The REST server collects all registered `ApiContributor`s from loaded plugins,
/// applies the optional `prefix()` to each contributor's routes, optionally adds
/// a global API version prefix (e.g. `/api/v1`), and assembles the final router.
///
/// # Example
/// The MetricsServer plugin registers:
///   prefix  = "/metrics"
///   routes  = [ GET / ]
/// The REST server exposes: GET /api/v1/metrics
pub trait ApiContributor: Send + Sync {
    /// Component name for logging/debugging (e.g. "metrics", "workspaces").
    fn name(&self) -> &str;

    /// Path prefix prepended to all routes from this contributor.
    /// Should start with '/' (e.g. "/metrics", "/workspaces").
    fn prefix(&self) -> &str;

    /// The route handlers this contributor provides.
    fn routes(&self) -> Vec<Box<dyn RouteHandler>>;
}

// ── REST Server ───────────────────────────────────────────────────────────────

/// Dependencies injected into the REST server at startup.
pub struct RestServerDeps {
    /// All API contributors collected from every loaded plugin.
    pub contributors: Vec<Arc<dyn ApiContributor>>,
}

/// A running REST server.
#[async_trait]
pub trait RestServer: Send + Sync {
    fn name(&self) -> &str;
    fn run(self: Box<Self>) -> ServerContext<(), ()>;
}

/// Plugin factory for creating a RestServer from configuration and injected dependencies.
pub trait RestServerFactory: Send + Sync {
    fn name(&self) -> &str;
    fn build(&self, config: Value, deps: RestServerDeps) -> Result<Box<dyn RestServer>, RestError>;
}
