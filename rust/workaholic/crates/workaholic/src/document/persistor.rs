use thiserror::Error;

use crate::document::Document;

#[derive(Debug, Error)]
pub enum PersistorError {
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
pub type EntityValue = Document;

/// Trait that all Persistence components must implement.
pub trait DocumentPersistor: Send + Sync {
    /// Persist an entity under the given key.
    fn put(&self, key: &EntityKey, data: EntityValue) -> Result<(), PersistorError>;

    /// Retrieve an entity by key.
    fn get(&self, key: &EntityKey) -> Result<EntityValue, PersistorError>;

    /// Delete an entity by key.
    fn delete(&self, key: &EntityKey) -> Result<(), PersistorError>;

    /// List all entity keys in a path.
    fn list(&self, path: &str) -> Result<Vec<EntityKey>, PersistorError>;
}