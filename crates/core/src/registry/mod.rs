//! Plugin component registry.
//!
//! [`Registry`] is the central catalogue of every provider builder and server
//! factory contributed by loaded plugins.  It is constructed once at startup
//! (see [`register_plugins`]) and then passed to the server and provider
//! setup stages.

use crate::logging::Logger;
use crate::plugin::LoadedPlugin;
use libloading::Library;
use orkester_common::plugin::{
    providers::{
        auth::AuthenticationProviderBuilder,
        authz::AuthorizationProviderBuilder,
        executor::ExecutorBuilder,
        persistence::PersistenceBuilder,
        registry::RegistryBuilder,
    },
    servers::ServerFactory,
    ComponentMetadata, PluginComponent,
};

// ── Registry ──────────────────────────────────────────────────────────────────

/// Holds all provider builders and server factories contributed by loaded
/// plugins.  Library handles are kept here to ensure `.so` files remain mapped
/// for as long as the registry (and all vtable-backed objects it owns) lives.
pub struct Registry {
    // ── Provider builders (last-registered wins per kind) ──────────────────
    pub auth: Option<Box<dyn AuthenticationProviderBuilder>>,
    pub authz: Option<Box<dyn AuthorizationProviderBuilder>>,
    pub executor: Option<Box<dyn ExecutorBuilder>>,
    pub persistence: Option<Box<dyn PersistenceBuilder>>,
    pub workflow_registry: Option<Box<dyn RegistryBuilder>>,

    // ── Server factories (ordered; multiple factories per server_type allowed)
    pub server_factories: Vec<Box<dyn ServerFactory>>,

    /// Shared-library handles — MUST be declared last so they are dropped
    /// after all trait objects whose vtables live inside the libraries.
    _libs: Vec<Library>,
}

impl Registry {
    fn new() -> Self {
        Registry {
            auth: None,
            authz: None,
            executor: None,
            persistence: None,
            workflow_registry: None,
            server_factories: Vec::new(),
            _libs: Vec::new(),
        }
    }

    /// Find a server factory by `server_type` (e.g. `"state"`, `"rest"`).
    pub fn server_factory(&self, server_type: &str) -> Option<&dyn ServerFactory> {
        self.server_factories
            .iter()
            .find(|f| f.server_type() == server_type)
            .map(|f| f.as_ref())
    }
}

// ── Registration ──────────────────────────────────────────────────────────────

/// Consume all loaded plugins and build a [`Registry`] from their components.
///
/// Provider kinds deduplicate — if two plugins export the same kind, the last
/// one wins and a warning is emitted.  Server factories are additive.
pub fn register_plugins(plugins: Vec<LoadedPlugin>) -> Registry {
    let mut registry = Registry::new();

    for lp in plugins {
        let plugin_name = lp.plugin.metadata.name.clone();
        let plugin_id = lp.plugin.metadata.id.clone();
        let plugin_version = lp.plugin.metadata.version.clone();

        Logger::info(format!(
            "Registering components from plugin '{}' v{} [{}]...",
            plugin_name, plugin_version, plugin_id
        ));

        // Transfer library handle into the registry so the .so stays mapped.
        if let Some(lib) = lp._lib {
            registry._libs.push(lib);
        }

        // Consume the boxed Plugin to iterate its components by value.
        let plugin = *lp.plugin;
        for comp in plugin.components {
            register_component(&mut registry, comp, &plugin_name);
        }
    }

    Logger::info(format!(
        "Registry ready — auth={}, authz={}, executor={}, persistence={}, \
         workflow_registry={}, server_factories={}",
        registry.auth.is_some(),
        registry.authz.is_some(),
        registry.executor.is_some(),
        registry.persistence.is_some(),
        registry.workflow_registry.is_some(),
        registry.server_factories.len(),
    ));

    registry
}

fn register_component(registry: &mut Registry, comp: ComponentMetadata, plugin_name: &str) {
    Logger::debug(format!(
        "  component '{}' [kind={}, id={}] from '{}'",
        comp.name, comp.kind, comp.id, plugin_name
    ));

    match comp.builder {
        PluginComponent::AuthenticationProvider(b) => {
            if registry.auth.is_some() {
                Logger::warn(format!(
                    "  AuthenticationProvider already registered; overriding with '{}' from '{}'",
                    comp.name, plugin_name
                ));
            }
            registry.auth = Some(b);
        }
        PluginComponent::AuthorizationProvider(b) => {
            if registry.authz.is_some() {
                Logger::warn(format!(
                    "  AuthorizationProvider already registered; overriding with '{}' from '{}'",
                    comp.name, plugin_name
                ));
            }
            registry.authz = Some(b);
        }
        PluginComponent::ExecutorProvider(b) => {
            if registry.executor.is_some() {
                Logger::warn(format!(
                    "  ExecutorProvider already registered; overriding with '{}' from '{}'",
                    comp.name, plugin_name
                ));
            }
            registry.executor = Some(b);
        }
        PluginComponent::PersistenceProvider(b) => {
            if registry.persistence.is_some() {
                Logger::warn(format!(
                    "  PersistenceProvider already registered; overriding with '{}' from '{}'",
                    comp.name, plugin_name
                ));
            }
            registry.persistence = Some(b);
        }
        PluginComponent::RegistryProvider(b) => {
            if registry.workflow_registry.is_some() {
                Logger::warn(format!(
                    "  RegistryProvider already registered; overriding with '{}' from '{}'",
                    comp.name, plugin_name
                ));
            }
            registry.workflow_registry = Some(b);
        }
        PluginComponent::Server(f) => {
            Logger::debug(format!(
                "  server factory: type='{}' name='{}'",
                f.server_type(),
                f.name()
            ));
            registry.server_factories.push(f);
        }
    }
}
