use async_trait::async_trait;
use orkester_common::plugin::providers::persistence::{
    EntityKey, PersistenceBuilder, PersistenceError, PersistenceProvider,
};
use serde_json::Value;
use std::path::PathBuf;

fn make_key(key: &EntityKey) -> String {
    format!("{}/{}", key.namespace, key.id)
}

/// A file-based persistence provider that stores each entity as a JSON file on disk.
///
/// # Config
///
/// ```yaml
/// provider: file-persistence
/// config:
///   root_dir: /var/lib/orkester/state
/// ```
///
/// Entities are stored at `<root_dir>/<namespace>/<id>.json`.
/// Namespace directories are created automatically on first write.
pub struct FilePersistenceProvider {
    root_dir: PathBuf,
}

impl FilePersistenceProvider {
    fn entity_path(&self, key: &EntityKey) -> PathBuf {
        self.root_dir
            .join(&key.namespace)
            .join(format!("{}.json", key.id))
    }

    fn namespace_dir(&self, namespace: &str) -> PathBuf {
        self.root_dir.join(namespace)
    }
}

#[async_trait]
impl PersistenceProvider for FilePersistenceProvider {
    async fn put(&self, key: &EntityKey, data: Value) -> Result<(), PersistenceError> {
        let path = self.entity_path(key);

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                PersistenceError::Internal(format!("create_dir_all '{}': {e}", parent.display()))
            })?;
        }

        let json = serde_json::to_string(&data)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        tracing::debug!(key = %make_key(key), path = %path.display(), "FilePersistenceProvider: put");

        tokio::fs::write(&path, json).await.map_err(|e| {
            PersistenceError::Internal(format!("write '{}': {e}", path.display()))
        })?;

        Ok(())
    }

    async fn get(&self, key: &EntityKey) -> Result<Value, PersistenceError> {
        let path = self.entity_path(key);

        let raw = tokio::fs::read_to_string(&path).await.map_err(|e| {
            match e.kind() {
                std::io::ErrorKind::NotFound => PersistenceError::NotFound(make_key(key)),
                _ => PersistenceError::Internal(format!("read '{}': {e}", path.display())),
            }
        })?;

        serde_json::from_str(&raw)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))
    }

    async fn delete(&self, key: &EntityKey) -> Result<(), PersistenceError> {
        let path = self.entity_path(key);

        tokio::fs::remove_file(&path).await.map_err(|e| {
            match e.kind() {
                std::io::ErrorKind::NotFound => PersistenceError::NotFound(make_key(key)),
                _ => PersistenceError::Internal(format!("remove '{}': {e}", path.display())),
            }
        })?;

        Ok(())
    }

    async fn list(&self, namespace: &str) -> Result<Vec<String>, PersistenceError> {
        let dir = self.namespace_dir(namespace);

        let mut read_dir = match tokio::fs::read_dir(&dir).await {
            Ok(rd) => rd,
            // Namespace directory not yet created — treat as empty.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => {
                return Err(PersistenceError::Internal(format!(
                    "read_dir '{}': {e}",
                    dir.display()
                )))
            }
        };

        let mut ids = Vec::new();
        while let Some(entry) = read_dir.next_entry().await.map_err(|e| {
            PersistenceError::Internal(format!("read_dir entry in '{}': {e}", dir.display()))
        })? {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(id) = name.strip_suffix(".json") {
                ids.push(id.to_string());
            }
        }

        Ok(ids)
    }
}

pub struct FilePersistenceBuilder;

impl PersistenceBuilder for FilePersistenceBuilder {
    fn build(&self, config: Value) -> Result<Box<dyn PersistenceProvider>, PersistenceError> {
        let root_dir = config
            .get("root_dir")
            .or_else(|| config.get("root-dir"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                PersistenceError::Configuration("'root_dir' is required".to_string())
            })?;

        Ok(Box::new(FilePersistenceProvider {
            root_dir: PathBuf::from(root_dir),
        }))
    }
}
