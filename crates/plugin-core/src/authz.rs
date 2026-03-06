use async_trait::async_trait;
use serde_json::Value;
use orkester_common::providers::authz::{
    AuthzError, AuthzProviderBuilder, AuthzRequest, AuthorizationProvider,
};

/// A basic authorization provider that allows or denies access based on a
/// static configuration. The default (no config) is to allow everything.
///
/// # Configuration (JSON)
/// ```json
/// {
///   "mode": "allow_all"    // "allow_all" (default) | "deny_all"
/// }
/// ```
pub struct BasicAuthorizationProvider {
    deny_all: bool,
}

#[async_trait]
impl AuthorizationProvider for BasicAuthorizationProvider {
    fn name(&self) -> &str {
        "basic-authz"
    }

    async fn authorize(&self, request: &AuthzRequest) -> Result<(), AuthzError> {
        if self.deny_all {
            tracing::warn!(
                subject = %request.identity.subject,
                resource = %request.resource,
                action  = %request.action,
                "BasicAuthorizationProvider: DENY (deny_all mode)"
            );
            return Err(AuthzError::Forbidden(format!(
                "Access denied for '{}' on '{}' (deny_all mode)",
                request.identity.subject, request.resource
            )));
        }
        tracing::debug!(
            subject = %request.identity.subject,
            resource = %request.resource,
            action  = %request.action,
            "BasicAuthorizationProvider: ALLOW"
        );
        Ok(())
    }
}

pub struct BasicAuthzProviderBuilder;

impl AuthzProviderBuilder for BasicAuthzProviderBuilder {
    fn name(&self) -> &str {
        "basic-authz"
    }

    fn build(&self, config: Value) -> Result<Box<dyn AuthorizationProvider>, AuthzError> {
        let deny_all = config
            .get("mode")
            .and_then(Value::as_str)
            .map(|m| m == "deny_all")
            .unwrap_or(false);

        Ok(Box::new(BasicAuthorizationProvider { deny_all }))
    }
}
