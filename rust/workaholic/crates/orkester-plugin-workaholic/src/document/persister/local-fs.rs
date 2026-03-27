pub struct LocalFsPersistor {
    base_path: std::path::PathBuf,
}

impl DocumentPersistor for LocalFsPersistor {
    fn put(&self, key: &EntityKey, data: EntityValue) -> Result<(), PersistorError> {
        let path = self.base_path.join(key);
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| PersistorError::Internal(e.to_string()))?;
        let file = std::fs::File::create(&path).map_err(|e| PersistorError::Internal(e.to_string()))?;
        serde_json::to_writer(file, &data).map_err(|e| PersistorError::Serialization(e.to_string()))
    }

    fn get(&self, key: &EntityKey) -> Result<EntityValue, PersistorError> {
        let path = self.base_path.join(key);
        if !path.exists() {
            return Err(PersistorError::NotFound(key.clone()));
        }
        let file = std::fs::File::open(&path).map_err(|e| PersistorError::Internal(e.to_string()))?;
        serde_json::from_reader(file).map_err(|e| PersistorError::Serialization(e.to_string()))
    }

    fn delete(&self, key: &EntityKey) -> Result<(), PersistorError> {
        let path = self.base_path.join(key);
        if path.exists() {
            std::fs::remove_file(path).map_err(|e| PersistorError::Internal(e.to_string()))
        } else {
            Err(PersistorError::NotFound(key.clone()))
        }
    }

    fn list(&self, path: &str) -> Result<Vec<EntityKey>, PersistorError> {
        let dir_path = self.base_path.join(path);
        if !dir_path.exists() || !dir_path.is_dir() {
            return Err(PersistorError::NotFound(path.to_string()));
        }
        let entries = std::fs::read_dir(dir_path).map_err(|e| PersistorError::Internal(e.to_string()))?;
        Ok(entries.filter_map(|entry| entry.ok().and_then(|e| e.file_name().into_string().ok())).collect())
    }
}

// === Export the component for use with orkester ===
pub struct LocalFsDocumentPersistor {
    persistor: LocalFsPersistor,
}

#[component(
    kind = "workaholic/LocalFsPersistor:1.0",
    name = "Local Filesystem Persistence Component",
    description = "Filesystem-based implementation of the DocumentPersistor trait for local development and testing."
)]
impl LocalFsDocumentPersistor {
    /// Constructor for the LocalFsDocumentPersistor.
    pub fn new(base_path: String) -> Self {
        Self {
            persistor: LocalFsPersistor {
                base_path: std::path::PathBuf::from(base_path),
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