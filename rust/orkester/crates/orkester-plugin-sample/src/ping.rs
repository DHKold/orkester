use orkester_plugin::prelude::*;
use serde::{Deserialize, Serialize};

// ── Messages ──────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct PingRequest {
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Serialize)]
pub struct PongResponse {
    pub status:       &'static str,
    pub echo:         Option<String>,
    pub timestamp_ms: u64,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ── Component ─────────────────────────────────────────────────────────────────

/// Minimal stateless ping-pong server.
///
/// Handles one action: `ping/Ping` → returns `{status:"pong", echo:<msg>}`.
#[derive(Default)]
pub struct PingServer;

#[component(
    kind        = "sample/PingServer:1.0",
    name        = "PingServer",
    description = "Stateless ping-pong server."
)]
impl PingServer {
    #[handle("ping/Ping")]
    fn ping(&mut self, req: PingRequest) -> Result<PongResponse> {
        log::debug!("[ping] received ping: {:?}", req.message);
        Ok(PongResponse {
            status:       "pong",
            echo:         req.message,
            timestamp_ms: now_ms(),
        })
    }
}
