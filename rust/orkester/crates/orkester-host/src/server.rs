use std::sync::{Arc, Mutex};

use orkester_plugin::prelude::*;
use orkester_plugin::sdk::message::CreateComponentRequest;
use orkester_plugin::sdk::ComponentMetadata;

use crate::catalog::Catalog;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ComponentInfo {
    pub name: String,
    pub kind: String,
}

/// Independent snapshot registry for introspection — never locked by the hub
/// dispatcher, so reading it from inside a component handler is safe.
pub type ComponentInfoRegistry = Arc<Mutex<Vec<ComponentInfo>>>;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct EmptyRequest {
    pub query: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_secs: u64,
}

pub struct HostServer {
    catalog:       Catalog,
    registry_info: ComponentInfoRegistry,
    startup_instant: std::time::Instant,
}

#[component(
    kind = "orkester/HostServer:1.0",
    name = "Orkester Host Server",
    description = "The main server component for Orkester Host, responsible for managing plugins and communication."
)]
impl HostServer {
    /// Create a new HostServer instance.
    pub fn new(catalog: Catalog, registry_info: ComponentInfoRegistry) -> Self {
        Self { catalog, registry_info, startup_instant: std::time::Instant::now() }
    }

    /// Health check endpoint to verify the server is running. Returns a simple status message with the server version and uptime.
    #[handle("orkester/HostServer/HealthCheck:1.0")]
    fn health_check(&mut self, _request: EmptyRequest) -> Result<HealthResponse> {
        Ok(HealthResponse {
            status: "ok".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_secs: self.startup_instant.elapsed().as_secs(),
        })
    }

    #[handle("orkester/HostServer/Catalog/CreateComponent:1.0")]
    fn create_component(&mut self, _request: CreateComponentRequest) -> Result<()> {
        Ok(()) // For now, we don't support dynamic component creation through the HostServer.
    }

    #[handle("orkester/HostServer/Catalog/ListPlugins:1.0")]
    fn list_plugins(&mut self, _query: EmptyRequest) -> Result<Vec<ComponentMetadata>> {
        let plugins = self.catalog.plugins.iter()
            .map(|entry| entry.1.metadata.clone())
            .collect();
        Ok(plugins)
    }

    #[handle("orkester/HostServer/Catalog/ListComponents:1.0")]
    fn list_components(&mut self, _query: EmptyRequest) -> Result<Vec<ComponentMetadata>> {
        let components = self.catalog.components.iter()
            .map(|entry| entry.1.metadata.clone())
            .collect();
        Ok(components)
    }

    #[handle("orkester/HostServer/Registry/List:1.0")]
    fn list_registry_components(&mut self, _query: EmptyRequest) -> Result<Vec<ComponentInfo>> {
        Ok(self.registry_info.lock().unwrap().clone())
    }
}