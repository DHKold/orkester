use std::{collections::HashMap, fmt, sync::Mutex};

use crate::{
    error::{Result, WorkaholicError},
    traits::PersistenceProvider,
};

/// In-memory persistence — not durable across restarts, suitable for testing
/// and development.
#[derive(Default)]
pub struct MemoryPersistenceProvider {
    /// `{ collection -> { id -> value } }`
    data: Mutex<HashMap<String, HashMap<String, serde_json::Value>>>,
}

impl MemoryPersistenceProvider {
    pub fn new() -> Self {
        Self::default()
    }
}

fn lock_err(e: impl fmt::Display) -> WorkaholicError {
    WorkaholicError::Persistence(format!("mutex poisoned: {e}"))
}

impl PersistenceProvider for MemoryPersistenceProvider {
    fn save(&self, collection: &str, id: &str, value: serde_json::Value) -> Result<()> {
        self.data
            .lock()
            .map_err(lock_err)?
            .entry(collection.to_string())
            .or_default()
            .insert(id.to_string(), value);
        Ok(())
    }

    fn load(&self, collection: &str, id: &str) -> Result<Option<serde_json::Value>> {
        Ok(self
            .data
            .lock()
            .map_err(lock_err)?
            .get(collection)
            .and_then(|c| c.get(id))
            .cloned())
    }

    fn list(&self, collection: &str) -> Result<Vec<serde_json::Value>> {
        Ok(self
            .data
            .lock()
            .map_err(lock_err)?
            .get(collection)
            .map(|c| c.values().cloned().collect())
            .unwrap_or_default())
    }

    fn delete(&self, collection: &str, id: &str) -> Result<()> {
        if let Some(coll) = self.data.lock().map_err(lock_err)?.get_mut(collection) {
            coll.remove(id);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::{persist, retrieve, retrieve_all};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct Item {
        id: String,
        value: u32,
    }

    #[test]
    fn save_and_load() {
        let p = MemoryPersistenceProvider::new();
        let item = Item { id: "a".into(), value: 42 };
        persist(&p, "items", "a", &item).unwrap();
        let loaded: Option<Item> = retrieve(&p, "items", "a").unwrap();
        assert_eq!(loaded, Some(item));
    }

    #[test]
    fn load_missing_returns_none() {
        let p = MemoryPersistenceProvider::new();
        let r: Option<Item> = retrieve(&p, "items", "nope").unwrap();
        assert!(r.is_none());
    }

    #[test]
    fn list_all() {
        let p = MemoryPersistenceProvider::new();
        persist(&p, "items", "a", &Item { id: "a".into(), value: 1 }).unwrap();
        persist(&p, "items", "b", &Item { id: "b".into(), value: 2 }).unwrap();
        let all: Vec<Item> = retrieve_all(&p, "items").unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn delete_removes_item() {
        let p = MemoryPersistenceProvider::new();
        persist(&p, "items", "a", &Item { id: "a".into(), value: 1 }).unwrap();
        p.delete("items", "a").unwrap();
        let r: Option<Item> = retrieve(&p, "items", "a").unwrap();
        assert!(r.is_none());
    }

    #[test]
    fn separate_collections_isolated() {
        let p = MemoryPersistenceProvider::new();
        persist(&p, "col_a", "x", &Item { id: "x".into(), value: 1 }).unwrap();
        let r: Option<Item> = retrieve(&p, "col_b", "x").unwrap();
        assert!(r.is_none());
    }
}
