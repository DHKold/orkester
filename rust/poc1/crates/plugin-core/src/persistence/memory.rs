use async_trait::async_trait;
use orkester_common::plugin::providers::persistence::{
    EntityKey, PersistenceBuilder, PersistenceError, PersistenceProvider,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

fn make_key(key: &EntityKey) -> String {
    format!("{}/{}", key.namespace, key.id)
}

/// An in-memory persistence provider backed by a `HashMap`.
/// All data is lost when the process exits. Suitable for development and testing.
#[derive(Default)]
pub struct MemoryPersistenceProvider {
    store: Arc<RwLock<HashMap<String, Value>>>,
}

#[async_trait]
impl PersistenceProvider for MemoryPersistenceProvider {
    async fn put(&self, key: &EntityKey, data: Value) -> Result<(), PersistenceError> {
        let mut store = self.store.write().await;
        tracing::debug!(key = %make_key(key), "MemoryPersistenceProvider: put");
        store.insert(make_key(key), data);
        Ok(())
    }

    async fn get(&self, key: &EntityKey) -> Result<Value, PersistenceError> {
        let store = self.store.read().await;
        store
            .get(&make_key(key))
            .cloned()
            .ok_or_else(|| PersistenceError::NotFound(make_key(key)))
    }

    async fn delete(&self, key: &EntityKey) -> Result<(), PersistenceError> {
        let mut store = self.store.write().await;
        let k = make_key(key);
        if store.remove(&k).is_none() {
            return Err(PersistenceError::NotFound(k));
        }
        Ok(())
    }

    async fn list(&self, namespace: &str) -> Result<Vec<String>, PersistenceError> {
        let store = self.store.read().await;
        let prefix = format!("{}/", namespace);
        Ok(store
            .keys()
            .filter(|k| k.starts_with(&prefix))
            .map(|k| k[prefix.len()..].to_string())
            .collect())
    }
}

pub struct MemoryPersistenceBuilder;

impl PersistenceBuilder for MemoryPersistenceBuilder {
    fn build(&self, _config: Value) -> Result<Box<dyn PersistenceProvider>, PersistenceError> {
        Ok(Box::new(MemoryPersistenceProvider::default()))
    }
}
