use std::collections::HashMap;
use std::sync::Mutex;
use serde::Deserialize;
use serde_json::Value;

use orkester_plugin::prelude::*;

use super::actions::*;
use super::request::*;

use crate::document::loader::local_fs::LocalFsChangeEvent;
use crate::document::loader::actions::*;

/// A simple in-memory catalog server that stores resources as type-erased JSON values.
/// Resources are keyed as `kind/namespace/name:version`.
/// Will be extended in the future to support persistence, indexing, and access control.

pub enum CatalogError {
    NotFound(String),
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CatalogError::NotFound(id) => write!(f, "resource not found: {id}"),
        }
    }
}

#[derive(Deserialize)]
pub struct CatalogServerConfig {
    // Future configuration options (e.g. persistence backend, indexing, etc.) can be added here.
}

pub struct CatalogServer {
    // host: orkester_plugin::sdk::Host,
    config: CatalogServerConfig,
    storage: Mutex<HashMap<String, Value>>,
}

#[component(
    kind = "workaholic/CatalogServer:1.0",
    name = "Workaholic Resources Catalog Server",
    description = "A server that provides a catalog of generic resources for the Workaholic plugin.",
)]
impl CatalogServer {
    /// Initializes the catalog server with an empty storage.
    pub fn new(host: *mut orkester_plugin::abi::AbiHost, config: CatalogServerConfig) -> Self {
        let host = unsafe { orkester_plugin::sdk::Host::from_abi(host) };
        Self {
            // host,
            config,
            storage: Mutex::new(HashMap::new()),
        }
    }

    /// Creates or overwrites a resource in the catalog. Returns the stored value.
    #[handle(ACTION_CATALOG_CREATE_RESOURCE)]
    fn create_resource(&mut self, request: ResourceCreationRequest) -> Result<Value, CatalogError> {
        let mut storage = self.storage.lock().unwrap();
        storage.insert(request.id, request.resource.clone());
        Ok(request.resource)
    }

    /// Retrieves a resource from the catalog by its ID.
    #[handle(ACTION_CATALOG_RETRIEVE_RESOURCE)]
    fn get_resource(&mut self, request: ResourceRetrievalRequest) -> Result<Value, CatalogError> {
        let storage = self.storage.lock().unwrap();
        if let Some(resource) = storage.get(&request.id) {
            Ok(resource.clone())
        } else {
            Err(CatalogError::NotFound(request.id))
        }
    }

    /// Updates an existing resource in the catalog.
    #[handle(ACTION_CATALOG_UPDATE_RESOURCE)]
    fn update_resource(&mut self, request: ResourceUpdateRequest) -> Result<Value, CatalogError> {
        let mut storage = self.storage.lock().unwrap();
        if storage.contains_key(&request.id) {
            storage.insert(request.id, request.resource.clone());
            Ok(request.resource)
        } else {
            Err(CatalogError::NotFound(request.id))
        }
    }

    /// Deletes a resource from the catalog by its ID.
    #[handle(ACTION_CATALOG_DELETE_RESOURCE)]
    fn delete_resource(&mut self, request: ResourceDeletionRequest) -> Result<bool, CatalogError> {
        let mut storage = self.storage.lock().unwrap();
        if storage.remove(&request.id).is_some() {
            Ok(true)
        } else {
            Err(CatalogError::NotFound(request.id))
        }
    }

    /// Searches for resources matching a `field=value` query.
    /// Supported fields: `kind`, `name`, `namespace`.
    #[handle(ACTION_CATALOG_SEARCH_RESOURCES)]
    fn search_resources(&mut self, request: ResourceSearchRequest) -> Result<Vec<Value>, CatalogError> {
        let query = request.query;
        let storage = self.storage.lock().unwrap();
        let result = storage
            .values()
            .filter(|resource| {
                if let Some(field_value) = query.strip_prefix("kind=") {
                    resource.get("kind").and_then(|v| v.as_str()) == Some(field_value)
                } else if let Some(field_value) = query.strip_prefix("name=") {
                    resource.get("name").and_then(|v| v.as_str()) == Some(field_value)
                } else if let Some(field_value) = query.strip_prefix("namespace=") {
                    resource
                        .get("metadata")
                        .and_then(|m| m.get("namespace"))
                        .and_then(|v| v.as_str()) == Some(field_value)
                } else {
                    false
                }
            })
            .cloned()
            .collect();
        Ok(result)
    }

    // Handle Loader events
    #[handle(EVENT_LOADER_DOCUMENT_ADDED)]
    #[handle(EVENT_LOADER_DOCUMENT_REMOVED)]
    #[handle(EVENT_LOADER_DOCUMENT_MODIFIED)]
    fn handle_document_added(&mut self, event: LocalFsChangeEvent) -> Result<()> {
        println!("Received document added event");
        Ok(())
    }
}