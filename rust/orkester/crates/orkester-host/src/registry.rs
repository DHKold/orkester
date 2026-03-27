use std::sync::{Arc, Mutex};

use orkester_plugin::{abi::AbiComponent, hub::ComponentEntry};

/// Shared registry of live ABI component instances.
pub type ComponentRegistry = Arc<Mutex<Vec<ComponentEntry>>>;

pub fn new_registry() -> ComponentRegistry {
    Arc::new(Mutex::new(Vec::new()))
}

pub fn register_component(registry: &ComponentRegistry, name: &str, component: *mut AbiComponent, kind: String) {
    let entry = ComponentEntry::new( name.to_string(), kind, component);
    let mut guard = registry.lock().unwrap();
    guard.push(entry);
}
