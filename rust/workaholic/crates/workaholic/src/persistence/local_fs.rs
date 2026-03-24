use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    error::{Result, WorkaholicError},
    traits::PersistenceProvider,
};

/// File-system backed persistence.
///
/// Layout:
/// ```text
/// <root>/
///   <collection>/
///     <id>.json
/// ```
pub struct LocalFsPersistenceProvider {
    root: PathBuf,
}

impl LocalFsPersistenceProvider {
    pub fn new(root: &Path) -> Result<Self> {
        fs::create_dir_all(root).map_err(WorkaholicError::Io)?;
        Ok(Self { root: root.to_owned() })
    }

    fn collection_dir(&self, collection: &str) -> PathBuf {
        self.root.join(collection)
    }

    fn item_path(&self, collection: &str, id: &str) -> PathBuf {
        self.collection_dir(collection).join(format!("{id}.json"))
    }
}

impl PersistenceProvider for LocalFsPersistenceProvider {
    fn save(&self, collection: &str, id: &str, value: serde_json::Value) -> Result<()> {
        let dir = self.collection_dir(collection);
        fs::create_dir_all(&dir).map_err(WorkaholicError::Io)?;
        let content =
            serde_json::to_string_pretty(&value).map_err(WorkaholicError::Json)?;
        fs::write(self.item_path(collection, id), content).map_err(WorkaholicError::Io)
    }

    fn load(&self, collection: &str, id: &str) -> Result<Option<serde_json::Value>> {
        let path = self.item_path(collection, id);
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path).map_err(WorkaholicError::Io)?;
        serde_json::from_str(&content)
            .map(Some)
            .map_err(WorkaholicError::Json)
    }

    fn list(&self, collection: &str) -> Result<Vec<serde_json::Value>> {
        let dir = self.collection_dir(collection);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut result = Vec::new();
        for entry in fs::read_dir(&dir).map_err(WorkaholicError::Io)? {
            let entry = entry.map_err(WorkaholicError::Io)?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            match fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
                    Ok(v) => result.push(v),
                    Err(e) => log::warn!("[persistence/fs] JSON parse error in {:?}: {e}", path),
                },
                Err(e) => log::warn!("[persistence/fs] read error {:?}: {e}", path),
            }
        }
        Ok(result)
    }

    fn delete(&self, collection: &str, id: &str) -> Result<()> {
        let path = self.item_path(collection, id);
        if path.exists() {
            fs::remove_file(&path).map_err(WorkaholicError::Io)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::{persist, retrieve, retrieve_all};
    use serde::{Deserialize, Serialize};
    use tempfile::TempDir;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct Item {
        id: String,
        value: u32,
    }

    fn tmp() -> TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn round_trip() {
        let dir = tmp();
        let p = LocalFsPersistenceProvider::new(dir.path()).unwrap();
        let item = Item { id: "x".into(), value: 7 };
        persist(&p, "items", "x", &item).unwrap();
        let loaded: Option<Item> = retrieve(&p, "items", "x").unwrap();
        assert_eq!(loaded, Some(item));
    }

    #[test]
    fn missing_returns_none() {
        let dir = tmp();
        let p = LocalFsPersistenceProvider::new(dir.path()).unwrap();
        let r: Option<Item> = retrieve(&p, "items", "nope").unwrap();
        assert!(r.is_none());
    }

    #[test]
    fn list_items() {
        let dir = tmp();
        let p = LocalFsPersistenceProvider::new(dir.path()).unwrap();
        persist(&p, "items", "a", &Item { id: "a".into(), value: 1 }).unwrap();
        persist(&p, "items", "b", &Item { id: "b".into(), value: 2 }).unwrap();
        let all: Vec<Item> = retrieve_all(&p, "items").unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn delete_removes() {
        let dir = tmp();
        let p = LocalFsPersistenceProvider::new(dir.path()).unwrap();
        persist(&p, "items", "a", &Item { id: "a".into(), value: 1 }).unwrap();
        p.delete("items", "a").unwrap();
        let r: Option<Item> = retrieve(&p, "items", "a").unwrap();
        assert!(r.is_none());
    }

    #[test]
    fn empty_collection_returns_empty_list() {
        let dir = tmp();
        let p = LocalFsPersistenceProvider::new(dir.path()).unwrap();
        let r: Vec<Item> = retrieve_all(&p, "nonexistent").unwrap();
        assert!(r.is_empty());
    }
}
