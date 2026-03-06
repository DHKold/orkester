use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;

use super::auth::Identity;

#[derive(Debug, Error)]
pub enum AuthzError {
    #[error("Access denied: {0}")]
    Forbidden(String),
    #[error("Configuration error: {0}")]
    Configuration(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Describes the action being requested for an authorization decision.
#[derive(Debug, Clone)]
pub struct AuthzRequest {
    pub identity: Identity,
    /// Resource being accessed (e.g., "/workspaces/my-ws/works/my-work").
    pub resource: String,
    /// Action being performed (e.g., "read", "execute", "delete").
    pub action: String,
    /// Additional context (e.g., request metadata).
    pub context: Value,
}

/// Trait that all Authorization Providers must implement.
#[async_trait]
pub trait AuthorizationProvider: Send + Sync {
    /// Provider's unique name (e.g., "opa", "rbac").
    fn name(&self) -> &str;

    /// Return `Ok(())` if the request is allowed, or `Err(AuthzError::Forbidden)` if denied.
    async fn authorize(&self, request: &AuthzRequest) -> Result<(), AuthzError>;
}

/// Builder that creates an [`AuthorizationProvider`] from a JSON configuration.
pub trait AuthzProviderBuilder: Send + Sync {
    fn name(&self) -> &str;
    fn build(&self, config: Value) -> Result<Box<dyn AuthorizationProvider>, AuthzError>;
}
