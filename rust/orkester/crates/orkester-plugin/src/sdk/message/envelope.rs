use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Generic request envelope sent to a component.
///
/// `action` routes the call; `params` carries the typed payload.
#[derive(Debug, Serialize, Deserialize)]
pub struct Request<P = Value> {
    pub action: String,
    pub params: P,
}

impl<P> Request<P> {
    pub fn new(action: impl Into<String>, params: P) -> Self {
        Self { action: action.into(), params }
    }
}

/// Request envelope that asks a component to create a sub-component.
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateComponentRequest<C = Value> {
    pub kind: String,
    pub config: Option<C>,
}

impl CreateComponentRequest {
    pub fn new(kind: impl Into<String>) -> Self {
        Self { kind: kind.into(), config: None }
    }
}

impl<C> CreateComponentRequest<C> {
    pub fn with_config(kind: impl Into<String>, config: C) -> Self {
        Self { kind: kind.into(), config: Some(config) }
    }
}
