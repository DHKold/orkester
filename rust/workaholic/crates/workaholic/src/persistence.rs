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

/// Generic key-value store abstraction for Workaholic.
pub type EntityKey = String;
pub type EntityValue = Value;

/// Trait that all Persistence components must implement.
pub trait PersistenceComponent: Send + Sync {
    /// Persist an entity under the given key.
    fn put(&self, key: &EntityKey, data: EntityValue) -> Result<(), PersistenceError>;

    /// Retrieve an entity by key.
    fn get(&self, key: &EntityKey) -> Result<EntityValue, PersistenceError>;

    /// Delete an entity by key.
    fn delete(&self, key: &EntityKey) -> Result<(), PersistenceError>;

    /// List all entity keys in a path.
    fn list(&self, path: &str) -> Result<Vec<EntityKey>, PersistenceError>;
}