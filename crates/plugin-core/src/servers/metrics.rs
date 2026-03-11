use async_trait::async_trait;
use orkester_common::plugin::servers::{
    metrics::{MetricsError, MetricsHandle, MetricsServer},
    rest::{ApiContributor, ApiRequest, ApiResponse, HttpMethod, RouteHandler},
    AnyServer, ServerBuildError, ServerContext, ServerFactory,
};
use serde_json::Value;
use std::sync::{mpsc, Arc};
use tracing::info;

// ── No-op Handle ──────────────────────────────────────────────────────────────

/// A no-op metrics handle: all recording operations are discarded.
/// `render()` returns an empty string.
#[derive(Clone)]
pub struct NoMetricsHandle;

#[async_trait]
impl MetricsHandle for NoMetricsHandle {
    async fn increment(&self, _name: &str, _delta: u64, _labels: &[(&str, &str)]) {}
    async fn gauge(&self, _name: &str, _value: f64, _labels: &[(&str, &str)]) {}
    async fn histogram(&self, _name: &str, _value: f64, _labels: &[(&str, &str)]) {}
    async fn render(&self) -> Result<String, MetricsError> {
        Ok(String::new())
    }
}

// ── /metrics API contributor ──────────────────────────────────────────────────

struct MetricsGetHandler {
    handle: NoMetricsHandle,
}

#[async_trait]
impl RouteHandler for MetricsGetHandler {
    fn method(&self) -> HttpMethod {
        HttpMethod::Get
    }

    fn path(&self) -> &str {
        "/"
    }

    async fn handle(&self, _request: ApiRequest) -> ApiResponse {
        match self.handle.render().await {
            Ok(text) => ApiResponse {
                status: 200,
                headers: {
                    let mut h = std::collections::HashMap::new();
                    h.insert(
                        "content-type".to_string(),
                        "text/plain; version=0.0.4".to_string(),
                    );
                    h
                },
                body: text.into_bytes(),
            },
            Err(e) => ApiResponse::error(500, &e.to_string()),
        }
    }
}

// ── Server ────────────────────────────────────────────────────────────────────

pub struct NoMetricsServer {
    handle: NoMetricsHandle,
}

#[async_trait]
impl MetricsServer for NoMetricsServer {
    fn name(&self) -> &str {
        "no-metrics-server"
    }

    fn handle(&self) -> Arc<dyn MetricsHandle> {
        Arc::new(self.handle.clone())
    }

    fn run(self: Box<Self>) -> ServerContext<(), ()> {
        let (h2s_sender, h2s_receiver) = mpsc::channel();
        let (s2h_sender, s2h_receiver) = mpsc::channel();
        let hd = std::thread::spawn(move || {
            // This server does nothing and runs indefinitely until shutdown.
            info!("NoMetricsServer is running (does nothing)");
            h2s_receiver.recv().ok();
            s2h_sender.send(()).ok();
            info!("NoMetricsServer has shut down");
        });
        ServerContext {
            receiver: Some(s2h_receiver),
            sender: Some(h2s_sender),
            handle: hd,
        }
    }
}

// ── AnyServer ─────────────────────────────────────────────────────────────────

/// `NoMetricsServer` also implements `ApiContributor` so Orkester's core can
/// discover and inject the /metrics route into the REST server via `as_any()`.
impl AnyServer for NoMetricsServer {
    fn name(&self) -> &str {
        "no-metrics-server"
    }

    fn server_type(&self) -> &str {
        "metrics"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl ApiContributor for NoMetricsServer {
    fn name(&self) -> &str {
        "metrics"
    }

    fn prefix(&self) -> &str {
        "/metrics"
    }

    fn routes(&self) -> Vec<Box<dyn RouteHandler>> {
        vec![Box::new(MetricsGetHandler {
            handle: self.handle.clone(),
        })]
    }
}

// ── Factory ───────────────────────────────────────────────────────────────────

pub struct NoMetricsServerFactory;

impl ServerFactory for NoMetricsServerFactory {
    fn server_type(&self) -> &str {
        "metrics"
    }

    fn name(&self) -> &str {
        "no-metrics-server"
    }

    fn build(&self, _config: Value) -> Result<Box<dyn AnyServer>, ServerBuildError> {
        Ok(Box::new(NoMetricsServer {
            handle: NoMetricsHandle,
        }))
    }
}
