use async_trait::async_trait;
use serde_json::{Value, json};
use orkester_common::providers::auth::{AuthError, AuthProviderBuilder, AuthenticationProvider, Identity};

/// A no-op authentication provider that accepts any credentials and returns
/// an anonymous identity. Suitable for development and open/internal deployments.
pub struct NoAuthenticationProvider;

#[async_trait]
impl AuthenticationProvider for NoAuthenticationProvider {
    fn name(&self) -> &str {
        "no-auth"
    }

    async fn authenticate(&self, _credentials: &Value) -> Result<Identity, AuthError> {
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

impl AuthProviderBuilder for NoAuthProviderBuilder {
    fn name(&self) -> &str {
        "no-auth"
    }

    fn build(&self, _config: Value) -> Result<Box<dyn AuthenticationProvider>, AuthError> {
        Ok(Box::new(NoAuthenticationProvider))
    }
}
