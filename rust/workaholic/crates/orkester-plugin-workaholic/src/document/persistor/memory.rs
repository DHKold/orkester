pub struct MemoryPersistor {
    store: std::sync::Mutex<std::collections::HashMap<EntityKey, EntityValue>>,
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

    fn list(&self, path: &str) -> Result<Vec<EntityKey>, PersistorError> {
        let store = self.store.lock().map_err(|e| PersistorError::Internal(e.to_string()))?;
        Ok(store.keys().filter(|k| k.starts_with(path)).cloned().collect())
    }
}

// === Export the component for use with orkester ===
pub struct MemoryDocumentPersistor {
    persistor: MemoryPersistor,
}

#[component(
    kind = "workaholic/MemoryPersistor:1.0",
    name = "Memory Persistence Component",
    description = "In-memory implementation of the DocumentPersistor trait for testing and development purposes."
)]
impl MemoryDocumentPersistor {
    /// Constructor for the MemoryDocumentPersistor.
    pub fn new() -> Self {
        Self {
            persistor: MemoryPersistor {
                store: std::sync::Mutex::new(std::collections::HashMap::new()),
            },
        }
    }

    #[handle(ACTION_PERSISTOR_PUT)]
    pub fn handle_put(&self, id: String, data: Document) -> Result<(), PersistorError> {
        self.persistor.put(&id, data)
    }

    #[handle(ACTION_PERSISTOR_GET)]
    pub fn handle_get(&self, id: String) -> Result<Document, PersistorError> {
        self.persistor.get(&id)
    }

    #[handle(ACTION_PERSISTOR_DELETE)]
    pub fn handle_delete(&self, id: String) -> Result<(), PersistorError> {
        self.persistor.delete(&id)
    }

    #[handle(ACTION_PERSISTOR_LIST)]
    pub fn handle_list(&self, path: String) -> Result<Vec<String>, PersistorError> {
        self.persistor.list(&path)
    }
}