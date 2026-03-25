//! Management API — registers and handles built-in platform endpoints.
//!
//! Routes registered with every REST server at startup:
//!
//! | Method | Path           | Description                          |
//! |--------|----------------|--------------------------------------|
//! | GET    | /v1/health     | Liveness check + uptime              |
//! | GET    | /v1/plugins    | Loaded plugin metadata               |
//! | GET    | /v1/config     | Current configuration snapshot       |
//! | GET    | /v1/servers    | Running server list                  |
//!
//! # How it works
//!
//! `ManagementApi` registers itself as a hub participant named `"orkester"`.
//! On startup it sends `register_route` messages to every REST server so that
//! incoming HTTP requests for the four paths are forwarded to it via the hub.
//! During the main loop [`ManagementApi::poll`] drains its inbound channel,
//! handles each `http_request`, and sends a response back to the REST server.

use std::sync::mpsc;
use std::time::Instant;

use orkester_common::messaging::Message;
use orkester_common::plugin::Registry;
use orkester_common::{log_debug, log_info, log_warn};
use serde_json::{json, Value};

use crate::config::ConfigTree;
use crate::messaging::{create, HubSide};
use crate::server::RunningServer;

/// The hub participant name used by the management API.
const PARTICIPANT: &str = "orkester";

/// Routes served by the management API.
const ROUTES: &[(&str, &str)] = &[
    ("GET", "/v1/health"),
    ("GET", "/v1/plugins"),
    ("GET", "/v1/config"),
    ("GET", "/v1/servers"),
];

// ── ManagementApi ─────────────────────────────────────────────────────────────

pub struct ManagementApi {
    to_hub: mpsc::Sender<Message>,
    from_hub: mpsc::Receiver<Message>,

    /// Instance names of REST servers routes should be registered with.
    rest_servers: Vec<String>,

    // Response snapshots captured once at construction time.
    plugins_json: Value,
    config_json: Value,
    servers_json: Value,

    started_at: Instant,
}

impl ManagementApi {
    /// Build the management API from the post-startup state.
    ///
    /// Returns `(api, hub_side)` — the caller must register `hub_side` with
    /// the [`Hub`](crate::messaging::Hub) before calling [`register_routes`].
    pub fn new(
        registry: &dyn Registry,
        config: &ConfigTree,
        running: &[RunningServer],
    ) -> (Self, HubSide) {
        let (hub_side, server_side) = create(PARTICIPANT);

        // Identify REST server instance names so we know where to send
        // `register_route` messages.  A server whose component key ends with
        // "rest-server" is considered a REST server.
        let rest_servers: Vec<String> = running
            .iter()
            .filter(|s| s.component_key.ends_with("rest-server"))
            .map(|s| s.instance_name.clone())
            .collect();

        let api = ManagementApi {
            to_hub: server_side.to_hub,
            from_hub: server_side.from_hub,
            rest_servers,
            plugins_json: build_plugins_json(registry),
            config_json: config.0.clone(),
            servers_json: build_servers_json(running),
            started_at: Instant::now(),
        };

        (api, hub_side)
    }

    /// Send `register_route` messages to all REST servers for each management
    /// path.  Call this once, after the hub has been set up, to activate the
    /// endpoints.
    pub fn register_routes(&self) {
        for rest in &self.rest_servers {
            for (method, path) in ROUTES {
                let msg = Message::new(
                    0,
                    "",
                    rest.as_str(),
                    "register_route",
                    json!({ "method": method, "path": path }),
                );
                if self.to_hub.send(msg).is_err() {
                    log_warn!(
                        "Management: could not send register_route for {} {} — hub disconnected",
                        method,
                        path
                    );
                }
            }
            log_info!("Management: sent route registrations to '{}'.", rest);
        }
    }

    /// Drain all pending inbound messages from the hub and handle them.
    /// Call this on every main-loop iteration.
    pub fn poll(&self) {
        loop {
            match self.from_hub.try_recv() {
                Ok(msg) => self.handle(msg),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => break,
            }
        }
    }

    // ── Internal ─────────────────────────────────────────────────────────────

    fn handle(&self, msg: Message) {
        match msg.message_type.as_str() {
            "http_request" => self.handle_http(msg),

            "route_registered" => {
                let method = msg
                    .content
                    .get("method")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let path = msg
                    .content
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                log_info!("Management: route registered — {} {}", method, path);
            }

            "error" => {
                log_warn!(
                    "Management: hub error — {}",
                    msg.content
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                );
            }

            other => {
                log_debug!("Management: ignoring unexpected message type '{}'", other);
            }
        }
    }

    fn handle_http(&self, msg: Message) {
        let path = msg
            .content
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let corr_id = msg
            .content
            .get("correlation_id")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let rest_server = msg.source.clone();

        log_debug!("Management: GET {} (correlation_id={})", path, corr_id);

        let body: Value = match path {
            "/v1/health" => json!({
                "status": "ok",
                "uptime_seconds": self.started_at.elapsed().as_secs(),
            }),
            "/v1/plugins" => self.plugins_json.clone(),
            "/v1/config" => self.config_json.clone(),
            "/v1/servers" => self.servers_json.clone(),
            other => {
                log_warn!(
                    "Management: received http_request for unhandled path '{}'",
                    other
                );
                json!({ "error": "not found" })
            }
        };

        let reply = Message::new(
            0,
            "",
            rest_server.as_str(),
            "http_response",
            json!({ "correlation_id": corr_id, "status": 200, "body": body }),
        );

        if self.to_hub.send(reply).is_err() {
            log_warn!(
                "Management: could not send response for {} — hub disconnected",
                path
            );
        }
    }
}

// ── Snapshot builders ─────────────────────────────────────────────────────────

fn build_plugins_json(registry: &dyn Registry) -> Value {
    let list: Vec<Value> = registry
        .plugins()
        .iter()
        .map(|m| {
            json!({
                "id":          m.id,
                "version":     m.version,
                "description": m.description,
                "authors":     m.authors,
            })
        })
        .collect();
    json!({ "plugins": list })
}

fn build_servers_json(running: &[RunningServer]) -> Value {
    let list: Vec<Value> = running
        .iter()
        .map(|s| {
            json!({
                "instance_name": s.instance_name,
                "component":     s.component_key,
            })
        })
        .collect();
    json!({ "servers": list })
}
