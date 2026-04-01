use orkester_plugin::prelude::*;
use workaholic::{DocumentPersistor, EntityKey, EntityValue, PersistorError};

use super::actions::*;
use super::request::*;

// ─── MemoryPersistor ────────────────────────────────────────────────────────────

pub struct MemoryPersistor {
    store: std::sync::Mutex<std::collections::HashMap<EntityKey, EntityValue>>,
}

impl MemoryPersistor {
    pub fn new() -> Self {
        Self { store: std::sync::Mutex::new(std::collections::HashMap::new()) }
    }
}

impl DocumentPersistor for MemoryPersistor {
    fn put(&self, key: &EntityKey, data: EntityValue) -> Result<(), PersistorError> {
        let mut store = self.store.lock().map_err(|e| PersistorError::Internal(e.to_string()))?;
        store.insert(key.clone(), data);
        Ok(())
    }

    fn get(&self, key: &EntityKey) -> Result<EntityValue, PersistorError> {
        let store = self.store.lock().map_err(|e| PersistorError::Internal(e.to_string()))?;
        store.get(key).cloned().ok_or_else(|| PersistorError::NotFound(key.clone()))
    }

    fn delete(&self, key: &EntityKey) -> Result<(), PersistorError> {
        let mut store = self.store.lock().map_err(|e| PersistorError::Internal(e.to_string()))?;
        if store.remove(key).is_some() {
            Ok(())
        } else {
            Err(PersistorError::NotFound(key.clone()))
        }
    }

    fn list(&self, prefix: &str) -> Result<Vec<EntityKey>, PersistorError> {
        let store = self.store.lock().map_err(|e| PersistorError::Internal(e.to_string()))?;
        Ok(store.keys().filter(|k| k.starts_with(prefix)).cloned().collect())
    }
}

// ─── MemoryDocumentPersistor component ────────────────────────────────────────

pub struct MemoryDocumentPersistor {
    persistor: MemoryPersistor,
}

#[component(
    kind = "workaholic/MemoryPersistor:1.0",
    name = "Memory Persistence Component",
    description = "In-memory persistence for testing and development."
)]
impl MemoryDocumentPersistor {
    pub fn new() -> Self {
        Self {
            persistor: MemoryPersistor {
                store: std::sync::Mutex::new(std::collections::HashMap::new()),
            },
        }
    }

    #[handle(ACTION_PERSISTOR_PUT)]
    pub fn handle_put(&mut self, req: PersistorPutRequest) -> Result<(), PersistorError> {
        self.persistor.put(&req.key, req.value)
    }

    #[handle(ACTION_PERSISTOR_GET)]
    pub fn handle_get(&mut self, id: EntityKey) -> Result<EntityValue, PersistorError> {
        self.persistor.get(&id)
    }

    #[handle(ACTION_PERSISTOR_DELETE)]
    pub fn handle_delete(&mut self, id: EntityKey) -> Result<(), PersistorError> {
        self.persistor.delete(&id)
    }

    #[handle(ACTION_PERSISTOR_LIST)]
    pub fn handle_list(&mut self, prefix: EntityKey) -> Result<Vec<EntityKey>, PersistorError> {
        self.persistor.list(&prefix)
    }
}