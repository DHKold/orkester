use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthenticationError {
    #[error("Authentication failed: {0}")]
    Unauthorized(String),
    #[error("Configuration error: {0}")]
    Configuration(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Represents the identity of an authenticated principal.
#[derive(Debug, Clone)]
pub struct Identity {
    /// Unique subject identifier.
    pub subject: String,
    /// Display name.
    pub name: Option<String>,
    /// Groups / roles this identity belongs to.
    pub groups: Vec<String>,
    /// Additional claims.
    pub claims: Value,
}

/// Trait that all Authentication Providers must implement.
#[async_trait]
pub trait AuthenticationProvider: Send + Sync {
    /// Validate the raw credentials/token and return an Identity on success.
    async fn authenticate(&self, credentials: &Value) -> Result<Identity, AuthenticationError>;
}

/// Builder that creates an [`AuthenticationProvider`] from a JSON configuration.
pub trait AuthenticationProviderBuilder: Send + Sync {
    fn build(&self, config: Value) -> Result<Box<dyn AuthenticationProvider>, AuthenticationError>;
}
