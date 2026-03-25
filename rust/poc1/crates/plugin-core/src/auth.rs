use async_trait::async_trait;
use orkester_common::plugin::providers::auth::{
    AuthenticationError, AuthenticationProvider, AuthenticationProviderBuilder, Identity,
};
use serde_json::{json, Value};

/// A no-op authentication provider that accepts any credentials and returns
/// an anonymous identity. Suitable for development and open/internal deployments.
pub struct NoAuthenticationProvider;

#[async_trait]
impl AuthenticationProvider for NoAuthenticationProvider {
    async fn authenticate(&self, _credentials: &Value) -> Result<Identity, AuthenticationError> {
        tracing::debug!("NoAuthenticationProvider: granting anonymous identity");
        Ok(Identity {
            subject: "anonymous".to_string(),
            name: Some("Anonymous".to_string()),
            groups: vec!["everyone".to_string()],
            claims: json!({}),
        })
    }
}

pub struct NoAuthProviderBuilder;

impl AuthenticationProviderBuilder for NoAuthProviderBuilder {
    fn build(
        &self,
        _config: Value,
    ) -> Result<Box<dyn AuthenticationProvider>, AuthenticationError> {
        Ok(Box::new(NoAuthenticationProvider))
    }
}
