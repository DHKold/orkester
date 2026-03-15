pub mod providers;
pub mod servers;

use std::collections::HashMap;
use std::sync::Arc;

use providers::{
    auth::AuthenticationProviderBuilder, authz::AuthorizationProviderBuilder,
    executor::ExecutorBuilder, persistence::PersistenceBuilder,
};
use servers::ServerBuilder;

use crate::logging::Logger;

/// A plugin component, which can be either a provider or a server.
pub enum PluginComponent {
    // ── Providers ──────────────────────────────────────────────────────────
    AuthenticationProvider(Box<dyn AuthenticationProviderBuilder>),
    AuthorizationProvider(Box<dyn AuthorizationProviderBuilder>),
    ExecutorProvider(Box<dyn ExecutorBuilder>),
    PersistenceProvider(Box<dyn PersistenceBuilder>),

    // ── Servers ────────────────────────────────────────────────────────────
    Server(Box<dyn ServerBuilder>),
}

/// Metadata describing a plugin.
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    /// Unique identifier for this plugin (e.g., `"orkester-auth-oidc"`).
    pub id: String,
    /// Semantic version string.
    pub version: String,
    /// Short description.
    pub description: String,
    /// Author(s).
    pub authors: Vec<String>,
}

/// Metadata describing a plugin component (provider or server).
pub struct ComponentMetadata {
    /// Type of provider (e.g. "auth", "authz", "registry", "persistence", "executor").
    pub kind: String,
    /// Unique identifier for this component (e.g., `"oidc-auth"`).
    pub id: String,
    /// Short description.
    pub description: String,
    /// Builder for constructing this component (e.g. an `AuthProviderBuilder` or `ServerFactory`).
    pub builder: PluginComponent,
}

/// The root structure that every Orkester plugin must provide.
pub struct Plugin {
    pub metadata: PluginMetadata,
    pub components: Vec<ComponentMetadata>,
}

// ── Registry ──────────────────────────────────────────────────────────────────
pub struct Registry {
    /// Metadata for every successfully loaded plugin (populated by `register_plugins`).
    pub plugins: Vec<PluginMetadata>,
    pub authentication_providers: HashMap<String, ComponentMetadata>,
    pub authorization_providers: HashMap<String, ComponentMetadata>,
    pub executor_providers: HashMap<String, ComponentMetadata>,
    pub persistence_providers: HashMap<String, ComponentMetadata>,
    pub server_builders: HashMap<String, ComponentMetadata>,
    _libs: Vec<Library>,
}

impl Registry {
    fn new() -> Self {
        Registry {
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

/// Type alias for the function pointer of the plugin entry point.
/// Signature: `extern "C" fn() -> *mut Plugin`
pub type PluginRegistrationFn = unsafe extern "C" fn() -> *mut Plugin;

/// The symbol name Orkester looks up in every loaded shared library.
pub const PLUGIN_REGISTRATION_SYMBOL: &str = "orkester_register_plugin";

/// Type alias for the optional logger-injection entry point exported by plugins.
/// Signature: `unsafe extern "C" fn(logger: *const Logger)`
pub type PluginSetLoggerFn = unsafe extern "C" fn(*const Logger);

/// Symbol name Orkester calls (if present) right after loading a shared library
/// to share the host process's global logger with the plugin.
pub const PLUGIN_SET_LOGGER_SYMBOL: &str = "orkester_set_logger";
