use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Configuration error: {0}")]
    Configuration(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

/// A generic key identifying a persisted entity.
#[derive(Debug, Clone)]
pub struct EntityKey {
    /// Namespace / collection (e.g., "workspaces", "executions").
    pub namespace: String,
    pub id: String,
}

/// Trait that all Persistence backends must implement.
///
/// Stores and retrieves all platform state: Workspaces, Works, Tasks,
/// Execution State, History, Logs, Metrics, and Configuration.
#[async_trait]
pub trait PersistenceProvider: Send + Sync {
    /// Persist an entity under the given key.
    async fn put(&self, key: &EntityKey, data: Value) -> Result<(), PersistenceError>;

    /// Retrieve an entity by key.
    async fn get(&self, key: &EntityKey) -> Result<Value, PersistenceError>;

    /// Delete an entity by key.
    async fn delete(&self, key: &EntityKey) -> Result<(), PersistenceError>;

    /// List all entity IDs in a namespace.
    async fn list(&self, namespace: &str) -> Result<Vec<String>, PersistenceError>;
}

/// Builder that creates a [`PersistenceProvider`] from a JSON configuration.
pub trait PersistenceBuilder: Send + Sync {
    fn build(&self, config: Value) -> Result<Box<dyn PersistenceProvider>, PersistenceError>;
}
