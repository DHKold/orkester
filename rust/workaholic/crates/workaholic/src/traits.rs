use std::path::Path;

use crate::{document::RawDocument, error::Result};

// ── DocumentsLoader ───────────────────────────────────────────────────────────

/// Loads raw documents from a source (file system, remote, etc.).
pub trait DocumentsLoader: Send + Sync {
    fn load(&self, path: &Path) -> Result<Vec<RawDocument>>;
}

// ── DocumentParser ────────────────────────────────────────────────────────────

/// Parses a text payload into a typed structure.
pub trait DocumentParser: Send + Sync {
    fn parse<T>(&self, content: &str) -> Result<T>
    where
        T: for<'de> serde::Deserialize<'de>;
}

// ── PersistenceProvider ───────────────────────────────────────────────────────

/// Generic key-value store abstraction.
///
/// Data is stored and retrieved as raw `serde_json::Value`.
/// Collections are arbitrary string namespaces (e.g. `"work_runs"`, `"task_runs"`).
///
/// For typed access, use the free-function helpers:
/// [`persist`], [`retrieve`], [`retrieve_all`].
pub trait PersistenceProvider: Send + Sync {
    /// Upsert a JSON value identified by `id` inside `collection`.
    fn save(&self, collection: &str, id: &str, value: serde_json::Value) -> Result<()>;

    /// Load a single item; returns `None` when not found.
    fn load(&self, collection: &str, id: &str) -> Result<Option<serde_json::Value>>;

    /// Return all items in `collection` (order not guaranteed).
    fn list(&self, collection: &str) -> Result<Vec<serde_json::Value>>;

    /// Remove an item; a no-op when it does not exist.
    fn delete(&self, collection: &str, id: &str) -> Result<()>;
}

// ── Typed free-function helpers ───────────────────────────────────────────────

/// Serialize `value` and store it under `id` in `collection`.
pub fn persist<T: serde::Serialize>(
    p: &dyn PersistenceProvider,
    collection: &str,
    id: &str,
    value: &T,
) -> Result<()> {
    let v = serde_json::to_value(value).map_err(crate::WorkaholicError::Json)?;
    p.save(collection, id, v)
}

/// Deserialize a single item from `collection`.
pub fn retrieve<T: serde::de::DeserializeOwned>(
    p: &dyn PersistenceProvider,
    collection: &str,
    id: &str,
) -> Result<Option<T>> {
    p.load(collection, id)?
        .map(|v| serde_json::from_value(v).map_err(crate::WorkaholicError::Json))
        .transpose()
}

/// Deserialize all items from `collection`.
pub fn retrieve_all<T: serde::de::DeserializeOwned>(
    p: &dyn PersistenceProvider,
    collection: &str,
) -> Result<Vec<T>> {
    p.list(collection)?
        .into_iter()
        .map(|v| serde_json::from_value(v).map_err(crate::WorkaholicError::Json))
        .collect()
}
