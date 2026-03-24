//! Persistence server components.
//!
//! Two implementations:
//! - `workaholic/LocalFsPersistence:1.0` — durable, file-backed
//! - `workaholic/MemoryPersistence:1.0`  — in-memory (dev/test only)
//!
//! Both handle the `persistence/*` action namespace:
//! - `persistence/Save`   `{ collection, id, value }`
//! - `persistence/Load`   `{ collection, id }` → `{ found: bool, value? }`
//! - `persistence/List`   `{ collection }` → `[value, …]`
//! - `persistence/Delete` `{ collection, id }`
//!
//! Components are registered in the host config under a name that contains
//! "persistence" so the name-based router can find them (e.g.
//! `name: local-fs-persistence`).

use std::path::Path;
use std::sync::Arc;

use orkester_plugin::prelude::*;
use serde::{Deserialize, Serialize};
use workaholic::{
    persistence::{LocalFsPersistenceProvider, MemoryPersistenceProvider},
    traits::PersistenceProvider,
};

// ── Wire types ────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct SaveRequest {
    pub collection: String,
    pub id: String,
    pub value: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
pub struct SaveAck { pub ok: bool }

#[derive(Serialize, Deserialize)]
pub struct LoadRequest {
    pub collection: String,
    pub id: String,
}

#[derive(Serialize, Deserialize)]
pub struct LoadResponse {
    pub found: bool,
    pub value: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
pub struct ListRequest {
    pub collection: String,
}

#[derive(Serialize, Deserialize)]
pub struct ListResponse {
    pub items: Vec<serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
pub struct DeleteRequest {
    pub collection: String,
    pub id: String,
}

#[derive(Serialize, Deserialize)]
pub struct DeleteAck { pub ok: bool }

// ── LocalFsPersistenceServer ──────────────────────────────────────────────────

/// Configuration for `workaholic/LocalFsPersistence:1.0`.
#[derive(Debug, Deserialize)]
pub struct LocalFsPersistenceConfig {
    /// Directory root for persisted data.
    pub path: String,
}

/// Durable, file-system-backed persistence component.
///
/// Data is stored as JSON files under `<path>/<collection>/<id>.json`.
/// Safe across process restarts.
pub struct LocalFsPersistenceServer {
    inner: Arc<LocalFsPersistenceProvider>,
}

#[component(
    kind        = "workaholic/LocalFsPersistence:1.0",
    name        = "LocalFsPersistence",
    description = "Durable file-system-backed persistence (JSON files)."
)]
impl LocalFsPersistenceServer {
    #[handle("persistence/Save")]
    fn save(&mut self, req: SaveRequest) -> Result<SaveAck> {
        self.inner.save(&req.collection, &req.id, req.value)
            .map_err(|e| -> Error { format!("persistence/Save: {e}").into() })?;
        Ok(SaveAck { ok: true })
    }

    #[handle("persistence/Load")]
    fn load(&mut self, req: LoadRequest) -> Result<LoadResponse> {
        let value = self.inner.load(&req.collection, &req.id)
            .map_err(|e| -> Error { format!("persistence/Load: {e}").into() })?;
        Ok(LoadResponse { found: value.is_some(), value })
    }

    #[handle("persistence/List")]
    fn list(&mut self, req: ListRequest) -> Result<ListResponse> {
        let items = self.inner.list(&req.collection)
            .map_err(|e| -> Error { format!("persistence/List: {e}").into() })?;
        Ok(ListResponse { items })
    }

    #[handle("persistence/Delete")]
    fn delete(&mut self, req: DeleteRequest) -> Result<DeleteAck> {
        self.inner.delete(&req.collection, &req.id)
            .map_err(|e| -> Error { format!("persistence/Delete: {e}").into() })?;
        Ok(DeleteAck { ok: true })
    }
}

// ── MemoryPersistenceServer ───────────────────────────────────────────────────

/// Configuration for `workaholic/MemoryPersistence:1.0` (none required).
#[derive(Debug, Default, Deserialize)]
pub struct MemoryPersistenceConfig {}

/// In-memory persistence component.  Data is lost on process restart.
/// Use for development and testing only.
pub struct MemoryPersistenceServer {
    inner: Arc<MemoryPersistenceProvider>,
}

#[component(
    kind        = "workaholic/MemoryPersistence:1.0",
    name        = "MemoryPersistence",
    description = "Volatile in-memory persistence (lost on restart). Use for dev/test."
)]
impl MemoryPersistenceServer {
    #[handle("persistence/Save")]
    fn save(&mut self, req: SaveRequest) -> Result<SaveAck> {
        self.inner.save(&req.collection, &req.id, req.value)
            .map_err(|e| -> Error { format!("persistence/Save: {e}").into() })?;
        Ok(SaveAck { ok: true })
    }

    #[handle("persistence/Load")]
    fn load(&mut self, req: LoadRequest) -> Result<LoadResponse> {
        let value = self.inner.load(&req.collection, &req.id)
            .map_err(|e| -> Error { format!("persistence/Load: {e}").into() })?;
        Ok(LoadResponse { found: value.is_some(), value })
    }

    #[handle("persistence/List")]
    fn list(&mut self, req: ListRequest) -> Result<ListResponse> {
        let items = self.inner.list(&req.collection)
            .map_err(|e| -> Error { format!("persistence/List: {e}").into() })?;
        Ok(ListResponse { items })
    }

    #[handle("persistence/Delete")]
    fn delete(&mut self, req: DeleteRequest) -> Result<DeleteAck> {
        self.inner.delete(&req.collection, &req.id)
            .map_err(|e| -> Error { format!("persistence/Delete: {e}").into() })?;
        Ok(DeleteAck { ok: true })
    }
}

// ── PersistenceClient ─────────────────────────────────────────────────────────

/// `PersistenceProvider` adapter that calls a persistence component via the
/// host ABI.  Workers and servers use this to stay decoupled from the concrete
/// persistence backend.
///
/// Obtained by calling [`HostClient::get_component`] with the persistence
/// component's registered name, then constructing via [`PersistenceClient::new`].
pub struct PersistenceClient {
    host: crate::host_client::HostClient,
    /// Registered name of the persistence component (used for log context only).
    #[allow(dead_code)]
    component_name: String,
}

impl PersistenceClient {
    pub fn new(host: crate::host_client::HostClient, component_name: impl Into<String>) -> Self {
        Self { host, component_name: component_name.into() }
    }
}

impl PersistenceProvider for PersistenceClient {
    fn save(&self, collection: &str, id: &str, value: serde_json::Value) -> workaholic::Result<()> {
        self.host.call::<_, SaveAck>(
            "persistence/Save",
            SaveRequest { collection: collection.into(), id: id.into(), value },
        ).map(drop).map_err(|e| {
            workaholic::WorkaholicError::Persistence(format!(
                "persistence/Save [{}/{}]: {e}", collection, id
            ))
        })
    }

    fn load(&self, collection: &str, id: &str) -> workaholic::Result<Option<serde_json::Value>> {
        self.host.call::<_, LoadResponse>(
            "persistence/Load",
            LoadRequest { collection: collection.into(), id: id.into() },
        ).map(|r| r.value).map_err(|e| {
            workaholic::WorkaholicError::Persistence(format!(
                "persistence/Load [{}/{}]: {e}", collection, id
            ))
        })
    }

    fn list(&self, collection: &str) -> workaholic::Result<Vec<serde_json::Value>> {
        self.host.call::<_, ListResponse>(
            "persistence/List",
            ListRequest { collection: collection.into() },
        ).map(|r| r.items).map_err(|e| {
            workaholic::WorkaholicError::Persistence(format!(
                "persistence/List [{}]: {e}", collection
            ))
        })
    }

    fn delete(&self, collection: &str, id: &str) -> workaholic::Result<()> {
        self.host.call::<_, DeleteAck>(
            "persistence/Delete",
            DeleteRequest { collection: collection.into(), id: id.into() },
        ).map(drop).map_err(|e| {
            workaholic::WorkaholicError::Persistence(format!(
                "persistence/Delete [{}/{}]: {e}", collection, id
            ))
        })
    }
}

// ── Constructors (called from root.rs factories) ──────────────────────────────

impl LocalFsPersistenceServer {
    pub fn new(cfg: LocalFsPersistenceConfig) -> workaholic::Result<Self> {
        let provider = LocalFsPersistenceProvider::new(Path::new(&cfg.path))?;
        Ok(Self { inner: Arc::new(provider) })
    }
}

impl MemoryPersistenceServer {
    pub fn new(_cfg: MemoryPersistenceConfig) -> Self {
        Self { inner: Arc::new(MemoryPersistenceProvider::new()) }
    }
}
