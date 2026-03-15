//! Plugin component registry.
//!
//! [`Registry`] is the central catalogue of every provider builder and server
//! factory contributed by loaded plugins.  It is constructed once at startup
//! (see [`register_plugins`]) and then passed to the server and provider
//! setup stages.

use std::collections::HashMap;
use std::sync::Arc;

use crate::plugin::LoadedPlugin;
use libloading::Library;
use orkester_common::plugin::{ComponentMetadata, PluginComponent, PluginMetadata, Registry};
use orkester_common::{log_debug, log_error, log_info};

pub struct DynamicRegistry {
    /// Metadata for every successfully loaded plugin (populated by `register_plugins`).
    pub plugins: Vec<PluginMetadata>,

    /// Every registered component, keyed by "plugin_id:component_id" (e.g. "orkester-foo:bar-auth").
    pub authentication_providers: HashMap<String, ComponentMetadata>,
    pub authorization_providers: HashMap<String, ComponentMetadata>,
    pub executor_providers: HashMap<String, ComponentMetadata>,
    pub persistence_providers: HashMap<String, ComponentMetadata>,
    pub server_builders: HashMap<String, ComponentMetadata>,
    
    /// Library handles for every loaded plugin. We must keep these around to prevent the .so files from being unmapped.
    _libs: Vec<Library>,
}

impl DynamicRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            authentication_providers: HashMap::new(),
            authorization_providers: HashMap::new(),
            executor_providers: HashMap::new(),
            persistence_providers: HashMap::new(),
            server_builders: HashMap::new(),
            _libs: Vec::new(),
        }
    }
}

impl Registry for DynamicRegistry {
    fn plugins(&self) -> &[PluginMetadata] {
        &self.plugins
    }

    fn authentication_provider(&self, id: &str) -> Result<&PluginComponent, String> {
        self.authentication_providers
            .values()
            .find(|comp| comp.id == id)
            .map(|comp| &comp.builder)
            .ok_or_else(|| format!("No authentication provider found for id '{}'.", id))
    }

    fn authorization_provider(&self, id: &str) -> Result<&PluginComponent, String> {
        self.authorization_providers
            .values()
            .find(|comp| comp.id == id)
            .map(|comp| &comp.builder)
            .ok_or_else(|| format!("No authorization provider found for id '{}'.", id))
    }

    fn executor_provider(&self, id: &str) -> Result<&PluginComponent, String> {
        self.executor_providers
            .values()
            .find(|comp| comp.id == id)
            .map(|comp| &comp.builder)
            .ok_or_else(|| format!("No executor provider found for id '{}'.", id))
    }

    fn persistence_provider(&self, id: &str) -> Result<&PluginComponent, String> {
        self.persistence_providers
            .values()
            .find(|comp| comp.id == id)
            .map(|comp| &comp.builder)
            .ok_or_else(|| format!("No persistence provider found for id '{}'.", id))
    }

    fn server_builder(&self, id: &str) -> Result<&PluginComponent, String> {
        self.server_builders
            .values()
            .find(|comp| comp.id == id)
            .map(|comp| &comp.builder)
            .ok_or_else(|| format!("No server builder found for id '{}'.", id))
    }
}

impl DynamicRegistry {
    /// Build an [`ExecutorRegistry`] populated with every executor contributed
    /// by loaded plugins. Keyed by component `id` (e.g. `"commands"`).
    pub fn build_executor_registry(
        &self,
    ) -> Arc<orkester_common::plugin::providers::executor::ExecutorRegistry> {
        use orkester_common::plugin::providers::executor::ExecutorRegistry;

        let mut reg = ExecutorRegistry::new();
        for (key, comp) in &self.executor_providers {
            if let PluginComponent::ExecutorProvider(builder) = &comp.builder {
                match builder.build(serde_json::Value::Null) {
                    Ok(executor) => {
                        log_info!("Registered executor '{}'.", comp.id);
                        reg.register(comp.id.clone(), Arc::from(executor));
                    }
                    Err(e) => {
                        log_error!("Failed to build executor '{}': {}", key, e);
                    }
                }
            }
        }
        Arc::new(reg)
    }
}

// ── Registration ──────────────────────────────────────────────────────────────
pub fn register_plugins(plugins: Vec<LoadedPlugin>) -> Arc<DynamicRegistry> {
    let mut registry = DynamicRegistry::new();

    for lp in plugins {
        let plugin_id = lp.plugin.metadata.id.clone();
        let plugin_version = lp.plugin.metadata.version.clone();

        log_info!(
            "Registering components from plugin '{}' v{}...",
            plugin_id,
            plugin_version
        );

        // Transfer library handle into the registry so the .so stays mapped.
        if let Some(lib) = lp._lib {
            registry._libs.push(lib);
        }

        // Consume the boxed Plugin to iterate its components by value.
        let plugin = *lp.plugin;
        // Save metadata before consuming components.
        registry.plugins.push(plugin.metadata.clone());
        for comp in plugin.components {
            register_component(&mut registry, comp, &plugin_id);
        }
    }

    Arc::new(registry)
}

fn register_component(registry: &mut DynamicRegistry, comp: ComponentMetadata, plugin_id: &str) {
    let component_key = plugin_id.to_string() + ":" + &comp.id;
    log_debug!("  component '{}' [kind={}]", component_key, comp.kind);

    match comp.builder {
        PluginComponent::AuthenticationProvider(ref _builder) => {
            registry
                .authentication_providers
                .insert(component_key, comp);
        }
        PluginComponent::AuthorizationProvider(ref _builder) => {
            registry.authorization_providers.insert(component_key, comp);
        }
        PluginComponent::ExecutorProvider(ref _builder) => {
            registry.executor_providers.insert(component_key, comp);
        }
        PluginComponent::PersistenceProvider(ref _builder) => {
            registry.persistence_providers.insert(component_key, comp);
        }
        PluginComponent::Server(ref _builder) => {
            registry.server_builders.insert(component_key, comp);
        }
    }
}
