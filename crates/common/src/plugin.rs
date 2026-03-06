use crate::providers::{
    auth::AuthProviderBuilder,
    authz::AuthzProviderBuilder,
    executor::ExecutorBuilder,
    persistence::PersistenceBuilder,
    registry::RegistryBuilder,
};

/// Metadata describing a plugin.
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    /// Unique identifier for this plugin (e.g., "orkester-auth-oidc").
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Semantic version string.
    pub version: String,
    /// Short description.
    pub description: String,
    /// Author(s).
    pub authors: Vec<String>,
}

/// A component provided by a plugin, represented as a named builder.
/// Each variant corresponds to an extensibility point in orkester.
pub enum PluginComponent {
    Authentication(Box<dyn AuthProviderBuilder>),
    Authorization(Box<dyn AuthzProviderBuilder>),
    WorkflowRegistry(Box<dyn RegistryBuilder>),
    Persistence(Box<dyn PersistenceBuilder>),
    TaskExecutor(Box<dyn ExecutorBuilder>),
}

/// The root structure that every Orkester plugin must provide.
///
/// When Orkester loads a plugin (`.so` / `.dll`), it calls the well-known
/// exported symbol `orkester_register_plugin`, which must return a `Plugin`.
///
/// # Dynamic-loading contract
/// ```c
/// // Symbol that every plugin shared library must export:
/// Plugin* orkester_register_plugin();
/// ```
pub struct Plugin {
    pub metadata: PluginMetadata,
    pub components: Vec<PluginComponent>,
}

/// Type alias for the function pointer of the well-known plugin entry point.
/// Signature: `extern "C" fn() -> *mut Plugin`
pub type PluginRegistrationFn = unsafe extern "C" fn() -> *mut Plugin;

/// The symbol name that Orkester will look up in every loaded shared library.
pub const PLUGIN_REGISTRATION_SYMBOL: &str = "orkester_register_plugin";
