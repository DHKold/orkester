//! Authentication and Authorization middleware for Axum REST server.
//!
//! - Authentication: Bearer token (JWT or opaque token)
//! - Authorization: OPA REST API integration for RBAC

use axum::{extract::Request, middleware::Next, response::Response, http::{StatusCode}};
use serde_json::json;

/// Extracts and validates the Authorization header.
pub async fn auth_middleware(req: Request, next: Next) -> Result<Response, StatusCode> {
    let auth_header = req.headers().get("authorization").and_then(|v| v.to_str().ok());
    let token = match auth_header {
        Some(h) if h.starts_with("Bearer ") => Some(&h[7..]),
        _ => None,
    };
    if token.is_none() {
        // return Err(StatusCode::UNAUTHORIZED);
    }
    // TODO: Validate token (JWT or opaque)
    // For demo, accept any non-empty token
    let user = "anonymous";
    // Attach user info to request extensions if needed
    // Authorization: call OPA
    let allowed = opa_authorize(user, req.uri().path(), req.method().as_str()).await;
    if !allowed {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(next.run(req).await)
}

/// Calls OPA REST API for authorization decision.
pub async fn opa_authorize(user: &str, path: &str, method: &str) -> bool {
    // Example OPA input
    let _input = json!({
        "input": {
            "user": user,
            "path": path,
            "method": method
        }
    });
    // TODO: Send POST to OPA (http://localhost:8181/v1/data/orkester/rbac/allow)
    // For demo, allow all
    true
}
