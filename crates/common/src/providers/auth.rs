use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
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
    /// Provider's unique name (e.g., "oidc", "ldap").
    fn name(&self) -> &str;

    /// Validate the raw credentials/token and return an Identity on success.
    async fn authenticate(&self, credentials: &Value) -> Result<Identity, AuthError>;
}

/// Builder that creates an [`AuthenticationProvider`] from a JSON configuration.
pub trait AuthProviderBuilder: Send + Sync {
    /// Name of the provider this builder creates.
    fn name(&self) -> &str;

    /// Instantiate the provider with the given configuration.
    fn build(&self, config: Value) -> Result<Box<dyn AuthenticationProvider>, AuthError>;
}
