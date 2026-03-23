use std::sync::{Arc, Mutex};

use anyhow::{bail, Result};

use orkester_plugin::{abi::AbiComponent, hub::ComponentEntry};

use crate::catalog::Catalog;
use crate::config::ServerConfig;

// â”€â”€ ComponentRegistry â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Shared registry of live ABI component instances.
pub type ComponentRegistry = Arc<Mutex<Vec<ComponentEntry>>>;

pub fn new_registry() -> ComponentRegistry {
    Arc::new(Mutex::new(Vec::new()))
}

// â”€â”€ Instantiation helpers â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Iterate plugins until one can produce a component of `server.kind`, then
/// register it under `server.name`.
pub fn instantiate_and_register(
    catalog:  &mut Catalog,
    registry: &ComponentRegistry,
    server:   &ServerConfig,
) -> Result<()> {
    let kind   = &server.kind;
    let name   = &server.name;
    let config = &server.config;

    // The factory call envelope expected by DispatchTable
    let req = serde_json::json!({
        "action": "orkester/CreateComponent",
        "params": { "kind": kind, "config": config }
    });

    for entry in &mut catalog.entries {
        let mut root = entry.plugin.get_root_component();
        match root.call_factory::<serde_json::Value>(&req) {
            Ok(ptr) if !ptr.is_null() => {
                log::info!("[registry] instantiated '{name}' ({kind})");
                registry.lock().unwrap().push(
                    ComponentEntry::new(name.clone(), kind.clone(), ptr)
                );
                return Ok(());
            }
            Ok(_) => continue, // null pointer â€” plugin declined
            Err(e) => {
                // When a plugin doesn't have a factory for this kind, the
                // dispatch table returns a JSON error which call_factory
                // surfaces as "expected format 'orkester/component'".  That
                // is a normal "not provided" signal — log at debug, not warn.
                log::debug!(
                    "[registry] plugin '{}' does not provide '{kind}': {e}",
                    entry.name
                );
                continue;
            }
        }
    }

    bail!("no loaded plugin provides component kind '{kind}'")
}

// â”€â”€ Lookup â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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

