use std::sync::Arc;

use serde_json::Value;
use thiserror::Error;

use crate::messaging::ServerSide;
use crate::plugin::providers::executor::ExecutorRegistry;

/// Error type for server failures, e.g. due to invalid config.
#[derive(Debug, Error)]
pub enum ServerError {
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Context passed to a server when it is started.
///
/// Bundles the hub channel together with shared platform services so that
/// servers can access them without coupling to the core app's registry directly.
pub struct ServerContext {
    /// The server's bi-directional hub channel.
    pub channel: ServerSide,
    /// Registry of available task executors (shared, thread-safe).
    pub executor_registry: Arc<ExecutorRegistry>,
}

/// Server
pub trait Server: Send + Sync {
    fn start(&self, ctx: ServerContext) -> Result<(), ServerError>;
    fn stop(&self) -> Result<(), ServerError>;
}

/// Server factory trait for dynamic server construction from config.
pub trait ServerBuilder: Send + Sync {
    fn build(&self, config: Value) -> Result<Box<dyn Server>, ServerError>;
}
