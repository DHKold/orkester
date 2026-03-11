use serde_json::Value;
use thiserror::Error;

/// Error type for server failures, e.g. due to invalid config.
#[derive(Debug, Error)]
pub enum ServerError {
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Server 
pub trait Server: Send + Sync {
    fn start(&self) -> Result<(), ServerError>;
    fn stop(&self) -> Result<(), ServerError>;
}

/// Server factory trait for dynamic server construction from config.
pub trait ServerBuilder: Send + Sync {
    fn build(&self, config: Value) -> Result<Box<dyn Server>, ServerError>;
}
