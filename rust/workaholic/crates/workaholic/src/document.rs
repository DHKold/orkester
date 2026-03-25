use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;

use crate::utils::{default_false, default_vec};

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
pub struct DocumentStatus {
    // Whether the document is marked as deleted (e.g. for soft-deletion or tombstoning).
    #[serde(default="default_false")]
    pub deleted: bool,
    // Additional free-form status fields (e.g. progress, logs, etc.).
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document<T> {
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
    // Runtime state; absent for catalog resources, present for execution objects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<DocumentStatus>,
}

#[derive(Debug, Error)]
pub enum LoaderError {
    #[error("Wrong file extension for path {0}: expected one of {1}")]
    WrongFileExtension(String, String),
    #[error("Invalid path {0}: {1}")]
    WrongPath(String, String),
    #[error("Failed to read file {0}: {1}")]
    ReadError(String, String),
}

pub trait DocumentsLoader: Send + Sync {
    fn load(&self) -> Result<Vec<Document<Value>>, Error>;
}

pub trait DocumentParser<T>: Send + Sync {
    fn parse(&self, content: &str) -> Result<Vec<Document<T>>, Error>;
}
