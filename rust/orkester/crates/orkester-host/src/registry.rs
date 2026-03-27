use std::sync::{Arc, Mutex};

use anyhow::{Result};

use orkester_plugin::{abi::AbiComponent, hub::ComponentEntry};

use crate::catalog::{Catalog, CatalogComponent, CatalogPlugin};
use crate::config::ServerConfig;

/// Shared registry of live ABI component instances.
pub type ComponentRegistry = Arc<Mutex<Vec<ComponentEntry>>>;

pub fn new_registry() -> ComponentRegistry {
    Arc::new(Mutex::new(Vec::new()))
}

/// Iterate plugins until one can produce a component of `server.kind`, then
/// register it under `server.name`.
pub fn instantiate_and_register(catalog: &mut Catalog, registry: &ComponentRegistry, server: &ServerConfig) -> Result<()> {
    let kind   = &server.kind;
    let name   = &server.name;
    let config = &server.config;

    // The factory call envelope expected by DispatchTable
    let req = serde_json::json!({
        "action": "orkester/CreateComponent",
        "params": { "kind": kind, "config": config }
    });

    // Look for the component kind in the catalog, then call its factory to get a live instance.
    let comp_entry = catalog.components.get(kind).ok_or_else(|| anyhow::anyhow!("No loaded plugin provides component kind '{kind}'"))?;
    let plugin: &mut CatalogPlugin = catalog.plugins.get_mut(&comp_entry.plugin_ref).ok_or_else(|| anyhow::anyhow!("Plugin '{}' not found for component kind '{kind}'", comp_entry.plugin_ref))?;
    let mut plugin = &mut plugin.plugin;
    let mut handle = plugin.get_root_component();
    match handle.call_factory(&req) {
        Ok(comp_ptr) => {
            let mut guard = registry.lock().unwrap();
            guard.push(ComponentEntry::new(name.clone(), kind.clone(), comp_ptr));
            log::info!("[registry] Registered component '{}' of kind '{}' from plugin '{}'", name, kind, comp_entry.plugin_ref);
            Ok(())
        }
        Err(e) => Err(anyhow::anyhow!("Failed to create component of kind '{kind}' from plugin '{}': {e}", comp_entry.plugin_ref)),
    }
}

/// Find a registered component by name.  Returns `None` if not found.
pub fn find_by_name(registry: &ComponentRegistry, name: &str) -> Option<*mut AbiComponent> {
    let guard = registry.lock().unwrap();
    guard.iter().find(|e| e.name == name).map(|e| e.ptr())
}

/// Describe the registry contents for logging.
pub fn describe(registry: &ComponentRegistry) -> Vec<(String, String)> {
    let guard = registry.lock().unwrap();
    guard.iter().map(|e| (e.name.clone(), e.kind.clone())).collect()
}

