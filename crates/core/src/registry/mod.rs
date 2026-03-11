//! Plugin component registry.
//!
//! [`Registry`] is the central catalogue of every provider builder and server
//! factory contributed by loaded plugins.  It is constructed once at startup
//! (see [`register_plugins`]) and then passed to the server and provider
//! setup stages.

use std::collections::HashMap;

use crate::logging::Logger;
use crate::plugin::LoadedPlugin;
use libloading::Library;
use orkester_common::plugin::{
    servers::{Server, ServerBuilder},
    ComponentMetadata, PluginComponent,
};

// ── Registry ──────────────────────────────────────────────────────────────────
pub struct Registry {
    pub authentication_providers: HashMap<String, ComponentMetadata>,
    pub authorization_providers: HashMap<String, ComponentMetadata>,
    pub executor_providers: HashMap<String, ComponentMetadata>,
    pub persistence_providers: HashMap<String, ComponentMetadata>,
    pub registry_providers: HashMap<String, ComponentMetadata>,
    pub server_builders: HashMap<String, ComponentMetadata>,
    _libs: Vec<Library>,
}

impl Registry {
    fn new() -> Self {
        Registry {
            authentication_providers: HashMap::new(),
            authorization_providers: HashMap::new(),
            executor_providers: HashMap::new(),
            persistence_providers: HashMap::new(),
            registry_providers: HashMap::new(),
            server_builders: HashMap::new(),
            _libs: Vec::new(),
        }
    }
}

// ── Registration ──────────────────────────────────────────────────────────────
pub fn register_plugins(plugins: Vec<LoadedPlugin>) -> Registry {
    let mut registry = Registry::new();

    for lp in plugins {
        let plugin_id = lp.plugin.metadata.id.clone();
        let plugin_version = lp.plugin.metadata.version.clone();

        Logger::info(format!("Registering components from plugin '{}' v{}...",plugin_id, plugin_version));

        // Transfer library handle into the registry so the .so stays mapped.
        if let Some(lib) = lp._lib {
            registry._libs.push(lib);
        }

        // Consume the boxed Plugin to iterate its components by value.
        let plugin = *lp.plugin;
        for comp in plugin.components {
            register_component(&mut registry, comp, &plugin_id);
        }
    }

    registry
}

fn register_component(registry: &mut Registry, comp: ComponentMetadata, plugin_id: &str) {
    let component_key = plugin_id.to_string() + ":" + &comp.id;
    Logger::debug(format!("  component '{}' [kind={}]", component_key, comp.kind));

    match comp.builder {
        PluginComponent::AuthenticationProvider(ref builder) => {
            registry.authentication_providers.insert(component_key, comp);
        }
        PluginComponent::AuthorizationProvider(ref builder) => {
            registry.authorization_providers.insert(component_key, comp);
        }
        PluginComponent::ExecutorProvider(ref builder) => {
            registry.executor_providers.insert(component_key, comp);
        }
        PluginComponent::PersistenceProvider(ref builder) => {
            registry.persistence_providers.insert(component_key, comp);
        }
        PluginComponent::RegistryProvider(ref builder) => {
            registry.registry_providers.insert(component_key, comp);
        }
        PluginComponent::Server(ref builder) => {
            registry.server_builders.insert(component_key, comp);
        }
    }
}
