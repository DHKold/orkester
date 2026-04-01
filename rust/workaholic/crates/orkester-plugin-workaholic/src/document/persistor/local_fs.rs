use orkester_plugin::prelude::*;

use workaholic::{DocumentPersistor, EntityKey, EntityValue, PersistorError};

use super::actions::*;
use super::request::*;

// ─── LocalFsPersistor ──────────────────────────────────────────────────────────

pub struct LocalFsPersistor {
    base_path: std::path::PathBuf,
}

impl LocalFsPersistor {
    pub fn new(base_path: impl Into<std::path::PathBuf>) -> Self {
        Self { base_path: base_path.into() }
    }
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

    fn list(&self, prefix: &str) -> Result<Vec<EntityKey>, PersistorError> {
        let dir_path = self.base_path.join(prefix);
        if !dir_path.exists() || !dir_path.is_dir() {
            return Ok(vec![]);
        }
        let entries = std::fs::read_dir(dir_path).map_err(|e| PersistorError::Internal(e.to_string()))?;
        let prefix_clean = prefix.trim_end_matches('/');
        Ok(entries
            .filter_map(|e| e.ok().and_then(|e| e.file_name().into_string().ok()))
            .map(|f| format!("{}/{}", prefix_clean, f))
            .collect())
    }
}

// ─── LocalFsDocumentPersistor component ──────────────────────────────────────

pub struct LocalFsDocumentPersistor {
    persistor: LocalFsPersistor,
}

#[component(
    kind = "workaholic/LocalFsPersistor:1.0",
    name = "Local Filesystem Persistence Component",
    description = "Filesystem-based persistence for local development and testing."
)]
impl LocalFsDocumentPersistor {
    pub fn new(base_path: String) -> Self {
        Self {
            persistor: LocalFsPersistor {
                base_path: std::path::PathBuf::from(base_path),
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