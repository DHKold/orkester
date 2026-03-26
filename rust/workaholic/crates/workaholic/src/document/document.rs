use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::utils::default_vec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMetadata {
    // Optional namespace to isolate documents (e.g. by team, project, etc.).
    pub namespace: Option<String>,
    // Optional ownership for access control, billing, etc.
    pub owner: Option<String>,
    // Optional description for human readers.
    pub description: Option<String>,
    // Optional tags for categorization, filtering, etc.
    #[serde(default="default_vec")]
    pub tags: Vec<String>,
    // Additional free-form metadata fields (e.g. author, team, etc.).
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document<T = Value, E = Value> {
    // Resource kind, e.g. "orkester/task:1.0".
    pub kind: String,
    // Resource name, unique within its namespace.
    pub name: String,
    // SemVer resource version.
    pub version: String,
    // Free-form metadata (description, tags, owner, etc.) with common fields and extensibility.
    pub metadata: DocumentMetadata,
    // Resource-specific specification.
    pub spec: T,
    // Runtime state and status information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<E>,
}